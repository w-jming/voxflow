use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use voxflow_control::bridge::{CoreBridge, ReconnectPolicy};
use voxflow_control::sample_status_snapshot;
use voxflow_ipc::{Envelope, MessageKind, PROTOCOL_VERSION};

#[tokio::test]
async fn bridge_performs_hello_status_subscribe_and_reads_events() {
    let (client, server_stream) = UnixStream::pair().unwrap();
    let server = tokio::spawn(async move {
        let (reader, mut writer) = server_stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        while let Some(line) = lines.next_line().await.unwrap() {
            let request: Envelope = serde_json::from_str(&line).unwrap();
            let response = match request.name.as_str() {
                "core.hello" => Envelope::response(
                    request.id.clone(),
                    "core.hello",
                    json!({
                        "selected_version": PROTOCOL_VERSION,
                        "core_version": "0.3.0",
                        "server": "voxflow-core",
                    }),
                ),
                "core.status" => Envelope::response(
                    request.id.clone(),
                    "core.status",
                    json!(sample_status_snapshot()),
                ),
                "core.subscribe" => {
                    let response = Envelope::response(
                        request.id.clone(),
                        "core.subscribe",
                        json!({ "accepted": true }),
                    );
                    write_envelope(&mut writer, &response).await;
                    write_envelope(
                        &mut writer,
                        &Envelope::event(
                            "core.notice",
                            json!({
                                "level": "warning",
                                "code": "audio.test",
                                "message": "test notice",
                                "action_hint": "open_audio_page",
                            }),
                        ),
                    )
                    .await;
                    continue;
                }
                _ => Envelope::error(
                    request.id.clone(),
                    request.name,
                    "core.unknown_command",
                    "unknown command",
                    true,
                    json!({}),
                ),
            };
            write_envelope(&mut writer, &response).await;
        }
    });

    let mut bridge = CoreBridge::from_stream("/tmp/voxflow-control-bridge-test", client);
    let hello = bridge.hello("ui", "0.3.0").await.unwrap();
    assert_eq!(hello.payload["selected_version"], PROTOCOL_VERSION);
    assert_eq!(bridge.info().core_version.as_deref(), Some("0.3.0"));

    let status = bridge.status().await.unwrap();
    assert_eq!(status.models.active_asr, "streaming-zh-en-small");

    let subscribe = bridge.subscribe(&["state", "model"]).await.unwrap();
    assert_eq!(subscribe.kind, MessageKind::Response);
    let event = bridge.read_next().await.unwrap().unwrap();
    assert_eq!(event.kind, MessageKind::Event);
    assert_eq!(event.name, "core.notice");

    drop(bridge);
    server.await.unwrap();
}

#[tokio::test]
async fn bridge_surfaces_core_error_replies() {
    let (client, server_stream) = UnixStream::pair().unwrap();
    let server = tokio::spawn(async move {
        let (reader, mut writer) = server_stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        if let Some(line) = lines.next_line().await.unwrap() {
            let request: Envelope = serde_json::from_str(&line).unwrap();
            write_envelope(
                &mut writer,
                &Envelope::error(
                    request.id.clone(),
                    request.name,
                    "core.proto_unsupported",
                    "protocol version is not supported",
                    false,
                    json!({ "supported": [PROTOCOL_VERSION] }),
                ),
            )
            .await;
        }
    });

    let mut bridge = CoreBridge::from_stream("/tmp/voxflow-control-bridge-error-test", client);
    let error = bridge.hello("ui", "0.3.0").await.unwrap_err();
    assert!(error.to_string().contains("core.proto_unsupported"));

    drop(bridge);
    server.await.unwrap();
}

#[test]
fn reconnect_policy_uses_capped_exponential_backoff() {
    let policy = ReconnectPolicy::default();

    assert_eq!(policy.delay_for_attempt(0).as_millis(), 500);
    assert_eq!(policy.delay_for_attempt(1).as_millis(), 1000);
    assert_eq!(policy.delay_for_attempt(2).as_millis(), 2000);
    assert_eq!(policy.delay_for_attempt(10).as_millis(), 10000);
}

async fn write_envelope(writer: &mut tokio::net::unix::OwnedWriteHalf, envelope: &Envelope) {
    let mut line = serde_json::to_vec(envelope).unwrap();
    line.push(b'\n');
    writer.write_all(&line).await.unwrap();
    writer.flush().await.unwrap();
}
