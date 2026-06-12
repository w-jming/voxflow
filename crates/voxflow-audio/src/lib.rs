#[cfg(feature = "pipewire-native")]
mod pipewire_native;
#[cfg(feature = "pipewire-native")]
pub use pipewire_native::PipeWireAudioSource;

use std::env;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use voxflow_asr::{AudioFrame, TimestampedAudioFrame};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureConfig {
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub frame_duration_ms: u32,
    pub queue_capacity_frames: usize,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            sample_rate_hz: 16_000,
            channels: 1,
            frame_duration_ms: 20,
            queue_capacity_frames: 128,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioDevice {
    pub id: String,
    pub label: String,
    pub backend: AudioBackend,
    pub is_default: bool,
    pub available: bool,
    pub bluetooth_profile: Option<String>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioBackend {
    PipeWire,
    PulseAudio,
    Alsa,
    Synthetic,
}

pub trait AudioSource {
    fn start(&mut self, config: CaptureConfig) -> Result<()>;
    fn next_frame(&mut self) -> Result<Option<TimestampedAudioFrame>>;
    fn stop(&mut self) -> Result<()>;
}

pub fn bounded_audio_queue(capacity_frames: usize) -> (BoundedAudioProducer, BoundedAudioConsumer) {
    let (sender, receiver) = sync_channel(capacity_frames);
    let dropped_frames = Arc::new(AtomicUsize::new(0));
    (
        BoundedAudioProducer {
            sender,
            dropped_frames: Arc::clone(&dropped_frames),
        },
        BoundedAudioConsumer {
            receiver,
            dropped_frames,
        },
    )
}

#[derive(Debug, Clone)]
pub struct BoundedAudioProducer {
    sender: SyncSender<TimestampedAudioFrame>,
    dropped_frames: Arc<AtomicUsize>,
}

impl BoundedAudioProducer {
    pub fn try_push(&self, frame: TimestampedAudioFrame) -> bool {
        match self.sender.try_send(frame) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                self.dropped_frames.fetch_add(1, Ordering::Relaxed);
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    pub fn dropped_frames(&self) -> usize {
        self.dropped_frames.load(Ordering::Relaxed)
    }
}

#[derive(Debug)]
pub struct BoundedAudioConsumer {
    receiver: Receiver<TimestampedAudioFrame>,
    dropped_frames: Arc<AtomicUsize>,
}

impl BoundedAudioConsumer {
    pub fn try_next(&self) -> Option<TimestampedAudioFrame> {
        self.receiver.try_recv().ok()
    }

    pub fn drain(&self) -> Vec<TimestampedAudioFrame> {
        let mut frames = Vec::new();
        while let Some(frame) = self.try_next() {
            frames.push(frame);
        }
        frames
    }

    pub fn dropped_frames(&self) -> usize {
        self.dropped_frames.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone)]
pub struct SyntheticAudioSource {
    total_duration_ms: u32,
    emitted_ms: u32,
    sample_i16: i16,
    config: Option<CaptureConfig>,
}

impl SyntheticAudioSource {
    pub fn silence(total_duration_ms: u32) -> Self {
        Self::constant(total_duration_ms, 0)
    }

    pub fn constant(total_duration_ms: u32, sample_i16: i16) -> Self {
        Self {
            total_duration_ms,
            emitted_ms: 0,
            sample_i16,
            config: None,
        }
    }
}

impl AudioSource for SyntheticAudioSource {
    fn start(&mut self, config: CaptureConfig) -> Result<()> {
        self.emitted_ms = 0;
        self.config = Some(config);
        Ok(())
    }

    fn next_frame(&mut self) -> Result<Option<TimestampedAudioFrame>> {
        let Some(config) = &self.config else {
            return Ok(None);
        };
        if self.emitted_ms >= self.total_duration_ms {
            return Ok(None);
        }

        let remaining = self.total_duration_ms - self.emitted_ms;
        let duration_ms = remaining.min(config.frame_duration_ms);
        let frame = AudioFrame::mono_constant(config.sample_rate_hz, duration_ms, self.sample_i16);
        let timestamped = TimestampedAudioFrame {
            elapsed_ms: self.emitted_ms as u64,
            frame,
        };
        self.emitted_ms += duration_ms;
        Ok(Some(timestamped))
    }

    fn stop(&mut self) -> Result<()> {
        self.config = None;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioLevel {
    pub peak: f32,
    pub rms: f32,
}

pub fn measure_level(frame: &AudioFrame) -> AudioLevel {
    if frame.pcm_i16.is_empty() {
        return AudioLevel {
            peak: 0.0,
            rms: 0.0,
        };
    }

    let mut peak = 0.0_f32;
    let mut sum_squares = 0.0_f64;
    for sample in &frame.pcm_i16 {
        let normalized = *sample as f32 / i16::MAX as f32;
        peak = peak.max(normalized.abs());
        sum_squares += (normalized as f64) * (normalized as f64);
    }

    AudioLevel {
        peak,
        rms: (sum_squares / frame.pcm_i16.len() as f64).sqrt() as f32,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipeWireRuntimeProbe {
    pub pipewire_command: bool,
    pub pw_cli_command: bool,
    pub wpctl_command: bool,
    pub pw_record_command: bool,
    pub libpipewire_runtime: bool,
    pub pkg_config_development_files: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioDeviceInventory {
    pub devices: Vec<AudioDevice>,
    pub default_device_id: Option<String>,
    pub warnings: Vec<String>,
    pub probe: PipeWireRuntimeProbe,
}

pub fn probe_pipewire_runtime() -> PipeWireRuntimeProbe {
    PipeWireRuntimeProbe {
        pipewire_command: command_exists("pipewire"),
        pw_cli_command: command_exists("pw-cli"),
        wpctl_command: command_exists("wpctl"),
        pw_record_command: command_exists("pw-record"),
        libpipewire_runtime: libpipewire_runtime_available(),
        pkg_config_development_files: pkg_config_has_pipewire(),
        version: pipewire_version(),
    }
}

pub fn list_input_devices() -> AudioDeviceInventory {
    let probe = probe_pipewire_runtime();
    let mut warnings = Vec::new();
    let mut devices = Vec::new();
    if !probe.wpctl_command {
        warnings.push("wpctl command is not available; input devices cannot be listed".to_string());
    } else {
        match Command::new("wpctl").arg("status").output() {
            Ok(output) if output.status.success() => {
                devices = parse_wpctl_sources(&String::from_utf8_lossy(&output.stdout));
                if devices.is_empty() {
                    warnings.push("wpctl status did not report input sources".to_string());
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                warnings.push(if stderr.is_empty() {
                    "wpctl status failed without diagnostic output".to_string()
                } else {
                    format!("wpctl status failed: {stderr}")
                });
            }
            Err(error) => warnings.push(format!("failed to run wpctl status: {error}")),
        }
    }
    let default_device_id = devices
        .iter()
        .find(|device| device.is_default)
        .or_else(|| devices.first())
        .map(|device| device.id.clone());
    AudioDeviceInventory {
        devices,
        default_device_id,
        warnings,
        probe,
    }
}

fn pipewire_version() -> Option<String> {
    let output = Command::new("pipewire").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_pipewire_version_output(&String::from_utf8_lossy(&output.stdout))
}

fn command_exists(command: &str) -> bool {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .any(|dir| dir.join(command).is_file())
}

fn libpipewire_runtime_available() -> bool {
    [
        "/lib/x86_64-linux-gnu/libpipewire-0.3.so.0",
        "/usr/lib/x86_64-linux-gnu/libpipewire-0.3.so.0",
    ]
    .iter()
    .any(|path| Path::new(path).exists())
}

fn pkg_config_has_pipewire() -> bool {
    Command::new("pkg-config")
        .args(["--exists", "libpipewire-0.3"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn parse_pipewire_version_output(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.strip_prefix("Compiled with libpipewire ")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

pub fn parse_wpctl_sources(output: &str) -> Vec<AudioDevice> {
    let mut in_sources = false;
    let mut devices = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with("Sources:") {
            in_sources = true;
            continue;
        }
        if in_sources && is_wpctl_section_header(trimmed) {
            break;
        }
        if !in_sources {
            continue;
        }
        let Some(device) = parse_wpctl_source_line(trimmed) else {
            continue;
        };
        devices.push(device);
    }
    devices
}

fn is_wpctl_section_header(trimmed: &str) -> bool {
    ["Devices:", "Sinks:", "Filters:", "Streams:", "Settings:"]
        .iter()
        .any(|section| trimmed.ends_with(section))
}

fn parse_wpctl_source_line(trimmed: &str) -> Option<AudioDevice> {
    let body = trimmed
        .trim_start_matches(|ch: char| !ch.is_ascii_digit() && ch != '*')
        .trim();
    if body.is_empty() {
        return None;
    }
    let is_default = body.starts_with('*');
    let body = body.trim_start_matches('*').trim();
    let (node_id, rest) = body.split_once('.')?;
    let node_id = node_id.trim();
    if node_id.is_empty() || !node_id.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let label = rest
        .split(" [")
        .next()
        .unwrap_or(rest)
        .trim()
        .trim_matches('"')
        .to_string();
    if label.is_empty() {
        return None;
    }
    Some(AudioDevice {
        id: format!("pipewire:{node_id}"),
        label: label.clone(),
        backend: AudioBackend::PipeWire,
        is_default,
        available: true,
        bluetooth_profile: infer_bluetooth_profile(&label),
        sample_rate_hz: None,
        channels: None,
    })
}

fn infer_bluetooth_profile(label: &str) -> Option<String> {
    let lower = label.to_ascii_lowercase();
    if lower.contains("headset") || lower.contains("handsfree") || lower.contains("hfp") {
        Some("headset".to_string())
    } else if lower.contains("a2dp") {
        Some("a2dp-sink".to_string())
    } else if lower.contains("bluez") || lower.contains("bluetooth") {
        Some("bluetooth".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_source_emits_fixed_duration_frames() {
        let mut source = SyntheticAudioSource::silence(45);
        source
            .start(CaptureConfig {
                frame_duration_ms: 20,
                ..CaptureConfig::default()
            })
            .unwrap();
        let frames = [
            source.next_frame().unwrap().unwrap(),
            source.next_frame().unwrap().unwrap(),
            source.next_frame().unwrap().unwrap(),
        ];
        assert_eq!(frames[0].elapsed_ms, 0);
        assert_eq!(frames[1].elapsed_ms, 20);
        assert_eq!(frames[2].elapsed_ms, 40);
        assert_eq!(frames[0].frame.duration_ms(), 20);
        assert_eq!(frames[2].frame.duration_ms(), 5);
        assert!(source.next_frame().unwrap().is_none());
    }

    #[test]
    fn bounded_queue_counts_dropped_frames() {
        let (producer, consumer) = bounded_audio_queue(1);
        assert!(producer.try_push(TimestampedAudioFrame {
            elapsed_ms: 0,
            frame: AudioFrame::mono_silence(16_000, 20),
        }));
        assert!(!producer.try_push(TimestampedAudioFrame {
            elapsed_ms: 20,
            frame: AudioFrame::mono_silence(16_000, 20),
        }));
        assert_eq!(producer.dropped_frames(), 1);
        assert_eq!(consumer.dropped_frames(), 1);
        assert_eq!(consumer.drain().len(), 1);
    }

    #[test]
    fn level_meter_reports_peak_and_rms() {
        let frame = AudioFrame {
            sample_rate_hz: 16_000,
            channels: 1,
            pcm_i16: vec![0, i16::MAX, -i16::MAX],
        };
        let level = measure_level(&frame);
        assert!((level.peak - 1.0).abs() < 0.0001);
        assert!((level.rms - 0.81649).abs() < 0.0001);
    }

    #[test]
    fn parse_pipewire_version_from_command_output() {
        let output = "pipewire\nCompiled with libpipewire 1.0.5\nLinked with libpipewire 1.0.5\n";
        assert_eq!(
            parse_pipewire_version_output(output),
            Some("1.0.5".to_string())
        );
    }

    #[test]
    fn parse_wpctl_sources_extracts_default_input_devices() {
        let output = r#"Audio
 ├─ Devices:
 │      73. OpenFit by Shokz
 ├─ Sinks:
 │  *   49. Built-in Audio Analog Stereo [vol: 0.40]
 ├─ Sources:
 │  *   55. Built-in Audio Analog Stereo [vol: 1.00]
 │      80. OpenFit by Shokz Headset [vol: 1.00]
 ├─ Filters:
"#;
        let devices = parse_wpctl_sources(output);

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "pipewire:55");
        assert_eq!(devices[0].label, "Built-in Audio Analog Stereo");
        assert!(devices[0].is_default);
        assert_eq!(devices[1].id, "pipewire:80");
        assert_eq!(devices[1].bluetooth_profile.as_deref(), Some("headset"));
    }

    #[test]
    fn parse_wpctl_sources_returns_empty_without_sources_section() {
        let devices = parse_wpctl_sources("Audio\n ├─ Sinks:\n │ * 49. Speaker\n");

        assert!(devices.is_empty());
    }
}
