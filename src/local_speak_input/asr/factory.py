from __future__ import annotations

from local_speak_input.config import ASRConfig

from .base import Recognizer


def build_recognizer(config: ASRConfig) -> Recognizer:
    backend = config.backend.strip().lower().replace("_", "-")

    if backend in {"faster-whisper", "whisper"}:
        from .faster_whisper_backend import FasterWhisperRecognizer

        return FasterWhisperRecognizer(
            model=config.model,
            device=config.device,
            compute_type=config.compute_type,
            language=config.language,
        )

    if backend in {"qwen", "qwen-asr", "qwen3-asr"}:
        from .qwen_backend import QwenRecognizer

        return QwenRecognizer(
            model=config.model if config.model != "large-v3" else "Qwen/Qwen3-ASR-0.6B",
            device=config.device,
            language=config.language,
        )

    if backend in {"openai-compatible", "openai", "api", "remote"}:
        from .openai_compatible import OpenAICompatibleRecognizer

        return OpenAICompatibleRecognizer(
            base_url=config.api_base,
            model=config.api_model,
            api_key_env=config.api_key_env,
            language=config.language,
            timeout=config.timeout,
        )

    raise ValueError(f"未知 ASR 后端：{config.backend}")
