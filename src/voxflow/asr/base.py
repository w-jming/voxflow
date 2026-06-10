from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol


@dataclass(slots=True)
class TranscriptionResult:
    text: str
    language: str | None = None
    duration: float | None = None
    confidence: float | None = None
    raw: Any | None = None


class Recognizer(Protocol):
    def transcribe(self, audio_path: str | Path) -> TranscriptionResult:
        """Transcribe an audio file into text."""
