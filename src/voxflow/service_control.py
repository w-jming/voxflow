from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import os
import signal
import subprocess
import sys
import time

from .paths import logs_dir, run_dir


APP_ID = "voxflow"
DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 8765


@dataclass(frozen=True, slots=True)
class ProcessStatus:
    running: bool
    pid: int | None
    log_file: Path


def state_dir() -> Path:
    path = run_dir()
    path.mkdir(parents=True, exist_ok=True)
    return path


def log_dir() -> Path:
    path = logs_dir()
    path.mkdir(parents=True, exist_ok=True)
    return path


def daemon_pid_file() -> Path:
    return state_dir() / "daemon.pid"


def daemon_log_file() -> Path:
    return log_dir() / "daemon.log"


def gui_pid_file() -> Path:
    return state_dir() / "gui.pid"


def gui_log_file() -> Path:
    return log_dir() / "gui.log"


def python_executable() -> str:
    configured = os.environ.get("VOXFLOW_PYTHON")
    if configured:
        return configured
    installed = Path("/opt/voxflow/venv/bin/python")
    if installed.exists():
        return str(installed)
    return sys.executable


def daemon_status() -> ProcessStatus:
    pid = _read_pid(daemon_pid_file())
    if pid and _pid_running(pid):
        return ProcessStatus(True, pid, daemon_log_file())
    _remove_stale_pid(daemon_pid_file())
    existing_pid = _find_daemon_pid()
    if existing_pid:
        return ProcessStatus(True, existing_pid, daemon_log_file())
    return ProcessStatus(False, None, daemon_log_file())


def gui_status() -> ProcessStatus:
    pid = _read_pid(gui_pid_file())
    if pid and _pid_running(pid):
        return ProcessStatus(True, pid, gui_log_file())
    _remove_stale_pid(gui_pid_file())
    return ProcessStatus(False, None, gui_log_file())


def start_daemon() -> ProcessStatus:
    current = daemon_status()
    if current.running:
        return current
    pid = _start_python_module("voxflow", ["daemon"], daemon_log_file())
    daemon_pid_file().write_text(f"{pid}\n", encoding="utf-8")
    return ProcessStatus(True, pid, daemon_log_file())


def stop_daemon(timeout: float = 3.0) -> ProcessStatus:
    pid = _read_pid(daemon_pid_file()) or _find_daemon_pid()
    if not pid:
        return daemon_status()
    _terminate_pid(pid, timeout=timeout)
    _remove_stale_pid(daemon_pid_file())
    return daemon_status()


def restart_daemon() -> ProcessStatus:
    stop_daemon()
    return start_daemon()


def start_gui(host: str = DEFAULT_HOST, port: int = DEFAULT_PORT) -> ProcessStatus:
    current = gui_status()
    if current.running:
        return current
    pid = _start_python_module(
        "voxflow",
        ["gui", "--host", host, "--port", str(port)],
        gui_log_file(),
    )
    gui_pid_file().write_text(f"{pid}\n", encoding="utf-8")
    return ProcessStatus(True, pid, gui_log_file())


def stop_gui(timeout: float = 3.0) -> ProcessStatus:
    pid = _read_pid(gui_pid_file()) or _find_gui_pid()
    if not pid:
        return gui_status()
    _terminate_pid(pid, timeout=timeout)
    _remove_stale_pid(gui_pid_file())
    return gui_status()


def open_gui_url(host: str = DEFAULT_HOST, port: int = DEFAULT_PORT) -> None:
    url = f"http://{host}:{port}"
    if _command_exists("xdg-open"):
        subprocess.Popen(["xdg-open", url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        print(url)


def open_state_dir() -> None:
    path = log_dir()
    if _command_exists("xdg-open"):
        subprocess.Popen(["xdg-open", str(path)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        print(path)


def _start_python_module(module: str, args: list[str], log_file: Path) -> int:
    env = os.environ.copy()
    env["PYTHONNOUSERSITE"] = "1"
    env["PYTHONUNBUFFERED"] = "1"
    log_file.parent.mkdir(parents=True, exist_ok=True)
    log = log_file.open("ab")
    process = subprocess.Popen(
        [python_executable(), "-m", module, *args],
        stdout=log,
        stderr=subprocess.STDOUT,
        stdin=subprocess.DEVNULL,
        env=env,
        start_new_session=True,
    )
    log.close()
    time.sleep(0.2)
    return process.pid


def _read_pid(path: Path) -> int | None:
    try:
        value = path.read_text(encoding="utf-8").strip()
    except OSError:
        return None
    if not value:
        return None
    try:
        return int(value)
    except ValueError:
        return None


def _pid_running(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    return True


def _terminate_pid(pid: int, timeout: float) -> None:
    if not _pid_running(pid):
        return
    try:
        os.kill(pid, signal.SIGTERM)
    except OSError:
        return
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if not _pid_running(pid):
            return
        time.sleep(0.05)
    try:
        os.kill(pid, signal.SIGKILL)
    except OSError:
        return


def _remove_stale_pid(path: Path) -> None:
    try:
        path.unlink(missing_ok=True)
    except OSError:
        return


def _command_exists(name: str) -> bool:
    paths = os.environ.get("PATH", "").split(os.pathsep)
    return any((Path(path) / name).exists() for path in paths if path)


def _find_daemon_pid() -> int | None:
    if not _command_exists("pgrep"):
        return None
    patterns = (r"[v]oxflow daemon", r"[v]oxflow-daemon", r"[l]ocal_speak_input daemon", r"[l]ocal-speak-daemon")
    return _find_pid_by_patterns(patterns)


def _find_gui_pid() -> int | None:
    if not _command_exists("pgrep"):
        return None
    patterns = (r"[v]oxflow gui", r"[v]oxflow-gui", r"[l]ocal_speak_input gui", r"[l]ocal-speak-gui")
    return _find_pid_by_patterns(patterns)


def _find_pid_by_patterns(patterns: tuple[str, ...]) -> int | None:
    for pattern in patterns:
        result = subprocess.run(["pgrep", "-f", pattern], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        if result.returncode != 0:
            continue
        for line in result.stdout.splitlines():
            try:
                pid = int(line)
            except ValueError:
                continue
            if pid != os.getpid() and _pid_running(pid):
                return pid
    return None
