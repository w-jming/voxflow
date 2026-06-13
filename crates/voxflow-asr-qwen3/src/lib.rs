//! Qwen3-ASR streaming backend over the official `qwen-asr` vLLM runtime.
//!
//! The model runs in a Python sidecar (`sidecar/qwen3_asr_sidecar.py`),
//! spoken to via line-delimited JSON on the child's stdin/stdout. The sidecar
//! holds the vLLM engine; this crate maps its incremental transcript into
//! partial/stable/final [`AsrEvent`]s (stable via [`StablePrefixStabilizer`]
//! over character tokens, final on `finish_session`).

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use anyhow::{bail, Context, Result};
use base64::Engine;
use serde::Deserialize;
use serde_json::json;
use voxflow_asr::{
    AsrEvent, AudioFrame, SessionId, StablePrefixStabilizer, StreamingRecognizer, Token,
};

#[derive(Debug, Clone)]
pub struct Qwen3SidecarOptions {
    pub python: String,
    pub sidecar_script: PathBuf,
    pub model: String,
    pub gpu_memory_utilization: f32,
    pub chunk_size_sec: f32,
    pub unfixed_chunk_num: u32,
    pub unfixed_token_num: u32,
    pub max_new_tokens: u32,
    pub max_model_len: u32,
    pub language: String,
}

impl Qwen3SidecarOptions {
    pub fn new(sidecar_script: impl Into<PathBuf>) -> Self {
        Self {
            python: "python3".to_string(),
            sidecar_script: sidecar_script.into(),
            model: "Qwen/Qwen3-ASR-1.7B".to_string(),
            gpu_memory_utilization: 0.7,
            chunk_size_sec: 2.0,
            unfixed_chunk_num: 2,
            unfixed_token_num: 5,
            max_new_tokens: 32,
            max_model_len: 16_384,
            language: "Chinese".to_string(),
        }
    }
}

/// Locates the sidecar script: explicit path, next to the executable, or the
/// repository `sidecar/` directory (dev runs).
pub fn resolve_sidecar_script(configured: &str) -> Option<PathBuf> {
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        return path.is_file().then_some(path);
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("qwen3_asr_sidecar.py"));
            candidates.push(dir.join("../sidecar/qwen3_asr_sidecar.py"));
        }
    }
    candidates.push(PathBuf::from("sidecar/qwen3_asr_sidecar.py"));
    candidates.into_iter().find(|path| path.is_file())
}

#[derive(Debug, Deserialize)]
struct SidecarReply {
    event: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    message: String,
}

struct SidecarProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl SidecarProcess {
    fn spawn(options: &Qwen3SidecarOptions) -> Result<Self> {
        let mut command = Command::new(&options.python);
        command
            .arg(&options.sidecar_script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        // Own process group: a Ctrl+C aimed at the host must not SIGINT the
        // sidecar mid-flight (an interrupted python skips vLLM's atexit and
        // orphans the EngineCore child holding ~10GB of VRAM). With stdin
        // closed on our exit the sidecar shuts down cleanly instead.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn().with_context(|| {
            format!(
                "spawn qwen3 sidecar: {} {}",
                options.python,
                options.sidecar_script.display()
            )
        })?;
        let stdin = child.stdin.take().context("sidecar stdin unavailable")?;
        let stdout = BufReader::new(child.stdout.take().context("sidecar stdout unavailable")?);
        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    fn request(&mut self, payload: serde_json::Value) -> Result<SidecarReply> {
        let mut line = serde_json::to_string(&payload)?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .context("write to qwen3 sidecar")?;
        self.stdin.flush()?;
        let mut reply_line = String::new();
        let read = self
            .stdout
            .read_line(&mut reply_line)
            .context("read from qwen3 sidecar")?;
        if read == 0 {
            bail!("qwen3 sidecar exited unexpectedly");
        }
        let reply: SidecarReply =
            serde_json::from_str(reply_line.trim()).context("parse sidecar reply")?;
        if reply.event == "error" {
            bail!("qwen3 sidecar error: {}", reply.message);
        }
        Ok(reply)
    }
}

impl Drop for SidecarProcess {
    fn drop(&mut self) {
        let _ = self.stdin.write_all(b"{\"cmd\":\"shutdown\"}\n");
        let _ = self.stdin.flush();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Sweep the whole process group so vLLM's spawned EngineCore can
        // never outlive us holding GPU memory.
        #[cfg(unix)]
        {
            let _ = Command::new("kill")
                .args(["-TERM", "--", &format!("-{}", self.child.id())])
                .status();
        }
    }
}

/// `StreamingRecognizer` backed by the Qwen3-ASR vLLM sidecar.
pub struct Qwen3SidecarRecognizer {
    options: Qwen3SidecarOptions,
    process: Option<SidecarProcess>,
    session_counter: u64,
    active: Option<SessionState>,
}

struct SessionState {
    id: SessionId,
    stabilizer: StablePrefixStabilizer,
    revision: u64,
    last_text: String,
    pending: VecDeque<AsrEvent>,
}

