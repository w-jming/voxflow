import tomllib

import pytest

from voxflow.config import (
    AppConfig,
    load_config,
    save_user_daemon_hotkey,
    save_user_daemon_settings,
    save_user_text_script,
    save_user_text_semantic_correction,
    save_user_text_semantic_intent_backend,
)


def test_save_user_daemon_hotkey_creates_user_config(tmp_path):
    path = tmp_path / "config.toml"

    save_user_daemon_hotkey("ctrl+shift+space", path)

    data = tomllib.loads(path.read_text(encoding="utf-8"))
    assert data["daemon"]["hotkey"] == "ctrl+shift+space"


def test_save_user_daemon_hotkey_preserves_existing_sections(tmp_path):
    path = tmp_path / "config.toml"
    path.write_text(
        "\n".join(
            [
                "[asr]",
                'backend = "faster-whisper"',
                "",
                "[daemon]",
                'hotkey = "ctrl+alt+space"',
                "restore_focus = true",
                "",
            ]
        ),
        encoding="utf-8",
    )

    save_user_daemon_hotkey("ctrl+shift+return", path)

    data = tomllib.loads(path.read_text(encoding="utf-8"))
    assert data["asr"]["backend"] == "faster-whisper"
    assert data["daemon"]["hotkey"] == "ctrl+shift+return"
    assert data["daemon"]["restore_focus"] is True


def test_save_user_daemon_hotkey_rejects_invalid_hotkey(tmp_path):
    with pytest.raises(ValueError):
        save_user_daemon_hotkey("ctrl+a+b", tmp_path / "config.toml")


def test_default_config_uses_simplified_toggle_ctrl_space():
    config = AppConfig()

    assert config.asr.backend == "faster-whisper"
    assert config.asr.model == "bundled:faster-whisper-tiny"
    assert config.text.script == "simplified"
    assert config.text.semantic_correction_enabled is True
    assert config.text.semantic_intent_backend == "rules"
    assert config.daemon.hotkey == "ctrl+space"
    assert config.daemon.hotkey_mode == "toggle"


def test_save_user_daemon_settings_writes_mode(tmp_path):
    path = tmp_path / "config.toml"

    save_user_daemon_settings(hotkey="ctrl+space", hotkey_mode="hold", path=path)

    data = tomllib.loads(path.read_text(encoding="utf-8"))
    assert data["daemon"]["hotkey"] == "ctrl+space"
    assert data["daemon"]["hotkey_mode"] == "hold"


def test_save_user_text_script_writes_script(tmp_path):
    path = tmp_path / "config.toml"

    save_user_text_script("traditional", path)

    data = tomllib.loads(path.read_text(encoding="utf-8"))
    assert data["text"]["script"] == "traditional"


def test_save_user_text_semantic_correction_writes_bool(tmp_path):
    path = tmp_path / "config.toml"

    save_user_text_semantic_correction(False, path)

    data = tomllib.loads(path.read_text(encoding="utf-8"))
    assert data["text"]["semantic_correction_enabled"] is False


def test_save_user_text_semantic_intent_backend_writes_backend(tmp_path):
    path = tmp_path / "config.toml"

    save_user_text_semantic_intent_backend("rule", path)

    data = tomllib.loads(path.read_text(encoding="utf-8"))
    assert data["text"]["semantic_intent_backend"] == "rules"


def test_load_config_normalizes_aliases(tmp_path):
    path = tmp_path / "config.toml"
    path.write_text(
        "\n".join(
            [
                "[text]",
                'script = "zh_cn"',
                'semantic_intent_backend = "qwen3"',
                "",
                "[daemon]",
                'hotkey_mode = "push_to_talk"',
                "",
            ]
        ),
        encoding="utf-8",
    )

    config = load_config(path)

    assert config.text.script == "simplified"
    assert config.text.semantic_intent_backend == "qwen3-embedding"
    assert config.daemon.hotkey_mode == "hold"
