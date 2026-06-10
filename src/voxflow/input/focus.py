from __future__ import annotations

import os
import shutil
import subprocess


def get_active_window() -> str | None:
    if not os.getenv("DISPLAY") or not shutil.which("xdotool"):
        return None
    result = subprocess.run(
        ["xdotool", "getactivewindow"],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    window_id = result.stdout.strip()
    return window_id if result.returncode == 0 and window_id else None


def activate_window(window_id: str | None) -> bool:
    if not window_id or not os.getenv("DISPLAY") or not shutil.which("xdotool"):
        return False
    result = subprocess.run(
        ["xdotool", "windowactivate", "--sync", window_id],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return result.returncode == 0