impl Qwen3SidecarRecognizer {
    /// Creates the recognizer without spawning anything; the sidecar starts
    /// (and loads the model) on the first `start_session`.
    pub fn new(options: Qwen3SidecarOptions) -> Self {
        Self {
            options,
            process: None,
            session_counter: 0,
            active: None,
        }
    }

    fn ensure_process(&mut self) -> Result<&mut SidecarProcess> {
        if self.process.is_none() {
            if !self.options.sidecar_script.is_file() {
                bail!(
                    "qwen3 sidecar script not found: {}",
                    self.options.sidecar_script.display()
                );
            }
            let mut process = SidecarProcess::spawn(&self.options)?;
            tracing::info!(model = self.options.model, "loading Qwen3-ASR via sidecar");
            let reply = process.request(json!({
                "cmd": "init",
                "model": self.options.model,
                "gpu_memory_utilization": self.options.gpu_memory_utilization,
                "chunk_size_sec": self.options.chunk_size_sec,
                "unfixed_chunk_num": self.options.unfixed_chunk_num,
                "unfixed_token_num": self.options.unfixed_token_num,
                "max_new_tokens": self.options.max_new_tokens,
                "max_model_len": self.options.max_model_len,
                "language": self.options.language,
            }))?;
            if reply.event != "ready" {
                bail!("unexpected sidecar init reply: {}", reply.event);
            }
            self.process = Some(process);
        }
        Ok(self.process.as_mut().expect("just ensured"))
    }

    fn record_text(&mut self, text: String) {
        let Some(state) = self.active.as_mut() else {
            return;
        };
        if text.is_empty() || text == state.last_text {
            return;
        }
        state.revision += 1;
        let tokens = char_tokens(&text);
        state.pending.push_back(AsrEvent::Partial {
            revision: state.revision,
            text: text.clone(),
            tokens: tokens.clone(),
        });
        if let Some(stable) = state
            .stabilizer
            .observe_partial(state.revision, &text, tokens)
        {
            state.pending.push_back(stable);
        }
        state.last_text = text;
    }
}

/// The sidecar reports plain text without token timestamps; character tokens
/// keep the stable-prefix logic working.
fn char_tokens(text: &str) -> Vec<Token> {
    text.chars()
        .map(|ch| Token {
            text: ch.to_string(),
            start_ms: 0,
            end_ms: 0,
        })
        .collect()
}

impl StreamingRecognizer for Qwen3SidecarRecognizer {
    fn start_session(&mut self) -> Result<SessionId> {
        self.ensure_process()?;
        let process = self.process.as_mut().expect("ensured above");
        let reply = process.request(json!({"cmd": "start"}))?;
        if reply.event != "started" {
            bail!("unexpected sidecar start reply: {}", reply.event);
        }
        self.session_counter += 1;
        let id = format!("qwen3-{}", self.session_counter);
        self.active = Some(SessionState {
            id: id.clone(),
            stabilizer: StablePrefixStabilizer::new(),
            revision: 0,
            last_text: String::new(),
            pending: VecDeque::new(),
        });
        Ok(id)
    }

    fn push_audio(&mut self, session: &SessionId, frame: AudioFrame) -> Result<()> {
        let active_id = self
            .active
            .as_ref()
            .map(|state| state.id.clone())
            .context("no active qwen3 session")?;
        if &active_id != session {
            bail!("unknown qwen3 session {session}");
        }
        if frame.channels != 1 {
            bail!("qwen3 backend expects mono audio");
        }
        let mut bytes = Vec::with_capacity(frame.pcm_i16.len() * 2);
        for sample in &frame.pcm_i16 {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let process = self.process.as_mut().context("sidecar not running")?;
        let reply = process.request(json!({
            "cmd": "audio",
            "sample_rate": frame.sample_rate_hz,
            "pcm_i16_b64": encoded,
        }))?;
        if reply.event == "partial" {
            self.record_text(reply.text);
        }
        Ok(())
    }

    fn poll_events(&mut self, session: &SessionId) -> Result<Vec<AsrEvent>> {
        let Some(state) = self.active.as_mut() else {
            return Ok(Vec::new());
        };
        if &state.id != session {
            return Ok(Vec::new());
        }
        Ok(state.pending.drain(..).collect())
    }

    fn finish_session(&mut self, session: &SessionId) -> Result<Vec<AsrEvent>> {
        let Some(state) = self.active.as_ref() else {
            bail!("no active qwen3 session");
        };
        if &state.id != session {
            bail!("unknown qwen3 session {session}");
        }
        let process = self.process.as_mut().context("sidecar not running")?;
        let reply = process.request(json!({"cmd": "finish"}))?;
        let mut state = self.active.take().expect("checked above");
        let mut events: Vec<AsrEvent> = state.pending.drain(..).collect();
        let final_text = if reply.text.is_empty() {
            state.last_text.clone()
        } else {
            reply.text
        };
        if !final_text.is_empty() {
            events.push(AsrEvent::Final {
                revision: state.revision + 1,
                text: final_text,
                segment_id: format!("{session}-seg-1"),
            });
        }
        Ok(events)
    }
}

/// vLLM engine / sidecar processes whose parent died (PPID 1). They keep
/// multi-GB GPU allocations alive, so hosts sweep them at startup.
pub fn find_orphaned_engines() -> Vec<i32> {
    let mut orphans = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return orphans;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<i32>().ok())
        else {
            continue;
        };
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        let Some((head, rest)) = stat.rsplit_once(')') else {
            continue;
        };
        let comm = head.split_once('(').map(|(_, comm)| comm).unwrap_or("");
        let ppid: i32 = rest
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse().ok())
            .unwrap_or(-1);
        if ppid != 1 {
            continue;
        }
        let cmdline = std::fs::read_to_string(format!("/proc/{pid}/cmdline"))
            .unwrap_or_default()
            .replace('\0', " ");
        if comm.starts_with("VLLM::EngineCor")
            || cmdline.contains("VLLM::EngineCore")
            || cmdline.contains("qwen3_asr_sidecar.py")
        {
            orphans.push(pid);
        }
    }
    orphans
}

