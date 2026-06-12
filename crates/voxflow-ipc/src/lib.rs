use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Command,
    Response,
    Event,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Envelope {
    pub version: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub kind: MessageKind,
    pub name: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recoverable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl Envelope {
    pub fn command(id: impl Into<String>, name: impl Into<String>, payload: Value) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            id: Some(id.into()),
            kind: MessageKind::Command,
            name: name.into(),
            payload,
            code: None,
            message: None,
            recoverable: None,
            details: None,
        }
    }

    pub fn response(id: Option<String>, name: impl Into<String>, payload: Value) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            id,
            kind: MessageKind::Response,
            name: name.into(),
            payload,
            code: None,
            message: None,
            recoverable: None,
            details: None,
        }
    }

    pub fn event(name: impl Into<String>, payload: Value) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            id: None,
            kind: MessageKind::Event,
            name: name.into(),
            payload,
            code: None,
            message: None,
            recoverable: None,
            details: None,
        }
    }

    pub fn error(
        id: Option<String>,
        name: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
        recoverable: bool,
        details: Value,
    ) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            id,
            kind: MessageKind::Error,
            name: name.into(),
            payload: json!({}),
            code: Some(code.into()),
            message: Some(message.into()),
            recoverable: Some(recoverable),
            details: Some(details),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictationState {
    Idle,
    Starting,
    Listening,
    Paused,
    Stopping,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendState {
    NotInstalled,
    Installed,
    Registered,
    Connected,
    Active,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoreInfo {
    pub version: String,
    pub state: String,
    pub uptime_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictationInfo {
    pub state: DictationState,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontendInfo {
    pub kind: Option<String>,
    pub state: FrontendState,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioInfo {
    pub device_id: Option<String>,
    pub label: Option<String>,
    pub available: bool,
    pub bluetooth_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelInfo {
    #[serde(default)]
    pub asr_backend: Option<String>,
    pub active_asr: String,
    pub active_refiner: Option<String>,
    pub intent_classifier: IntentClassifierInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentClassifierInfo {
    pub state: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathInfo {
    pub home: String,
    pub logs: String,
    pub models: String,
    pub cache: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub core: CoreInfo,
    pub dictation: DictationInfo,
    pub frontend: FrontendInfo,
    pub audio: AudioInfo,
    pub models: ModelInfo,
    pub paths: PathInfo,
    pub config_revision: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_envelope_matches_contract_shape() {
        let error = Envelope::error(
            Some("c-1".to_string()),
            "model.download",
            "model.not_found",
            "model not found",
            true,
            json!({"model_id": "missing"}),
        );
        let encoded = serde_json::to_value(error).unwrap();
        assert_eq!(encoded["kind"], "error");
        assert_eq!(encoded["code"], "model.not_found");
        assert_eq!(encoded["recoverable"], true);
    }
}
