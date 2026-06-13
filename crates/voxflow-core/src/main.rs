use std::env;
use std::sync::Arc;

use anyhow::Result;
use serde_json::json;
use tokio::sync::Mutex;

use voxflow_core::core::VoxflowCore;
use voxflow_core::instance::InstanceGuard;
use voxflow_core::ipc::Envelope;
use voxflow_core::paths::VoxflowPaths;
use voxflow_core::pipeline::{run_streaming_pipeline, StreamingPipelineOptions};
use voxflow_core::recognizer::{
    AudioFrame, EnergyVad, EnergyVadConfig, LatencyBudget, MockRecognizer, ReplayBenchmark,
    ReplayCaseReport, ReplaySuiteReport, TimestampedAudioFrame,
};
use voxflow_core::server::serve;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some("serve") => {
            let swept = voxflow_asr_qwen3::sweep_orphaned_engines();
            if swept > 0 {
                tracing::warn!(swept, "swept orphaned vLLM engine processes at startup");
            }
            let paths = VoxflowPaths::from_env()?;
            let _guard = InstanceGuard::acquire(&paths)?;
            let socket = paths.socket.clone();
            let core = Arc::new(Mutex::new(VoxflowCore::load(paths)?));
            serve(core, &socket).await
        }
        Some("status") => {
            let paths = VoxflowPaths::from_env()?;
            let core = VoxflowCore::load(paths)?;
            println!("{}", serde_json::to_string_pretty(&core.status_snapshot())?);
            Ok(())
        }
        Some("mock-session") => {
            let paths = VoxflowPaths::from_env()?;
            let mut core = VoxflowCore::load(paths)?;
            let request = Envelope::command(
                "cli-1",
                "dictation.start",
                json!({ "frontend": "cli", "mode": "continuous" }),
            );
            let outcome = core.handle_command(request);
            println!("{}", serde_json::to_string(&outcome.response)?);
            for event in outcome.events {
                println!("{}", serde_json::to_string(&event)?);
            }
            Ok(())
        }
        Some("models") => {
            let paths = VoxflowPaths::from_env()?;
            let mut core = VoxflowCore::load(paths)?;
            let outcome = core.handle_command(Envelope::command("cli-1", "model.list", json!({})));
            println!(
                "{}",
                serde_json::to_string_pretty(&outcome.response.payload)?
            );
            Ok(())
        }
        Some("model-import") => {
            let Some(model_id) = args.get(2) else {
                eprintln!("model-import requires MODEL_ID PATH [copy|symlink]");
                std::process::exit(2);
            };
            let Some(path) = args.get(3) else {
                eprintln!("model-import requires MODEL_ID PATH [copy|symlink]");
                std::process::exit(2);
            };
            let mode = args.get(4).map(String::as_str).unwrap_or("copy");
            let paths = VoxflowPaths::from_env()?;
            let mut core = VoxflowCore::load(paths)?;
            let outcome = core.handle_command(Envelope::command(
                "cli-1",
                "model.import",
                json!({ "model_id": model_id, "path": path, "mode": mode }),
            ));
            println!("{}", serde_json::to_string_pretty(&outcome.response)?);
            if outcome.response.kind == voxflow_core::ipc::MessageKind::Error {
                std::process::exit(1);
            }
            Ok(())
        }
        Some("model-activate") => {
            let Some(model_id) = args.get(2) else {
                eprintln!("model-activate requires MODEL_ID");
                std::process::exit(2);
            };
            let paths = VoxflowPaths::from_env()?;
            let mut core = VoxflowCore::load(paths)?;
            let outcome = core.handle_command(Envelope::command(
                "cli-1",
                "model.activate",
                json!({ "model_id": model_id }),
            ));
            println!("{}", serde_json::to_string_pretty(&outcome.response)?);
            if outcome.response.kind == voxflow_core::ipc::MessageKind::Error {
                std::process::exit(1);
            }
            Ok(())
        }
        Some("model-delete") => {
            let Some(model_id) = args.get(2) else {
                eprintln!("model-delete requires MODEL_ID");
                std::process::exit(2);
            };
            let paths = VoxflowPaths::from_env()?;
            let mut core = VoxflowCore::load(paths)?;
            let outcome = core.handle_command(Envelope::command(
                "cli-1",
                "model.delete",
                json!({ "model_id": model_id }),
            ));
            println!("{}", serde_json::to_string_pretty(&outcome.response)?);
            if outcome.response.kind == voxflow_core::ipc::MessageKind::Error {
                std::process::exit(1);
            }
            Ok(())
        }
        Some("doctor") => {
            let paths = VoxflowPaths::from_env()?;
            let mut core = VoxflowCore::load(paths)?;
            let outcome =
                core.handle_command(Envelope::command("cli-1", "diagnostics.run", json!({})));
            println!(
                "{}",
                serde_json::to_string_pretty(&outcome.response.payload)?
            );
            Ok(())
        }
        Some("asr-benchmark-mock") => {
            let mut recognizer = MockRecognizer::default();
            let mut vad = EnergyVad::new(EnergyVadConfig {
                speech_start_frames: 1,
                speech_end_frames: 8,
                ..EnergyVadConfig::default()
            });
            let frames = (0..50)
                .map(|index| TimestampedAudioFrame {
                    elapsed_ms: index * 20,
                    frame: AudioFrame::mono_constant(16_000, 20, 2000),
                })
                .collect::<Vec<_>>();
            let report = ReplayBenchmark.run_with_vad(&mut recognizer, &mut vad, frames)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(())
        }
        Some("asr-suite-mock") => {
            let report = run_mock_replay_suite()?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(())
        }
        Some("audio-probe") => {
            let probe = voxflow_audio::probe_pipewire_runtime();
            println!("{}", serde_json::to_string_pretty(&probe)?);
            Ok(())
        }
        Some("audio-devices") => {
            let inventory = voxflow_audio::list_input_devices();
            println!("{}", serde_json::to_string_pretty(&inventory)?);
            Ok(())
        }
        Some("pipeline-smoke") => {
            let mut source = voxflow_audio::SyntheticAudioSource::constant(1_000, 2000);
            let mut recognizer = MockRecognizer::default();
            let mut vad = EnergyVad::new(EnergyVadConfig {
                speech_start_frames: 1,
                speech_end_frames: 8,
                ..EnergyVadConfig::default()
            });
            let run = run_streaming_pipeline(
                &mut source,
                &mut recognizer,
                &mut vad,
                voxflow_audio::CaptureConfig::default(),
                StreamingPipelineOptions::default(),
            )?;
            println!("{}", serde_json::to_string_pretty(&run)?);
            Ok(())
        }
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_help();
            std::process::exit(2);
        }
    }
}

