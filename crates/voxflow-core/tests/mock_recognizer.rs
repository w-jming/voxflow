use voxflow_core::recognizer::{AsrEvent, MockRecognizer, StreamingRecognizer};

#[test]
fn custom_script_is_replayed_in_order() {
    let script = vec![
        AsrEvent::Partial {
            revision: 1,
            text: "hello".to_string(),
            tokens: Vec::new(),
        },
        AsrEvent::Final {
            revision: 2,
            text: "hello world".to_string(),
            segment_id: "seg-1".to_string(),
        },
    ];
    let mut recognizer = MockRecognizer::with_script(script.clone());
    let session = recognizer.start_session().unwrap();
    assert_eq!(recognizer.poll_events(&session).unwrap(), script);
}
