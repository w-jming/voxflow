use std::env;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::json;
use voxflow_input::{
    frontend_register_command, frontend_report_command, DictationProjector, FrontendCapabilities,
    FrontendEvent, FrontendKind,
};
use voxflow_ipc::{Envelope, MessageKind, PROTOCOL_VERSION};

use crate::engine::{IbusEngineAdapter, IbusOperation};

const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_millis(150);

pub struct CoreIpcClient {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

pub trait IbusCoreBridge: Send + Sync {
    fn report_frontend_event(&mut self, event: FrontendEvent) -> Result<()>;
    fn start_dictation(&mut self) -> Result<Vec<IbusOperation>>;
    fn stop_dictation(&mut self) -> Result<Vec<IbusOperation>>;
}

pub struct CoreEngineSession {
    client: CoreIpcClient,
    projector: DictationProjector,
    adapter: IbusEngineAdapter,
    command_counter: u64,
}

impl CoreIpcClient {
    pub fn connect(socket: PathBuf) -> Result<Self> {
        let stream = UnixStream::connect(&socket)
            .with_context(|| format!("connect Core socket {}", socket.display()))?;
        let reader_stream = stream
            .try_clone()
            .context("clone Core socket for JSONL reader")?;
        reader_stream
            .set_read_timeout(Some(DEFAULT_IDLE_TIMEOUT))
            .context("set Core socket read timeout")?;
        Ok(Self {
            reader: BufReader::new(reader_stream),
            writer: stream,
        })
    }

    pub fn send_command(&mut self, command: Envelope) -> Result<Vec<Envelope>> {
        let mut line = serde_json::to_vec(&command)?;
        line.push(b'\n');
        self.writer.write_all(&line)?;
        self.writer.flush()?;

        let mut envelopes = Vec::new();
        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let envelope = serde_json::from_str::<Envelope>(&line)
                        .with_context(|| format!("parse Core JSONL response: {line}"))?;
                    envelopes.push(envelope);
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    break;
                }
                Err(error) => return Err(error).context("read Core JSONL response"),
            }
        }

        if envelopes.is_empty() {
            bail!("Core returned no response for {}", command.name);
        }
        let response = &envelopes[0];
        match response.kind {
            MessageKind::Response => Ok(envelopes),
            MessageKind::Error => bail!(
                "Core returned error for {}: {}",
                response.name,
                response
                    .code
                    .as_deref()
                    .unwrap_or("core.error_without_code")
            ),
            _ => bail!("first Core reply for {} was not a response", command.name),
        }
    }
}

pub fn default_core_socket() -> Result<PathBuf> {
    if let Some(socket) = env::var_os("VOXFLOW_CORE_SOCKET") {
        return Ok(PathBuf::from(socket));
    }
    if let Some(runtime) = env::var_os("XDG_RUNTIME_DIR") {
        return Ok(PathBuf::from(runtime).join("voxflow").join("core.sock"));
    }
    if let Some(home) = env::var_os("VOXFLOW_HOME") {
        return Ok(PathBuf::from(home)
            .join("run")
            .join("voxflow")
            .join("core.sock"));
    }
    let home = env::var_os("HOME").context("HOME is not set; set VOXFLOW_CORE_SOCKET")?;
    Ok(PathBuf::from(home)
        .join(".voxflow")
        .join("run")
        .join("voxflow")
        .join("core.sock"))
}

impl CoreEngineSession {
    pub fn connect(socket: PathBuf) -> Result<Self> {
        let mut client = CoreIpcClient::connect(socket)?;
        client.send_command(Envelope::command(
            "ibus-hello-1",
            "core.hello",
            json!({
                "client": "voxflow-ibus",
                "client_version": env!("CARGO_PKG_VERSION"),
                "proto_versions": [PROTOCOL_VERSION],
            }),
        ))?;
        client.send_command(frontend_register_command(
            "ibus-register-1",
            FrontendKind::Ibus,
            env!("CARGO_PKG_VERSION"),
            &FrontendCapabilities::full(),
        ))?;
        client.send_command(Envelope::command(
            "ibus-subscribe-1",
            "core.subscribe",
            json!({ "groups": ["dictation", "state", "correction"] }),
        ))?;
        Ok(Self {
            client,
            projector: DictationProjector::default(),
            adapter: IbusEngineAdapter::new(FrontendCapabilities::full()),
            command_counter: 1,
        })
    }

    fn next_id(&mut self, prefix: &str) -> String {
        self.command_counter += 1;
        format!("{prefix}-{}", self.command_counter)
    }

    fn project_envelopes(&mut self, envelopes: Vec<Envelope>) -> Result<Vec<IbusOperation>> {
        let mut operations = Vec::new();
        for envelope in envelopes {
            if envelope.kind != MessageKind::Event {
                continue;
            }
            if !matches!(
                envelope.name.as_str(),
                "dictation.state_changed"
                    | "dictation.partial"
                    | "dictation.stable"
                    | "dictation.final"
                    | "correction.applied"
            ) {
                continue;
            }
            for input_event in self
                .projector
                .project(&envelope)
                .context("project Core input event")?
            {
                operations.extend(self.adapter.translate(input_event));
            }
        }
        Ok(operations)
    }
}

impl IbusCoreBridge for CoreEngineSession {
    fn report_frontend_event(&mut self, event: FrontendEvent) -> Result<()> {
        let id = self.next_id("ibus-report");
        self.client
            .send_command(frontend_report_command(id, &event))?;
        Ok(())
    }

    fn start_dictation(&mut self) -> Result<Vec<IbusOperation>> {
        let id = self.next_id("ibus-dictation-start");
        let envelopes = self.client.send_command(Envelope::command(
            id,
            "dictation.start",
            json!({ "frontend": "ibus", "mode": "continuous" }),
        ))?;
        self.project_envelopes(envelopes)
    }

    fn stop_dictation(&mut self) -> Result<Vec<IbusOperation>> {
        let id = self.next_id("ibus-dictation-stop");
        let envelopes =
            self.client
                .send_command(Envelope::command(id, "dictation.stop", json!({})))?;
        self.project_envelopes(envelopes)
    }
}

pub fn run_mock_roundtrip(socket: PathBuf) -> Result<Vec<IbusOperation>> {
    let mut session = CoreEngineSession::connect(socket)?;
    session.report_frontend_event(FrontendEvent::Focused { app_hint: None })?;
    session.start_dictation()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_socket_prefers_explicit_env() {
        let old = env::var_os("VOXFLOW_CORE_SOCKET");
        env::set_var("VOXFLOW_CORE_SOCKET", "/tmp/voxflow-core-explicit.sock");
        assert_eq!(
            default_core_socket().unwrap(),
            PathBuf::from("/tmp/voxflow-core-explicit.sock")
        );
        match old {
            Some(value) => env::set_var("VOXFLOW_CORE_SOCKET", value),
            None => env::remove_var("VOXFLOW_CORE_SOCKET"),
        }
    }
}
