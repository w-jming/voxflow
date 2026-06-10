from __future__ import annotations

from pathlib import Path
from collections.abc import Callable

from .asr import TranscriptionResult, build_recognizer
from .audio import EnergyVadRecorder, sleep_before_next_utterance
from .config import AppConfig
from .input import apply_actions, build_injector
from .input.focus import activate_window
from .postprocess import DictationSession, EditAction


def process_audio_file(
    audio_path: str | Path,
    config: AppConfig,
    *,
    inject: bool = False,
    dry_run: bool = False,
) -> tuple[TranscriptionResult, list[EditAction]]:
    recognizer = build_recognizer(config.asr)
    session = DictationSession(
        remove_spoken_fillers=config.text.remove_fillers,
        auto_punctuation=config.text.auto_punctuation,
    )
    result = recognizer.transcribe(audio_path)
    actions = session.process(result.text)
    if inject:
        injector = build_injector(config.input, force_dry_run=dry_run)
        apply_actions(injector, actions)
    return result, actions


class DictationRunner:
    def __init__(self, config: AppConfig, *, dry_run: bool = False) -> None:
        self.config = config
        self.recognizer = build_recognizer(config.asr)
        self.session = DictationSession(
            remove_spoken_fillers=config.text.remove_fillers,
            auto_punctuation=config.text.auto_punctuation,
        )
        self.injector = build_injector(config.input, force_dry_run=dry_run)
        self.recorder = EnergyVadRecorder(config.audio)

    def run_once(
        self,
        target_window: str | None = None,
        on_recorded: Callable[[Path], None] | None = None,
    ) -> tuple[TranscriptionResult, list[EditAction]]:
        audio_path = self.recorder.record_once()
        try:
            if on_recorded:
                on_recorded(Path(audio_path))
            result = self.recognizer.transcribe(audio_path)
            actions = self.session.process(result.text)
            if target_window:
                activate_window(target_window)
            apply_actions(self.injector, actions)
            return result, actions
        finally:
            Path(audio_path).unlink(missing_ok=True)

    def run_forever(self) -> None:
        while True:
            self.run_once()
            sleep_before_next_utterance()
