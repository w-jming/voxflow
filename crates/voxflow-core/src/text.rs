//! Output-script post-processing (config `text.output_script`).
//!
//! Streaming ASR can emit either Chinese script depending on the model and the
//! audio (Qwen sometimes returns Traditional). We normalise to the configured
//! script with OpenCC phrase-level conversion (`ferrous-opencc`, embedded
//! dictionaries) rather than a hand-rolled character table, so词组 like
//! 「裏面/里面」「鼠標/鼠标」convert correctly.

use std::sync::Arc;

use ferrous_opencc::config::BuiltinConfig;
use ferrous_opencc::OpenCC;

use crate::config::OutputScript;

/// Converts recognized text to the configured script. Cheap to clone (the
/// loaded OpenCC dictionary set is shared behind an `Arc`).
#[derive(Clone)]
pub struct TextConverter {
    converter: Option<Arc<OpenCC>>,
}

impl std::fmt::Debug for TextConverter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextConverter")
            .field("active", &self.converter.is_some())
            .finish()
    }
}

impl Default for TextConverter {
    fn default() -> Self {
        Self::new(&OutputScript::Simplified)
    }
}

impl TextConverter {
    pub fn new(script: &OutputScript) -> Self {
        let builtin = match script {
            // Recognizer output may contain Traditional spans; fold to Simplified.
            OutputScript::Simplified => Some(BuiltinConfig::T2s),
            OutputScript::Traditional => Some(BuiltinConfig::S2t),
            OutputScript::Original => None,
        };
        let converter = builtin.and_then(|config| match OpenCC::from_config(config) {
            Ok(opencc) => Some(Arc::new(opencc)),
            Err(error) => {
                tracing::warn!(%error, "OpenCC init failed; passing text through unchanged");
                None
            }
        });
        Self { converter }
    }

    pub fn convert(&self, text: &str) -> String {
        match &self.converter {
            Some(opencc) => opencc.convert(text),
            None => text.to_string(),
        }
    }

    /// Returns the event with its text normalised to the configured script.
    pub fn convert_event(&self, event: crate::recognizer::AsrEvent) -> crate::recognizer::AsrEvent {
        use crate::recognizer::AsrEvent;
        if self.converter.is_none() {
            return event;
        }
        match event {
            AsrEvent::Partial {
                revision,
                text,
                tokens,
            } => AsrEvent::Partial {
                revision,
                text: self.convert(&text),
                tokens,
            },
            AsrEvent::Stable {
                revision,
                text,
                token_start,
                token_end,
            } => AsrEvent::Stable {
                revision,
                text: self.convert(&text),
                token_start,
                token_end,
            },
            AsrEvent::Final {
                revision,
                text,
                segment_id,
            } => AsrEvent::Final {
                revision,
                text: self.convert(&text),
                segment_id,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplified_folds_traditional_phrases() {
        let converter = TextConverter::new(&OutputScript::Simplified);
        // OpenCC t2s, phrase-aware orthographic conversion (裏→里 only in the
        // right context, 鼠標→鼠标).
        assert_eq!(converter.convert("開放中文轉換"), "开放中文转换");
        assert_eq!(converter.convert("滑鼠的鼠標在桌上"), "滑鼠的鼠标在桌上");
        assert_eq!(converter.convert("已经是简体"), "已经是简体");
    }

    #[test]
    fn traditional_converts_from_simplified() {
        let converter = TextConverter::new(&OutputScript::Traditional);
        assert_eq!(converter.convert("开放中文转换"), "開放中文轉換");
    }

    #[test]
    fn original_is_identity() {
        let converter = TextConverter::new(&OutputScript::Original);
        assert_eq!(converter.convert("繁體與简体混合"), "繁體與简体混合");
    }
}
