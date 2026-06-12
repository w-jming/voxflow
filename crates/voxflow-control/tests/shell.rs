use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use voxflow_control::bridge::ReconnectPolicy;
use voxflow_control::sample_status_snapshot;
use voxflow_control::shell::{
    disconnected_retry_event, CoreCommandInvocation, ShellIpcSession, VecEventSink,
    DEFAULT_UI_SUBSCRIPTIONS, TAURI_CONNECTION_EVENT, TAURI_CORE_EVENT, TAURI_SNAPSHOT_EVENT,
};
use voxflow_ipc::{Envelope, PROTOCOL_VERSION};

#[tokio::test]
async fn shell_session_emits_connection_and_snapshot_events_on_connect() {
    let (client, server_stream) = UnixStream::pair().unwrap();
    let server = tokio::spawn(async move {
        serve_scripted_shell(server_stream, false).await;
    });
    let mut sink = VecEventSink::default();

    let session = ShellIpcSession::from_stream(
        "/tmp/voxflow-shell-connect",
        client,
        DEFAULT_UI_SUBSCRIPTIONS,
        &mut sink,
    )
    .await
    .unwrap();

    assert_eq!(session.subscriptions(), DEFAULT_UI_SUBSCRIPTIONS);
    assert_eq!(sink.events[0].name, TAURI_CONNECTION_EVENT);
    assert_eq!(sink.events[0].payload["state"], "connecting");
    assert!(
        sink.events
            .iter()
            .any(|event| event.name == TAURI_CONNECTION_EVENT
                && event.payload["state"] == "connected")
    );
    assert!(sink
        .events
        .iter()
        .any(|event| event.name == TAURI_SNAPSHOT_EVENT
            && event.payload["current_model"] == "streaming-zh-en-small"));

    drop(session);
    server.await.unwrap();
}

#[tokio::test]
async fn shell_command_forwards_reply_and_emits_core_events_before_reply() {
    let (client, server_stream) = UnixStream::pair().unwrap();
    let server = tokio::spawn(async move {
        serve_scripted_shell(server_stream, true).await;
    });
    let mut sink = VecEventSink::default();
    let mut session = ShellIpcSession::from_stream(
        "/tmp/voxflow-shell-command",
        client,
        DEFAULT_UI_SUBSCRIPTIONS,
        &mut sink,
    )
    .await
    .unwrap();

    let reply = session
        .invoke_core_command(
            CoreCommandInvocation {
                name: "diagnostics.run".to_string(),
                payload: json!({}),
            },
            &mut sink,
        )
        .await
        .unwrap();

    assert_eq!(reply.name, "diagnostics.run");
    assert_eq!(reply.payload["checks"][0]["name"], "mock");
    assert!(sink
        .events
        .iter()
        .any(|event| event.name == TAURI_CORE_EVENT && event.payload["name"] == "core.notice"));

    drop(session);
    server.await.unwrap();
}

#[test]
fn disconnected_retry_event_uses_policy_delay() {
    let event = disconnected_retry_event("socket closed", 2, ReconnectPolicy::default());

    assert_eq!(event.name, TAURI_CONNECTION_EVENT);
    assert_eq!(event.payload["state"], "disconnected");
    assert_eq!(event.payload["retry_after_ms"], 2000);
    assert_eq!(event.payload["error"], "socket closed");
}

async fn serve_scripted_shell(stream: UnixStream, send_event_before_diagnostics_reply: bool) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await.unwrap() {
        let request: Envelope = serde_json::from_str(&line).unwrap();
        match request.name.as_str() {
            "core.hello" => {
                write_envelope(
                    &mut writer,
                    &Envelope::response(
                        request.id.clone(),
                        "core.hello",
                        json!({
                            "selected_version": PROTOCOL_VERSION,
                            "core_version": "0.3.0",
                            "server": "voxflow-core",
                        }),
                    ),
                )
                .await;
            }
            "core.subscribe" => {
                write_envelope(
                    &mut writer,
                    &Envelope::response(
                        request.id.clone(),
                        "core.subscribe",
                        json!({ "accepted": true }),
                    ),
                )
                .await;
            }
            "core.status" => {
                write_envelope(
                    &mut writer,
                    &Envelope::response(
                        request.id.clone(),
                        "core.status",
                        json!(sample_status_snapshot()),
                    ),
                )
                .await;
            }
            "diagnostics.run" => {
                if send_event_before_diagnostics_reply {
                    write_envelope(
                        &mut writer,
                        &Envelope::event(
                            "core.notice",
                            json!({
                                "level": "warning",
                                "code": "diagnostics.mock",
                                "message": "mock notice",
                            }),
                        ),
                    )
                    .await;
                }
                write_envelope(
                    &mut writer,
                    &Envelope::response(
                        request.id.clone(),
                        "diagnostics.run",
                        json!({ "checks": [{ "name": "mock", "status": "passed" }] }),
                    ),
                )
                .await;
            }
            other => {
                write_envelope(
                    &mut writer,
                    &Envelope::error(
                        request.id.clone(),
                        other,
                        "core.unknown_command",
                        "unknown command",
                        true,
                        json!({}),
                    ),
                )
                .await;
            }
        }
    }
}

async fn write_envelope(writer: &mut tokio::net::unix::OwnedWriteHalf, envelope: &Envelope) {
    let mut line = serde_json::to_vec(envelope).unwrap();
    line.push(b'\n');
    writer.write_all(&line).await.unwrap();
    writer.flush().await.unwrap();
}
