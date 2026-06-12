//! Background model downloads (`model.download` / `pause` / `resume` /
//! `cancel`, progress via `model.progress` events; ipc-api §3.6/§4.3).
//!
//! Files are fetched one by one into `<models>/.staging-<id>/` with `.part`
//! suffixes and HTTP Range resume, verified against the profile sha256 set,
//! then atomically renamed to `<models>/<id>/` with a manifest lock. Models
//! always live under the user data dir (`VOXFLOW_HOME`, default `~/.voxflow`),
//! never under system paths.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::sync::broadcast;

use crate::ipc::Envelope;
use crate::model::{write_manifest_lock, ModelProfileDocument};

const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const ENOSPC: i32 = 28;

#[derive(Debug, Default)]
struct DownloadControl {
    pause: AtomicBool,
    cancel: AtomicBool,
}

#[derive(Debug)]
struct DownloadTask {
    task_id: String,
    control: Arc<DownloadControl>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl DownloadTask {
    fn is_running(&self) -> bool {
        self.join
            .as_ref()
            .map(|join| !join.is_finished())
            .unwrap_or(false)
    }
}

/// Owns the background download workers, keyed by model id.
#[derive(Debug, Default)]
pub struct DownloadManager {
    tasks: HashMap<String, DownloadTask>,
    task_counter: u64,
}

impl DownloadManager {
    pub fn is_running(&self, model_id: &str) -> bool {
        self.tasks
            .get(model_id)
            .map(DownloadTask::is_running)
            .unwrap_or(false)
    }

    /// Starts (or resumes — `.part` files are continued) a download.
    pub fn start(
        &mut self,
        profile: ModelProfileDocument,
        models_dir: PathBuf,
        events: broadcast::Sender<Envelope>,
    ) -> Result<String> {
        let model_id = profile.profile.id.clone();
        if self.is_running(&model_id) {
            bail!("core.busy: download already running for {model_id}");
        }
        self.task_counter += 1;
        let task_id = format!("dl-{}", self.task_counter);
        let control = Arc::new(DownloadControl::default());
        let worker = DownloadWorker {
            task_id: task_id.clone(),
            profile,
            models_dir,
            control: Arc::clone(&control),
            events,
        };
        let join = std::thread::Builder::new()
            .name(format!("voxflow-dl-{model_id}"))
            .spawn(move || worker.run())
            .context("spawn download worker")?;
        self.tasks.insert(
            model_id,
            DownloadTask {
                task_id: task_id.clone(),
                control,
                join: Some(join),
            },
        );
        Ok(task_id)
    }

    /// Stops the worker, keeping `.part` files for a later resume.
    pub fn pause(&mut self, model_id: &str) -> Result<String> {
        let task = self
            .tasks
            .get_mut(model_id)
            .filter(|task| task.is_running())
            .with_context(|| format!("model.not_found: no active download for {model_id}"))?;
        task.control.pause.store(true, Ordering::Relaxed);
        let task_id = task.task_id.clone();
        if let Some(join) = task.join.take() {
            let _ = join.join();
        }
        Ok(task_id)
    }

    /// Stops the worker and removes all partially downloaded data.
    pub fn cancel(&mut self, model_id: &str, models_dir: &Path) -> Result<String> {
        let task_id = match self.tasks.get_mut(model_id) {
            Some(task) => {
                task.control.cancel.store(true, Ordering::Relaxed);
                if let Some(join) = task.join.take() {
                    let _ = join.join();
                }
                task.task_id.clone()
            }
            None => "dl-none".to_string(),
        };
        self.tasks.remove(model_id);
        let staging = staging_dir(models_dir, model_id);
        if staging.exists() {
            fs::remove_dir_all(&staging)
                .with_context(|| format!("remove staging {}", staging.display()))?;
        }
        Ok(task_id)
    }
}

pub fn staging_dir(models_dir: &Path, model_id: &str) -> PathBuf {
    models_dir.join(format!(".staging-{model_id}"))
}

enum Outcome {
    Done,
    Stopped,
}

struct DownloadWorker {
    task_id: String,
    profile: ModelProfileDocument,
    models_dir: PathBuf,
    control: Arc<DownloadControl>,
    events: broadcast::Sender<Envelope>,
}

impl DownloadWorker {
    fn run(self) {
        let model_id = self.profile.profile.id.clone();
        match self.execute() {
            Ok(Outcome::Done) => {}
            Ok(Outcome::Stopped) => {
                tracing::info!(model_id, "model download stopped by request");
            }
            Err(error) => {
                let code = error_code(&error);
                tracing::warn!(model_id, %error, code, "model download failed");
                self.emit(json!({
                    "task_id": self.task_id,
                    "model_id": model_id,
                    "phase": "failed",
                    "code": code,
                    "message": error.to_string(),
                }));
            }
        }
    }

