use serde::{Deserialize, Serialize};
use voxflow_input::{FrontendCapabilities, InputEvent};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IbusOperation {
    UpdatePreeditText {
        text: String,
        cursor_pos: usize,
        underline: bool,
    },
    CommitText {
        text: String,
    },
    DeleteSurroundingText {
        chars: usize,
    },
    ClearPreedit,
    SessionStarted,
    SessionStopped,
}

#[derive(Debug, Clone)]
pub struct IbusEngineAdapter {
    capabilities: FrontendCapabilities,
    preedit_visible: bool,
}

impl IbusEngineAdapter {
    pub fn new(capabilities: FrontendCapabilities) -> Self {
        Self {
            capabilities,
            preedit_visible: false,
        }
    }

    pub fn translate(&mut self, event: InputEvent) -> Vec<IbusOperation> {
        match event {
            InputEvent::SetPreedit {
                text,
                cursor_pos,
                underline,
            } => {
                if !self.capabilities.preedit {
                    return Vec::new();
                }
                self.preedit_visible = true;
                vec![IbusOperation::UpdatePreeditText {
                    text,
                    cursor_pos,
                    underline,
                }]
            }
            InputEvent::Commit { text } => vec![IbusOperation::CommitText { text }],
            InputEvent::DeleteBeforeCursor { chars } => {
                if self.capabilities.delete_surrounding {
                    vec![IbusOperation::DeleteSurroundingText { chars }]
                } else {
                    Vec::new()
                }
            }
            InputEvent::ClearPreedit => {
                if self.preedit_visible {
                    self.preedit_visible = false;
                    vec![IbusOperation::ClearPreedit]
                } else {
                    Vec::new()
                }
            }
            InputEvent::SessionStarted => vec![IbusOperation::SessionStarted],
            InputEvent::SessionStopped => {
                let mut operations = Vec::new();
                if self.preedit_visible {
                    self.preedit_visible = false;
                    operations.push(IbusOperation::ClearPreedit);
                }
                operations.push(IbusOperation::SessionStopped);
                operations
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_becomes_ibus_preedit_operation() {
        let mut adapter = IbusEngineAdapter::new(FrontendCapabilities::full());
        let operations = adapter.translate(InputEvent::SetPreedit {
            text: "今天下午".to_string(),
            cursor_pos: 4,
            underline: true,
        });
        assert_eq!(
            operations,
            vec![IbusOperation::UpdatePreeditText {
                text: "今天下午".to_string(),
                cursor_pos: 4,
                underline: true,
            }]
        );
    }

    #[test]
    fn commit_only_mode_suppresses_preedit() {
        let mut adapter = IbusEngineAdapter::new(FrontendCapabilities::commit_only());
        let operations = adapter.translate(InputEvent::SetPreedit {
            text: "今天下午".to_string(),
            cursor_pos: 4,
            underline: true,
        });
        assert!(operations.is_empty());
    }

    #[test]
    fn delete_requires_delete_surrounding_capability() {
        let mut full = IbusEngineAdapter::new(FrontendCapabilities::full());
        assert_eq!(
            full.translate(InputEvent::DeleteBeforeCursor { chars: 2 }),
            vec![IbusOperation::DeleteSurroundingText { chars: 2 }]
        );
        let mut limited = IbusEngineAdapter::new(FrontendCapabilities::commit_only());
        assert!(limited
            .translate(InputEvent::DeleteBeforeCursor { chars: 2 })
            .is_empty());
    }
}
