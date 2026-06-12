use serde_json::json;
use voxflow_core::config::{Config, ThemeMode};

#[test]
fn default_hotkey_is_alt_s() {
    assert_eq!(Config::default().input.hotkey, "Alt+S");
}

#[test]
fn config_patch_preserves_unspecified_fields() {
    let mut config = Config::default();
    config
        .apply_json_patch(json!({ "ui": { "theme": "dark" } }))
        .unwrap();
    assert_eq!(config.ui.theme, ThemeMode::Dark);
    assert!(config.text.auto_punctuation);
    assert!(config.correction.enabled);
}
