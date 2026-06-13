use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, watch, Mutex};

use crate::core::VoxflowCore;
use crate::ipc::Envelope;

pub async fn serve(core: Arc<Mutex<VoxflowCore>>, socket: &Path) -> Result<()> {
    if let Some(parent) = socket.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("create socket dir {}", parent.display()))?;
    }
    if socket.exists() {
        fs::remove_file(socket)
            .await
            .with_context(|| format!("remove stale socket {}", socket.display()))?;
    }
    let listener =
        UnixListener::bind(socket).with_context(|| format!("bind {}", socket.display()))?;
    tracing::info!(socket = %socket.display(), "voxflow-core listening");
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let (event_tx, _) = broadcast::channel(256);
    core.lock().await.set_event_sender(event_tx.clone());
    #[cfg(feature = "live-asr")]
    crate::runtime::spawn_preload(core.clone());

    loop {
        tokio::select! {
            signal = tokio::signal::ctrl_c() => {
                signal.context("listen for Ctrl+C")?;
                tracing::info!("voxflow-core stopping after Ctrl+C");
                break;
            }
            changed = shutdown_rx.changed() => {
                if changed.is_ok() && *shutdown_rx.borrow() {
                    tracing::info!("voxflow-core stopping after IPC shutdown");
                    break;
                }
            }
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let core = Arc::clone(&core);
                let shutdown_tx = shutdown_tx.clone();
                let event_tx = event_tx.clone();
                tokio::spawn(async move {
                    if let Err(error) = handle_client(stream, core, shutdown_tx, event_tx).await {
                        tracing::warn!(%error, "ipc client failed");
                    }
                });
            }
        }
    }

    if socket.exists() {
        fs::remove_file(socket)
            .await
            .with_context(|| format!("remove socket {}", socket.display()))?;
    }
    Ok(())
}

async fn handle_client(
    stream: UnixStream,
    core: Arc<Mutex<VoxflowCore>>,
    shutdown_tx: watch::Sender<bool>,
    event_tx: broadcast::Sender<Envelope>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let mut event_rx = event_tx.subscribe();
    let mut subscribed_groups: Option<HashSet<String>> = None;
    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line? else {
                    break;
                };
                if line.trim().is_empty() {
                    continue;
                }
                let request: Envelope =
                    serde_json::from_str(&line).with_context(|| format!("parse ipc line: {line}"))?;
                let subscribe_groups = if request.name == "core.subscribe" {
                    Some(parse_subscribe_groups(&request))
                } else {
                    None
                };
                let outcome = core.lock().await.handle_command(request);
                write_envelope(&mut writer, &outcome.response).await?;
                if let Some(groups) = subscribe_groups {
                    subscribed_groups = Some(groups);
                }
                for event in outcome.events {
                    let _ = event_tx.send(event.clone());
                    if subscribed_groups.is_none() {
                        write_envelope(&mut writer, &event).await?;
                    }
                }
                if outcome.shutdown {
                    let _ = shutdown_tx.send(true);
                    break;
                }
            }
            event = event_rx.recv() => {
                match event {
                    Ok(event) => {
                        if should_deliver(&subscribed_groups, &event) {
                            write_envelope(&mut writer, &event).await?;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "ipc subscriber lagged behind event stream");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    Ok(())
}

async fn write_envelope(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    envelope: &Envelope,
) -> Result<()> {
    let mut line = serde_json::to_vec(envelope)?;
    line.push(b'\n');
    writer.write_all(&line).await?;
    writer.flush().await?;
    Ok(())
}

fn parse_subscribe_groups(request: &Envelope) -> HashSet<String> {
    request
        .payload
        .get("groups")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn should_deliver(groups: &Option<HashSet<String>>, event: &Envelope) -> bool {
    let Some(groups) = groups else {
        return false;
    };
    groups.is_empty() || groups.contains(event_group(&event.name))
}

fn event_group(name: &str) -> &str {
    if name.starts_with("dictation.") {
        "dictation"
    } else if name.starts_with("audio.level") {
        "audio_level"
    } else if name.starts_with("model.") {
        "model"
    } else if name.starts_with("correction.") {
        "correction"
    } else {
        "state"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn subscribe_groups_filter_events_by_documented_group() {
        let request = Envelope::command(
            "s-1",
            "core.subscribe",
            json!({ "groups": ["dictation", "state"] }),
        );
        let groups = Some(parse_subscribe_groups(&request));
        assert!(should_deliver(
            &groups,
            &Envelope::event("dictation.partial", json!({}))
        ));
        assert!(should_deliver(
            &groups,
            &Envelope::event("frontend.state_changed", json!({}))
        ));
        assert!(!should_deliver(
            &groups,
            &Envelope::event("model.progress", json!({}))
        ));
    }

    #[test]
    fn empty_subscription_receives_all_event_groups() {
        let request = Envelope::command("s-1", "core.subscribe", json!({}));
        let groups = Some(parse_subscribe_groups(&request));
        assert!(should_deliver(
            &groups,
            &Envelope::event("model.progress", json!({}))
        ));
    }
}
