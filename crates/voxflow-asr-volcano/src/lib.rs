//! 火山引擎大模型流式语音识别(Doubao/Seed-ASR API)backend。
//!
//! One WebSocket connection per dictation session. A dedicated IO thread owns
//! the socket: it drains an outbound audio channel, reads server frames with a
//! short socket timeout, and maps responses into [`AsrEvent`]s (partial from
//! the running text, stable via [`StablePrefixStabilizer`], final per
//! `definite` utterance and at stream end).
//!
//! Credentials (app key / access key) come from user config only — never the
//! repository.

pub mod protocol;

use std::collections::VecDeque;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};
use voxflow_asr::{
    AsrEvent, AudioFrame, SessionId, StablePrefixStabilizer, StreamingRecognizer, Token,
};

const SOCKET_POLL_TIMEOUT: Duration = Duration::from_millis(50);
const FINAL_WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct VolcanoOptions {
    pub endpoint: String,
    pub app_key: String,
    pub access_key: String,
    pub resource_id: String,
    pub model_name: String,
    pub enable_itn: bool,
    pub enable_punc: bool,
    pub sample_rate_hz: u32,
}

impl VolcanoOptions {
    pub fn validate(&self) -> Result<()> {
        if self.app_key.trim().is_empty() || self.access_key.trim().is_empty() {
            bail!("volcano app_key/access_key is not configured");
        }
        if !self.endpoint.starts_with("wss://") {
            bail!("volcano endpoint must be wss://");
        }
        Ok(())
    }

    fn session_payload(&self) -> Value {
        json!({
            "user": { "uid": "voxflow" },
            "audio": {
                "format": "pcm",
                "rate": self.sample_rate_hz,
                "bits": 16,
                "channel": 1,
            },
            "request": {
                "model_name": self.model_name,
                "enable_itn": self.enable_itn,
                "enable_punc": self.enable_punc,
                "show_utterances": true,
                "result_type": "full",
            },
        })
    }
}

/// Pure response→event mapping; kept separate from IO so it is unit-testable.
struct Mapper {
    session_id: SessionId,
    events: Arc<Mutex<VecDeque<AsrEvent>>>,
    stabilizer: StablePrefixStabilizer,
    revision: u64,
    last_text: String,
    finals_emitted: usize,
}

impl Mapper {
    fn new(session_id: SessionId, events: Arc<Mutex<VecDeque<AsrEvent>>>) -> Self {
        Self {
            session_id,
            events,
            stabilizer: StablePrefixStabilizer::new(),
            revision: 0,
            last_text: String::new(),
            finals_emitted: 0,
        }
    }

    fn push(&self, event: AsrEvent) {
        self.events.lock().expect("events lock").push_back(event);
    }

    fn apply_payload(&mut self, payload: &Value) {
        let result = &payload["result"];
        let text = result["text"].as_str().unwrap_or_default().to_string();
        if !text.is_empty() && text != self.last_text {
            self.revision += 1;
            let tokens = char_tokens(&text);
            self.push(AsrEvent::Partial {
                revision: self.revision,
                text: text.clone(),
                tokens: tokens.clone(),
            });
            if let Some(stable) = self
                .stabilizer
                .observe_partial(self.revision, &text, tokens)
            {
                self.push(stable);
            }
            self.last_text = text;
        }
        if let Some(utterances) = result["utterances"].as_array() {
            let definite = utterances
                .iter()
                .filter(|utterance| utterance["definite"].as_bool().unwrap_or(false))
                .collect::<Vec<_>>();
            while self.finals_emitted < definite.len() {
                let utterance_text = definite[self.finals_emitted]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                self.finals_emitted += 1;
                if !utterance_text.is_empty() {
                    self.revision += 1;
                    let segment = self.finals_emitted;
                    self.push(AsrEvent::Final {
                        revision: self.revision,
                        text: utterance_text,
                        segment_id: format!("{}-seg-{segment}", self.session_id),
                    });
                }
            }
        }
    }

    /// If no utterance was ever marked definite, surface the remaining text
    /// as a final when the stream ends.
    fn finish(&mut self) {
        if self.finals_emitted == 0 && !self.last_text.is_empty() {
            self.revision += 1;
            self.push(AsrEvent::Final {
                revision: self.revision,
                text: self.last_text.clone(),
                segment_id: format!("{}-seg-1", self.session_id),
            });
        }
    }
}

fn char_tokens(text: &str) -> Vec<Token> {
    text.chars()
        .map(|ch| Token {
            text: ch.to_string(),
            start_ms: 0,
            end_ms: 0,
        })
        .collect()
}

enum IoCommand {
    Audio(Vec<u8>),
    Finish,
}

