//! Captures a few seconds from the default PipeWire source and prints frame
//! statistics. Stage-3 manual/agent smoke for the native capture path.
//!
//! Usage: pipewire-smoke [seconds] [target-node]

use std::time::{Duration, Instant};

use anyhow::Result;
use voxflow_audio::{measure_level, AudioSource, CaptureConfig, PipeWireAudioSource};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let seconds: u64 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(2);
    let mut source = match args.next() {
        Some(target) => PipeWireAudioSource::with_target(target),
        None => PipeWireAudioSource::new(),
    };

    let config = CaptureConfig::default();
    source.start(config.clone())?;
    println!(
        "capturing {seconds}s at {} Hz mono, {} ms frames...",
        config.sample_rate_hz, config.frame_duration_ms
    );

    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut frames = 0_usize;
    let mut peak = 0.0_f32;
    let mut last_elapsed_ms = 0_u64;
    while Instant::now() < deadline {
        match source.next_frame()? {
            Some(frame) => {
                frames += 1;
                last_elapsed_ms = frame.elapsed_ms;
                peak = peak.max(measure_level(&frame.frame).peak);
            }
            None => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    source.stop()?;

    println!("frames: {frames}");
    println!("audio clock: {last_elapsed_ms} ms");
    println!("peak level: {peak:.4}");
    let expected = (seconds * 1000 / u64::from(config.frame_duration_ms)) as f64;
    let coverage = frames as f64 / expected;
    println!("coverage vs wall clock: {:.1}%", coverage * 100.0);
    if frames == 0 {
        anyhow::bail!("no frames captured");
    }
    Ok(())
}
