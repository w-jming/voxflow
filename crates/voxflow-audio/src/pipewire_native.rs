//! PipeWire native capture (D-3 main path).
//!
//! A dedicated thread runs the PipeWire main loop; the realtime process
//! callback slices S16LE mono samples into [`CaptureConfig::frame_duration_ms`]
//! frames and pushes them through the bounded queue. `next_frame` is
//! non-blocking, matching the [`AudioSource`] contract.

use std::sync::mpsc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context as _, Result};
use pipewire as pw;
use pw::{properties::properties, spa};
use spa::pod::Pod;
use voxflow_asr::{AudioFrame, TimestampedAudioFrame};

use crate::{
    bounded_audio_queue, AudioSource, BoundedAudioConsumer, BoundedAudioProducer, CaptureConfig,
};

const START_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct PipeWireAudioSource {
    /// PipeWire node id or node name to capture from; `None` = default source.
    target: Option<String>,
    worker: Option<Worker>,
    consumer: Option<BoundedAudioConsumer>,
}

struct Worker {
    join: std::thread::JoinHandle<()>,
    quit: pw::channel::Sender<()>,
}

impl PipeWireAudioSource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_target(target: impl Into<String>) -> Self {
        let mut source = Self::default();
        source.target = Some(target.into());
        source
    }
}

struct CaptureState {
    producer: BoundedAudioProducer,
    sample_rate_hz: u32,
    channels: u8,
    frame_samples: usize,
    pending: Vec<i16>,
    elapsed_ms: u64,
    frame_duration_ms: u32,
}

impl CaptureState {
    fn ingest(&mut self, bytes: &[u8]) {
        for pair in bytes.chunks_exact(2) {
            self.pending.push(i16::from_le_bytes([pair[0], pair[1]]));
        }
        while self.pending.len() >= self.frame_samples {
            let rest = self.pending.split_off(self.frame_samples);
            let pcm_i16 = std::mem::replace(&mut self.pending, rest);
            let frame = TimestampedAudioFrame {
                elapsed_ms: self.elapsed_ms,
                frame: AudioFrame {
                    sample_rate_hz: self.sample_rate_hz,
                    channels: self.channels,
                    pcm_i16,
                },
            };
            self.elapsed_ms += u64::from(self.frame_duration_ms);
            // Queue-full drops are counted by the producer; realtime capture
            // must never block here.
            let _ = self.producer.try_push(frame);
        }
    }
}

impl AudioSource for PipeWireAudioSource {
    fn start(&mut self, config: CaptureConfig) -> Result<()> {
        if self.worker.is_some() {
            bail!("PipeWire capture already started");
        }
        if config.channels != 1 {
            bail!(
                "only mono capture is supported (got {} channels)",
                config.channels
            );
        }
        let (producer, consumer) = bounded_audio_queue(config.queue_capacity_frames);
        let (quit_tx, quit_rx) = pw::channel::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<std::result::Result<(), String>>();
        let target = self.target.clone();
        let join = std::thread::Builder::new()
            .name("voxflow-pw-capture".to_string())
            .spawn(move || capture_loop(config, target, producer, quit_rx, ready_tx))
            .context("failed to spawn PipeWire capture thread")?;

        match ready_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(())) => {
                self.worker = Some(Worker {
                    join,
                    quit: quit_tx,
                });
                self.consumer = Some(consumer);
                Ok(())
            }
            Ok(Err(message)) => {
                let _ = join.join();
                bail!("PipeWire capture failed to start: {message}");
            }
            Err(_) => bail!("PipeWire capture did not become ready within {START_TIMEOUT:?}"),
        }
    }

    fn next_frame(&mut self) -> Result<Option<TimestampedAudioFrame>> {
        let Some(consumer) = &self.consumer else {
            return Ok(None);
        };
        Ok(consumer.try_next())
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(worker) = self.worker.take() {
            let _ = worker.quit.send(());
            worker
                .join
                .join()
                .map_err(|_| anyhow!("PipeWire capture thread panicked"))?;
        }
        self.consumer = None;
        Ok(())
    }
}

