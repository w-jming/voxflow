//! Real-microphone dictation smoke across the configured ASR backends (D-22).
//!
//! Loads the user's real config (~/.voxflow), builds the selected backend via
//! the same factory the daemon uses, and streams PipeWire mic audio into it.
//!
//! Usage:
//!   asr-live [--backend qwen3_vllm|volcano_api|zipformer_local] [--seconds N]

use std::io::Write as _;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use voxflow_core::backend::build_recognizer;
use voxflow_core::config::{AsrBackend, Config};
use voxflow_core::recognizer::AsrEvent;
use voxflow_core::VoxflowPaths;
// AudioSource is the capture trait; PipeWire impl needs the live-asr feature.
use voxflow_audio::{AudioSource, CaptureConfig, PipeWireAudioSource};

fn parse_backend(value: &str) -> Result<AsrBackend> {
    Ok(match value {
        "qwen3_vllm" => AsrBackend::Qwen3Vllm,
        "volcano_api" => AsrBackend::VolcanoApi,
        "zipformer_local" => AsrBackend::ZipformerLocal,
        "mock" => AsrBackend::Mock,
        other => bail!("unknown backend: {other}"),
    })
}

/// Terminal-safe single-line status renderer: truncates to the real terminal
/// width (wrapped lines break \r-based redraw and flood the screen) and
/// skips no-op redraws.
struct StatusLine {
    width: usize,
    last: String,
}

impl StatusLine {
    fn new() -> Self {
        // Resolve columns once; tput works for interactive terminals, 80 is a
        // safe floor elsewhere. CJK glyphs are ~2 cells wide, so budget half.
        let width = std::process::Command::new("tput")
            .arg("cols")
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .and_then(|cols| cols.trim().parse::<usize>().ok())
            .unwrap_or(80);
        Self {
            width,
            last: String::new(),
        }
    }

    fn budget(&self) -> usize {
        (self.width.saturating_sub(12) / 2).max(20)
    }

    fn draw(&mut self, label: &str, text: &str) -> Result<()> {
        let budget = self.budget();
        let chars: Vec<char> = text.chars().collect();
        let tail: String = if chars.len() > budget {
            std::iter::once('…')
                .chain(chars[chars.len() - budget..].iter().copied())
                .collect()
        } else {
            text.to_string()
        };
        let line = format!("{label}: {tail}");
        if line != self.last {
            print!("\r\x1b[2K{line}");
            std::io::stdout().flush()?;
            self.last = line;
        }
        Ok(())
    }

    fn newline(&mut self) {
        self.last.clear();
    }
}

fn main() -> Result<()> {
    let mut backend_override = None;
    let mut seconds = 60_u64;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--backend" => {
                backend_override = Some(parse_backend(
                    &args.next().context("--backend expects a value")?,
                )?)
            }
            "--seconds" => {
                seconds = args
                    .next()
                    .context("--seconds expects a value")?
                    .parse()
                    .context("--seconds expects an integer")?
            }
            other => bail!("unknown argument: {other}"),
        }
    }

    let paths = VoxflowPaths::from_env()?;
    let mut config = Config::load_or_default(&paths.config)?;
    if let Some(backend) = backend_override {
        config.asr.backend = backend;
    }
    eprintln!(
        "backend: {} (config: {})",
        voxflow_core::backend::backend_label(config.asr.backend),
        paths.config.display()
    );

    let swept = voxflow_asr_qwen3::sweep_orphaned_engines();
    if swept > 0 {
        eprintln!("swept {swept} orphaned vLLM engine process(es) holding GPU memory");
    }
    eprintln!("building recognizer (qwen3 first session loads the model — wait for it)...");
    let mut recognizer = build_recognizer(&config, &paths)?;
    let session = recognizer.start_session()?;
    eprintln!("session {session} ready");

    let mut source = PipeWireAudioSource::new();
    source.start(CaptureConfig {
        // 20s of headroom: decode stalls must not drop microphone audio.
        queue_capacity_frames: 1000,
        ..CaptureConfig::default()
    })?;
    eprintln!("listening for {seconds}s — speak now (Ctrl+C to stop)\n");

    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut status = StatusLine::new();
    while Instant::now() < deadline {
        let Some(frame) = source.next_frame()? else {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        };
        recognizer.push_audio(&session, frame.frame)?;
        for event in recognizer.poll_events(&session)? {
            match event {
                AsrEvent::Partial { text, .. } => status.draw("partial", &text)?,
                AsrEvent::Stable { text, .. } => status.draw("stable ", &text)?,
                AsrEvent::Final { text, .. } => {
                    println!("\r\x1b[2Kfinal  : {text}");
                    status.newline();
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
