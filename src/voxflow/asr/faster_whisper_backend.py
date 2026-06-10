from __future__ import annotations

from pathlib import Path

from .base import TranscriptionResult


class FasterWhisperRecognizer:
    def __init__(
        self,
        model: str = "large-v3",
        device: str = "auto",
        compute_type: str = "auto",
        language: str = "auto",
    ) -> None:
        try:
            from faster_whisper import WhisperModel
        except ImportError as exc:
            raise RuntimeError(
                "缺少 faster-whisper。请运行：pip install -e '.[whisper]'"
            ) from exc

        resolved_device = _resolve_device(device)
        resolved_compute_type = _resolve_compute_type(compute_type, resolved_device)

        self.language = None if language in {"", "auto", None} else language
        self.model = WhisperModel(
            model,
            device=resolved_device,
            compute_type=resolved_compute_type,
        )

    def transcribe(self, audio_path: str | Path) -> TranscriptionResult:
        segments, info = self.model.transcribe(
            str(audio_path),
            language=self.language,
            beam_size=5,
            vad_filter=True,
            condition_on_previous_text=True,
        )
        text = "".join(segment.text for segment in segments).strip()
        return TranscriptionResult(
            text=text,
            language=getattr(info, "language", None),
            duration=getattr(info, "duration", None),
            raw=info,
        )


def _resolve_device(device: str) -> str:
    if device != "auto":
        return device
    try:
        import ctranslate2

        if ctranslate2.get_cuda_device_count() > 0:
            return "cuda"
    except Exception:
        pass
    return "cpu"


def _resolve_compute_type(compute_type: str, device: str) -> str:
    if compute_type != "auto":
        return compute_type
    return "float16" if device == "cuda" else "int8"