    fn execute(&self) -> Result<Outcome> {
        let model_id = &self.profile.profile.id;
        let base_url = &self.profile.source.url;
        if !base_url.ends_with('/') {
            bail!("model.profile_invalid: source.url must be a base URL ending in '/'");
        }
        let staging = staging_dir(&self.models_dir, model_id);
        fs::create_dir_all(&staging)
            .with_context(|| format!("create staging {}", staging.display()))?;

        let total: u64 = self
            .profile
            .files
            .iter()
            .map(|file| file.size_bytes.unwrap_or(0))
            .sum();
        let agent = ureq::builder()
            .timeout_read(READ_TIMEOUT)
            .timeout_connect(Duration::from_secs(15))
            .build();

        let mut progress = ProgressEmitter::new(self, total);
        // Bytes from files finished in an earlier (paused) run.
        for file in &self.profile.files {
            if staging.join(&file.path).is_file() {
                progress.completed += file.size_bytes.unwrap_or(0);
            }
        }

        for file in &self.profile.files {
            let target = staging.join(&file.path);
            if target.is_file() {
                continue;
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let url = format!("{base_url}{}", file.path);
            match self.fetch_file(&agent, &url, &target, &mut progress)? {
                Outcome::Done => {}
                Outcome::Stopped => return Ok(Outcome::Stopped),
            }
            progress.completed += file.size_bytes.unwrap_or(0);
        }

        self.emit_phase("verifying", &mut progress);
        for file in &self.profile.files {
            let path = staging.join(&file.path);
            let actual = sha256_of(&path)?;
            if !actual.eq_ignore_ascii_case(&file.sha256) {
                // Drop the corrupt file so the next attempt re-downloads it.
                let _ = fs::remove_file(&path);
                bail!("model.checksum_failed:{}", file.path);
            }
        }

        self.emit_phase("installing", &mut progress);
        let final_dir = self.models_dir.join(model_id);
        if final_dir.exists() {
            bail!("model.already_installed: {}", final_dir.display());
        }
        fs::rename(&staging, &final_dir)
            .with_context(|| format!("install {} -> {}", staging.display(), final_dir.display()))?;
        write_manifest_lock(&self.profile, &final_dir)?;

        self.emit(json!({
            "task_id": self.task_id,
            "model_id": model_id,
            "phase": "done",
            "downloaded": progress.completed,
            "total": total,
        }));
        Ok(Outcome::Done)
    }

    fn fetch_file(
        &self,
        agent: &ureq::Agent,
        url: &str,
        target: &Path,
        progress: &mut ProgressEmitter,
    ) -> Result<Outcome> {
        let part = target.with_extension(match target.extension() {
            Some(extension) => format!("{}.part", extension.to_string_lossy()),
            None => "part".to_string(),
        });
        let mut offset = fs::metadata(&part).map(|meta| meta.len()).unwrap_or(0);

        let mut request = agent.get(url);
        if offset > 0 {
            request = request.set("Range", &format!("bytes={offset}-"));
        }
        let response = request
            .call()
            .map_err(|error| anyhow::anyhow!("model.source_unreachable: {url}: {error}"))?;
        let resumed = response.status() == 206;
        if !resumed {
            offset = 0;
        }
        let mut reader = response.into_reader();
        let mut output = fs::OpenOptions::new()
            .create(true)
            .append(resumed)
            .truncate(!resumed)
            .write(true)
            .open(&part)
            .with_context(|| format!("open {}", part.display()))?;

        let mut buffer = vec![0_u8; 64 * 1024];
        let mut received = offset;
        loop {
            if self.control.cancel.load(Ordering::Relaxed)
                || self.control.pause.load(Ordering::Relaxed)
            {
                output.flush()?;
                return Ok(Outcome::Stopped);
            }
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read]).map_err(map_write_error)?;
            received += read as u64;
            progress.tick(received);
        }
        output.flush()?;
        drop(output);
        fs::rename(&part, target).with_context(|| format!("finalize {}", target.display()))?;
        Ok(Outcome::Done)
    }

