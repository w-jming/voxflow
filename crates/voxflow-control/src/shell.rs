use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::UnixStream;
use voxflow_ipc::{Envelope, MessageKind};

use crate::bridge::{BridgeCommandOutcome, CoreBridge, ReconnectPolicy};
use crate::{ConnectionState, ControlCenterSnapshot};

pub const TAURI_CORE_EVENT: &str = "core-event";
pub const TAURI_CONNECTION_EVENT: &str = "connection-changed";
pub const TAURI_SNAPSHOT_EVENT: &str = "control-snapshot";
pub const DEFAULT_UI_SUBSCRIPTIONS: &[&str] = &["state", "model", "correction"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoreCommandInvocation {
    pub name: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShellEvent {
    pub name: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionChanged {
    pub state: ConnectionState,
    pub attempt: u32,
    pub retry_after_ms: Option<u64>,
    pub error: Option<String>,
}

pub trait ShellEventSink {
    fn emit(&mut self, event: ShellEvent);
}

#[derive(Debug, Default)]
pub struct VecEventSink {
    pub events: Vec<ShellEvent>,
}

impl ShellEventSink for VecEventSink {
    fn emit(&mut self, event: ShellEvent) {
        self.events.push(event);
    }
}

pub struct ShellIpcSession {
    bridge: CoreBridge,
    subscriptions: Vec<String>,
}

impl ShellIpcSession {
    pub async fn connect(
        socket: impl AsRef<Path>,
        subscriptions: &[&str],
        sink: &mut impl ShellEventSink,
    ) -> Result<Self> {
        emit_connection(sink, ConnectionState::Connecting, 0, None, None);
        let bridge = CoreBridge::connect(socket).await?;
        Self::from_connected_bridge(bridge, subscriptions, sink).await
    }

    pub async fn from_stream(
        label: impl Into<std::path::PathBuf>,
        stream: UnixStream,
        subscriptions: &[&str],
        sink: &mut impl ShellEventSink,
    ) -> Result<Self> {
        emit_connection(sink, ConnectionState::Connecting, 0, None, None);
        let bridge = CoreBridge::from_stream(label, stream);
        Self::from_connected_bridge(bridge, subscriptions, sink).await
    }

    pub fn subscriptions(&self) -> &[String] {
        &self.subscriptions
    }

    pub async fn invoke_core_command(
        &mut self,
        invocation: CoreCommandInvocation,
        sink: &mut impl ShellEventSink,
    ) -> Result<Envelope> {
        let outcome = self
            .bridge
            .command(&invocation.name, invocation.payload)
            .await
            .with_context(|| format!("forward core command {}", invocation.name))?;
        emit_outcome_events(sink, &outcome);
        Ok(outcome.reply)
    }

    pub async fn poll_core_event_once(&mut self, sink: &mut impl ShellEventSink) -> Result<bool> {
        let Some(envelope) = self.bridge.read_next().await? else {
            emit_connection(
                sink,
                ConnectionState::Disconnected,
                0,
                None,
                Some("core socket closed".to_string()),
            );
            return Ok(false);
        };
        if envelope.kind == MessageKind::Event {
            emit_core_event(sink, envelope);
        }
        Ok(true)
    }

    async fn from_connected_bridge(
        mut bridge: CoreBridge,
        subscriptions: &[&str],
        sink: &mut impl ShellEventSink,
    ) -> Result<Self> {
        bridge.hello("ui", env!("CARGO_PKG_VERSION")).await?;
        bridge.subscribe(subscriptions).await?;
        let status = bridge.status().await?;
        emit_connection(sink, ConnectionState::Connected, 0, None, None);
        emit_snapshot(
            sink,
            ControlCenterSnapshot::from_status(status, ConnectionState::Connected),
        )?;
        Ok(Self {
            bridge,
            subscriptions: subscriptions.iter().map(|item| item.to_string()).collect(),
        })
    }
}

pub fn disconnected_retry_event(
    error: impl Into<String>,
    attempt: u32,
    policy: ReconnectPolicy,
) -> ShellEvent {
    let retry_after_ms = policy.delay_for_attempt(attempt).as_millis() as u64;
    ShellEvent {
        name: TAURI_CONNECTION_EVENT.to_string(),
        payload: json!(ConnectionChanged {
            state: ConnectionState::Disconnected,
            attempt,
            retry_after_ms: Some(retry_after_ms),
            error: Some(error.into()),
        }),
    }
}

fn emit_connection(
    sink: &mut impl ShellEventSink,
    state: ConnectionState,
    attempt: u32,
    retry_after_ms: Option<u64>,
    error: Option<String>,
) {
    sink.emit(ShellEvent {
        name: TAURI_CONNECTION_EVENT.to_string(),
        payload: json!(ConnectionChanged {
            state,
            attempt,
            retry_after_ms,
            error,
        }),
    });
}

fn emit_snapshot(sink: &mut impl ShellEventSink, snapshot: ControlCenterSnapshot) -> Result<()> {
    sink.emit(ShellEvent {
        name: TAURI_SNAPSHOT_EVENT.to_string(),
        payload: serde_json::to_value(snapshot)?,
    });
    Ok(())
}

fn emit_outcome_events(sink: &mut impl ShellEventSink, outcome: &BridgeCommandOutcome) {
    for event in &outcome.events_before_reply {
        emit_core_event(sink, event.clone());
    }
}

fn emit_core_event(sink: &mut impl ShellEventSink, envelope: Envelope) {
    sink.emit(ShellEvent {
        name: TAURI_CORE_EVENT.to_string(),
        payload: json!(envelope),
    });
}
