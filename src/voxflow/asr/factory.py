from __future__ import annotations

from importlib import resources
from pathlib import Path

from voxflow.config import ASRConfig

from .base import Recognizer


def build_recognizer(config: ASRConfig) -> Recognizer:
    backend = config.backend.strip().lower().replace("_", "-")

    if backend in {"faster-whisper", "whisper"}:
        from .faster_whisper_backend import FasterWhisperRecognizer

        return FasterWhisperRecognizer(
            model=resolve_model_reference(config.model),
            device=config.device,
            compute_type=config.compute_type,
            language=config.language,
        )

    if backend in {"qwen", "qwen-asr", "qwen3-asr"}:
        from .qwen_backend import QwenRecognizer

        return QwenRecognizer(
            model=resolve_model_reference(config.model if config.model != "large-v3" else "Qwen/Qwen3-ASR-0.6B"),
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


def resolve_model_reference(model: str) -> str:
    if not model.startswith("bundled:"):
        return model
    name = model.split(":", 1)[1]
    if name != "faster-whisper-tiny":
        raise ValueError(f"未知内置模型：{model}")

    package_path = resources.files("voxflow").joinpath("bundled", "faster-whisper-tiny")
    if package_path.is_dir():
        return str(package_path)

    candidates = [
        Path("/opt/voxflow/bundled/faster-whisper-tiny"),
        Path(__file__).resolve().parents[3] / "src" / "voxflow" / "bundled" / "faster-whisper-tiny",
    ]
    for candidate in candidates:
        if candidate.is_dir():
            return str(candidate)
    raise RuntimeError("找不到内置 faster-whisper-tiny 模型。请重新安装完整 VoxFlow 包。")
