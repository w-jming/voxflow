from __future__ import annotations

from pathlib import Path
import os


APP_HOME_ENV = "VOXFLOW_HOME"
APP_HOME_POINTER_PATH = Path("~/.config/voxflow/home").expanduser()
DEFAULT_APP_HOME = Path("~/.voxflow").expanduser()


def app_home() -> Path:
    env_value = os.environ.get(APP_HOME_ENV)
    if env_value:
        return Path(env_value).expanduser()
    pointer = app_home_pointer_path()
    if pointer.exists():
        text = pointer.read_text(encoding="utf-8").strip()
        if text:
            return Path(text).expanduser()
    return DEFAULT_APP_HOME


def app_home_source() -> str:
    if os.environ.get(APP_HOME_ENV):
        return "env"
    if app_home_pointer_path().exists():
        return "pointer"
    return "default"


def app_home_pointer_path() -> Path:
    return Path(os.environ.get("VOXFLOW_HOME_POINTER", str(APP_HOME_POINTER_PATH))).expanduser()


def set_app_home(path: str | os.PathLike[str]) -> Path:
    target = Path(path).expanduser()
    if not str(target).strip():
        raise ValueError("数据目录不能为空")
    if not target.is_absolute():
        raise ValueError("数据目录必须使用绝对路径")
    target.mkdir(parents=True, exist_ok=True)
    for child in ("models", "logs", "run", "cache"):
        (target / child).mkdir(parents=True, exist_ok=True)
    pointer = app_home_pointer_path()
    pointer.parent.mkdir(parents=True, exist_ok=True)
    pointer.write_text(str(target) + "\n", encoding="utf-8")
    return target


def user_config_path() -> Path:
    return app_home() / "config.toml"


def legacy_user_config_path() -> Path:
    return Path("~/.config/voxflow/config.toml").expanduser()


def logs_dir() -> Path:
    return app_home() / "logs"


def run_dir() -> Path:
    return app_home() / "run"


def models_dir() -> Path:
    return app_home() / "models"


def cache_dir() -> Path:
    return app_home() / "cache"
