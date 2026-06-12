//! sherpa-onnx streaming backend for [`voxflow_asr::StreamingRecognizer`].
//!
//! Per D-17 this crate is the only place that touches sherpa bindings. It
//! wraps the sherpa-onnx *online* (streaming) C API directly via
//! `sherpa-rs-sys`, because the high-level `sherpa-rs` crate only exposes
//! offline recognizers as of 0.6.8.

pub mod vad;

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::mem;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use voxflow_asr::{
    AsrEvent, AudioFrame, SessionId, StablePrefixStabilizer, StreamingRecognizer, Token,
};

/// Resolved on-disk model files for a streaming transducer (Zipformer) model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SherpaModelFiles {
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub joiner: PathBuf,
    pub tokens: PathBuf,
    pub int8: bool,
}

impl SherpaModelFiles {
    /// Locates encoder/decoder/joiner/tokens in `dir`. int8 weights are
    /// preferred per component, falling back to fp32 independently — newer
    /// upstream releases ship mixed sets (e.g. int8 encoder + fp32 decoder).
    pub fn detect(dir: &Path) -> Result<Self> {
        let tokens = dir.join("tokens.txt");
        if !tokens.is_file() {
            bail!("tokens.txt not found in {}", dir.display());
        }
        let mut any_int8 = false;
        let mut resolve = |role: &str| -> Result<PathBuf> {
            for int8 in [true, false] {
                if let Some(path) = find_component(dir, role, int8)? {
                    any_int8 |= int8;
                    return Ok(path);
                }
            }
            bail!("no {role} onnx file found in {}", dir.display());
        };
        let encoder = resolve("encoder")?;
        let decoder = resolve("decoder")?;
        let joiner = resolve("joiner")?;
        Ok(Self {
            encoder,
            decoder,
            joiner,
            tokens,
            int8: any_int8,
        })
    }

    pub fn total_size_bytes(&self) -> u64 {
        [&self.encoder, &self.decoder, &self.joiner, &self.tokens]
            .iter()
            .filter_map(|path| std::fs::metadata(path).ok())
            .map(|meta| meta.len())
            .sum()
    }
}

fn find_component(dir: &Path, role: &str, int8: bool) -> Result<Option<PathBuf>> {
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read model dir {}", dir.display()))?;
    let mut candidates = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let matches_quant = if int8 {
            name.ends_with(".int8.onnx")
        } else {
            name.ends_with(".onnx") && !name.ends_with(".int8.onnx")
        };
        if name.starts_with(role) && matches_quant {
            candidates.push(path);
        }
    }
    candidates.sort();
    Ok(candidates.into_iter().next())
}

/// Configuration for [`SherpaStreamingRecognizer`].
///
/// Endpoint rule defaults follow the sherpa-onnx online recognizer defaults;
/// they map to the latency budget discussion in streaming-asr §4.
#[derive(Debug, Clone)]
pub struct SherpaStreamingConfig {
    pub model: SherpaModelFiles,
    pub num_threads: i32,
    pub sample_rate_hz: u32,
    pub feature_dim: i32,
    pub decoding_method: String,
    pub enable_endpoint: bool,
    pub rule1_min_trailing_silence_s: f32,
    pub rule2_min_trailing_silence_s: f32,
    pub rule3_min_utterance_length_s: f32,
    pub provider: String,
    pub debug: bool,
}

impl SherpaStreamingConfig {
    pub fn from_model_dir(dir: &Path) -> Result<Self> {
        Ok(Self::new(SherpaModelFiles::detect(dir)?))
    }

    pub fn new(model: SherpaModelFiles) -> Self {
        Self {
            model,
            num_threads: 2,
            sample_rate_hz: 16_000,
            feature_dim: 80,
            decoding_method: "greedy_search".to_string(),
            enable_endpoint: true,
            rule1_min_trailing_silence_s: 2.4,
            rule2_min_trailing_silence_s: 1.2,
            rule3_min_utterance_length_s: 20.0,
            provider: "cpu".to_string(),
            debug: false,
        }
    }
}

struct SessionState {
    stream: *const sherpa_rs_sys::SherpaOnnxOnlineStream,
    stabilizer: StablePrefixStabilizer,
    revision: u64,
    last_partial_text: String,
    segment_counter: u64,
}

/// Streaming recognizer backed by a sherpa-onnx online transducer model.
pub struct SherpaStreamingRecognizer {
    recognizer: *const sherpa_rs_sys::SherpaOnnxOnlineRecognizer,
    sessions: HashMap<SessionId, SessionState>,
    session_counter: u64,
    sample_rate_hz: u32,
    enable_endpoint: bool,
}