    fn emit_phase(&self, phase: &str, progress: &mut ProgressEmitter) {
        self.emit(json!({
            "task_id": self.task_id,
            "model_id": self.profile.profile.id,
            "phase": phase,
            "downloaded": progress.completed,
            "total": progress.total,
        }));
    }

    fn emit(&self, payload: serde_json::Value) {
        let _ = self.events.send(Envelope::event("model.progress", payload));
    }
}

struct ProgressEmitter<'a> {
    worker: &'a DownloadWorker,
    total: u64,
    completed: u64,
    started: Instant,
    last_emit: Instant,
}

impl<'a> ProgressEmitter<'a> {
    fn new(worker: &'a DownloadWorker, total: u64) -> Self {
        let now = Instant::now();
        Self {
            worker,
            total,
            completed: 0,
            started: now,
            last_emit: now - PROGRESS_INTERVAL,
        }
    }

    /// Emits a throttled `downloading` progress event (≤ 4/s per ipc-api §4.3).
    fn tick(&mut self, current_file_bytes: u64) {
        if self.last_emit.elapsed() < PROGRESS_INTERVAL {
            return;
        }
        self.last_emit = Instant::now();
        let downloaded = self.completed + current_file_bytes;
        let elapsed = self.started.elapsed().as_secs_f64().max(0.001);
        let speed_bps = (downloaded as f64 / elapsed) as u64;
        let eta_s = if speed_bps > 0 && self.total > downloaded {
            (self.total - downloaded) / speed_bps
        } else {
            0
        };
        self.worker.emit(json!({
            "task_id": self.worker.task_id,
            "model_id": self.worker.profile.profile.id,
            "phase": "downloading",
            "downloaded": downloaded,
            "total": self.total,
            "speed_bps": speed_bps,
            "eta_s": eta_s,
        }));
    }
}

fn map_write_error(error: std::io::Error) -> anyhow::Error {
    if error.raw_os_error() == Some(ENOSPC) {
        anyhow::anyhow!("model.disk_full: {error}")
    } else {
        anyhow::anyhow!(error)
    }
}

fn error_code(error: &anyhow::Error) -> String {
    let text = error.to_string();
    for code in [
        "model.source_unreachable",
        "model.checksum_failed",
        "model.disk_full",
        "model.already_installed",
        "model.profile_invalid",
    ] {
        if text.starts_with(code) {
            return code.to_string();
        }
    }
    "model.download_failed".to_string()
}

