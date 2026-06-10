from __future__ import annotations

from dataclasses import asdict, dataclass
import hashlib
import json
from pathlib import Path
import shutil

from .paths import models_dir


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


@dataclass(frozen=True, slots=True)
class ModelFileSpec:
    path: str
    size: int | None = None
    sha256: str | None = None


@dataclass(frozen=True, slots=True)
class ModelValidationSpec:
    revision: str
    required_files: tuple[str, ...]
    weight_files: tuple[ModelFileSpec, ...]
    config_model_type: str
    config_architecture: str


@dataclass(frozen=True, slots=True)
class ModelValidationResult:
    path: Path
    revision: str
    checked_files: tuple[str, ...]
    warnings: tuple[str, ...] = ()


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
        package_default=True,
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
    ),
}


MODEL_VALIDATION_SPECS = {
    "qwen3-asr-0.6b": ModelValidationSpec(
        revision="5eb144179a02acc5e5ba31e748d22b0cf3e303b0",
        required_files=(
            "chat_template.json",
            "config.json",
            "generation_config.json",
            "merges.txt",
            "preprocessor_config.json",
            "tokenizer_config.json",
            "vocab.json",
        ),
        weight_files=(
            ModelFileSpec(
                "model.safetensors",
                size=1_876_091_704,
                sha256="79d6cbd4c98c7bbffe9db2edac07f56cd6637d0d5944b27f6c2b8353840323ea",
            ),
        ),
        config_model_type="qwen3_asr",
        config_architecture="Qwen3ASRForConditionalGeneration",
    ),
    "qwen3-asr-1.7b": ModelValidationSpec(
        revision="7278e1e70fe206f11671096ffdd38061171dd6e5",
        required_files=(
            "chat_template.json",
            "config.json",
            "generation_config.json",
            "merges.txt",
            "model.safetensors.index.json",
            "preprocessor_config.json",
            "tokenizer_config.json",
            "vocab.json",
        ),
        weight_files=(
            ModelFileSpec(
                "model-00001-of-00002.safetensors",
                size=4_220_320_824,
                sha256="a4cd1f1a04d90b757dc7f7dd26254e69a013b19e80efe590a83c6a3bde8608d6",
            ),
            ModelFileSpec(
                "model-00002-of-00002.safetensors",
                size=478_200_688,
                sha256="6e0b9d9e09e2e0238e7ef3cc8a484ab387e91b90f1900bedf88bc92d7929ccfc",
            ),
        ),
        config_model_type="qwen3_asr",
        config_architecture="Qwen3ASRForConditionalGeneration",
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
    return models_dir()


def model_local_dir(profile_id: str, target_dir: Path | None = None) -> Path:
    profile = get_model_profile(profile_id)
    if profile.model.startswith("bundled:"):
        from .asr.factory import resolve_model_reference

        return Path(resolve_model_reference(profile.model))
    return (target_dir or model_cache_dir()) / profile.model.split("/")[-1]


def model_expected_bytes(profile_id: str) -> int:
    spec = MODEL_VALIDATION_SPECS.get(profile_id)
    if spec is None:
        return 0
    return sum(file_spec.size or 0 for file_spec in spec.weight_files)


def model_downloaded_bytes(profile_id: str, target_dir: Path | None = None) -> int:
    spec = MODEL_VALIDATION_SPECS.get(profile_id)
    if spec is None:
        return 0
    local_dir = model_local_dir(profile_id, target_dir)
    completed = 0
    missing_weight = False
    for file_spec in spec.weight_files:
        file_path = local_dir / file_spec.path
        if file_path.exists():
            size = file_path.stat().st_size
            completed += min(size, file_spec.size or size)
        else:
            missing_weight = True
    if not missing_weight:
        return min(completed, model_expected_bytes(profile_id))

    partial = 0
    cache_dir = local_dir / ".cache" / "huggingface" / "download"
    if cache_dir.exists():
        for path in cache_dir.glob("*.incomplete"):
            if path.is_file():
                partial += path.stat().st_size
    total = model_expected_bytes(profile_id)
    return min(completed + partial, total) if total else completed + partial


def download_model_profile(profile_id: str, target_dir: Path | None = None) -> Path:
    profile = get_model_profile(profile_id)
    if profile.model.startswith("bundled:"):
        from .asr.factory import resolve_model_reference

        return Path(resolve_model_reference(profile.model))

    root = target_dir or model_cache_dir()
    root.mkdir(parents=True, exist_ok=True)
    local_dir = model_local_dir(profile_id, root)
    try:
        from huggingface_hub import snapshot_download
    except ImportError as exc:
        raise RuntimeError("缺少 huggingface_hub。完整 deb 包会内置该依赖。") from exc
    snapshot_download(
        repo_id=profile.model,
        local_dir=str(local_dir),
        local_dir_use_symlinks=False,
    )
    validate_model_profile(profile.id, local_dir)
    return local_dir


def validate_model_profile(
    profile_id: str,
    model_path: Path,
    *,
    verify_hashes: bool = True,
) -> ModelValidationResult:
    profile = get_model_profile(profile_id)
    path = model_path.expanduser().resolve()
    if profile.model.startswith("bundled:"):
        from .asr.factory import resolve_model_reference

        bundled_path = Path(resolve_model_reference(profile.model))
        if path != bundled_path.resolve():
            raise ValueError(f"内置模型路径不匹配：{path}")
        return ModelValidationResult(path=path, revision="bundled", checked_files=("model.bin",))

    spec = MODEL_VALIDATION_SPECS.get(profile.id)
    if spec is None:
        raise ValueError(f"模型档位缺少校验清单：{profile.id}")
    if not path.exists():
        raise FileNotFoundError(f"模型路径不存在：{path}")
    if not path.is_dir():
        raise ValueError(f"模型路径必须是目录：{path}")

    checked_files: list[str] = []
    warnings: list[str] = []
    for relative in spec.required_files:
        _require_regular_file(path / relative)
        checked_files.append(relative)

    config = _read_json(path / "config.json")
    if config.get("model_type") != spec.config_model_type:
        raise ValueError(f"config.json model_type 不匹配：期望 {spec.config_model_type}")
    architectures = config.get("architectures") or []
    if spec.config_architecture not in architectures:
        raise ValueError(f"config.json architectures 不包含 {spec.config_architecture}")
    support_languages = set(config.get("support_languages") or [])
    if support_languages and not {"Chinese", "English"}.issubset(support_languages):
        raise ValueError("config.json support_languages 缺少 Chinese/English")

    if (path / "model.safetensors.index.json").exists():
        checked_files.extend(_validate_safetensors_index(path / "model.safetensors.index.json", path))

    for file_spec in spec.weight_files:
        file_path = path / file_spec.path
        _require_regular_file(file_path)
        checked_files.append(file_spec.path)
        if file_spec.size is not None and file_path.stat().st_size != file_spec.size:
            raise ValueError(
                f"{file_spec.path} 文件大小不匹配："
                f"期望 {file_spec.size}，实际 {file_path.stat().st_size}"
            )
        _validate_safetensors_header(file_path, warnings)
        if verify_hashes and file_spec.sha256:
            digest = _sha256_file(file_path)
            if digest != file_spec.sha256:
                raise ValueError(
                    f"{file_spec.path} SHA256 不匹配："
                    f"期望 {file_spec.sha256}，实际 {digest}"
                )

    return ModelValidationResult(
        path=path,
        revision=spec.revision,
        checked_files=tuple(dict.fromkeys(checked_files)),
        warnings=tuple(warnings),
    )


def import_model_profile(
    profile_id: str,
    source_path: Path,
    target_dir: Path | None = None,
    *,
    symlink: bool = False,
) -> Path:
    profile = get_model_profile(profile_id)
    if profile.model.startswith("bundled:"):
        raise ValueError("内置模型不需要导入。")

    source = source_path.expanduser().resolve()
    validate_model_profile(profile.id, source, verify_hashes=False)

    root = target_dir or model_cache_dir()
    root.mkdir(parents=True, exist_ok=True)
    destination = root / profile.model.split("/")[-1]
    if destination.exists() or destination.is_symlink():
        validate_model_profile(profile.id, destination, verify_hashes=True)
        return destination

    if symlink:
        destination.symlink_to(source, target_is_directory=True)
    else:
        shutil.copytree(source, destination, ignore=shutil.ignore_patterns(".cache"))
    validate_model_profile(profile.id, destination, verify_hashes=True)
    return destination


def _require_regular_file(path: Path) -> None:
    if not path.exists():
        raise FileNotFoundError(f"缺少模型文件：{path.name}")
    if not path.is_file():
        raise ValueError(f"模型文件不是普通文件：{path}")


def _read_json(path: Path) -> dict:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ValueError(f"JSON 文件格式错误：{path.name}") from exc
    if not isinstance(data, dict):
        raise ValueError(f"JSON 文件顶层必须是对象：{path.name}")
    return data


def _validate_safetensors_index(index_path: Path, model_path: Path) -> tuple[str, ...]:
    index = _read_json(index_path)
    weight_map = index.get("weight_map")
    if not isinstance(weight_map, dict) or not weight_map:
        raise ValueError("model.safetensors.index.json 缺少 weight_map")
    referenced_files = sorted({value for value in weight_map.values() if isinstance(value, str)})
    if not referenced_files:
        raise ValueError("model.safetensors.index.json 没有引用任何权重分片")
    for relative in referenced_files:
        _require_regular_file(model_path / relative)
    return tuple(referenced_files)


def _validate_safetensors_header(path: Path, warnings: list[str]) -> None:
    try:
        from safetensors import safe_open
    except ImportError:
        warnings.append("未安装 safetensors，已跳过 safetensors 头部可读性检查")
        return
    try:
        with safe_open(str(path), framework="np", device="cpu") as handle:
            keys = handle.keys()
            if not keys:
                raise ValueError(f"{path.name} 没有任何 tensor")
    except Exception as exc:
        raise ValueError(f"{path.name} safetensors 头部不可读：{exc}") from exc


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(16 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()