struct SessionHandle {
    id: SessionId,
    commands: Sender<IoCommand>,
    events: Arc<Mutex<VecDeque<AsrEvent>>>,
    error: Arc<Mutex<Option<String>>>,
    finished: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

pub struct VolcanoRecognizer {
    options: VolcanoOptions,
    session_counter: u64,
    active: Option<SessionHandle>,
}

impl VolcanoRecognizer {
    pub fn new(options: VolcanoOptions) -> Result<Self> {
        options.validate()?;
        Ok(Self {
            options,
            session_counter: 0,
            active: None,
        })
    }

    fn take_error(&self) -> Option<String> {
        self.active
            .as_ref()
            .and_then(|handle| handle.error.lock().expect("error lock").clone())
    }
}

impl StreamingRecognizer for VolcanoRecognizer {
    fn start_session(&mut self) -> Result<SessionId> {
        if self.active.is_some() {
            bail!("volcano session already active");
        }
        self.session_counter += 1;
        let id = format!("volcano-{}", self.session_counter);
        let (command_tx, command_rx) = mpsc::channel();
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let error = Arc::new(Mutex::new(None));
        let finished = Arc::new(AtomicBool::new(false));

        let socket = connect(&self.options)?;
        let payload = self.options.session_payload();
        let worker = IoWorker {
            socket,
            commands: command_rx,
            error: Arc::clone(&error),
            finished: Arc::clone(&finished),
            mapper: Mapper::new(id.clone(), Arc::clone(&events)),
            sequence: 1,
        };
        let join = std::thread::Builder::new()
            .name("voxflow-volcano-io".to_string())
            .spawn(move || worker.run(payload))
            .context("spawn volcano io thread")?;

        self.active = Some(SessionHandle {
            id: id.clone(),
            commands: command_tx,
            events,
            error,
            finished,
            join: Some(join),
        });
        Ok(id)
    }

    fn push_audio(&mut self, session: &SessionId, frame: AudioFrame) -> Result<()> {
        if let Some(message) = self.take_error() {
            bail!("volcano stream error: {message}");
        }
        let handle = self.active.as_ref().context("no active volcano session")?;
        if &handle.id != session {
            bail!("unknown volcano session {session}");
        }
        if frame.channels != 1 {
            bail!("volcano backend expects mono audio");
        }
        let mut bytes = Vec::with_capacity(frame.pcm_i16.len() * 2);
        for sample in &frame.pcm_i16 {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        handle
            .commands
            .send(IoCommand::Audio(bytes))
            .context("volcano io thread stopped")?;
        Ok(())
    }

    fn poll_events(&mut self, session: &SessionId) -> Result<Vec<AsrEvent>> {
        let Some(handle) = self.active.as_ref() else {
            return Ok(Vec::new());
        };
        if &handle.id != session {
            return Ok(Vec::new());
        }
        Ok(handle
            .events
            .lock()
            .expect("events lock")
            .drain(..)
            .collect())
    }

    fn finish_session(&mut self, session: &SessionId) -> Result<Vec<AsrEvent>> {
        let Some(handle) = self.active.as_mut() else {
            bail!("no active volcano session");
        };
        if &handle.id != session {
            bail!("unknown volcano session {session}");
        }
        let _ = handle.commands.send(IoCommand::Finish);
        let deadline = std::time::Instant::now() + FINAL_WAIT;
        while !handle.finished.load(Ordering::Relaxed) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let handle = self.active.take().expect("checked above");
        if let Some(join) = handle.join {
            let _ = join.join();
        }
        let events = handle
            .events
            .lock()
            .expect("events lock")
            .drain(..)
            .collect();
        if let Some(message) = handle.error.lock().expect("error lock").clone() {
            tracing::warn!(%message, "volcano session ended with error");
        }
        Ok(events)
    }
}

type WsStream = WebSocket<MaybeTlsStream<TcpStream>>;

fn connect(options: &VolcanoOptions) -> Result<WsStream> {
    let mut request = options
        .endpoint
        .as_str()
        .into_client_request()
        .context("build volcano ws request")?;
    let connect_id = format!(
        "voxflow-{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|t| t.as_nanos())
            .unwrap_or(0)
    );
    let headers = request.headers_mut();
    headers.insert("X-Api-App-Key", HeaderValue::from_str(&options.app_key)?);
    headers.insert(
        "X-Api-Access-Key",
        HeaderValue::from_str(&options.access_key)?,
    );
    headers.insert(
        "X-Api-Resource-Id",
        HeaderValue::from_str(&options.resource_id)?,
    );
    headers.insert("X-Api-Connect-Id", HeaderValue::from_str(&connect_id)?);

    let (mut socket, _response) =
        tungstenite::connect(request).context("connect volcano ws endpoint")?;
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => {
            stream.set_read_timeout(Some(SOCKET_POLL_TIMEOUT))?;
        }
        MaybeTlsStream::Rustls(stream) => {
            stream
                .get_mut()
                .set_read_timeout(Some(SOCKET_POLL_TIMEOUT))?;
        }
        _ => {}
    }
    Ok(socket)
}

struct IoWorker {
    socket: WsStream,
    commands: Receiver<IoCommand>,
    error: Arc<Mutex<Option<String>>>,
    finished: Arc<AtomicBool>,
    mapper: Mapper,
    sequence: i32,
}

impl IoWorker {
    fn run(mut self, session_payload: Value) {
        if let Err(error) = self.run_inner(&session_payload) {
            *self.error.lock().expect("error lock") = Some(error.to_string());
        }
        self.finished.store(true, Ordering::Relaxed);
    }

