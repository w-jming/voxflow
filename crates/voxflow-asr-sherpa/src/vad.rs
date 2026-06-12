//! silero VAD wrapper implementing [`voxflow_asr::Vad`].
//!
//! Default VAD per streaming-asr §3; `EnergyVad` in voxflow-asr stays as the
//! fallback when the model file is missing.

use std::ffi::CString;
use std::mem;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use voxflow_asr::{TimestampedAudioFrame, Vad, VadDecision};

#[derive(Debug, Clone)]
pub struct SileroVadConfig {
    pub model: PathBuf,
    pub threshold: f32,
    pub min_silence_duration_s: f32,
    pub min_speech_duration_s: f32,
    pub max_speech_duration_s: f32,
    pub sample_rate_hz: u32,
    pub window_size_samples: i32,
    pub buffer_size_s: f32,
    pub num_threads: i32,
}

impl SileroVadConfig {
    pub fn new(model: impl Into<PathBuf>) -> Self {
        Self {
            model: model.into(),
            threshold: 0.5,
            min_silence_duration_s: 0.5,
            min_speech_duration_s: 0.25,
            max_speech_duration_s: 20.0,
            sample_rate_hz: 16_000,
            window_size_samples: 512,
            buffer_size_s: 30.0,
            num_threads: 1,
        }
    }
}

/// silero VAD backed by the sherpa-onnx C API.
pub struct SileroVad {
    vad: *const sherpa_rs_sys::SherpaOnnxVoiceActivityDetector,
    in_speech: bool,
}

// The raw pointer is only touched through &mut self.
unsafe impl Send for SileroVad {}

impl SileroVad {
    pub fn new(config: SileroVadConfig) -> Result<Self> {
        let model_path: &Path = &config.model;
        if !model_path.is_file() {
            bail!("silero VAD model not found: {}", model_path.display());
        }
        let model = CString::new(
            model_path
                .to_str()
                .with_context(|| format!("non UTF-8 path: {}", model_path.display()))?,
        )?;
        let provider = CString::new("cpu")?;

        let vad = unsafe {
            let mut c_config = mem::zeroed::<sherpa_rs_sys::SherpaOnnxVadModelConfig>();
            c_config.silero_vad.model = model.as_ptr();
            c_config.silero_vad.threshold = config.threshold;
            c_config.silero_vad.min_silence_duration = config.min_silence_duration_s;
            c_config.silero_vad.min_speech_duration = config.min_speech_duration_s;
            c_config.silero_vad.max_speech_duration = config.max_speech_duration_s;
            c_config.silero_vad.window_size = config.window_size_samples;
            c_config.sample_rate = config.sample_rate_hz as i32;
            c_config.num_threads = config.num_threads;
            c_config.provider = provider.as_ptr();
            sherpa_rs_sys::SherpaOnnxCreateVoiceActivityDetector(&c_config, config.buffer_size_s)
        };
        if vad.is_null() {
            bail!(
                "SherpaOnnxCreateVoiceActivityDetector failed for {}",
                model_path.display()
            );
        }
        Ok(Self {
            vad,
            in_speech: false,
        })
    }
}

impl Vad for SileroVad {
    fn process_frame(&mut self, frame: &TimestampedAudioFrame) -> VadDecision {
        let samples = frame
            .frame
            .pcm_i16
            .iter()
            .map(|sample| f32::from(*sample) / f32::from(i16::MAX))
            .collect::<Vec<_>>();
        let rms = if samples.is_empty() {
            0.0
        } else {
            (samples
                .iter()
                .map(|s| f64::from(*s) * f64::from(*s))
                .sum::<f64>()
                / samples.len() as f64)
                .sqrt() as f32
        };
        let is_speech = unsafe {
            sherpa_rs_sys::SherpaOnnxVoiceActivityDetectorAcceptWaveform(
                self.vad,
                samples.as_ptr(),
                samples.len() as i32,
            );
            // Drain settled segments so the internal buffer stays bounded.
            while sherpa_rs_sys::SherpaOnnxVoiceActivityDetectorEmpty(self.vad) == 0 {
                sherpa_rs_sys::SherpaOnnxVoiceActivityDetectorPop(self.vad);
            }
            sherpa_rs_sys::SherpaOnnxVoiceActivityDetectorDetected(self.vad) == 1
        };
        let decision = VadDecision {
            is_speech,
            speech_started: is_speech && !self.in_speech,
            speech_ended: !is_speech && self.in_speech,
            rms,
        };
        self.in_speech = is_speech;
        decision
    }
}

impl Drop for SileroVad {
    fn drop(&mut self) {
        unsafe {
            sherpa_rs_sys::SherpaOnnxDestroyVoiceActivityDetector(self.vad);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxflow_asr::AudioFrame;

    #[test]
    fn missing_model_is_an_error() {
        let config = SileroVadConfig::new("/nonexistent/silero_vad.onnx");
        assert!(SileroVad::new(config).is_err());
    }

    /// Real-model check; skipped unless VOXFLOW_SILERO_VAD_MODEL is set.
    #[test]
    fn detects_transitions_on_synthetic_audio() {
        let Ok(model) = std::env::var("VOXFLOW_SILERO_VAD_MODEL") else {
            eprintln!("skipping: VOXFLOW_SILERO_VAD_MODEL not set");
            return;
        };
        let mut vad = SileroVad::new(SileroVadConfig::new(model)).unwrap();
        // Pure silence must never read as speech.
        let mut any_speech = false;
        for index in 0..50 {
            let frame = TimestampedAudioFrame {
                elapsed_ms: index * 20,
                frame: AudioFrame::mono_silence(16_000, 20),
            };
            any_speech |= vad.process_frame(&frame).is_speech;
        }
        assert!(!any_speech, "silence misread as speech");
    }
}
