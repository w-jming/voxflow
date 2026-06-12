use std::fs;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use voxflow_core::core::VoxflowCore;
use voxflow_core::ipc::{Envelope, FrontendState, MessageKind};
use voxflow_core::model::PROFILE_DIR_ENV;
use voxflow_core::recognizer::AsrEvent;
use voxflow_core::{Config, VoxflowPaths};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn test_core() -> VoxflowCore {
    let paths = VoxflowPaths {
        home: "/tmp/voxflow-core-test".into(),
        config: "/tmp/voxflow-core-test/config.toml".into(),
        models: "/tmp/voxflow-core-test/models".into(),
        cache: "/tmp/voxflow-core-test/cache".into(),
        logs: "/tmp/voxflow-core-test/logs".into(),
        run: "/tmp/voxflow-core-test/run".into(),
        ledger: "/tmp/voxflow-core-test/ledger".into(),
        runtime_dir: "/tmp/voxflow-core-test/run/voxflow".into(),
        socket: "/tmp/voxflow-core-test/run/voxflow/core.sock".into(),
    };
    VoxflowCore::with_config(paths, Config::default())
}

fn test_core_with_home(home: std::path::PathBuf) -> VoxflowCore {
    let paths = VoxflowPaths {
        config: home.join("config.toml"),
        models: home.join("models"),
        cache: home.join("cache"),
        logs: home.join("logs"),
        run: home.join("run"),
        ledger: home.join("ledger"),
        runtime_dir: home.join("run").join("voxflow"),
        socket: home.join("run").join("voxflow").join("core.sock"),
        home,
    };
    VoxflowCore::with_config(paths, Config::default())
}

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "voxflow-core-ipc-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn write_test_profile(
    profile_dir: &std::path::Path,
    model_id: &str,
    file_name: &str,
    sha256: &str,
) {
    fs::create_dir_all(profile_dir).unwrap();
    fs::write(
        profile_dir.join(format!("{model_id}.toml")),
        format!(
            r#"[profile]
id = "{model_id}"
label = "Test Model"
kind = "asr-streaming"
backend = "sherpa-onnx"
version = "2026.06"
license = "Apache-2.0"
languages = ["zh", "en"]
streaming = true
recommended = true
min_ram_mb = 1024

[source]
url = "https://models.example.test/{model_id}/"
size_bytes = 3

[[files]]
path = "{file_name}"
sha256 = "{sha256}"
"#
        ),
    )
    .unwrap();
}

#[test]
fn hello_selects_protocol_version() {
    let mut core = test_core();
    let response = core
        .handle_command(Envelope::command(
            "h-1",
            "core.hello",
            json!({ "client": "cli", "client_version": "0.3.0", "proto_versions": [1] }),
        ))
        .response;
    assert_eq!(response.kind, MessageKind::Response);
    assert_eq!(response.payload["selected_version"], 1);
}

#[test]
fn status_contains_core_paths_and_default_model() {
    let mut core = test_core();
    let response = core
        .handle_command(Envelope::command("s-1", "core.status", json!({})))
        .response;
    assert_eq!(response.kind, MessageKind::Response);
    assert_eq!(
        response.payload["models"]["active_asr"],
        "streaming-zh-en-small"
    );
    assert_eq!(response.payload["paths"]["home"], "/tmp/voxflow-core-test");
}

#[test]
fn audio_list_devices_returns_inventory_shape() {
    let mut core = test_core();
    let response = core
        .handle_command(Envelope::command("a-1", "audio.list_devices", json!({})))
        .response;

    assert_eq!(response.kind, MessageKind::Response);
    assert!(response.payload["devices"].is_array());
    assert!(response.payload["warnings"].is_array());
    assert!(response.payload["probe"].is_object());
    assert!(response.payload["probe"]["wpctl_command"].is_boolean());
}

#[test]
fn model_list_reports_profile_and_local_state() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = unique_temp_dir("model-list");
    let profile_dir = root.join("profiles");
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(
        profile_dir.join("streaming-zh-en-small.toml"),
        r#"[profile]
id = "streaming-zh-en-small"
label = "VoxFlow Streaming Small"
kind = "asr-streaming"
backend = "sherpa-onnx"
version = "2026.06"
license = "Apache-2.0"
languages = ["zh", "en"]
streaming = true
recommended = true
min_ram_mb = 1024

[source]
url = "https://models.example.test/streaming-zh-en-small/"
size_bytes = 0

[[files]]
path = "encoder.int8.onnx"
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
"#,
    )
    .unwrap();
    std::env::set_var(PROFILE_DIR_ENV, &profile_dir);
    let mut core = test_core();
    let response = core
        .handle_command(Envelope::command("m-1", "model.list", json!({})))
        .response;
    std::env::remove_var(PROFILE_DIR_ENV);
    let _ = fs::remove_dir_all(root);

    assert_eq!(response.kind, MessageKind::Response);
    assert_eq!(
        response.payload["models"][0]["profile"]["id"],
        "streaming-zh-en-small"
    );
    assert_eq!(
        response.payload["models"][0]["local"]["state"],
        "not_installed"
    );
}