    fn run_inner(&mut self, session_payload: &Value) -> Result<()> {
        let frame = protocol::encode_full_request(session_payload, self.sequence)?;
        self.socket
            .send(Message::Binary(frame))
            .context("send volcano full request")?;

        let mut finishing = false;
        loop {
            if !finishing {
                loop {
                    match self.commands.try_recv() {
                        Ok(IoCommand::Audio(pcm)) => {
                            self.sequence += 1;
                            let frame = protocol::encode_audio_request(&pcm, self.sequence, false)?;
                            self.socket
                                .send(Message::Binary(frame))
                                .context("send volcano audio")?;
                        }
                        Ok(IoCommand::Finish) | Err(mpsc::TryRecvError::Disconnected) => {
                            self.sequence += 1;
                            let frame = protocol::encode_audio_request(&[], self.sequence, true)?;
                            self.socket
                                .send(Message::Binary(frame))
                                .context("send volcano last packet")?;
                            finishing = true;
                            break;
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
            }

            match self.socket.read() {
                Ok(Message::Binary(frame)) => {
                    let message = protocol::decode_server_message(&frame)?;
                    if message.message_type == protocol::SERVER_ERROR_RESPONSE {
                        bail!(
                            "volcano error {}: {}",
                            message.error_code,
                            message
                                .payload
                                .map(|payload| payload.to_string())
                                .unwrap_or_default()
                        );
                    }
                    if let Some(payload) = &message.payload {
                        self.mapper.apply_payload(payload);
                    }
                    if message.is_last_package {
                        self.mapper.finish();
                        return Ok(());
                    }
                }
                Ok(Message::Close(_)) => {
                    self.mapper.finish();
                    return Ok(());
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(error))
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        || error.kind() == std::io::ErrorKind::TimedOut => {}
                Err(error) => {
                    if finishing {
                        self.mapper.finish();
                        return Ok(());
                    }
                    return Err(error).context("read volcano ws");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mapper() -> (Mapper, Arc<Mutex<VecDeque<AsrEvent>>>) {
        let events = Arc::new(Mutex::new(VecDeque::new()));
        (
            Mapper::new("volcano-1".to_string(), Arc::clone(&events)),
            events,
        )
    }

    fn drain(events: &Arc<Mutex<VecDeque<AsrEvent>>>) -> Vec<AsrEvent> {
        events.lock().unwrap().drain(..).collect()
    }

    #[test]
    fn growing_text_produces_partial_then_stable() {
        let (mut mapper, events) = mapper();
        mapper.apply_payload(&json!({"result": {"text": "今天"}}));
        mapper.apply_payload(&json!({"result": {"text": "今天天气"}}));
        let collected = drain(&events);
        assert!(matches!(collected[0], AsrEvent::Partial { .. }));
        assert!(collected
            .iter()
            .any(|event| matches!(event, AsrEvent::Stable { .. })));
    }

    #[test]
    fn definite_utterances_become_finals_once() {
        let (mut mapper, events) = mapper();
        let payload = json!({"result": {
            "text": "今天天气很好。",
            "utterances": [{"text": "今天天气很好。", "definite": true}],
        }});
        mapper.apply_payload(&payload);
        mapper.apply_payload(&payload);
        let finals = drain(&events)
            .into_iter()
            .filter(|event| matches!(event, AsrEvent::Final { .. }))
            .count();
        assert_eq!(finals, 1);
    }

    #[test]
    fn finish_emits_trailing_final_when_no_definite_utterance() {
        let (mut mapper, events) = mapper();
        mapper.apply_payload(&json!({"result": {"text": "尚未定稿"}}));
        mapper.finish();
        let collected = drain(&events);
        match collected.last().unwrap() {
            AsrEvent::Final { text, .. } => assert_eq!(text, "尚未定稿"),
            other => panic!("expected final, got {other:?}"),
        }
    }

    #[test]
    fn missing_credentials_rejected() {
        let options = VolcanoOptions {
            endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel".to_string(),
            app_key: String::new(),
            access_key: String::new(),
            resource_id: "volc.bigasr.sauc.duration".to_string(),
            model_name: "bigmodel".to_string(),
            enable_itn: true,
            enable_punc: true,
            sample_rate_hz: 16_000,
        };
        assert!(VolcanoRecognizer::new(options).is_err());
    }
}