// The raw pointers are only touched through &mut self, never shared.
unsafe impl Send for SherpaStreamingRecognizer {}

impl SherpaStreamingRecognizer {
    pub fn new(config: SherpaStreamingConfig) -> Result<Self> {
        let encoder = cstring_from_path(&config.model.encoder)?;
        let decoder = cstring_from_path(&config.model.decoder)?;
        let joiner = cstring_from_path(&config.model.joiner)?;
        let tokens = cstring_from_path(&config.model.tokens)?;
        let provider = CString::new(config.provider.as_str())?;
        let decoding_method = CString::new(config.decoding_method.as_str())?;

        let recognizer = unsafe {
            // Zero-init so newly added optional fields stay NULL/0; the C API
            // substitutes its own defaults for empty values.
            let mut c_config = mem::zeroed::<sherpa_rs_sys::SherpaOnnxOnlineRecognizerConfig>();
            c_config.feat_config.sample_rate = config.sample_rate_hz as i32;
            c_config.feat_config.feature_dim = config.feature_dim;
            c_config.model_config.transducer.encoder = encoder.as_ptr();
            c_config.model_config.transducer.decoder = decoder.as_ptr();
            c_config.model_config.transducer.joiner = joiner.as_ptr();
            c_config.model_config.tokens = tokens.as_ptr();
            c_config.model_config.num_threads = config.num_threads;
            c_config.model_config.provider = provider.as_ptr();
            c_config.model_config.debug = i32::from(config.debug);
            c_config.decoding_method = decoding_method.as_ptr();
            c_config.enable_endpoint = i32::from(config.enable_endpoint);
            c_config.rule1_min_trailing_silence = config.rule1_min_trailing_silence_s;
            c_config.rule2_min_trailing_silence = config.rule2_min_trailing_silence_s;
            c_config.rule3_min_utterance_length = config.rule3_min_utterance_length_s;

            sherpa_rs_sys::SherpaOnnxCreateOnlineRecognizer(&c_config)
        };
        if recognizer.is_null() {
            bail!(
                "SherpaOnnxCreateOnlineRecognizer failed for model {}",
                config.model.encoder.display()
            );
        }

        Ok(Self {
            recognizer,
            sessions: HashMap::new(),
            session_counter: 0,
            sample_rate_hz: config.sample_rate_hz,
            enable_endpoint: config.enable_endpoint,
        })
    }

    fn session_mut(&mut self, session: &SessionId) -> Result<&mut SessionState> {
        self.sessions
            .get_mut(session)
            .with_context(|| format!("unknown sherpa session {session}"))
    }

    fn drain_decoder(&self, state: &SessionState) {
        unsafe {
            while sherpa_rs_sys::SherpaOnnxIsOnlineStreamReady(self.recognizer, state.stream) == 1 {
                sherpa_rs_sys::SherpaOnnxDecodeOnlineStream(self.recognizer, state.stream);
            }
        }
    }

    fn read_result(&self, state: &SessionState) -> (String, Vec<Token>) {
        unsafe {
            let result =
                sherpa_rs_sys::SherpaOnnxGetOnlineStreamResult(self.recognizer, state.stream);
            if result.is_null() {
                return (String::new(), Vec::new());
            }
            let raw = &*result;
            let text = if raw.text.is_null() {
                String::new()
            } else {
                CStr::from_ptr(raw.text).to_string_lossy().into_owned()
            };
            let tokens = read_tokens(raw);
            sherpa_rs_sys::SherpaOnnxDestroyOnlineRecognizerResult(result);
            (text, tokens)
        }
    }

    fn collect_events(&mut self, session: &SessionId) -> Result<Vec<AsrEvent>> {
        let state = self
            .sessions
            .get(session)
            .with_context(|| format!("unknown sherpa session {session}"))?;
        self.drain_decoder(state);
        let (text, tokens) = self.read_result(state);
        let is_endpoint = self.enable_endpoint
            && unsafe {
                sherpa_rs_sys::SherpaOnnxOnlineStreamIsEndpoint(self.recognizer, state.stream) == 1
            };
        let recognizer = self.recognizer;
        let state = self.session_mut(session)?;

        let mut events = Vec::new();
        if !text.is_empty() && text != state.last_partial_text {
            state.revision += 1;
            events.push(AsrEvent::Partial {
                revision: state.revision,
                text: text.clone(),
                tokens: tokens.clone(),
            });
            if let Some(stable) = state
                .stabilizer
                .observe_partial(state.revision, &text, tokens)
            {
                events.push(stable);
            }
            state.last_partial_text = text;
        }

        if is_endpoint {
            if !state.last_partial_text.is_empty() {
                state.revision += 1;
                state.segment_counter += 1;
                events.push(AsrEvent::Final {
                    revision: state.revision,
                    text: state.last_partial_text.clone(),
                    segment_id: format!("{session}-seg-{}", state.segment_counter),
                });
            }
            unsafe {
                sherpa_rs_sys::SherpaOnnxOnlineStreamReset(recognizer, state.stream);
            }
            state.stabilizer = StablePrefixStabilizer::new();
            state.last_partial_text.clear();
        }
        Ok(events)
    }
}

