//! D-17 / stage-3 POC harness: replays wav files through the sherpa-onnx
//! streaming backend and reports latency + RTF figures.
//!
//! Usage:
//!   sherpa-poc --model-dir DIR [--wav FILE]... [--realtime] [--threads N]
//!              [--loop-minutes N]
//!
//! Without --wav, all files under DIR/test_wavs/*.wav (16 kHz) are used.
//! --realtime paces frames at audio speed and measures wall-clock latency
//! from energy-VAD speech start to the first partial/stable event.
//! --loop-minutes cycles the inputs (with silence gaps) through one session
//! until N minutes of audio are consumed — the stage-3 stability check.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use voxflow_asr::{
    AsrEvent, AudioFrame, EnergyVad, EnergyVadConfig, StreamingRecognizer, TimestampedAudioFrame,
    Vad,
};
use voxflow_asr_sherpa::vad::{SileroVad, SileroVadConfig};
use voxflow_asr_sherpa::{SherpaStreamingConfig, SherpaStreamingRecognizer};

const FRAME_MS: u64 = 20;

#[derive(Debug, Serialize)]
struct CaseReport {
    file: String,
    audio_ms: u64,
    processing_ms: u64,
    rtf: f64,
    vad_speech_start_ms: Option<u64>,
    first_partial_wall_ms: Option<u64>,
    first_stable_wall_ms: Option<u64>,
    partial_count: usize,
    final_text: String,
}

#[derive(Debug, Serialize)]
struct StabilityReport {
    audio_minutes: f64,
    processing_ms: u64,
    rtf: f64,
    partial_count: usize,
    final_count: usize,
    peak_rss_kb: Option<u64>,
}

#[derive(Debug, Serialize)]
struct PocReport {
    model_dir: String,
    int8: bool,
    model_size_bytes: u64,
    num_threads: i32,
    realtime: bool,
    load_ms: u64,
    cases: Vec<CaseReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stability: Option<StabilityReport>,
}

struct Options {
    model_dir: PathBuf,
    wavs: Vec<PathBuf>,
    realtime: bool,
    threads: i32,
    loop_minutes: Option<u64>,
    silero_model: Option<PathBuf>,
}

fn make_vad(silero_model: &Option<PathBuf>) -> Result<Box<dyn Vad>> {
    match silero_model {
        Some(model) => Ok(Box::new(SileroVad::new(SileroVadConfig::new(model))?)),
        None => Ok(Box::new(EnergyVad::new(EnergyVadConfig::default()))),
    }
}

fn parse_args() -> Result<Options> {
    let mut model_dir = None;
    let mut wavs = Vec::new();
    let mut realtime = false;
    let mut threads = 2;
    let mut loop_minutes = None;
    let mut silero_model = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--model-dir" => model_dir = Some(PathBuf::from(required(&mut args, "--model-dir")?)),
            "--wav" => wavs.push(PathBuf::from(required(&mut args, "--wav")?)),
            "--realtime" => realtime = true,
            "--threads" => {
                threads = required(&mut args, "--threads")?
                    .parse()
                    .context("--threads expects an integer")?
            }
            "--loop-minutes" => {
                loop_minutes = Some(
                    required(&mut args, "--loop-minutes")?
                        .parse()
                        .context("--loop-minutes expects an integer")?,
                )
            }
            "--silero-model" => {
                silero_model = Some(PathBuf::from(required(&mut args, "--silero-model")?))
            }
            other => bail!("unknown argument: {other}"),
        }
    }
    let Some(model_dir) = model_dir else {
        bail!("--model-dir is required");
    };
    Ok(Options {
        model_dir,
        wavs,
        realtime,
        threads,
        loop_minutes,
        silero_model,
    })
}

fn required(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    args.next()
        .with_context(|| format!("{flag} expects a value"))
}

