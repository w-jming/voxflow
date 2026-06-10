from __future__ import annotations

import sys
import threading
import time

from .audio import NoSpeechDetected
from .config import AppConfig
from .hotkey import X11HotkeyListener
from .input.focus import get_active_window
from .notify import Notifier
from .runtime import DictationRunner


class VoiceInputDaemon:
    def __init__(self, config: AppConfig, *, hotkey: str | None = None, dry_run: bool = False) -> None:
        self.config = config
        self.hotkey = hotkey or config.daemon.hotkey
        self.runner = DictationRunner(config, dry_run=dry_run)
        self.listener = X11HotkeyListener(self.hotkey)
        self.notifier = Notifier()
        self._busy = threading.Lock()

    def run(self) -> None:
        _log(f"后台语音输入已启动，快捷键：{self.hotkey}")
        _log("把光标放在目标输入框，按快捷键开始一次语音输入。")
        self.notifier.send("语音输入已就绪", f"快捷键：{self.hotkey}")
        try:
            self.listener.run(self.trigger_once)
        finally:
            self.listener.close()

    def trigger_once(self) -> None:
        if not self._busy.acquire(blocking=False):
            _log("上一段语音仍在处理，忽略本次快捷键。")
            return
        try:
            target_window = get_active_window() if self.config.daemon.restore_focus else None
            started = time.monotonic()
            _log("开始录音...")
            self.notifier.send("开始录音", "请说话")
            result, actions = self.runner.run_once(
                target_window=target_window,
                on_recorded=lambda _path: self.notifier.send("正在识别", "语音已录制，正在转换为文本"),
            )
            elapsed = time.monotonic() - started
            inserted = "".join(action.insert for action in actions if action.insert)
            if inserted:
                _log(f"已输入：{inserted} ({elapsed:.1f}s)")
                self.notifier.send("已输入", inserted)
            elif result.text:
                _log(f"识别到文本但后处理为空：{result.text} ({elapsed:.1f}s)")
                self.notifier.send("识别完成", "识别到文本，但没有可输入内容")
            else:
                _log(f"没有识别到有效语音。({elapsed:.1f}s)")
                self.notifier.send("没有识别到有效语音", "请确认麦克风输入正常", urgency="normal")
        except NoSpeechDetected as exc:
            _log(str(exc))
            self.notifier.send("没有检测到语音", str(exc), urgency="normal")
        except Exception as exc:
            _log(f"语音输入失败：{exc}")
            self.notifier.send("语音输入失败", str(exc), urgency="critical")
        finally:
            self._busy.release()


def _log(message: str) -> None:
    print(f"[local-speak-daemon] {message}", file=sys.stderr, flush=True)
