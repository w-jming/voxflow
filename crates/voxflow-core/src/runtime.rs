//! Live dictation runtime (feature `live-asr`): the real audio pump and the
//! engine preload thread behind the instant-hotkey acceptance bar.
//!
//! The pump owns the PipeWire capture and shares the recognizer through
//! [`EngineSlot`] with the IPC layer (short std-mutex locks per frame batch).
//! Partial/stable events are projected with the same shapes as the mock path
//! so input-method frontends need no changes. Finals currently bypass the
//! semantic-correction ledger (documented gap; correction integration is the
//! next batch).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

use crate::config::AsrBackend;
use crate::core::{asr_to_ipc_event, dictation_final_event, EngineSlot, PumpHandle, VoxflowCore};
use crate::ipc::Envelope;
use crate::recognizer::{AsrEvent, SessionId};
use voxflow_audio::{AudioSource, CaptureConfig, PipeWireAudioSource};

fn project(session: &SessionId, event: AsrEvent) -> Envelope {
    match event {
        AsrEvent::Final {
            revision,
            text,
            segment_id,
        } => dictation_final_event(session, revision, &text, &segment_id),
        other => asr_to_ipc_event(session, other),
    }
}

/// Spawns the capture→recognizer→events loop for one dictation session.
pub fn spawn_pump(
    slot: EngineSlot,
    sender: broadcast::Sender<Envelope>,
    session: SessionId,
) -> PumpHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    let join = std::thread::Builder::new()
        .name("voxflow-dictation-pump".to_string())
        .spawn(move || {
            if let Err(error) = pump_loop(slot, sender, session, stop_flag) {
                tracing::warn!(%error, "dictation pump ended with error");
            }
        })
        .ok();
    PumpHandle { stop, join }
}

fn pump_loop(
    slot: EngineSlot,
    sender: broadcast::Sender<Envelope>,
    session: SessionId,
    stop: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let mut source = PipeWireAudioSource::new();
    source.start(CaptureConfig {
        // Decode stalls must never drop microphone audio (20s headroom).
        queue_capacity_frames: 1000,
        ..CaptureConfig::default()
    })?;

    while !stop.load(Ordering::Relaxed) {
        let Some(frame) = source.next_frame()? else {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        };
        let events = {
            let mut engine = slot.lock().expect("engine slot");
            let Some(recognizer) = engine.as_mut() else {
                break;
            };
            recognizer.push_audio(&session, frame.frame)?;
            recognizer.poll_events(&session)?
        };
        for event in events {
            let _ = sender.send(project(&session, event));
        }
    }

    let final_events = {
        let mut engine = slot.lock().expect("engine slot");
        match engine.as_mut() {
            Some(recognizer) => recognizer.finish_session(&session).unwrap_or_default(),
            None => Vec::new(),
        }
    };
    for event in final_events {
        let _ = sender.send(project(&session, event));
    }
    source.stop()?;
    Ok(())
}

/// Preloads and warms the configured engine at daemon startup so the first
/// hotkey press is instant (owner acceptance bar: hot path < 200 ms).
pub fn spawn_preload(core: Arc<tokio::sync::Mutex<VoxflowCore>>) {
    std::thread::Builder::new()
        .name("voxflow-engine-preload".to_string())
        .spawn(move || {
            let (config, paths, slot, sender) = {
                let mut guard = core.blocking_lock();
                guard.set_engine_state("loading");
                (
                    guard.config_clone(),
                    guard.paths_clone(),
                    guard.engine_slot(),
                    guard.event_sender_clone(),
                )
            };
            let backend = config.asr.backend;
            if backend == AsrBackend::Mock {
                core.blocking_lock().mark_engine_ready(backend);
                return;
            }
            tracing::info!(
                backend = crate::backend::backend_label(backend),
                "preloading ASR engine"
            );
            let result =
                crate::backend::build_recognizer(&config, &paths).and_then(|mut recognizer| {
                    // First session forces the heavy load (qwen3 sidecar init
                    // includes its own decode warmup).
                    let warm = recognizer.start_session()?;
                    let _ = recognizer.finish_session(&warm)?;
                    Ok(recognizer)
                });
            match result {
                Ok(recognizer) => {
                    *slot.lock().expect("engine slot") = Some(recognizer);
                    core.blocking_lock().mark_engine_ready(backend);
                    tracing::info!("ASR engine resident and warm");
                    if let Some(sender) = sender {
                        let _ = sender.send(Envelope::event(
                            "core.notice",
                            serde_json::json!({
                                "level": "info",
                                "code": "asr.engine_ready",
                                "message": "语音识别引擎已就绪",
                            }),
                        ));
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "engine preload failed");
                    core.blocking_lock()
                        .set_engine_state(format!("error: {error}"));
                }
            }
        })
        .expect("spawn preload thread");
}