fn main() -> Result<()> {
    let options = parse_args()?;
    let mut config = SherpaStreamingConfig::from_model_dir(&options.model_dir)?;
    config.num_threads = options.threads;
    let int8 = config.model.int8;
    let model_size_bytes = config.model.total_size_bytes();

    let load_started = Instant::now();
    let mut recognizer = SherpaStreamingRecognizer::new(config)?;
    let load_ms = load_started.elapsed().as_millis() as u64;

    let wavs = if options.wavs.is_empty() {
        default_wavs(&options.model_dir)?
    } else {
        options.wavs.clone()
    };
    if wavs.is_empty() {
        bail!("no wav inputs found; pass --wav or add test_wavs/ to the model dir");
    }

    let mut cases = Vec::new();
    if options.loop_minutes.is_none() {
        for wav in &wavs {
            let mut vad = make_vad(&options.silero_model)?;
            cases.push(run_case(
                &mut recognizer,
                vad.as_mut(),
                wav,
                options.realtime,
            )?);
        }
    }
    let stability = match options.loop_minutes {
        Some(minutes) => Some(run_stability(&mut recognizer, &wavs, minutes)?),
        None => None,
    };

    let report = PocReport {
        model_dir: options.model_dir.display().to_string(),
        int8,
        model_size_bytes,
        num_threads: options.threads,
        realtime: options.realtime,
        load_ms,
        cases,
        stability,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run_stability(
    recognizer: &mut SherpaStreamingRecognizer,
    wavs: &[PathBuf],
    minutes: u64,
) -> Result<StabilityReport> {
    let mut clips = Vec::new();
    let mut sample_rate = 16_000_u32;
    for wav in wavs {
        let mut reader = hound::WavReader::open(wav)?;
        sample_rate = reader.spec().sample_rate;
        clips.push(reader.samples::<i16>().collect::<Result<Vec<_>, _>>()?);
    }
    if clips.is_empty() {
        bail!("stability run needs at least one wav input");
    }
    let frame_samples = (sample_rate as u64 * FRAME_MS / 1000) as usize;
    let gap = vec![0_i16; frame_samples * 25]; // 500 ms silence between clips
    let target_samples = sample_rate as u64 * 60 * minutes;

    let session = recognizer.start_session()?;
    let mut consumed = 0_u64;
    let mut partial_count = 0_usize;
    let mut final_count = 0_usize;
    let mut processing = Duration::ZERO;
    'outer: loop {
        for clip in &clips {
            for source in [clip.as_slice(), gap.as_slice()] {
                for chunk in source.chunks(frame_samples) {
                    let frame = AudioFrame {
                        sample_rate_hz: sample_rate,
                        channels: 1,
                        pcm_i16: chunk.to_vec(),
                    };
                    let push_started = Instant::now();
                    recognizer.push_audio(&session, frame)?;
                    let events = recognizer.poll_events(&session)?;
                    processing += push_started.elapsed();
                    for event in &events {
                        match event {
                            AsrEvent::Partial { .. } => partial_count += 1,
                            AsrEvent::Final { .. } => final_count += 1,
                            AsrEvent::Stable { .. } => {}
                        }
                    }
                    consumed += chunk.len() as u64;
                    if consumed >= target_samples {
                        break 'outer;
                    }
                }
            }
        }
    }
    let finish_started = Instant::now();
    let events = recognizer.finish_session(&session)?;
    processing += finish_started.elapsed();
    final_count += events
        .iter()
        .filter(|event| matches!(event, AsrEvent::Final { .. }))
        .count();

    let audio_seconds = consumed as f64 / sample_rate as f64;
    Ok(StabilityReport {
        audio_minutes: audio_seconds / 60.0,
        processing_ms: processing.as_millis() as u64,
        rtf: processing.as_secs_f64() / audio_seconds,
        partial_count,
        final_count,
        peak_rss_kb: read_peak_rss_kb(),
    })
}

fn read_peak_rss_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        line.strip_prefix("VmHWM:")
            .and_then(|rest| rest.trim().trim_end_matches(" kB").trim().parse().ok())
    })
}

fn default_wavs(model_dir: &Path) -> Result<Vec<PathBuf>> {
    let dir = model_dir.join("test_wavs");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut wavs = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("wav") {
            // Skip non-16k fixtures such as 8k.wav; the model expects 16 kHz.
            let reader = hound::WavReader::open(&path)?;
            if reader.spec().sample_rate == 16_000 {
                wavs.push(path);
            }
        }
    }
    wavs.sort();
    Ok(wavs)
}

