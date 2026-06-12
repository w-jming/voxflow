use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use voxflow_ipc::{Envelope, MessageKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendKind {
    Ibus,
    Fcitx5,
    Compatibility,
}

impl FrontendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ibus => "ibus",
            Self::Fcitx5 => "fcitx5",
            Self::Compatibility => "compatibility",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrontendCapabilities {
    pub preedit: bool,
    pub surrounding_text: bool,
    pub delete_surrounding: bool,
}

impl FrontendCapabilities {
    pub fn full() -> Self {
        Self {
            preedit: true,
            surrounding_text: true,
            delete_surrounding: true,
        }
    }

    pub fn commit_only() -> Self {
        Self {
            preedit: false,
            surrounding_text: false,
            delete_surrounding: false,
        }
    }

    pub fn to_ipc_list(&self) -> Vec<&'static str> {
        let mut capabilities = Vec::new();
        if self.preedit {
            capabilities.push("preedit");
        }
        if self.surrounding_text {
            capabilities.push("surrounding_text");
        }
        if self.delete_surrounding {
            capabilities.push("delete_surrounding");
        }
        capabilities
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputEvent {
    SetPreedit {
        text: String,
        cursor_pos: usize,
        underline: bool,
    },
    Commit {
        text: String,
    },
    DeleteBeforeCursor {
        chars: usize,
    },
    ClearPreedit,
    SessionStarted,
    SessionStopped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FrontendEvent {
    Focused { app_hint: Option<String> },
    Blurred,
    Activated,
    Deactivated,
    Capabilities { capabilities: FrontendCapabilities },
    SurroundingTextChanged { before_cursor_tail: String },
}

impl FrontendEvent {
    pub fn as_report_payload(&self) -> serde_json::Value {
        match self {
            Self::Focused { app_hint } => json!({ "event": "focused", "app_hint": app_hint }),
            Self::Blurred => json!({ "event": "blurred" }),
            Self::Activated => json!({ "event": "activated" }),
            Self::Deactivated => json!({ "event": "deactivated" }),
            Self::Capabilities { capabilities } => {
                json!({ "event": "capabilities", "capabilities": capabilities.to_ipc_list() })
            }
            Self::SurroundingTextChanged { before_cursor_tail } => {
                json!({ "event": "surrounding_text_changed", "before_cursor_tail": before_cursor_tail })
            }
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProjectionError {
    #[error("event {0} is not supported by the input projector")]
    UnsupportedEvent(String),
    #[error("dictation event {0} is missing text")]
    MissingText(String),
    #[error("correction event {0} is missing input_events")]
    MissingInputEvents(String),
    #[error("correction event {0} has invalid input_events")]
    InvalidInputEvents(String),
    #[error("preedit text looks like a status placeholder: {0}")]
    StatusPlaceholder(String),
}

#[derive(Debug, Default)]
pub struct DictationProjector {
    committed_text: String,
    preedit_text: String,
}

impl DictationProjector {
    pub fn project(&mut self, envelope: &Envelope) -> Result<Vec<InputEvent>, ProjectionError> {
        if envelope.kind != MessageKind::Event {
            return Err(ProjectionError::UnsupportedEvent(envelope.name.clone()));
        }
        match envelope.name.as_str() {
            "dictation.state_changed" => self.project_state(envelope),
            "dictation.partial" => self.project_partial(envelope),
            "dictation.stable" => self.project_commit_like(envelope),
            "dictation.final" => self.project_commit_like(envelope),
            "correction.applied" => self.project_correction_applied(envelope),
            other => Err(ProjectionError::UnsupportedEvent(other.to_string())),
        }
    }

    fn project_state(&mut self, envelope: &Envelope) -> Result<Vec<InputEvent>, ProjectionError> {
        let state = envelope
            .payload
            .get("state")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        match state {
            "starting" | "listening" => {
                self.committed_text.clear();
                self.preedit_text.clear();
                Ok(vec![InputEvent::SessionStarted])
            }
            "idle" | "stopping" => {
                self.committed_text.clear();
                let mut events = Vec::new();
                if !self.preedit_text.is_empty() {
                    events.push(InputEvent::ClearPreedit);
                    self.preedit_text.clear();
                }
                events.push(InputEvent::SessionStopped);
                Ok(events)
            }
            _ => Ok(Vec::new()),
        }
    }

    fn project_partial(&mut self, envelope: &Envelope) -> Result<Vec<InputEvent>, ProjectionError> {
        let text = dictation_text(envelope)?;
        if is_status_placeholder(text) {
            return Err(ProjectionError::StatusPlaceholder(text.to_string()));
        }
        self.preedit_text = text.to_string();
        Ok(vec![InputEvent::SetPreedit {
            text: text.to_string(),
            cursor_pos: text.chars().count(),
            underline: true,
        }])
    }

    fn project_commit_like(
        &mut self,
        envelope: &Envelope,
    ) -> Result<Vec<InputEvent>, ProjectionError> {
        let text = dictation_text(envelope)?;
        let suffix =
            uncommitted_suffix(&self.committed_text, text).unwrap_or_else(|| text.to_string());
        let mut events = Vec::new();
        if !self.preedit_text.is_empty() {
            self.preedit_text.clear();
            events.push(InputEvent::ClearPreedit);
        }
        if !suffix.is_empty() {
            self.committed_text.push_str(&suffix);
            events.push(InputEvent::Commit { text: suffix });
        }
        Ok(events)
    }

    fn project_correction_applied(
        &mut self,
        envelope: &Envelope,
    ) -> Result<Vec<InputEvent>, ProjectionError> {
        let input_events = envelope
            .payload
            .get("input_events")
            .cloned()
            .ok_or_else(|| ProjectionError::MissingInputEvents(envelope.name.clone()))
            .and_then(|value| {
                serde_json::from_value::<Vec<InputEvent>>(value)
                    .map_err(|_| ProjectionError::InvalidInputEvents(envelope.name.clone()))
            })?;
        let mut events = Vec::new();
        if !self.preedit_text.is_empty() {
            self.preedit_text.clear();
            events.push(InputEvent::ClearPreedit);
        }
        self.committed_text.clear();
        events.extend(input_events);
        Ok(events)
    }
}

pub fn frontend_register_command(
    id: impl Into<String>,
    kind: FrontendKind,
    frontend_version: impl Into<String>,
    capabilities: &FrontendCapabilities,
) -> Envelope {
    Envelope::command(
        id,
        "frontend.register",
        json!({
            "kind": kind.as_str(),
            "frontend_version": frontend_version.into(),
            "capabilities": capabilities.to_ipc_list(),
        }),
    )
}

pub fn frontend_report_command(id: impl Into<String>, event: &FrontendEvent) -> Envelope {
    Envelope::command(id, "frontend.report", event.as_report_payload())
}

pub fn is_status_placeholder(text: &str) -> bool {
    matches!(
        text.trim(),
        "听写中" | "正在听写" | "录音中" | "处理中" | "dictating" | "listening" | "processing"
    )
}

fn dictation_text(envelope: &Envelope) -> Result<&str, ProjectionError> {
    envelope
        .payload
        .get("text")
        .and_then(|value| value.as_str())
        .ok_or_else(|| ProjectionError::MissingText(envelope.name.clone()))
}

fn uncommitted_suffix(committed: &str, text: &str) -> Option<String> {
    text.strip_prefix(committed).map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn register_command_uses_ipc_capability_names() {
        let command = frontend_register_command(
            "ibus-1",
            FrontendKind::Ibus,
            "0.3.0",
            &FrontendCapabilities::full(),
        );
        assert_eq!(command.name, "frontend.register");
        assert_eq!(command.payload["kind"], "ibus");
        assert_eq!(
            command.payload["capabilities"],
            json!(["preedit", "surrounding_text", "delete_surrounding"])
        );
    }

    #[test]
    fn partial_projects_to_real_preedit_only() {
        let mut projector = DictationProjector::default();
        let events = projector
            .project(&Envelope::event(
                "dictation.partial",
                json!({ "session_id": "s", "revision": 1, "text": "今天下午" }),
            ))
            .unwrap();
        assert_eq!(
            events,
            vec![InputEvent::SetPreedit {
                text: "今天下午".to_string(),
                cursor_pos: 4,
                underline: true,
            }]
        );
    }

    #[test]
    fn status_placeholder_is_rejected_from_preedit() {
        let mut projector = DictationProjector::default();
        let error = projector
            .project(&Envelope::event(
                "dictation.partial",
                json!({ "session_id": "s", "revision": 1, "text": "听写中" }),
            ))
            .unwrap_err();
        assert_eq!(
            error,
            ProjectionError::StatusPlaceholder("听写中".to_string())
        );
    }

    #[test]
    fn stable_and_final_commit_only_uncommitted_suffix() {
        let mut projector = DictationProjector::default();
        projector
            .project(&Envelope::event(
                "dictation.partial",
                json!({ "session_id": "s", "revision": 1, "text": "今天下午" }),
            ))
            .unwrap();
        let stable = projector
            .project(&Envelope::event(
                "dictation.stable",
                json!({ "session_id": "s", "revision": 2, "text": "今天下午" }),
            ))
            .unwrap();
        assert_eq!(
            stable,
            vec![
                InputEvent::ClearPreedit,
                InputEvent::Commit {
                    text: "今天下午".to_string()
                }
            ]
        );
        let final_events = projector
            .project(&Envelope::event(
                "dictation.final",
                json!({ "session_id": "s", "revision": 3, "text": "今天下午三点开会" }),
            ))
            .unwrap();
        assert_eq!(
            final_events,
            vec![InputEvent::Commit {
                text: "三点开会".to_string()
            }]
        );
    }

    #[test]
    fn correction_applied_projects_embedded_input_events_and_clears_preedit() {
        let mut projector = DictationProjector::default();
        projector
            .project(&Envelope::event(
                "dictation.partial",
                json!({ "session_id": "s", "revision": 1, "text": "不对四点" }),
            ))
            .unwrap();
        let events = projector
            .project(&Envelope::event(
                "correction.applied",
                json!({
                    "operation_id": "op-1",
                    "input_events": [
                        { "type": "delete_before_cursor", "chars": 8 },
                        { "type": "commit", "text": "今天下午四点开会" }
                    ]
                }),
            ))
            .unwrap();

        assert_eq!(
            events,
            vec![
                InputEvent::ClearPreedit,
                InputEvent::DeleteBeforeCursor { chars: 8 },
                InputEvent::Commit {
                    text: "今天下午四点开会".to_string(),
                }
            ]
        );
    }
}
