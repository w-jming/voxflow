import hashlib
import json
from pathlib import Path

import pytest

import voxflow.model_registry as registry
from voxflow.model_registry import (
    ModelFileSpec,
    ModelValidationSpec,
    import_model_profile,
    list_model_profiles,
    model_cache_dir,
    package_default_profile,
    source_default_profile,
    validate_model_profile,
)
from voxflow.asr.factory import resolve_model_reference


def _write_fake_qwen_model(path: Path, *, payload: bytes = b"weights") -> str:
    path.mkdir()
    (path / "config.json").write_text(
        json.dumps(
            {
                "model_type": "qwen3_asr",
                "architectures": ["Qwen3ASRForConditionalGeneration"],
                "support_languages": ["Chinese", "English"],
            }
        ),
        encoding="utf-8",
    )
    (path / "model.safetensors").write_bytes(payload)
    return hashlib.sha256(payload).hexdigest()


def _install_fake_qwen_validation(monkeypatch, profile_id: str, digest: str, size: int = 7) -> None:
    monkeypatch.setitem(
        registry.MODEL_VALIDATION_SPECS,
        profile_id,
        ModelValidationSpec(
            revision="test-revision",
            required_files=("config.json",),
            weight_files=(ModelFileSpec("model.safetensors", size=size, sha256=digest),),
            config_model_type="qwen3_asr",
            config_architecture="Qwen3ASRForConditionalGeneration",
        ),
    )
    monkeypatch.setattr(registry, "_validate_safetensors_header", lambda path, warnings: None)


def test_source_default_is_lightweight_apache_model():
    profile = source_default_profile()

    assert profile.id == "bundled-faster-whisper-tiny"
    assert "Tiny" in profile.label
    assert profile.license == "MIT"
    assert profile.model.startswith("bundled:")


def test_package_default_is_lightweight_bundled_model():
    profile = package_default_profile()

    assert profile.id == "bundled-faster-whisper-tiny"
    assert profile.model == "bundled:faster-whisper-tiny"


def test_model_cache_dir_uses_voxflow_home(monkeypatch, tmp_path):
    monkeypatch.setenv("VOXFLOW_HOME", str(tmp_path / "custom-home"))

    assert model_cache_dir() == tmp_path / "custom-home" / "models"


def test_validate_model_profile_checks_qwen_hash_and_config(monkeypatch, tmp_path):
    source = tmp_path / "source"
    digest = _write_fake_qwen_model(source)
    _install_fake_qwen_validation(monkeypatch, "qwen3-asr-1.7b", digest)

    result = validate_model_profile("qwen3-asr-1.7b", source)

    assert result.path == source
    assert result.revision == "test-revision"
    assert "model.safetensors" in result.checked_files


def test_validate_model_profile_rejects_hash_mismatch(monkeypatch, tmp_path):
    source = tmp_path / "source"
    _write_fake_qwen_model(source)
    _install_fake_qwen_validation(monkeypatch, "qwen3-asr-1.7b", "0" * 64)

    with pytest.raises(ValueError, match="SHA256"):
        validate_model_profile("qwen3-asr-1.7b", source)


def test_import_model_profile_copies_existing_model_without_hf_cache(monkeypatch, tmp_path):
    source = tmp_path / "source"
    digest = _write_fake_qwen_model(source)
    _install_fake_qwen_validation(monkeypatch, "qwen3-asr-1.7b", digest)
    (source / ".cache").mkdir()
    (source / ".cache" / "ignored").write_text("x", encoding="utf-8")

    imported = import_model_profile("qwen3-asr-1.7b", source, tmp_path / "models")

    assert imported == tmp_path / "models" / "Qwen3-ASR-1.7B"
    assert (imported / "config.json").exists()
    assert not (imported / ".cache").exists()


def test_import_model_profile_can_symlink_existing_model(monkeypatch, tmp_path):
    source = tmp_path / "source"
    digest = _write_fake_qwen_model(source)
    _install_fake_qwen_validation(monkeypatch, "qwen3-asr-0.6b", digest)

    imported = import_model_profile("qwen3-asr-0.6b", source, tmp_path / "models", symlink=True)

    assert imported.is_symlink()
    assert imported.resolve() == source


def test_import_model_profile_reuses_existing_valid_cache(monkeypatch, tmp_path):
    source = tmp_path / "source"
    digest = _write_fake_qwen_model(source)
    _install_fake_qwen_validation(monkeypatch, "qwen3-asr-0.6b", digest)
    models = tmp_path / "models"
    existing = import_model_profile("qwen3-asr-0.6b", source, models, symlink=True)

    reused = import_model_profile("qwen3-asr-0.6b", source, models, symlink=True)

    assert reused == existing


def test_all_model_profiles_include_license_and_official_source():
    for profile in list_model_profiles():
        assert profile.license
        assert profile.license_url.startswith("https://")
        assert profile.source.startswith("https://")


def test_bundled_model_reference_resolves_to_committed_weights():
    path = resolve_model_reference("bundled:faster-whisper-tiny")

    assert path.endswith("faster-whisper-tiny")
    assert Path(path, "model.bin").exists()
    assert Path(path, "vocabulary.txt").exists()
    assert Path(path, "VOXFLOW_MODEL_LICENSE.md").exists()