fn run_case(
    recognizer: &mut SherpaStreamingRecognizer,
    vad: &mut dyn Vad,
    wav: &Path,
    realtime: bool,
) -> Result<CaseReport> {
    let mut reader =
        hound::WavReader::open(wav).with_context(|| format!("failed to open {}", wav.display()))?;
    let spec = reader.spec();
    if spec.channels != 1 {
        bail!("{} is not mono", wav.display());
    }
    let sample_rate = spec.sample_rate;
    let samples = reader.samples::<i16>().collect::<Result<Vec<_>, _>>()?;
    let frame_samples = (sample_rate as u64 * FRAME_MS / 1000) as usize;

    let session = recognizer.start_session()?;
    let started = Instant::now();
    let mut elapsed_ms = 0_u64;
    let mut vad_speech_start_ms = None;
    let mut speech_start_wall = None;
    let mut first_partial_wall_ms = None;
    let mut first_stable_wall_ms = None;
    let mut partial_count = 0_usize;
    let mut final_text = String::new();
    let mut processing = Duration::ZERO;

    let record_events = |events: &[AsrEvent],
                         speech_start_wall: &Option<Instant>,
                         first_partial_wall_ms: &mut Option<u64>,
                         first_stable_wall_ms: &mut Option<u64>,
                         partial_count: &mut usize,
                         final_text: &mut String| {
        for event in events {
            match event {
                AsrEvent::Partial { .. } => {
                    *partial_count += 1;
                    if first_partial_wall_ms.is_none() {
                        if let Some(start) = speech_start_wall {
                            *first_partial_wall_ms = Some(start.elapsed().as_millis() as u64);
                        }
                    }
                }
                AsrEvent::Stable { .. } => {
                    if first_stable_wall_ms.is_none() {
                        if let Some(start) = speech_start_wall {
                            *first_stable_wall_ms = Some(start.elapsed().as_millis() as u64);
                        }
                    }
                }
                AsrEvent::Final { text, .. } => {
                    if !final_text.is_empty() {
                        final_text.push(' ');
                    }
                    final_text.push_str(text);
                }
            }
        }
    };

    for chunk in samples.chunks(frame_samples) {
        let frame = AudioFrame {
            sample_rate_hz: sample_rate,
            channels: 1,
            pcm_i16: chunk.to_vec(),
        };
        let frame_duration = frame.duration_ms();
        let timestamped = TimestampedAudioFrame {
            elapsed_ms,
            frame: frame.clone(),
        };
        if realtime {
            let target = Duration::from_millis(elapsed_ms);
            let now = started.elapsed();
            if target > now {
                std::thread::sleep(target - now);
            }
        }
        let decision = vad.process_frame(&timestamped);
        if decision.speech_started && vad_speech_start_ms.is_none() {
            vad_speech_start_ms = Some(elapsed_ms);
            speech_start_wall = Some(Instant::now());
        }

        let push_started = Instant::now();
        recognizer.push_audio(&session, frame)?;
        let events = recognizer.poll_events(&session)?;
        processing += push_started.elapsed();
        record_events(
            &events,
            &speech_start_wall,
            &mut first_partial_wall_ms,
            &mut first_stable_wall_ms,
            &mut partial_count,
            &mut final_text,
        );
        elapsed_ms += frame_duration;
    }

    let finish_started = Instant::now();
    let events = recognizer.finish_session(&session)?;
    processing += finish_started.elapsed();
    record_events(
        &events,
        &speech_start_wall,
        &mut first_partial_wall_ms,
        &mut first_stable_wall_ms,
        &mut partial_count,
        &mut final_text,
    );

    let audio_ms = elapsed_ms.max(1);
    Ok(CaseReport {
        file: wav.display().to_string(),
        audio_ms,
        processing_ms: processing.as_millis() as u64,
        rtf: processing.as_secs_f64() / (audio_ms as f64 / 1000.0),
        vad_speech_start_ms,
        first_partial_wall_ms: if realtime {
            first_partial_wall_ms
        } else {
            None
        },
        first_stable_wall_ms: if realtime { first_stable_wall_ms } else { None },
        partial_count,
        final_text,
    })
}
