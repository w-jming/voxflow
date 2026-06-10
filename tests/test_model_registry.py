from pathlib import Path

from voxflow.model_registry import list_model_profiles, package_default_profile, source_default_profile
from voxflow.asr.factory import resolve_model_reference


def test_source_default_is_lightweight_apache_model():
    profile = source_default_profile()

    assert profile.id == "bundled-faster-whisper-tiny"
    assert "Tiny" in profile.label
    assert profile.license == "MIT"
    assert profile.model.startswith("bundled:")


def test_package_default_is_high_accuracy_qwen_model():
    profile = package_default_profile()

    assert profile.id == "qwen3-asr-1.7b"
    assert profile.license == "Apache-2.0"


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
