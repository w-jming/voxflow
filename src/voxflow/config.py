from __future__ import annotations

from dataclasses import dataclass, field, fields
from pathlib import Path
from typing import Any
import json
import os
import tomllib


SYSTEM_CONFIG_PATH = Path("/etc/voxflow/config.toml")
DEFAULT_CONFIG_PATH = Path("~/.config/voxflow/config.toml").expanduser()


@dataclass(slots=True)
class ASRConfig:
    backend: str = "faster-whisper"
    model: str = "bundled:faster-whisper-tiny"
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
    script: str = "simplified"
    semantic_correction_enabled: bool = True
    semantic_intent_backend: str = "rules"


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
    hotkey: str = "ctrl+space"
    hotkey_mode: str = "toggle"
    hold_min_s: float = 0.25
    toggle_debounce_s: float = 0.8
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
        return _normalize_config(_load_single_config(Path(path).expanduser(), AppConfig()))

    config = AppConfig()
    if SYSTEM_CONFIG_PATH.exists():
        config = _load_single_config(SYSTEM_CONFIG_PATH, config)
    if DEFAULT_CONFIG_PATH.exists():
        config = _load_single_config(DEFAULT_CONFIG_PATH, config)
    return _normalize_config(config)


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
    return save_user_daemon_settings(hotkey=hotkey, path=path)


def save_user_daemon_settings(
    *,
    hotkey: str | None = None,
    hotkey_mode: str | None = None,
    path: str | os.PathLike[str] | None = None,
) -> Path:
    from .hotkey import parse_hotkey

    config_path = Path(path).expanduser() if path else DEFAULT_CONFIG_PATH
    config_path.parent.mkdir(parents=True, exist_ok=True)
    text = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    updated = text
    if hotkey is not None:
        parse_hotkey(hotkey)
        updated = _set_toml_value(updated, "daemon", "hotkey", hotkey)
    if hotkey_mode is not None:
        updated = _set_toml_value(updated, "daemon", "hotkey_mode", normalize_hotkey_mode(hotkey_mode))
    tomllib.loads(updated)
    config_path.write_text(updated, encoding="utf-8")
    return config_path


def save_user_text_script(script: str, path: str | os.PathLike[str] | None = None) -> Path:
    config_path = Path(path).expanduser() if path else DEFAULT_CONFIG_PATH
    config_path.parent.mkdir(parents=True, exist_ok=True)
    text = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    updated = _set_toml_value(text, "text", "script", normalize_script(script))
    tomllib.loads(updated)
    config_path.write_text(updated, encoding="utf-8")
    return config_path


def save_user_text_semantic_correction(enabled: bool, path: str | os.PathLike[str] | None = None) -> Path:
    config_path = Path(path).expanduser() if path else DEFAULT_CONFIG_PATH
    config_path.parent.mkdir(parents=True, exist_ok=True)
    text = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    updated = _set_toml_value(text, "text", "semantic_correction_enabled", enabled)
    tomllib.loads(updated)
    config_path.write_text(updated, encoding="utf-8")
    return config_path


def save_user_text_semantic_intent_backend(
    backend: str,
    path: str | os.PathLike[str] | None = None,
) -> Path:
    config_path = Path(path).expanduser() if path else DEFAULT_CONFIG_PATH
    config_path.parent.mkdir(parents=True, exist_ok=True)
    text = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    updated = _set_toml_value(text, "text", "semantic_intent_backend", normalize_semantic_intent_backend(backend))
    tomllib.loads(updated)
    config_path.write_text(updated, encoding="utf-8")
    return config_path


def save_user_asr_settings(
    *,
    backend: str | None = None,
    model: str | None = None,
    device: str | None = None,
    language: str | None = None,
    path: str | os.PathLike[str] | None = None,
) -> Path:
    config_path = Path(path).expanduser() if path else DEFAULT_CONFIG_PATH
    config_path.parent.mkdir(parents=True, exist_ok=True)
    updated = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    for key, value in {
        "backend": backend,
        "model": model,
        "device": device,
        "language": language,
    }.items():
        if value is not None:
            updated = _set_toml_value(updated, "asr", key, str(value))
    tomllib.loads(updated)
    config_path.write_text(updated, encoding="utf-8")
    return config_path


def normalize_script(script: str) -> str:
    value = script.strip().lower().replace("-", "_")
    aliases = {
        "s": "simplified",
        "zh_cn": "simplified",
        "cn": "simplified",
        "simplified_chinese": "simplified",
        "简体": "simplified",
        "t": "traditional",
        "zh_tw": "traditional",
        "tw": "traditional",
        "traditional_chinese": "traditional",
        "繁体": "traditional",
        "none": "original",
        "raw": "original",
        "原文": "original",
    }
    normalized = aliases.get(value, value)
    if normalized not in {"simplified", "traditional", "original"}:
        raise ValueError("文本字形必须是 simplified、traditional 或 original")
    return normalized


def normalize_hotkey_mode(mode: str) -> str:
    value = mode.strip().lower().replace("-", "_")
    aliases = {
        "press": "toggle",
        "press_to_talk": "toggle",
        "toggle_recording": "toggle",
        "按键切换": "toggle",
        "按住": "hold",
        "hold_to_talk": "hold",
        "push_to_talk": "hold",
    }
    normalized = aliases.get(value, value)
    if normalized not in {"toggle", "hold"}:
        raise ValueError("快捷键模式必须是 toggle 或 hold")
    return normalized


def normalize_semantic_intent_backend(backend: str) -> str:
    value = backend.strip().lower().replace("_", "-")
    aliases = {
        "rule": "rules",
        "rules-state-machine": "rules",
        "minilm": "minilm-setfit",
        "setfit": "minilm-setfit",
        "qwen3": "qwen3-embedding",
        "qwen3-embedding-0.6b": "qwen3-embedding",
        "llm": "llm-arbiter",
    }
    normalized = aliases.get(value, value)
    if normalized not in {"rules", "minilm-setfit", "qwen3-embedding", "llm-arbiter"}:
        raise ValueError("语义意图后端必须是 rules、minilm-setfit、qwen3-embedding 或 llm-arbiter")
    return normalized


def _normalize_config(config: AppConfig) -> AppConfig:
    config.text.script = normalize_script(config.text.script)
    config.text.semantic_intent_backend = normalize_semantic_intent_backend(config.text.semantic_intent_backend)
    config.daemon.hotkey_mode = normalize_hotkey_mode(config.daemon.hotkey_mode)
    return config


def _set_toml_value(text: str, section: str, key: str, value: Any) -> str:
    value_line = f"{key} = {_toml_value(value)}"
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


def _toml_value(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return str(value)
    return json.dumps(str(value), ensure_ascii=False)