impl Drop for PipeWireAudioSource {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn capture_loop(
    config: CaptureConfig,
    target: Option<String>,
    producer: BoundedAudioProducer,
    quit_rx: pw::channel::Receiver<()>,
    ready_tx: mpsc::Sender<std::result::Result<(), String>>,
) {
    if let Err(error) = run_capture(config, target, producer, quit_rx, &ready_tx) {
        // If start() already returned the send fails silently, which is fine:
        // the error also ends the thread and stop() reaps it.
        let _ = ready_tx.send(Err(error.to_string()));
    }
}

fn run_capture(
    config: CaptureConfig,
    target: Option<String>,
    producer: BoundedAudioProducer,
    quit_rx: pw::channel::Receiver<()>,
    ready_tx: &mpsc::Sender<std::result::Result<(), String>>,
) -> Result<()> {
    pw::init();
    let mainloop = pw::main_loop::MainLoop::new(None).context("MainLoop::new")?;
    let context = pw::context::Context::new(&mainloop).context("Context::new")?;
    let core = context.connect(None).context("Context::connect")?;

    let mut props = properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        *pw::keys::MEDIA_ROLE => "Communication",
        *pw::keys::APP_NAME => "voxflow",
        *pw::keys::NODE_NAME => "voxflow-capture",
    };
    if let Some(target) = &target {
        props.insert(*pw::keys::TARGET_OBJECT, target.as_str());
    }

    let stream = pw::stream::Stream::new(&core, "voxflow-capture", props).context("Stream::new")?;

    let frame_samples =
        (config.sample_rate_hz as u64 * u64::from(config.frame_duration_ms) / 1000) as usize;
    let state = CaptureState {
        producer,
        sample_rate_hz: config.sample_rate_hz,
        channels: config.channels,
        frame_samples: frame_samples.max(1),
        pending: Vec::with_capacity(frame_samples * 2),
        elapsed_ms: 0,
        frame_duration_ms: config.frame_duration_ms,
    };

    let _listener = stream
        .add_local_listener_with_user_data(state)
        .process(|stream, state| {
            while let Some(mut buffer) = stream.dequeue_buffer() {
                let datas = buffer.datas_mut();
                if datas.is_empty() {
                    continue;
                }
                let data = &mut datas[0];
                let size = data.chunk().size() as usize;
                if let Some(bytes) = data.data() {
                    state.ingest(&bytes[..size.min(bytes.len())]);
                }
            }
        })
        .register()
        .context("stream listener register")?;

    // Request S16LE mono at the configured rate; the PipeWire graph resamples
    // and downmixes as needed.
    let mut audio_info = spa::param::audio::AudioInfoRaw::new();
    audio_info.set_format(spa::param::audio::AudioFormat::S16LE);
    audio_info.set_rate(config.sample_rate_hz);
    audio_info.set_channels(u32::from(config.channels));
    let values = pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(pw::spa::pod::Object {
            type_: pw::spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
            id: pw::spa::param::ParamType::EnumFormat.as_raw(),
            properties: audio_info.into(),
        }),
    )
    .context("format pod serialize")?
    .0
    .into_inner();
    let mut params = [Pod::from_bytes(&values).context("format pod parse")?];

    stream
        .connect(
            spa::utils::Direction::Input,
            None,
            pw::stream::StreamFlags::AUTOCONNECT
                | pw::stream::StreamFlags::MAP_BUFFERS
                | pw::stream::StreamFlags::RT_PROCESS,
            &mut params,
        )
        .context("stream connect")?;

    let _receiver = quit_rx.attach(mainloop.loop_(), {
        let mainloop = mainloop.clone();
        move |_| mainloop.quit()
    });

    ready_tx
        .send(Ok(()))
        .map_err(|_| anyhow!("start() abandoned the capture thread"))?;
    mainloop.run();
    Ok(())
}