fn sha256_of(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("open for checksum {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 256 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    /// Minimal HTTP server with Range support for offline download tests.
    fn spawn_server(body: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let join = std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let mut reader = BufReader::new(stream);
                let mut range_start = None;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let line = line.trim_end().to_string();
                    if line.is_empty() {
                        break;
                    }
                    if let Some(spec) = line.strip_prefix("Range: bytes=") {
                        range_start = spec
                            .trim_end_matches('-')
                            .parse::<usize>()
                            .ok()
                            .filter(|start| *start < body.len());
                    }
                }
                let mut stream = reader.into_inner();
                let (status, slice) = match range_start {
                    Some(start) => ("206 Partial Content", &body[start..]),
                    None => ("200 OK", &body[..]),
                };
                let header = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    slice.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(slice);
            }
        });
        (format!("http://{addr}/"), join)
    }

    fn profile_for(url: &str, body: &[u8], sha256: &str) -> ModelProfileDocument {
        let toml = format!(
            r#"
[profile]
id = "test-model"
label = "Test"
kind = "asr-streaming"
backend = "sherpa-onnx"
version = "1"
license = "MIT"
languages = ["zh"]
streaming = true
recommended = false
min_ram_mb = 1

[source]
url = "{url}"
size_bytes = {size}

[[files]]
path = "weights.onnx"
size_bytes = {size}
sha256 = "{sha256}"
"#,
            size = body.len(),
        );
        toml::from_str(&toml).unwrap()
    }

    fn temp_models_dir(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("voxflow-dl-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn collect_phases(receiver: &mut broadcast::Receiver<Envelope>) -> Vec<String> {
        let mut phases = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            if let Some(phase) = event.payload.get("phase").and_then(|phase| phase.as_str()) {
                phases.push(phase.to_string());
            }
        }
        phases
    }

    #[test]
    fn download_verifies_and_installs_model() {
        let body = vec![7_u8; 200_000];
        let sha = hex::encode(Sha256::digest(&body));
        let (url, _server) = spawn_server(body);
        let models_dir = temp_models_dir("ok");
        let (events, mut receiver) = broadcast::channel(64);

        let mut manager = DownloadManager::default();
        let task_id = manager
            .start(
                profile_for(&url, &[7; 200_000], &sha),
                models_dir.clone(),
                events,
            )
            .unwrap();
        assert!(task_id.starts_with("dl-"));
        let join = manager.tasks.get_mut("test-model").unwrap().join.take();
        join.unwrap().join().unwrap();

        let installed = models_dir.join("test-model");
        assert!(installed.join("weights.onnx").is_file());
        assert!(installed.join("manifest.lock").is_file());
        assert!(!staging_dir(&models_dir, "test-model").exists());
        let phases = collect_phases(&mut receiver);
        assert!(phases.contains(&"verifying".to_string()));
        assert_eq!(phases.last().map(String::as_str), Some("done"));
        fs::remove_dir_all(&models_dir).unwrap();
    }

    #[test]
    fn download_resumes_from_part_file() {
        let body: Vec<u8> = (0..150_000_u32).map(|value| value as u8).collect();
        let sha = hex::encode(Sha256::digest(&body));
        let (url, _server) = spawn_server(body.clone());
        let models_dir = temp_models_dir("resume");
        // Seed half the file as a previous paused download.
        let staging = staging_dir(&models_dir, "test-model");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("weights.onnx.part"), &body[..70_000]).unwrap();

        let (events, mut receiver) = broadcast::channel(64);
        let mut manager = DownloadManager::default();
        manager
            .start(profile_for(&url, &body, &sha), models_dir.clone(), events)
            .unwrap();
        let join = manager.tasks.get_mut("test-model").unwrap().join.take();
        join.unwrap().join().unwrap();

        let installed = fs::read(models_dir.join("test-model/weights.onnx")).unwrap();
        assert_eq!(installed, body);
        assert_eq!(
            collect_phases(&mut receiver).last().map(String::as_str),
            Some("done")
        );
        fs::remove_dir_all(&models_dir).unwrap();
    }

    #[test]
    fn checksum_mismatch_fails_and_removes_corrupt_file() {
        let body = vec![1_u8; 50_000];
        let (url, _server) = spawn_server(body.clone());
        let models_dir = temp_models_dir("badsum");
        let (events, mut receiver) = broadcast::channel(64);
        let mut manager = DownloadManager::default();
        manager
            .start(
                profile_for(&url, &body, &"0".repeat(64)),
                models_dir.clone(),
                events,
            )
            .unwrap();
        let join = manager.tasks.get_mut("test-model").unwrap().join.take();
        join.unwrap().join().unwrap();

        assert!(!models_dir.join("test-model").exists());
        assert!(!staging_dir(&models_dir, "test-model")
            .join("weights.onnx")
            .exists());
        let phases = collect_phases(&mut receiver);
        assert_eq!(phases.last().map(String::as_str), Some("failed"));
        fs::remove_dir_all(&models_dir).unwrap();
    }
}