#[test]
fn model_import_copy_writes_manifest_and_reports_ready_model() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = unique_temp_dir("model-import");
    let profile_dir = root.join("profiles");
    let source_dir = root.join("source");
    fs::create_dir_all(&profile_dir).unwrap();
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("tokens.txt"), b"abc").unwrap();
    fs::write(
        profile_dir.join("streaming-zh-en-small.toml"),
        r#"[profile]
id = "streaming-zh-en-small"
label = "VoxFlow Streaming Small"
kind = "asr-streaming"
backend = "sherpa-onnx"
version = "2026.06"
license = "Apache-2.0"
languages = ["zh", "en"]
streaming = true
recommended = true
min_ram_mb = 1024

[source]
url = "https://models.example.test/streaming-zh-en-small/"
size_bytes = 3

[[files]]
path = "tokens.txt"
sha256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
"#,
    )
    .unwrap();
    std::env::set_var(PROFILE_DIR_ENV, &profile_dir);
    let mut core = test_core_with_home(root.join("home"));
    let response = core
        .handle_command(Envelope::command(
            "m-2",
            "model.import",
            json!({
                "model_id": "streaming-zh-en-small",
                "path": source_dir,
                "mode": "copy"
            }),
        ))
        .response;
    std::env::remove_var(PROFILE_DIR_ENV);

    assert_eq!(response.kind, MessageKind::Response);
    assert!(response.payload["task_id"]
        .as_str()
        .unwrap()
        .starts_with("import-streaming-zh-en-small-"));
    assert_eq!(
        response.payload["import"]["model"]["local"]["state"],
        "active"
    );
    assert!(root
        .join("home/models/streaming-zh-en-small/manifest.lock")
        .is_file());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn model_activate_ready_model_updates_config_and_emits_events() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = unique_temp_dir("model-activate");
    let profile_dir = root.join("profiles");
    let source_dir = root.join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("tokens.txt"), b"abc").unwrap();
    write_test_profile(
        &profile_dir,
        "alt-model",
        "tokens.txt",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    );
    std::env::set_var(PROFILE_DIR_ENV, &profile_dir);
    let mut core = test_core_with_home(root.join("home"));
    let import = core
        .handle_command(Envelope::command(
            "m-3",
            "model.import",
            json!({
                "model_id": "alt-model",
                "path": source_dir,
                "mode": "copy"
            }),
        ))
        .response;
    assert_eq!(import.kind, MessageKind::Response);

    let outcome = core.handle_command(Envelope::command(
        "m-4",
        "model.activate",
        json!({ "model_id": "alt-model" }),
    ));
    std::env::remove_var(PROFILE_DIR_ENV);

    assert_eq!(outcome.response.kind, MessageKind::Response);
    assert_eq!(outcome.response.payload["active_asr"], "alt-model");
    assert_eq!(
        outcome.response.payload["model"]["local"]["state"],
        "active"
    );
    assert!(outcome
        .events
        .iter()
        .any(|event| event.name == "config.changed"));
    assert!(outcome
        .events
        .iter()
        .any(|event| event.name == "model.state_changed"));
    assert!(fs::read_to_string(root.join("home/config.toml"))
        .unwrap()
        .contains("active_asr = \"alt-model\""));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn model_delete_removes_non_active_model() {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = unique_temp_dir("model-delete");
    let profile_dir = root.join("profiles");
    let source_dir = root.join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("tokens.txt"), b"abc").unwrap();
    write_test_profile(
        &profile_dir,
        "alt-model",
        "tokens.txt",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    );
    std::env::set_var(PROFILE_DIR_ENV, &profile_dir);
    let mut core = test_core_with_home(root.join("home"));
    core.handle_command(Envelope::command(
        "m-5",
        "model.import",
        json!({
            "model_id": "alt-model",
            "path": source_dir,
            "mode": "copy"
        }),
    ));
    let outcome = core.handle_command(Envelope::command(
        "m-6",
        "model.delete",
        json!({ "model_id": "alt-model" }),
    ));
    std::env::remove_var(PROFILE_DIR_ENV);

    assert_eq!(outcome.response.kind, MessageKind::Response);
    assert_eq!(outcome.response.payload["delete"]["deleted"], true);
    assert!(!root.join("home/models/alt-model").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn correction_list_recent_returns_records_shape() {
    let mut core = test_core();
    let response = core
        .handle_command(Envelope::command(
            "c-1",
            "correction.list_recent",
            json!({}),
        ))
        .response;

    assert_eq!(response.kind, MessageKind::Response);
    assert_eq!(response.payload["records"].as_array().unwrap().len(), 0);
}

#[test]
fn correction_applied_event_includes_projectable_input_events() {
    let mut core = test_core();
    core.handle_command(Envelope::command(
        "f-1",
        "frontend.register",
        json!({
            "kind": "ibus",
            "frontend_version": "0.3.0",
            "capabilities": ["preedit", "surrounding_text", "delete_surrounding"]
        }),
    ));
    core.set_mock_script(vec![AsrEvent::Final {
        revision: 1,
        text: "今天下午三点开会".to_string(),
        segment_id: "seg-1".to_string(),
    }]);
    core.handle_command(Envelope::command(
        "d-1",
        "dictation.start",
        json!({ "frontend": "ibus", "mode": "continuous" }),
    ));
    core.handle_command(Envelope::command(
        "f-2",
        "frontend.report",
        json!({
            "event": "surrounding_text_changed",
            "before_cursor_tail": "今天下午三点开会"
        }),
    ));

    core.set_mock_script(vec![AsrEvent::Final {
        revision: 2,
        text: "三点不对,四点".to_string(),
        segment_id: "seg-correction".to_string(),
    }]);
    let outcome = core.handle_command(Envelope::command(
        "d-2",
        "dictation.start",
        json!({ "frontend": "ibus", "mode": "continuous" }),
    ));
    let applied = outcome
        .events
        .iter()
        .find(|event| event.name == "correction.applied")
        .expect("correction.applied event");

    assert_eq!(applied.payload["operation_id"], "op-1");
    assert_eq!(applied.payload["intent"], "replace_entity");
    assert_eq!(applied.payload["segments"], json!(["seg-1"]));
    assert_eq!(
        applied.payload["input_events"],
        json!([
            { "type": "delete_before_cursor", "chars": 8 },
            { "type": "commit", "text": "今天下午四点开会" }
        ])
    );
    assert!(!outcome
        .events
        .iter()
        .any(|event| event.name == "dictation.final"));

    let recent = core
        .handle_command(Envelope::command(
            "c-2",
            "correction.list_recent",
            json!({}),
        ))
        .response;
    assert_eq!(recent.payload["records"][0]["applied"], true);
    assert_eq!(
        recent.payload["records"][0]["reason_code"],
        "repair_marker_and_entity_pair"
    );
}

#[test]
fn dictation_start_emits_partial_stable_final_events() {
    let mut core = test_core();
    let outcome = core.handle_command(Envelope::command(
        "d-1",
        "dictation.start",
        json!({ "frontend": "cli", "mode": "continuous" }),
    ));
    let names: Vec<_> = outcome
        .events
        .iter()
        .map(|event| event.name.as_str())
        .collect();
    assert!(names.contains(&"dictation.partial"));
    assert!(names.contains(&"dictation.stable"));
    assert!(names.contains(&"dictation.final"));
}

#[test]
fn shutdown_command_marks_outcome_for_server_exit() {
    let mut core = test_core();
    let outcome = core.handle_command(Envelope::command("x-1", "core.shutdown", json!({})));
    assert!(outcome.shutdown);
    assert_eq!(outcome.response.kind, MessageKind::Response);
}

#[test]
fn frontend_register_updates_status_snapshot() {
    let mut core = test_core();
    let outcome = core.handle_command(Envelope::command(
        "f-1",
        "frontend.register",
        json!({
            "kind": "ibus",
            "frontend_version": "0.3.0",
            "capabilities": ["preedit", "surrounding_text", "delete_surrounding"]
        }),
    ));
    assert_eq!(outcome.response.kind, MessageKind::Response);
    assert!(outcome
        .events
        .iter()
        .any(|event| event.name == "frontend.state_changed"));
    let snapshot = core.status_snapshot();
    assert_eq!(snapshot.frontend.kind.as_deref(), Some("ibus"));
    assert_eq!(snapshot.frontend.state, FrontendState::Connected);
    assert_eq!(
        snapshot.frontend.capabilities,
        vec![
            "preedit".to_string(),
            "surrounding_text".to_string(),
            "delete_surrounding".to_string()
        ]
    );
}

#[test]
fn frontend_focus_report_marks_frontend_active() {
    let mut core = test_core();
    core.handle_command(Envelope::command(
        "f-1",
        "frontend.register",
        json!({ "kind": "ibus", "frontend_version": "0.3.0", "capabilities": ["preedit"] }),
    ));
    core.handle_command(Envelope::command(
        "f-2",
        "frontend.report",
        json!({ "event": "focused", "app_hint": "org.gnome.TextEditor" }),
    ));
    assert_eq!(core.status_snapshot().frontend.state, FrontendState::Active);
}
