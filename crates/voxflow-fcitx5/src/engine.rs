use serde::{Deserialize, Serialize};
use voxflow_input::{FrontendCapabilities, InputEvent};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Fcitx5Operation {
    SetPreedit {
        text: String,
        cursor_pos: usize,
        underline: bool,
    },
    CommitString {
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
pub struct Fcitx5EngineAdapter {
    capabilities: FrontendCapabilities,
    preedit_visible: bool,
}

impl Fcitx5EngineAdapter {
    pub fn new(capabilities: FrontendCapabilities) -> Self {
        Self {
            capabilities,
            preedit_visible: false,
        }
    }

    pub fn translate(&mut self, event: InputEvent) -> Vec<Fcitx5Operation> {
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
                vec![Fcitx5Operation::SetPreedit {
                    text,
                    cursor_pos,
                    underline,
                }]
            }
            InputEvent::Commit { text } => vec![Fcitx5Operation::CommitString { text }],
            InputEvent::DeleteBeforeCursor { chars } => {
                if self.capabilities.delete_surrounding {
                    vec![Fcitx5Operation::DeleteSurroundingText { chars }]
                } else {
                    Vec::new()
                }
            }
            InputEvent::ClearPreedit => {
                if self.preedit_visible {
                    self.preedit_visible = false;
                    vec![Fcitx5Operation::ClearPreedit]
                } else {
                    Vec::new()
                }
            }
            InputEvent::SessionStarted => vec![Fcitx5Operation::SessionStarted],
            InputEvent::SessionStopped => {
                let mut operations = Vec::new();
                if self.preedit_visible {
                    self.preedit_visible = false;
                    operations.push(Fcitx5Operation::ClearPreedit);
                }
                operations.push(Fcitx5Operation::SessionStopped);
                operations
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_becomes_fcitx5_preedit_operation() {
        let mut adapter = Fcitx5EngineAdapter::new(FrontendCapabilities::full());
        assert_eq!(
            adapter.translate(InputEvent::SetPreedit {
                text: "今天下午".to_string(),
                cursor_pos: 4,
                underline: true,
            }),
            vec![Fcitx5Operation::SetPreedit {
                text: "今天下午".to_string(),
                cursor_pos: 4,
                underline: true,
            }]
        );
    }

    #[test]
    fn commit_only_mode_suppresses_preedit() {
        let mut adapter = Fcitx5EngineAdapter::new(FrontendCapabilities::commit_only());
        assert!(adapter
            .translate(InputEvent::SetPreedit {
                text: "今天下午".to_string(),
                cursor_pos: 4,
                underline: true,
            })
            .is_empty());
    }

    #[test]
    fn delete_requires_surrounding_delete_capability() {
        let mut full = Fcitx5EngineAdapter::new(FrontendCapabilities::full());
        assert_eq!(
            full.translate(InputEvent::DeleteBeforeCursor { chars: 2 }),
            vec![Fcitx5Operation::DeleteSurroundingText { chars: 2 }]
        );
        let mut limited = Fcitx5EngineAdapter::new(FrontendCapabilities::commit_only());
        assert!(limited
            .translate(InputEvent::DeleteBeforeCursor { chars: 2 })
            .is_empty());
    }
}
