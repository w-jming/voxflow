use std::fs;

use voxflow_control::{
    sample_control_center_snapshot, write_static_bundle, ConnectionState, ControlCenterSnapshot,
    StatusTone, STATIC_ASSETS,
};
use voxflow_ipc::{
    AudioInfo, CoreInfo, DictationInfo, DictationState, FrontendInfo, FrontendState,
    IntentClassifierInfo, ModelInfo, PathInfo, StatusSnapshot,
};

#[test]
fn sample_snapshot_has_required_navigation_and_status_cards() {
    let snapshot = sample_control_center_snapshot();

    assert_eq!(snapshot.connection, ConnectionState::Connected);
    assert_eq!(snapshot.nav.len(), 8);
    assert_eq!(snapshot.overview_cards.len(), 4);
    assert!(snapshot
        .overview_cards
        .iter()
        .any(|card| card.id == "audio" && card.tone == StatusTone::Error));
    assert!(snapshot
        .overview_cards
        .iter()
        .any(|card| card.id == "model" && card.tone == StatusTone::Degraded));
}

#[test]
fn connected_ready_status_aggregates_to_input_available() {
    let status = StatusSnapshot {
        core: CoreInfo {
            version: "0.3.0".to_string(),
            state: "running".to_string(),
            uptime_ms: 1,
        },
        dictation: DictationInfo {
            state: DictationState::Idle,
            session_id: None,
        },
        frontend: FrontendInfo {
            kind: Some("ibus".to_string()),
            state: FrontendState::Active,
            capabilities: vec![
                "preedit".to_string(),
                "surrounding_text".to_string(),
                "delete_surrounding".to_string(),
            ],
        },
        audio: AudioInfo {
            device_id: Some("default".to_string()),
            label: Some("默认麦克风".to_string()),
            available: true,
            bluetooth_profile: None,
        },
        models: ModelInfo {
            asr_backend: Some("mock".to_string()),
            engine_state: Some("ready".to_string()),
            active_asr: "streaming-zh-en-small".to_string(),
            active_refiner: None,
            intent_classifier: IntentClassifierInfo {
                state: "ready".to_string(),
                version: Some("0.1.0".to_string()),
            },
        },
        paths: PathInfo {
            home: "~/.voxflow".to_string(),
            logs: "~/.voxflow/logs".to_string(),
            models: "~/.voxflow/models".to_string(),
            cache: "~/.voxflow/cache".to_string(),
        },
        config_revision: 7,
    };

    let snapshot = ControlCenterSnapshot::from_status(status, ConnectionState::Connected);

    assert_eq!(snapshot.global_status.label, "可输入");
    assert_eq!(snapshot.global_status.tone, StatusTone::Ready);
    assert!(snapshot
        .overview_cards
        .iter()
        .all(|card| card.tone == StatusTone::Ready));
}

#[test]
fn static_bundle_contains_html_state_and_brand_assets() {
    let dir = std::env::temp_dir().join(format!("voxflow-control-web-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    write_static_bundle(&dir).unwrap();

    assert!(dir.join("index.html").exists());
    assert!(dir.join("app.css").exists());
    assert!(dir.join("app.js").exists());
    assert!(dir.join("mock-state.json").exists());
    for asset in STATIC_ASSETS {
        assert!(
            dir.join("assets").join(asset.name).exists(),
            "{}",
            asset.name
        );
    }

    let html = fs::read_to_string(dir.join("index.html")).unwrap();
    assert!(html.contains("VoxFlow"));
    assert!(html.contains("app.js"));
    let state = fs::read_to_string(dir.join("mock-state.json")).unwrap();
    assert!(state.contains("\"overview_cards\""));

    fs::remove_dir_all(&dir).unwrap();
}
