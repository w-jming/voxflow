use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub input: InputConfig,
    pub text: TextConfig,
    pub correction: CorrectionConfig,
    pub ui: UiConfig,
    pub models: ModelConfig,
    /// ASR 后端选择(D-22:默认 Qwen3-ASR-1.7B + vLLM;可切火山 API 或本地 zipformer)。
    #[serde(default)]
    pub asr: AsrConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AsrConfig {
    pub backend: AsrBackend,
    pub qwen3: Qwen3SidecarConfig,
    pub volcano: VolcanoApiConfig,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AsrBackend {
    /// 本地 Qwen3-ASR + vLLM sidecar(默认,D-22)。
    Qwen3Vllm,
    /// 火山引擎大模型流式语音识别 API(需用户配置密钥)。
    VolcanoApi,
    /// 本地 sherpa-onnx streaming zipformer(CPU 兜底)。
    ZipformerLocal,
    /// 测试用脚本化识别器。
    Mock,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Qwen3SidecarConfig {
    /// venv Python 解释器;部署脚本写入绝对路径。
    pub python: String,
    /// sidecar 脚本路径;空 = 在可执行文件旁与仓库 `sidecar/` 中查找。
    pub sidecar_script: String,
    /// HF 模型 ID 或本地权重目录。
    pub model: String,
    pub gpu_memory_utilization: f32,
    pub chunk_size_sec: f32,
    pub unfixed_chunk_num: u32,
    pub unfixed_token_num: u32,
    pub max_new_tokens: u32,
    pub max_model_len: u32,
    /// 转写语言:"zh"(默认,中文+内嵌英文,避免被整体翻译成英文)/ "en" / "" 自动。
    pub language: String,
}

impl Default for Qwen3SidecarConfig {
    fn default() -> Self {
        Self {
            python: "python3".to_string(),
            sidecar_script: String::new(),
            model: "Qwen/Qwen3-ASR-1.7B".to_string(),
            gpu_memory_utilization: 0.7,
            chunk_size_sec: 2.0,
            unfixed_chunk_num: 2,
            unfixed_token_num: 5,
            max_new_tokens: 32,
            max_model_len: 16_384,
            language: "zh".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VolcanoApiConfig {
    /// 控制台获取的 APP ID(X-Api-App-Key)。
    pub app_key: String,
    /// Access Token(X-Api-Access-Key);仅存本机用户配置,不入仓库。
    pub access_key: String,
    pub resource_id: String,
    pub model_name: String,
    pub endpoint: String,
    pub enable_itn: bool,
    pub enable_punc: bool,
}

impl Default for VolcanoApiConfig {
    fn default() -> Self {
        Self {
            app_key: String::new(),
            access_key: String::new(),
            resource_id: "volc.bigasr.sauc.duration".to_string(),
            model_name: "bigmodel".to_string(),
            endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel".to_string(),
            enable_itn: true,
            enable_punc: true,
        }
    }
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            backend: AsrBackend::Qwen3Vllm,
            qwen3: Qwen3SidecarConfig::default(),
            volcano: VolcanoApiConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputConfig {
    pub hotkey: String,
    pub mode: DictationMode,
    pub frontend: FrontendMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DictationMode {
    Toggle,
    Hold,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendMode {
    InputMethod,
    CompatibilityInjection,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextConfig {
    pub output_script: OutputScript,
    pub auto_punctuation: bool,
    pub filler_cleanup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OutputScript {
    Simplified,
    Traditional,
    Original,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CorrectionConfig {
    pub enabled: bool,
    pub threshold_mode: ThresholdMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdMode {
    Conservative,
    Standard,
    Aggressive,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiConfig {
    pub theme: ThemeMode,
    pub reduce_motion: bool,
    pub status_indicator: StatusIndicatorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusIndicatorConfig {
    pub enabled: bool,
    pub show_mode: IndicatorShowMode,
    pub position: IndicatorPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IndicatorShowMode {
    Always,
    DictationOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IndicatorPosition {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelConfig {
    pub active_asr: String,
    pub active_refiner: Option<String>,
    pub intent_classifier: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input: InputConfig {
                hotkey: "Alt+S".to_string(),
                mode: DictationMode::Toggle,
                frontend: FrontendMode::InputMethod,
            },
            text: TextConfig {
                output_script: OutputScript::Simplified,
                auto_punctuation: true,
                filler_cleanup: true,
            },
            correction: CorrectionConfig {
                enabled: true,
                threshold_mode: ThresholdMode::Standard,
            },
            ui: UiConfig {
                theme: ThemeMode::System,
                reduce_motion: false,
                status_indicator: StatusIndicatorConfig {
                    enabled: true,
                    show_mode: IndicatorShowMode::Always,
                    position: IndicatorPosition::TopRight,
                },
            },
            models: ModelConfig {
                active_asr: "streaming-zh-en-small".to_string(),
                active_refiner: None,
                intent_classifier: "semantic-intent-small".to_string(),
            },
            asr: AsrConfig::default(),
        }
    }
}

impl Config {
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text =
            fs::read_to_string(path).with_context(|| format!("read config {}", path.display()))?;
        match toml::from_str::<Self>(&text) {
            Ok(config) => Ok(config),
            Err(err) => {
                let broken = path.with_extension(format!("toml.broken.{}", unix_seconds()));
                fs::rename(path, &broken).with_context(|| {
                    format!(
                        "move broken config {} to {}",
                        path.display(),
                        broken.display()
                    )
                })?;
                tracing::warn!(error = %err, broken = %broken.display(), "config parse failed");
                Ok(Self::default())
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create config parent {}", parent.display()))?;
        }
        let text = toml::to_string_pretty(self)?;
        fs::write(path, text).with_context(|| format!("write config {}", path.display()))
    }

    pub fn apply_json_patch(&mut self, patch: Value) -> Result<()> {
        let mut value = serde_json::to_value(&*self)?;
        merge_value(&mut value, patch);
        *self = serde_json::from_value(value)?;
        Ok(())
    }
}

fn merge_value(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                merge_value(target.entry(key).or_insert(Value::Null), value);
            }
        }
        (target, patch) => *target = patch,
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_redesign_decisions() {
        let config = Config::default();
        assert_eq!(config.input.hotkey, "Alt+S");
        assert_eq!(config.input.mode, DictationMode::Toggle);
        assert_eq!(config.ui.theme, ThemeMode::System);
        assert!(config.correction.enabled);
    }

    #[test]
    fn json_patch_updates_nested_fields() {
        let mut config = Config::default();
        config
            .apply_json_patch(serde_json::json!({
                "ui": { "theme": "dark" },
                "correction": { "enabled": false }
            }))
            .unwrap();
        assert_eq!(config.ui.theme, ThemeMode::Dark);
        assert!(!config.correction.enabled);
    }
}
