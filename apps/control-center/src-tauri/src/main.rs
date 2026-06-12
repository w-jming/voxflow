//! VoxFlow Control Center — Tauri 2 shell over the core IPC bridge.
//!
//! One pump task owns the `ShellIpcSession`: it forwards core events to the
//! WebView on the three fixed channels (frontend/tauri-ui.md) and serves
//! `core_command` invocations sent through an mpsc channel. Reconnects with
//! backoff when the core socket drops.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};
use voxflow_control::bridge::ReconnectPolicy;
use voxflow_control::shell::{
    disconnected_retry_event, CoreCommandInvocation, ShellEvent, ShellEventSink, ShellIpcSession,
    DEFAULT_UI_SUBSCRIPTIONS,
};

/// Forwards shell events to the WebView.
struct AppSink(AppHandle);

impl ShellEventSink for AppSink {
    fn emit(&mut self, event: ShellEvent) {
        if let Err(error) = Emitter::emit(&self.0, event.name.as_str(), event.payload) {
            tracing::warn!(%error, channel = event.name, "failed to emit shell event");
        }
    }
}

type CommandRequest = (
    CoreCommandInvocation,
    oneshot::Sender<Result<Value, String>>,
);

struct CommandQueue(mpsc::Sender<CommandRequest>);

fn core_socket_path() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("voxflow").join("core.sock")
}

#[tauri::command]
async fn core_command(
    state: tauri::State<'_, CommandQueue>,
    invocation: CoreCommandInvocation,
) -> Result<Value, String> {
    let (reply_tx, reply_rx) = oneshot::channel();
    state
        .0
        .send((invocation, reply_tx))
        .await
        .map_err(|_| "core.disconnected: bridge task stopped".to_string())?;
    reply_rx
        .await
        .map_err(|_| "core.disconnected: no reply from bridge task".to_string())?
}

async fn connection_pump(app: AppHandle, mut commands: mpsc::Receiver<CommandRequest>) {
    let policy = ReconnectPolicy::default();
    let mut attempt: u32 = 0;
    loop {
        let mut sink = AppSink(app.clone());
        let socket = core_socket_path();
        match ShellIpcSession::connect(&socket, DEFAULT_UI_SUBSCRIPTIONS, &mut sink).await {
            Ok(mut session) => {
                attempt = 0;
                pump_session(&app, &mut session, &mut commands).await;
            }
            Err(error) => {
                attempt = attempt.saturating_add(1);
                sink.emit(disconnected_retry_event(error.to_string(), attempt, policy));
            }
        }
        // Refuse queued commands while down so the UI fails fast.
        while let Ok((_, reply)) = commands.try_recv() {
            let _ = reply.send(Err("core.disconnected: core is not running".to_string()));
        }
        tokio::time::sleep(policy.delay_for_attempt(attempt.max(1))).await;
    }
}

async fn pump_session(
    app: &AppHandle,
    session: &mut ShellIpcSession,
    commands: &mut mpsc::Receiver<CommandRequest>,
) {
    let mut sink = AppSink(app.clone());
    loop {
        while let Ok((invocation, reply)) = commands.try_recv() {
            let result = session
                .invoke_core_command(invocation, &mut sink)
                .await
                .map_err(|error| format!("core.disconnected: {error}"))
                .and_then(|envelope| {
                    serde_json::to_value(envelope).map_err(|error| error.to_string())
                });
            let failed = result.is_err();
            let _ = reply.send(result);
            if failed {
                return;
            }
        }
        // next_line() is cancellation-safe, so bounding the poll lets queued
        // commands interleave with the event stream on a single session.
        match tokio::time::timeout(
            Duration::from_millis(100),
            session.poll_core_event_once(&mut sink),
        )
        .await
        {
            Ok(Ok(true)) => {}
            Ok(Ok(false)) | Ok(Err(_)) => return,
            Err(_timeout) => {}
        }
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let (command_tx, command_rx) = mpsc::channel::<CommandRequest>(32);

    tauri::Builder::default()
        .manage(CommandQueue(command_tx))
        .invoke_handler(tauri::generate_handler![core_command])
        .setup(move |app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(connection_pump(handle, command_rx));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running VoxFlow Control Center");
}
