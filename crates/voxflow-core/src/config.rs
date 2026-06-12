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
