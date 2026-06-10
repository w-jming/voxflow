from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import queue
import sys
import threading
import time
import weakref

from .audio import EnergyVadRecorder, NoSpeechDetected
from .composition import InjectionLedger, actions_to_composition_commands
from .config import AppConfig, load_config
from .postprocess import DictationSession
from .asr import Recognizer, build_recognizer


ENGINE_NAME = "voxflow"
BUS_NAME = "org.freedesktop.IBus.VoxFlow"
ENGINE_PATH = "/org/freedesktop/IBus/Engine/VoxFlow"


@dataclass(frozen=True, slots=True)
class EngineEvent:
    kind: str
    value: str | int = ""


class StreamingDictationController:
    """Chunked dictation controller used by the IBus engine.

    The controller records short windows, shows a preedit placeholder while ASR
    runs, commits stable recognized text, and keeps a ledger so correction
    actions delete only VoxFlow-committed text.
    """

    def __init__(self, config: AppConfig, *, dry_run: bool = False) -> None:
        self.config = config
        self.recorder = EnergyVadRecorder(config.audio)
        self.recognizer: Recognizer | None = None
        self.session = DictationSession(
            remove_spoken_fillers=config.text.remove_fillers,
            auto_punctuation=config.text.auto_punctuation,
            script=config.text.script,
            semantic_correction_enabled=config.text.semantic_correction_enabled,
        )
        self.ledger = InjectionLedger()
        self.events: "queue.Queue[EngineEvent]" = queue.Queue()
        self._thread: threading.Thread | None = None
        self._stop_event = threading.Event()
        self.dry_run = dry_run

    @property
    def running(self) -> bool:
        return bool(self._thread and self._thread.is_alive())

    def start(self) -> None:
        if self.running:
            return
        self._stop_event.clear()
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._stop_event.set()

    def poll_events(self) -> list[EngineEvent]:
        events: list[EngineEvent] = []
        while True:
            try:
                events.append(self.events.get_nowait())
            except queue.Empty:
                return events

    def process_stable_text(self, text: str) -> list[EngineEvent]:
        actions = self.session.process(text)
        commands = actions_to_composition_commands(actions, self.ledger)
        events: list[EngineEvent] = []
        for command in commands:
            events.append(EngineEvent(command.kind, command.value))
        return events

    def _run(self) -> None:
        while not self._stop_event.is_set():
            self.events.put(EngineEvent("preedit", "正在听写..."))
            chunk_stop = threading.Event()
            chunk_timer = threading.Timer(max(0.5, min(2.0, self.config.audio.max_utterance_s)), chunk_stop.set)
            chunk_timer.daemon = True
            chunk_timer.start()
            audio_path: Path | None = None
            try:
                audio_path = self.recorder.record_until_stop(chunk_stop)
                self.events.put(EngineEvent("preedit", "正在识别..."))
                result = self.get_recognizer().transcribe(audio_path)
                if result.text:
                    self.events.put(EngineEvent("preedit", result.text.strip()))
                for event in self.process_stable_text(result.text):
                    self.events.put(event)
                self.events.put(EngineEvent("preedit", ""))
            except NoSpeechDetected:
                self.events.put(EngineEvent("preedit", ""))
            except Exception as exc:
                self.events.put(EngineEvent("preedit", f"VoxFlow 错误：{exc}"))
                time.sleep(1.0)
            finally:
                chunk_timer.cancel()
                if audio_path:
                    audio_path.unlink(missing_ok=True)

    def get_recognizer(self) -> Recognizer:
        if self.recognizer is None:
            self.recognizer = build_recognizer(self.config.asr)
        return self.recognizer


class DryRunIBusEngine:
    def __init__(self, config: AppConfig) -> None:
        self.controller = StreamingDictationController(config, dry_run=True)

    def commit_text(self, text: str) -> None:
        print(f"COMMIT {text}")

    def delete_surrounding_text(self, offset: int, nchars: int) -> None:
        print(f"DELETE offset={offset} nchars={nchars}")

    def update_preedit_text(self, text: str) -> None:
        print(f"PREEDIT {text}")

    def apply_event(self, event: EngineEvent) -> None:
        if event.kind == "commit":
            self.commit_text(str(event.value))
        elif event.kind == "delete_before_cursor":
            self.delete_surrounding_text(-int(event.value), int(event.value))
        elif event.kind == "preedit":
            self.update_preedit_text(str(event.value))


