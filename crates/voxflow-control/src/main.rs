use anyhow::{bail, Result};
use voxflow_control::bridge::CoreBridge;
use voxflow_control::shell::{
    CoreCommandInvocation, ShellIpcSession, VecEventSink, DEFAULT_UI_SUBSCRIPTIONS,
};
use voxflow_control::{
    sample_control_center_snapshot, write_static_bundle, ConnectionState, ControlCenterSnapshot,
};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some("snapshot-json") => {
            println!(
                "{}",
                serde_json::to_string_pretty(&sample_control_center_snapshot())?
            );
            Ok(())
        }
        Some("write-web") => {
            let dir = args
                .next()
                .unwrap_or_else(|| "target/voxflow-control-web".to_string());
            write_static_bundle(&dir)?;
            println!("{dir}");
            Ok(())
        }
        Some("bridge-status") => {
            let Some(socket) = args.next() else {
                bail!("bridge-status requires SOCKET");
            };
            let mut bridge = CoreBridge::connect(&socket).await?;
            bridge.hello("ui", env!("CARGO_PKG_VERSION")).await?;
            let status = bridge.status().await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&ControlCenterSnapshot::from_status(
                    status,
                    ConnectionState::Connected
                ))?
            );
            Ok(())
        }
        Some("shell-status") => {
            let Some(socket) = args.next() else {
                bail!("shell-status requires SOCKET");
            };
            let mut sink = VecEventSink::default();
            let _session =
                ShellIpcSession::connect(&socket, DEFAULT_UI_SUBSCRIPTIONS, &mut sink).await?;
            println!("{}", serde_json::to_string_pretty(&sink.events)?);
            Ok(())
        }
        Some("shell-command") => {
            let Some(socket) = args.next() else {
                bail!("shell-command requires SOCKET NAME [PAYLOAD_JSON]");
            };
            let Some(name) = args.next() else {
                bail!("shell-command requires SOCKET NAME [PAYLOAD_JSON]");
            };
            let payload = args
                .next()
                .map(|text| serde_json::from_str(&text))
                .transpose()?
                .unwrap_or_else(|| serde_json::json!({}));
            let mut sink = VecEventSink::default();
            let mut session =
                ShellIpcSession::connect(&socket, DEFAULT_UI_SUBSCRIPTIONS, &mut sink).await?;
            let reply = session
                .invoke_core_command(CoreCommandInvocation { name, payload }, &mut sink)
                .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "reply": reply,
                    "events": sink.events,
                }))?
            );
            Ok(())
        }
        Some(other) => bail!("unknown command: {other}"),
    }
}

fn print_help() {
    println!(
        "voxflow-control 0.3.0\n\nUSAGE:\n  voxflow-control snapshot-json\n  voxflow-control write-web [output-dir]\n  voxflow-control bridge-status SOCKET\n  voxflow-control shell-status SOCKET\n  voxflow-control shell-command SOCKET NAME [PAYLOAD_JSON]\n"
    );
}
