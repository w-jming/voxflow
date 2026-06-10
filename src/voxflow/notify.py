from __future__ import annotations

import shutil
import subprocess


class Notifier:
    def __init__(self, app_name: str = "声流输入法") -> None:
        self.app_name = app_name
        self.available = shutil.which("notify-send") is not None

    def send(self, title: str, body: str = "", urgency: str = "normal") -> None:
        if not self.available:
            return
        command = ["notify-send", "-a", self.app_name, "-u", urgency, title]
        if body:
            command.append(body)
        try:
            subprocess.run(command, check=False, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=2)
        except (OSError, subprocess.TimeoutExpired):
            return
