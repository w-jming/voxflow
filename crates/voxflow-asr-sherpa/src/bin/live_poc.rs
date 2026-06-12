//! Live dictation POC: real microphone → PipeWire native capture → silero VAD
//! → sherpa-onnx streaming Zipformer → partial/stable/final on the terminal.
//!
//! This is the stage-3 go/no-go manual-verification tool (one command, no
//! daemon needed).
//!
//! Usage:
//!   live-poc --model-dir DIR [--silero-model FILE] [--seconds N] [--target NODE]

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use voxflow_asr::{AsrEvent, EnergyVad, EnergyVadConfig, StreamingRecognizer, Vad};
use voxflow_asr_sherpa::vad::{SileroVad, SileroVadConfig};
use voxflow_asr_sherpa::{SherpaStreamingConfig, SherpaStreamingRecognizer};
use voxflow_audio::{AudioSource, CaptureConfig, PipeWireAudioSource};

struct Options {
    model_dir: PathBuf,
    silero_model: Option<PathBuf>,
    seconds: u64,
    target: Option<String>,
}

fn parse_args() -> Result<Options> {
    let mut model_dir = None;
    let mut silero_model = None;
    let mut seconds = 30;
    let mut target = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |flag: &str| {
            args.next()
                .with_context(|| format!("{flag} expects a value"))
        };
        match arg.as_str() {
            "--model-dir" => model_dir = Some(PathBuf::from(value("--model-dir")?)),
            "--silero-model" => silero_model = Some(PathBuf::from(value("--silero-model")?)),
            "--seconds" => seconds = value("--seconds")?.parse().context("--seconds")?,
            "--target" => target = Some(value("--target")?),
            other => bail!("unknown argument: {other}"),
        }
    }
    let Some(model_dir) = model_dir else {
        bail!("--model-dir is required");
    };
    Ok(Options {
        model_dir,
        silero_model,
        seconds,
        target,
    })
}

fn main() -> Result<()> {
    let options = parse_args()?;

    eprintln!("loading model from {} ...", options.model_dir.display());
    let config = SherpaStreamingConfig::from_model_dir(&options.model_dir)?;
    let mut recognizer = SherpaStreamingRecognizer::new(config)?;
    let mut vad: Box<dyn Vad> = match &options.silero_model {
        Some(model) => Box::new(SileroVad::new(SileroVadConfig::new(model))?),
        None => {
            eprintln!("note: --silero-model not set, falling back to EnergyVad");
            Box::new(EnergyVad::new(EnergyVadConfig::default()))
        }
    };

    let mut source = match &options.target {
        Some(target) => PipeWireAudioSource::with_target(target.clone()),
        None => PipeWireAudioSource::new(),
    };
    source.start(CaptureConfig::default())?;
    eprintln!(
        "listening for {} s — speak now (Ctrl+C to stop early)\n",
        options.seconds
    );

    let session = recognizer.start_session()?;
    let deadline = Instant::now() + Duration::from_secs(options.seconds);
    let mut speech_started_at: Option<Instant> = None;
    let mut first_partial_reported = false;

    while Instant::now() < deadline {
        let Some(frame) = source.next_frame()? else {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        };
        let decision = vad.process_frame(&frame);
        if decision.speech_started {
            speech_started_at = Some(Instant::now());
            first_partial_reported = false;
        }
        recognizer.push_audio(&session, frame.frame)?;
        for event in recognizer.poll_events(&session)? {
            match event {
                AsrEvent::Partial { text, .. } => {
                    if !first_partial_reported {
                        if let Some(start) = speech_started_at {
                            eprintln!(
                                "[latency] first partial {} ms after VAD speech start",
                                start.elapsed().as_millis()
                            );
                        }
                        first_partial_reported = true;
                    }
                    print!("\r\x1b[2Kpartial: {text}");
                    std::io::stdout().flush()?;
                }
                AsrEvent::Stable { text, .. } => {
                    print!("\r\x1b[2Kstable : {text}");
                    std::io::stdout().flush()?;
                }
                AsrEvent::Final { text, .. } => {
                    println!("\r\x1b[2Kfinal  : {text}");
                    speech_started_at = None;
                }
            }
        }
    }

    for event in recognizer.finish_session(&session)? {
        if let AsrEvent::Final { text, .. } = event {
            println!("\r\x1b[2Kfinal  : {text}");
        }
    }
    source.stop()?;
    Ok(())
}
