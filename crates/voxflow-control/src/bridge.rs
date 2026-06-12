use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;
use voxflow_ipc::{Envelope, MessageKind, StatusSnapshot, PROTOCOL_VERSION};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeConnectionInfo {
    pub socket: PathBuf,
    pub connected: bool,
    pub selected_protocol: Option<u16>,
    pub core_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BridgeCommandOutcome {
    pub reply: Envelope,
    pub events_before_reply: Vec<Envelope>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReconnectPolicy {
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub multiplier: u64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            initial_delay_ms: 500,
            max_delay_ms: 10_000,
            multiplier: 2,
        }
    }
}

impl ReconnectPolicy {
    pub fn delay_for_attempt(self, attempt: u32) -> Duration {
        let multiplier = self.multiplier.max(1);
        let mut delay = self.initial_delay_ms.max(1);
        for _ in 0..attempt {
            delay = delay.saturating_mul(multiplier).min(self.max_delay_ms);
        }
        Duration::from_millis(delay.min(self.max_delay_ms))
    }
}

pub struct CoreBridge {
    socket: PathBuf,
    reader: Lines<BufReader<OwnedReadHalf>>,
    writer: OwnedWriteHalf,
    next_id: u64,
    selected_protocol: Option<u16>,
    core_version: Option<String>,
}

impl CoreBridge {
    pub async fn connect(socket: impl AsRef<Path>) -> Result<Self> {
        let socket = socket.as_ref().to_path_buf();
        let stream = UnixStream::connect(&socket)
            .await
            .with_context(|| format!("connect core socket {}", socket.display()))?;
        Ok(Self::from_stream(socket, stream))
    }

    pub fn from_stream(socket: impl Into<PathBuf>, stream: UnixStream) -> Self {
        let socket = socket.into();
        let (reader, writer) = stream.into_split();
        Self {
            socket,
            reader: BufReader::new(reader).lines(),
            writer,
            next_id: 0,
            selected_protocol: None,
            core_version: None,
        }
    }

    pub fn info(&self) -> BridgeConnectionInfo {
        BridgeConnectionInfo {
            socket: self.socket.clone(),
            connected: true,
            selected_protocol: self.selected_protocol,
            core_version: self.core_version.clone(),
        }
    }

    pub async fn hello(&mut self, client: &str, client_version: &str) -> Result<Envelope> {
        let outcome = self
            .command(
                "core.hello",
                json!({
                    "client": client,
                    "client_version": client_version,
                    "proto_versions": [PROTOCOL_VERSION],
                }),
            )
            .await?;
        ensure_non_error(&outcome.reply)?;
        self.selected_protocol = outcome
            .reply
            .payload
            .get("selected_version")
            .and_then(|value| value.as_u64())
            .and_then(|version| u16::try_from(version).ok());
        self.core_version = outcome
            .reply
            .payload
            .get("core_version")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        Ok(outcome.reply)
    }

    pub async fn status(&mut self) -> Result<StatusSnapshot> {
        let outcome = self.command("core.status", json!({})).await?;
        ensure_non_error(&outcome.reply)?;
        serde_json::from_value(outcome.reply.payload).context("decode core.status payload")
    }

    pub async fn subscribe(&mut self, groups: &[&str]) -> Result<Envelope> {
        let outcome = self
            .command("core.subscribe", json!({ "groups": groups.to_vec() }))
            .await?;
        ensure_non_error(&outcome.reply)?;
        Ok(outcome.reply)
    }

    pub async fn command(&mut self, name: &str, payload: Value) -> Result<BridgeCommandOutcome> {
        let id = self.next_command_id();
        let command = Envelope::command(id.clone(), name, payload);
        self.write_envelope(&command).await?;

        let mut events_before_reply = Vec::new();
        loop {
            let Some(envelope) = self.read_next().await? else {
                bail!("core socket closed before response to {name}");
            };
            match envelope.kind {
                MessageKind::Response | MessageKind::Error
                    if envelope.id.as_deref() == Some(&id) =>
                {
                    return Ok(BridgeCommandOutcome {
                        reply: envelope,
                        events_before_reply,
                    });
                }
                MessageKind::Event => events_before_reply.push(envelope),
                _ => {
                    bail!(
                        "unexpected envelope while waiting for response to {name}: kind={:?} name={} id={:?}",
                        envelope.kind,
                        envelope.name,
                        envelope.id
                    );
                }
            }
        }
    }

    pub async fn read_next(&mut self) -> Result<Option<Envelope>> {
        loop {
            let Some(line) = self.reader.next_line().await? else {
                return Ok(None);
            };
            if line.trim().is_empty() {
                continue;
            }
            return serde_json::from_str(&line)
                .with_context(|| format!("parse core IPC line: {line}"));
        }
    }

    async fn write_envelope(&mut self, envelope: &Envelope) -> Result<()> {
        let mut line = serde_json::to_vec(envelope)?;
        line.push(b'\n');
        self.writer.write_all(&line).await?;
        self.writer.flush().await?;
        Ok(())
    }

    fn next_command_id(&mut self) -> String {
        self.next_id += 1;
        format!("ui-{}", self.next_id)
    }
}

fn ensure_non_error(envelope: &Envelope) -> Result<()> {
    if envelope.kind == MessageKind::Error {
        bail!(
            "core returned {}: {}",
            envelope.code.as_deref().unwrap_or("core.unknown_error"),
            envelope.message.as_deref().unwrap_or("unknown error")
        );
    }
    Ok(())
}
