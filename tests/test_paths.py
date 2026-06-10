from voxflow import service_control
from voxflow.paths import (
    app_home,
    app_home_pointer_path,
    app_home_source,
    cache_dir,
    logs_dir,
    models_dir,
    run_dir,
    set_app_home,
    user_config_path,
)


def test_voxflow_home_controls_user_data_paths(monkeypatch, tmp_path):
    home = tmp_path / "vf"
    monkeypatch.setenv("VOXFLOW_HOME", str(home))

    assert app_home() == home
    assert user_config_path() == home / "config.toml"
    assert models_dir() == home / "models"
    assert logs_dir() == home / "logs"
    assert run_dir() == home / "run"
    assert cache_dir() == home / "cache"


def test_service_control_uses_voxflow_home_for_logs_and_pids(monkeypatch, tmp_path):
    home = tmp_path / "vf"
    monkeypatch.setenv("VOXFLOW_HOME", str(home))

    assert service_control.daemon_pid_file() == home / "run" / "daemon.pid"
    assert service_control.daemon_log_file() == home / "logs" / "daemon.log"


def test_app_home_can_be_persisted_with_pointer(monkeypatch, tmp_path):
    pointer = tmp_path / "config" / "home"
    home = tmp_path / "custom-vf"
    monkeypatch.delenv("VOXFLOW_HOME", raising=False)
    monkeypatch.setenv("VOXFLOW_HOME_POINTER", str(pointer))

    saved = set_app_home(str(home))

    assert saved == home
    assert app_home() == home
    assert app_home_source() == "pointer"
    assert app_home_pointer_path() == pointer
    assert (home / "models").is_dir()
    assert (home / "logs").is_dir()