def run_ibus_engine(config: AppConfig, *, dry_run: bool = False) -> int:
    if dry_run:
        engine = DryRunIBusEngine(config)
        print("VoxFlow IBus dry-run engine is available.")
        for event in engine.controller.process_stable_text("這是語音輸入測試"):
            engine.apply_event(event)
        return 0

    try:
        import gi

        gi.require_version("IBus", "1.0")
        from gi.repository import GLib, IBus
    except Exception as exc:
        print(f"无法启动 IBus 引擎：缺少 IBus GI 运行时：{exc}", file=sys.stderr)
        return 1

    IBus.init()

    engine_instances: "weakref.WeakSet[object]" = weakref.WeakSet()

    class VoxFlowIBusEngine(IBus.Engine):
        def __init__(self, **kwargs: object) -> None:
            super().__init__(**kwargs)
            self.controller = StreamingDictationController(config)
            engine_instances.add(self)

        def do_enable(self) -> None:  # type: ignore[override]
            self.controller.start()

        def do_disable(self) -> None:  # type: ignore[override]
            self.controller.stop()
            self.hide_preedit_text()

        def do_focus_in(self) -> None:  # type: ignore[override]
            self.controller.start()

        def do_focus_out(self) -> None:  # type: ignore[override]
            self.controller.stop()
            self.hide_preedit_text()

        def do_process_key_event(self, keyval: int, keycode: int, state: int) -> bool:  # type: ignore[override]
            return False

        def flush_events(self) -> bool:
            for event in self.controller.poll_events():
                if event.kind == "commit":
                    self.commit_text(IBus.Text.new_from_string(str(event.value)))
                elif event.kind == "delete_before_cursor":
                    self.delete_surrounding_text(-int(event.value), int(event.value))
                elif event.kind == "preedit":
                    text = str(event.value)
                    if text:
                        self.update_preedit_text(IBus.Text.new_from_string(text), len(text), True)
                    else:
                        self.hide_preedit_text()
            return True

    bus = IBus.Bus()
    connection = bus.get_connection()
    if connection is None:
        print("无法连接 IBus daemon。请确认当前桌面会话已启动 IBus。", file=sys.stderr)
        return 1
    factory = IBus.Factory.new(connection)
    factory.add_engine(ENGINE_NAME, VoxFlowIBusEngine)
    bus.request_name(BUS_NAME, 0)
    loop = GLib.MainLoop()

    def flush_all_engines() -> bool:
        for engine in list(engine_instances):
            engine.flush_events()
        return True

    GLib.timeout_add(50, flush_all_engines)
    print("VoxFlow IBus engine started.", file=sys.stderr, flush=True)
    try:
        loop.run()
    except KeyboardInterrupt:
        return 0
    return 0


def write_component_xml(path: Path, *, exec_path: str = "/usr/bin/voxflow-ibus-engine") -> None:
    path.write_text(
        f"""<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<component>
  <name>org.freedesktop.IBus.VoxFlow</name>
  <description>VoxFlow voice input method</description>
  <exec>{exec_path}</exec>
  <version>0.2.0</version>
  <author>Jiaming Wang</author>
  <license>MIT</license>
  <homepage>https://github.com/w-jming/voxflow</homepage>
  <textdomain>voxflow</textdomain>
  <engines>
    <engine>
      <name>{ENGINE_NAME}</name>
      <language>zh</language>
      <license>MIT</license>
      <author>Jiaming Wang</author>
      <icon>/usr/share/icons/hicolor/scalable/apps/voxflow.svg</icon>
      <layout>us</layout>
      <longname>VoxFlow Input</longname>
      <description>Voice input method with live composition</description>
      <rank>80</rank>
    </engine>
  </engines>
</component>
""",
        encoding="utf-8",
    )


def main(argv: list[str] | None = None) -> int:
    import argparse

    parser = argparse.ArgumentParser(prog="voxflow-ibus-engine")
    parser.add_argument("--config")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--write-component", type=Path)
    args = parser.parse_args(argv)

    if args.write_component:
        write_component_xml(args.write_component)
        return 0
    return run_ibus_engine(load_config(args.config), dry_run=args.dry_run)


if __name__ == "__main__":
    raise SystemExit(main())