impl StreamingRecognizer for SherpaStreamingRecognizer {
    fn start_session(&mut self) -> Result<SessionId> {
        let stream = unsafe { sherpa_rs_sys::SherpaOnnxCreateOnlineStream(self.recognizer) };
        if stream.is_null() {
            bail!("SherpaOnnxCreateOnlineStream failed");
        }
        self.session_counter += 1;
        let session = format!("sherpa-{}", self.session_counter);
        self.sessions.insert(
            session.clone(),
            SessionState {
                stream,
                stabilizer: StablePrefixStabilizer::new(),
                revision: 0,
                last_partial_text: String::new(),
                segment_counter: 0,
            },
        );
        Ok(session)
    }

    fn push_audio(&mut self, session: &SessionId, frame: AudioFrame) -> Result<()> {
        if frame.channels != 1 {
            bail!(
                "sherpa backend expects mono audio, got {} channels",
                frame.channels
            );
        }
        let sample_rate = if frame.sample_rate_hz == 0 {
            self.sample_rate_hz
        } else {
            frame.sample_rate_hz
        };
        let samples = frame
            .pcm_i16
            .iter()
            .map(|sample| f32::from(*sample) / f32::from(i16::MAX))
            .collect::<Vec<_>>();
        let state = self.session_mut(session)?;
        unsafe {
            sherpa_rs_sys::SherpaOnnxOnlineStreamAcceptWaveform(
                state.stream,
                sample_rate as i32,
                samples.as_ptr(),
                samples.len() as i32,
            );
        }
        Ok(())
    }

    fn poll_events(&mut self, session: &SessionId) -> Result<Vec<AsrEvent>> {
        self.collect_events(session)
    }

    fn finish_session(&mut self, session: &SessionId) -> Result<Vec<AsrEvent>> {
        // Push a short silence tail so the final frames flush through the model.
        let tail = AudioFrame::mono_silence(self.sample_rate_hz, 600);
        self.push_audio(session, tail)?;
        {
            let state = self.session_mut(session)?;
            unsafe {
                sherpa_rs_sys::SherpaOnnxOnlineStreamInputFinished(state.stream);
            }
        }
        let mut events = self.collect_events(session)?;
        let state = self
            .sessions
            .remove(session)
            .with_context(|| format!("unknown sherpa session {session}"))?;
        // Emit a final for any trailing text the endpoint detector didn't close.
        if !state.last_partial_text.is_empty() {
            events.push(AsrEvent::Final {
                revision: state.revision + 1,
                text: state.last_partial_text.clone(),
                segment_id: format!("{session}-seg-{}", state.segment_counter + 1),
            });
        }
        unsafe {
            sherpa_rs_sys::SherpaOnnxDestroyOnlineStream(state.stream);
        }
        Ok(events)
    }
}

impl Drop for SherpaStreamingRecognizer {
    fn drop(&mut self) {
        unsafe {
            for state in self.sessions.values() {
                sherpa_rs_sys::SherpaOnnxDestroyOnlineStream(state.stream);
            }
            sherpa_rs_sys::SherpaOnnxDestroyOnlineRecognizer(self.recognizer);
        }
    }
}

unsafe fn read_tokens(raw: &sherpa_rs_sys::SherpaOnnxOnlineRecognizerResult) -> Vec<Token> {
    let count = raw.count.max(0) as usize;
    if count == 0 || raw.tokens_arr.is_null() {
        return Vec::new();
    }
    let token_ptrs = std::slice::from_raw_parts(raw.tokens_arr, count);
    let timestamps = if raw.timestamps.is_null() {
        None
    } else {
        Some(std::slice::from_raw_parts(raw.timestamps, count))
    };
    let mut tokens = Vec::with_capacity(count);
    for (index, token_ptr) in token_ptrs.iter().enumerate() {
        let text = if token_ptr.is_null() {
            String::new()
        } else {
            CStr::from_ptr(*token_ptr).to_string_lossy().into_owned()
        };
        let start_ms = timestamps
            .and_then(|values| values.get(index))
            .map(|seconds| (seconds * 1000.0) as u32)
            .unwrap_or(0);
        let end_ms = timestamps
            .and_then(|values| values.get(index + 1))
            .map(|seconds| (seconds * 1000.0) as u32)
            .unwrap_or(start_ms);
        tokens.push(Token {
            text,
            start_ms,
            end_ms,
        });
    }
    tokens
}

