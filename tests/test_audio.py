import sys
import types

import pytest

from local_speak_input.audio import EnergyVadRecorder, NoSpeechDetected
from local_speak_input.config import AudioConfig


class SilentRawInputStream:
    def __init__(self, **_kwargs):
        pass

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False

    def read(self, frame_samples):
        return b"\0\0" * frame_samples, False


def test_energy_vad_times_out_when_no_speech(monkeypatch):
    fake_sounddevice = types.SimpleNamespace(RawInputStream=SilentRawInputStream)
    monkeypatch.setitem(sys.modules, "sounddevice", fake_sounddevice)

    recorder = EnergyVadRecorder(
        AudioConfig(
            frame_ms=10,
            sample_rate=16000,
            activation_timeout_s=0.02,
            energy_threshold=0.5,
        )
    )

    with pytest.raises(NoSpeechDetected):
        recorder.record_once()
