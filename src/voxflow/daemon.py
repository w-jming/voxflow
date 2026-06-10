from __future__ import annotations

import sys
import threading
import time

from .audio import NoSpeechDetected
from .config import AppConfig
from .hotkey import HotkeyEvent, X11HotkeyListener
from .input.focus import get_active_window
from .notify import Notifier
from .runtime import DictationRunner


class VoiceInputDaemon:
    def __init__(self, config: AppConfig, *, hotkey: str | None = None, dry_run: bool = False) -> None:
        self.config = config
        self.hotkey = hotkey or config.daemon.hotkey
        self.mode = config.daemon.hotkey_mode
        self.runner = DictationRunner(config, dry_run=dry_run)
        self.listener = X11HotkeyListener(self.hotkey)
        self.notifier = Notifier()
        self._state_lock = threading.Lock()
        self._recording_thread: threading.Thread | None = None
        self._stop_event: threading.Event | None = None
        self._hold_timer: threading.Timer | None = None
        self._started_at = 0.0

    def run(self) -> None:
        _log(f"后台语音输入已启动，快捷键：{self.hotkey}")
        if self.mode == "hold":
            _log("把光标放在目标输入框，按住快捷键开始录音，松开后识别并输入。")
            self.notifier.send("语音输入已就绪", f"按住 {self.hotkey} 录音")
        else:
            _log("把光标放在目标输入框，按一次快捷键开始录音，再按一次停止并输入。")
            self.notifier.send("语音输入已就绪", f"按 {self.hotkey} 开始/停止")
        try:
            self.listener.run_events(self.handle_hotkey_event)
        finally:
            self.listener.close()

    def handle_hotkey_event(self, event: HotkeyEvent) -> None:
        if self.mode == "hold":
            if event.kind == "press":
                self._hold_press()
            elif event.kind == "release":
                self._hold_release()
            return

        if event.kind == "press":
            self._toggle_press()

    def _toggle_press(self) -> None:
        with self._state_lock:
            if self._recording_thread and self._recording_thread.is_alive():
                if self._stop_event and not self._stop_event.is_set():
                    elapsed = time.monotonic() - self._started_at
                    if elapsed < self.config.daemon.toggle_debounce_s:
                        _log("忽略疑似按键重复事件。")
                        return
                    self._stop_recording_locked()
                    return
                _log("上一段语音仍在识别，忽略本次快捷键。")
                return

        target_window = get_active_window() if self.config.daemon.restore_focus else None
        self._start_recording(target_window)

    def _hold_press(self) -> None:
        with self._state_lock:
            if self._recording_thread and self._recording_thread.is_alive():
                return
            if self._hold_timer and self._hold_timer.is_alive():
                return

            target_window = get_active_window() if self.config.daemon.restore_focus else None
            self._hold_timer = threading.Timer(
                self.config.daemon.hold_min_s,
                self._start_recording,
                args=(target_window,),
            )
            self._hold_timer.daemon = True
            self._hold_timer.start()

    def _hold_release(self) -> None:
        with self._state_lock:
            if self._hold_timer and self._hold_timer.is_alive():
                self._hold_timer.cancel()
                self._hold_timer = None
                _log("按住时间过短，未开始录音。")
                return
            if self._stop_event and not self._stop_event.is_set():
                self._stop_recording_locked()

    def _start_recording(self, target_window: str | None) -> None:
        with self._state_lock:
            if self._recording_thread and self._recording_thread.is_alive():
                _log("上一段语音仍在处理，忽略本次快捷键。")
                return
            stop_event = threading.Event()
            self._stop_event = stop_event
            self._started_at = time.monotonic()
            self._recording_thread = threading.Thread(
                target=self._recording_worker,
                args=(stop_event, target_window),
                daemon=True,
            )
            self._recording_thread.start()

        _log("开始录音...")
        self.notifier.send("开始录音", "再次按快捷键停止" if self.mode == "toggle" else "松开快捷键停止")

    def _stop_recording_locked(self) -> None:
        if not self._stop_event:
            return
        self._stop_event.set()
        _log("停止录音，准备识别...")
        self.notifier.send("停止录音", "正在准备识别")

    def _recording_worker(self, stop_event: threading.Event, target_window: str | None) -> None:
        started = self._started_at
        try:
            result, actions = self.runner.run_until_stop(
                stop_event,
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
            with self._state_lock:
                if self._stop_event is stop_event:
                    self._stop_event = None
                if self._recording_thread is threading.current_thread():
                    self._recording_thread = None


def _log(message: str) -> None:
    print(f"[voxflow-daemon] {message}", file=sys.stderr, flush=True)
