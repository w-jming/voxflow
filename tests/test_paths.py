from voxflow import service_control
from voxflow.paths import app_home, cache_dir, logs_dir, models_dir, run_dir, user_config_path


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
