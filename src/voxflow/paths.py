from __future__ import annotations

from pathlib import Path
import os


APP_HOME_ENV = "VOXFLOW_HOME"


def app_home() -> Path:
    return Path(os.environ.get(APP_HOME_ENV, "~/.voxflow")).expanduser()


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
