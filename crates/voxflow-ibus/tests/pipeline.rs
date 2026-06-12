use serde_json::json;
use voxflow_ibus::engine::{IbusEngineAdapter, IbusOperation};
use voxflow_input::{DictationProjector, FrontendCapabilities};
use voxflow_ipc::Envelope;

#[test]
fn mock_dictation_events_translate_to_ibus_operations_without_status_text() {
    let mut projector = DictationProjector::default();
    let mut adapter = IbusEngineAdapter::new(FrontendCapabilities::full());
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
            IbusOperation::SessionStarted,
            IbusOperation::UpdatePreeditText {
                text: "今天下午".to_string(),
                cursor_pos: 4,
                underline: true,
            },
            IbusOperation::ClearPreedit,
            IbusOperation::CommitText {
                text: "今天下午".to_string(),
            },
            IbusOperation::CommitText {
                text: "三点开会".to_string(),
            },
        ]
    );
    assert!(!operations
        .iter()
        .any(|operation| format!("{operation:?}").contains("听写中")));
}

#[test]
fn correction_applied_event_translates_to_ibus_delete_and_commit() {
    let mut projector = DictationProjector::default();
    let mut adapter = IbusEngineAdapter::new(FrontendCapabilities::full());
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
            IbusOperation::UpdatePreeditText {
                text: "三点不对四点".to_string(),
                cursor_pos: 6,
                underline: true,
            },
            IbusOperation::ClearPreedit,
            IbusOperation::DeleteSurroundingText { chars: 8 },
            IbusOperation::CommitText {
                text: "今天下午四点开会".to_string(),
            },
        ]
    );
}
