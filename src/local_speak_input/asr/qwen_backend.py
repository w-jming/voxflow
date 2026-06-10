from __future__ import annotations

from pathlib import Path

from .base import TranscriptionResult


class QwenRecognizer:
    def __init__(
        self,
        model: str = "Qwen/Qwen3-ASR-0.6B",
        device: str = "auto",
        language: str = "auto",
    ) -> None:
        try:
            import torch
            from qwen_asr import Qwen3ASRModel
        except ImportError as exc:
            raise RuntimeError("缺少 qwen-asr。请运行：pip install -e '.[qwen]'") from exc

        dtype = torch.bfloat16 if device in {"auto", "cuda"} else torch.float32
        device_map = "cuda:0" if device in {"auto", "cuda"} and torch.cuda.is_available() else "cpu"
        self.language = _qwen_language(language)
        self.model = Qwen3ASRModel.from_pretrained(
            model,
            dtype=dtype,
            device_map=device_map,
            max_new_tokens=512,
        )

    def transcribe(self, audio_path: str | Path) -> TranscriptionResult:
        results = self.model.transcribe(
            audio=str(audio_path),
            language=self.language,
        )
        result = results[0] if isinstance(results, list) else results
        return TranscriptionResult(
            text=str(getattr(result, "text", "")).strip(),
            language=getattr(result, "language", None),
            raw=result,
        )


def _qwen_language(language: str) -> str | None:
    normalized = (language or "auto").lower()
    if normalized in {"auto", ""}:
        return None
    if normalized in {"zh", "cn", "zh-cn", "chinese", "中文"}:
        return "Chinese"
    if normalized in {"en", "english", "英文"}:
        return "English"
    return language
