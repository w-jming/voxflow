import tomllib

import pytest

from local_speak_input.config import save_user_daemon_hotkey


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
