use serde_json::json;
use voxflow_fcitx5::engine::{Fcitx5EngineAdapter, Fcitx5Operation};
use voxflow_input::{DictationProjector, FrontendCapabilities};
use voxflow_ipc::Envelope;

#[test]
fn mock_dictation_events_translate_to_fcitx5_operations_without_status_text() {
    let mut projector = DictationProjector::default();
    let mut adapter = Fcitx5EngineAdapter::new(FrontendCapabilities::full());
    let ipc_events = [
        Envelope::event(
            "dictation.state_changed",
            json!({ "state": "listening", "session_id": "s" }),
        ),
        Envelope::event(
            "dictation.partial",
            json!({ "session_id": "s", "revision": 1, "text": "今天下午" }),
        ),
        Envelope::event(
            "dictation.stable",
            json!({ "session_id": "s", "revision": 2, "text": "今天下午" }),
        ),
        Envelope::event(
            "dictation.final",
            json!({ "session_id": "s", "revision": 3, "text": "今天下午三点开会" }),
        ),
    ];
    let mut operations = Vec::new();
    for event in ipc_events {
        for input_event in projector.project(&event).unwrap() {
            operations.extend(adapter.translate(input_event));
        }
    }

    assert_eq!(
        operations,
        vec![
            Fcitx5Operation::SessionStarted,
            Fcitx5Operation::SetPreedit {
                text: "今天下午".to_string(),
                cursor_pos: 4,
                underline: true,
            },
            Fcitx5Operation::ClearPreedit,
            Fcitx5Operation::CommitString {
                text: "今天下午".to_string(),
            },
            Fcitx5Operation::CommitString {
                text: "三点开会".to_string(),
            },
        ]
    );
    assert!(!operations
        .iter()
        .any(|operation| format!("{operation:?}").contains("听写中")));
}

#[test]
fn correction_applied_event_translates_to_fcitx5_delete_and_commit() {
    let mut projector = DictationProjector::default();
    let mut adapter = Fcitx5EngineAdapter::new(FrontendCapabilities::full());
    let ipc_events = [
        Envelope::event(
            "dictation.partial",
            json!({ "session_id": "s", "revision": 1, "text": "三点不对四点" }),
        ),
        Envelope::event(
            "correction.applied",
            json!({
                "operation_id": "op-1",
                "intent": "replace_entity",
                "input_events": [
                    { "type": "delete_before_cursor", "chars": 8 },
                    { "type": "commit", "text": "今天下午四点开会" }
                ]
            }),
        ),
    ];
    let mut operations = Vec::new();
    for event in ipc_events {
        for input_event in projector.project(&event).unwrap() {
            operations.extend(adapter.translate(input_event));
        }
    }

    assert_eq!(
        operations,
        vec![
            Fcitx5Operation::SetPreedit {
                text: "三点不对四点".to_string(),
                cursor_pos: 6,
                underline: true,
            },
            Fcitx5Operation::ClearPreedit,
            Fcitx5Operation::DeleteSurroundingText { chars: 8 },
            Fcitx5Operation::CommitString {
                text: "今天下午四点开会".to_string(),
            },
        ]
    );
}