/// Terminates orphaned engine processes (TERM, then KILL for stragglers).
/// Returns how many were targeted.
pub fn sweep_orphaned_engines() -> usize {
    let orphans = find_orphaned_engines();
    for pid in &orphans {
        let _ = Command::new("kill").arg(pid.to_string()).status();
    }
    if !orphans.is_empty() {
        std::thread::sleep(std::time::Duration::from_millis(1200));
        for pid in find_orphaned_engines() {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
    }
    orphans.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stub sidecar covering the whole protocol, so the recognizer is tested
    /// offline without vLLM or a GPU.
    const FAKE_SIDECAR: &str = r#"
import json, sys
texts = ["jin", "jin tian", "jin tian tian qi"]
i = 0
for line in sys.stdin:
    msg = json.loads(line)
    cmd = msg.get("cmd")
    if cmd == "init":
        print(json.dumps({"event": "ready"}), flush=True)
    elif cmd == "start":
        i = 0
        print(json.dumps({"event": "started"}), flush=True)
    elif cmd == "audio":
        text = texts[min(i, len(texts) - 1)]
        i += 1
        print(json.dumps({"event": "partial", "text": text}), flush=True)
    elif cmd == "finish":
        print(json.dumps({"event": "final", "text": "jin tian tian qi hen hao"}), flush=True)
    elif cmd == "shutdown":
        print(json.dumps({"event": "bye"}), flush=True)
        break
"#;

    fn write_fake_sidecar(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "voxflow-fake-qwen3-sidecar-{label}-{}.py",
            std::process::id()
        ));
        std::fs::write(&path, FAKE_SIDECAR).unwrap();
        path
    }

    fn frame() -> AudioFrame {
        AudioFrame::mono_constant(16_000, 20, 1000)
    }

    #[test]
    fn full_session_produces_partial_stable_final() {
        let script = write_fake_sidecar("full");
        let mut recognizer = Qwen3SidecarRecognizer::new(Qwen3SidecarOptions::new(&script));
        let session = recognizer.start_session().unwrap();

        let mut events = Vec::new();
        for _ in 0..3 {
            recognizer.push_audio(&session, frame()).unwrap();
            events.extend(recognizer.poll_events(&session).unwrap());
        }
        events.extend(recognizer.finish_session(&session).unwrap());

        assert!(events
            .iter()
            .any(|event| matches!(event, AsrEvent::Partial { .. })));
        assert!(events
            .iter()
            .any(|event| matches!(event, AsrEvent::Stable { .. })));
        match events.last().unwrap() {
            AsrEvent::Final { text, .. } => {
                assert_eq!(text, "jin tian tian qi hen hao");
            }
            other => panic!("expected final, got {other:?}"),
        }
        std::fs::remove_file(&script).unwrap();
    }

    #[test]
    fn second_session_reuses_sidecar() {
        let script = write_fake_sidecar("reuse");
        let mut recognizer = Qwen3SidecarRecognizer::new(Qwen3SidecarOptions::new(&script));
        let first = recognizer.start_session().unwrap();
        recognizer.push_audio(&first, frame()).unwrap();
        recognizer.finish_session(&first).unwrap();

        let second = recognizer.start_session().unwrap();
        assert_ne!(first, second);
        recognizer.push_audio(&second, frame()).unwrap();
        let final_events = recognizer.finish_session(&second).unwrap();
        assert!(matches!(
            final_events.last().unwrap(),
            AsrEvent::Final { .. }
        ));
        std::fs::remove_file(&script).unwrap();
    }

    #[test]
    fn missing_script_fails_at_session_start() {
        let mut recognizer =
            Qwen3SidecarRecognizer::new(Qwen3SidecarOptions::new("/nonexistent/sidecar.py"));
        assert!(recognizer.start_session().is_err());
    }

    /// `resolve_sidecar_script` with explicit config path.
    #[test]
    fn resolve_prefers_configured_path() {
        let script = write_fake_sidecar("resolve");
        assert_eq!(
            resolve_sidecar_script(script.to_str().unwrap()),
            Some(script.clone())
        );
        assert_eq!(resolve_sidecar_script("/definitely/not/there.py"), None);
        std::fs::remove_file(&script).unwrap();
    }
}
