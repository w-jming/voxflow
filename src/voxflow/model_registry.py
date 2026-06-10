from __future__ import annotations

from dataclasses import asdict, dataclass
from pathlib import Path


@dataclass(frozen=True, slots=True)
class ModelProfile:
    id: str
    label: str
    backend: str
    model: str
    source: str
    license: str
    license_url: str
    size: str
    languages: str
    package_default: bool = False
    source_default: bool = False

    def to_dict(self) -> dict[str, str | bool]:
        return asdict(self)


MODEL_PROFILES = {
    "bundled-faster-whisper-tiny": ModelProfile(
        id="bundled-faster-whisper-tiny",
        label="内置 Whisper Tiny 轻量模型",
        backend="faster-whisper",
        model="bundled:faster-whisper-tiny",
        source="https://huggingface.co/Systran/faster-whisper-tiny",
        license="MIT",
        license_url="https://huggingface.co/Systran/faster-whisper-tiny",
        size="39M parameters",
        languages="99 languages, including Chinese and English",
        source_default=True,
    ),
    "qwen3-asr-0.6b": ModelProfile(
        id="qwen3-asr-0.6b",
        label="Qwen3-ASR 0.6B 轻量高精度",
        backend="qwen",
        model="Qwen/Qwen3-ASR-0.6B",
        source="https://huggingface.co/Qwen/Qwen3-ASR-0.6B",
        license="Apache-2.0",
        license_url="https://huggingface.co/Qwen/Qwen3-ASR-0.6B",
        size="0.6B",
        languages="52 languages and dialects, Chinese/English prioritized",
    ),
    "qwen3-asr-1.7b": ModelProfile(
        id="qwen3-asr-1.7b",
        label="Qwen3-ASR 1.7B 高准确率",
        backend="qwen",
        model="Qwen/Qwen3-ASR-1.7B",
        source="https://huggingface.co/Qwen/Qwen3-ASR-1.7B",
        license="Apache-2.0",
        license_url="https://huggingface.co/Qwen/Qwen3-ASR-1.7B",
        size="1.7B",
        languages="52 languages and dialects, Chinese/English prioritized",
        package_default=True,
    ),
}


def list_model_profiles() -> list[ModelProfile]:
    return list(MODEL_PROFILES.values())


def get_model_profile(profile_id: str) -> ModelProfile:
    try:
        return MODEL_PROFILES[profile_id]
    except KeyError as exc:
        raise ValueError(f"未知模型档位：{profile_id}") from exc


def source_default_profile() -> ModelProfile:
    return next(profile for profile in MODEL_PROFILES.values() if profile.source_default)


def package_default_profile() -> ModelProfile:
    return next(profile for profile in MODEL_PROFILES.values() if profile.package_default)


def model_cache_dir() -> Path:
    return Path("~/.local/share/voxflow/models").expanduser()


def download_model_profile(profile_id: str, target_dir: Path | None = None) -> Path:
    profile = get_model_profile(profile_id)
    if profile.model.startswith("bundled:"):
        from .asr.factory import resolve_model_reference

        return Path(resolve_model_reference(profile.model))

    root = target_dir or model_cache_dir()
    root.mkdir(parents=True, exist_ok=True)
    local_dir = root / profile.model.split("/")[-1]
    try:
        from huggingface_hub import snapshot_download
    except ImportError as exc:
        raise RuntimeError("缺少 huggingface_hub。完整 deb 包会内置该依赖。") from exc
    snapshot_download(
        repo_id=profile.model,
        local_dir=str(local_dir),
        local_dir_use_symlinks=False,
    )
    return local_dir