fn cstring_from_path(path: &Path) -> Result<CString> {
    let value = path
        .to_str()
        .with_context(|| format!("model path is not valid UTF-8: {}", path.display()))?;
    Ok(CString::new(value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"x").unwrap();
    }

    fn temp_model_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "voxflow-sherpa-test-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detect_prefers_int8_weights() {
        let dir = temp_model_dir("int8");
        for name in [
            "tokens.txt",
            "encoder-epoch-99-avg-1.onnx",
            "encoder-epoch-99-avg-1.int8.onnx",
            "decoder-epoch-99-avg-1.onnx",
            "decoder-epoch-99-avg-1.int8.onnx",
            "joiner-epoch-99-avg-1.onnx",
            "joiner-epoch-99-avg-1.int8.onnx",
        ] {
            touch(&dir, name);
        }
        let files = SherpaModelFiles::detect(&dir).unwrap();
        assert!(files.int8);
        assert!(files
            .encoder
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with(".int8.onnx"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detect_falls_back_to_fp32_weights() {
        let dir = temp_model_dir("fp32");
        for name in [
            "tokens.txt",
            "encoder-epoch-99-avg-1.onnx",
            "decoder-epoch-99-avg-1.onnx",
            "joiner-epoch-99-avg-1.onnx",
        ] {
            touch(&dir, name);
        }
        let files = SherpaModelFiles::detect(&dir).unwrap();
        assert!(!files.int8);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detect_supports_mixed_precision_sets() {
        // Layout of sherpa-onnx-streaming-zipformer-zh-int8-2025-06-30.
        let dir = temp_model_dir("mixed");
        for name in [
            "tokens.txt",
            "encoder.int8.onnx",
            "decoder.onnx",
            "joiner.int8.onnx",
        ] {
            touch(&dir, name);
        }
        let files = SherpaModelFiles::detect(&dir).unwrap();
        assert!(files.int8);
        assert!(files.decoder.ends_with("decoder.onnx"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detect_fails_without_tokens() {
        let dir = temp_model_dir("missing");
        touch(&dir, "encoder.onnx");
        assert!(SherpaModelFiles::detect(&dir).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// End-to-end decode against a real model; skipped unless
    /// VOXFLOW_SHERPA_MODEL_DIR points at a streaming zipformer dir.
    #[test]
    fn streaming_decode_with_real_model() {
        let Ok(model_dir) = std::env::var("VOXFLOW_SHERPA_MODEL_DIR") else {
            eprintln!("skipping: VOXFLOW_SHERPA_MODEL_DIR not set");
            return;
        };
        let model_dir = PathBuf::from(model_dir);
        let wav = model_dir.join("test_wavs/0.wav");
        let config = SherpaStreamingConfig::from_model_dir(&model_dir).unwrap();
        let mut recognizer = SherpaStreamingRecognizer::new(config).unwrap();
        let session = recognizer.start_session().unwrap();

        let mut reader = hound::WavReader::open(&wav).unwrap();
        let sample_rate = reader.spec().sample_rate;
        let samples = reader
            .samples::<i16>()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut events = Vec::new();
        for chunk in samples.chunks(sample_rate as usize / 50) {
            recognizer
                .push_audio(
                    &session,
                    AudioFrame {
                        sample_rate_hz: sample_rate,
                        channels: 1,
                        pcm_i16: chunk.to_vec(),
                    },
                )
                .unwrap();
            events.extend(recognizer.poll_events(&session).unwrap());
        }
        events.extend(recognizer.finish_session(&session).unwrap());

        assert!(events
            .iter()
            .any(|event| matches!(event, AsrEvent::Partial { .. })));
        let final_text = events
            .iter()
            .rev()
            .find_map(|event| match event {
                AsrEvent::Final { text, .. } => Some(text.clone()),
                _ => None,
            })
            .expect("expected a final event");
        assert!(!final_text.is_empty());
        eprintln!("final text: {final_text}");
    }
}
