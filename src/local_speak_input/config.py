from __future__ import annotations

from dataclasses import dataclass, field, fields
from pathlib import Path
from typing import Any
import json
import os
import tomllib


SYSTEM_CONFIG_PATH = Path("/etc/local-speak-input/config.toml")
DEFAULT_CONFIG_PATH = Path("~/.config/local-speak-input/config.toml").expanduser()


@dataclass(slots=True)
class ASRConfig:
    backend: str = "faster-whisper"
    model: str = "large-v3"
    device: str = "auto"
    compute_type: str = "auto"
    language: str = "auto"
    api_base: str = "http://127.0.0.1:8000/v1"
    api_model: str = "whisper-1"
    api_key_env: str = "OPENAI_API_KEY"
    timeout: float = 120.0


@dataclass(slots=True)
class TextConfig:
    remove_fillers: bool = True
    auto_punctuation: bool = True


@dataclass(slots=True)
class InputConfig:
    injector: str = "auto"
    dry_run: bool = False
    backspace_delay: float = 0.0


@dataclass(slots=True)
class AudioConfig:
    sample_rate: int = 16000
    frame_ms: int = 30
    silence_ms: int = 700
    max_utterance_s: float = 15.0
    activation_timeout_s: float = 8.0
    min_voice_ms: int = 240
    energy_threshold: float = 0.012


@dataclass(slots=True)
class DaemonConfig:
    hotkey: str = "ctrl+alt+space"
    restore_focus: bool = True


@dataclass(slots=True)
class AppConfig:
    asr: ASRConfig = field(default_factory=ASRConfig)
    text: TextConfig = field(default_factory=TextConfig)
    input: InputConfig = field(default_factory=InputConfig)
    audio: AudioConfig = field(default_factory=AudioConfig)
    daemon: DaemonConfig = field(default_factory=DaemonConfig)


def _coerce_dataclass(cls: type[Any], data: dict[str, Any] | None, base: Any | None = None) -> Any:
    current = base if base is not None else cls()
    if not data:
        return current
    valid = {field.name for field in fields(cls)}
    values = {field.name: getattr(current, field.name) for field in fields(cls)}
    values.update({key: value for key, value in data.items() if key in valid})
    return cls(**values)


def load_config(path: str | os.PathLike[str] | None = None) -> AppConfig:
    if path:
        return _load_single_config(Path(path).expanduser(), AppConfig())

    config = AppConfig()
    if SYSTEM_CONFIG_PATH.exists():
        config = _load_single_config(SYSTEM_CONFIG_PATH, config)
    if DEFAULT_CONFIG_PATH.exists():
        config = _load_single_config(DEFAULT_CONFIG_PATH, config)
    return config


def _load_single_config(config_path: Path, base: AppConfig) -> AppConfig:
    if not config_path.exists():
        return base
    with config_path.open("rb") as fh:
        data = tomllib.load(fh)
    return AppConfig(
        asr=_coerce_dataclass(ASRConfig, data.get("asr"), base.asr),
        text=_coerce_dataclass(TextConfig, data.get("text"), base.text),
        input=_coerce_dataclass(InputConfig, data.get("input"), base.input),
        audio=_coerce_dataclass(AudioConfig, data.get("audio"), base.audio),
        daemon=_coerce_dataclass(DaemonConfig, data.get("daemon"), base.daemon),
    )


def save_user_daemon_hotkey(hotkey: str, path: str | os.PathLike[str] | None = None) -> Path:
    from .hotkey import parse_hotkey

    parse_hotkey(hotkey)
    config_path = Path(path).expanduser() if path else DEFAULT_CONFIG_PATH
    config_path.parent.mkdir(parents=True, exist_ok=True)
    text = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    updated = _set_toml_value(text, "daemon", "hotkey", hotkey)
    tomllib.loads(updated)
    config_path.write_text(updated, encoding="utf-8")
    return config_path


def _set_toml_value(text: str, section: str, key: str, value: str) -> str:
    value_line = f"{key} = {json.dumps(value, ensure_ascii=False)}"
    lines = text.splitlines()
    section_header = f"[{section}]"
    section_start: int | None = None
    section_end = len(lines)

    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped == section_header:
            section_start = index
            continue
        if section_start is not None and index > section_start and stripped.startswith("[") and stripped.endswith("]"):
            section_end = index
            break

    if section_start is None:
        if lines and lines[-1].strip():
            lines.append("")
        lines.extend([section_header, value_line])
        return "\n".join(lines) + "\n"

    for index in range(section_start + 1, section_end):
        stripped = lines[index].strip()
        if stripped.startswith(f"{key} ") or stripped.startswith(f"{key}="):
            lines[index] = value_line
            return "\n".join(lines) + "\n"

    lines.insert(section_start + 1, value_line)
    return "\n".join(lines) + "\n"