fn print_help() {
    println!(
        "voxflow-core 0.3.0\n\nUSAGE:\n  voxflow-core serve\n  voxflow-core status\n  voxflow-core mock-session\n  voxflow-core models\n  voxflow-core model-import MODEL_ID PATH [copy|symlink]\n  voxflow-core model-activate MODEL_ID\n  voxflow-core model-delete MODEL_ID\n  voxflow-core asr-benchmark-mock\n  voxflow-core asr-suite-mock\n  voxflow-core audio-probe\n  voxflow-core audio-devices\n  voxflow-core pipeline-smoke\n  voxflow-core doctor\n"
    );
}

fn run_mock_replay_suite() -> Result<ReplaySuiteReport> {
    let mut cases = Vec::new();
    for (name, frame_count) in [
        ("zh-short", 50_usize),
        ("en-short", 50_usize),
        ("mixed-short", 75_usize),
    ] {
        let mut recognizer = MockRecognizer::default();
        let mut vad = EnergyVad::new(EnergyVadConfig {
            speech_start_frames: 1,
            speech_end_frames: 8,
            ..EnergyVadConfig::default()
        });
        let frames = (0..frame_count)
            .map(|index| TimestampedAudioFrame {
                elapsed_ms: (index as u64) * 20,
                frame: AudioFrame::mono_constant(16_000, 20, 2000),
            })
            .collect::<Vec<_>>();
        let report = ReplayBenchmark.run_with_vad(&mut recognizer, &mut vad, frames)?;
        cases.push(ReplayCaseReport {
            name: name.to_string(),
            report,
        });
    }
    Ok(ReplaySuiteReport::from_cases(
        cases,
        LatencyBudget::default(),
    ))
}
