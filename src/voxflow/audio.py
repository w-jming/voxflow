from __future__ import annotations

from collections import deque
from pathlib import Path
import audioop
import shutil
import subprocess
import tempfile
import threading
import time
import wave

from .config import AudioConfig


class NoSpeechDetected(RuntimeError):
    pass


class EnergyVadRecorder:
    def __init__(self, config: AudioConfig) -> None:
        self.config = config

    def record_once(self) -> Path:
        try:
            import sounddevice as sd
        except (ImportError, OSError) as exc:
            if shutil.which("pw-record"):
                return PipeWireRecorder(self.config).record_once()
            raise RuntimeError("缺少 sounddevice 或 PortAudio。请运行：pip install -e '.[mic]' 并安装 libportaudio2") from exc

        frame_samples = int(self.config.sample_rate * self.config.frame_ms / 1000)
        silence_frames_limit = max(1, self.config.silence_ms // self.config.frame_ms)
        min_voice_frames = max(1, self.config.min_voice_ms // self.config.frame_ms)
        max_frames = int(self.config.max_utterance_s * 1000 / self.config.frame_ms)
        pre_roll = deque(maxlen=max(1, 180 // self.config.frame_ms))

        frames: list[bytes] = []
        started = False
        voiced_frames = 0
        silence_frames = 0
        listen_started_at = time.monotonic()

        with sd.RawInputStream(
            samplerate=self.config.sample_rate,
            channels=1,
            dtype="int16",
            blocksize=frame_samples,
        ) as stream:
            while True:
                block, _overflowed = stream.read(frame_samples)
                data = bytes(block)
                rms = audioop.rms(data, 2) / 32768.0
                is_voice = rms >= self.config.energy_threshold

                if not started:
                    pre_roll.append(data)
                    if is_voice:
                        started = True
                        frames.extend(pre_roll)
                        voiced_frames += 1
                    elif time.monotonic() - listen_started_at >= self.config.activation_timeout_s:
                        raise NoSpeechDetected(
                            f"{self.config.activation_timeout_s:.0f} 秒内没有检测到语音，请检查默认麦克风或降低 energy_threshold。"
                        )
                    continue

                frames.append(data)
                voiced_frames += int(is_voice)
                silence_frames = 0 if is_voice else silence_frames + 1

                if voiced_frames >= min_voice_frames and silence_frames >= silence_frames_limit:
                    break
                if len(frames) >= max_frames:
                    break

        return _write_wav(frames, self.config.sample_rate)

    def record_until_stop(self, stop_event: threading.Event) -> Path:
        try:
            import sounddevice as sd
        except (ImportError, OSError) as exc:
            if shutil.which("pw-record"):
                return PipeWireRecorder(self.config).record_until_stop(stop_event)
            raise RuntimeError("缺少 sounddevice 或 PortAudio。请运行：pip install -e '.[mic]' 并安装 libportaudio2") from exc

        frame_samples = int(self.config.sample_rate * self.config.frame_ms / 1000)
        min_voice_frames = max(1, self.config.min_voice_ms // self.config.frame_ms)
        max_frames = int(self.config.max_utterance_s * 1000 / self.config.frame_ms)
        pre_roll = deque(maxlen=max(1, 180 // self.config.frame_ms))

        frames: list[bytes] = []
        started = False
        voiced_frames = 0

        with sd.RawInputStream(
            samplerate=self.config.sample_rate,
            channels=1,
            dtype="int16",
            blocksize=frame_samples,
        ) as stream:
            while not stop_event.is_set() and len(frames) < max_frames:
                block, _overflowed = stream.read(frame_samples)
                data = bytes(block)
                rms = audioop.rms(data, 2) / 32768.0
                is_voice = rms >= self.config.energy_threshold

                if not started:
                    pre_roll.append(data)
                    if is_voice:
                        started = True
                        frames.extend(pre_roll)
                        voiced_frames += 1
                    continue

                frames.append(data)
                voiced_frames += int(is_voice)

        if not started or voiced_frames < min_voice_frames:
            raise NoSpeechDetected("没有录到足够语音，请确认麦克风输入正常并靠近说话。")
        return _write_wav(frames, self.config.sample_rate)


def _write_wav(frames: list[bytes], sample_rate: int) -> Path:
    tmp = tempfile.NamedTemporaryFile(prefix="voxflow-", suffix=".wav", delete=False)
    path = Path(tmp.name)
    tmp.close()
    with wave.open(str(path), "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(sample_rate)
        wav.writeframes(b"".join(frames))
    return path


class PipeWireRecorder:
    """Fallback recorder for Linux systems without PortAudio.

    This keeps microphone input usable on PipeWire systems. It records a fixed
    utterance window because command-line pw-record does not expose frame-level
    samples for this process to run VAD on.
    """

    def __init__(self, config: AudioConfig) -> None:
        self.config = config

    def record_once(self) -> Path:
        tmp = tempfile.NamedTemporaryFile(prefix="voxflow-pw-", suffix=".wav", delete=False)
        path = Path(tmp.name)
        tmp.close()

        process = subprocess.Popen(
            [
                "pw-record",
                "--rate",
                str(self.config.sample_rate),
                "--channels",
                "1",
                str(path),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        try:
            time.sleep(self.config.max_utterance_s)
        finally:
            process.terminate()
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()

        if not path.exists() or path.stat().st_size <= 44:
            path.unlink(missing_ok=True)
            raise RuntimeError("pw-record 未能录到有效音频，请检查 PipeWire 默认输入源。")
        return path

    def record_until_stop(self, stop_event: threading.Event) -> Path:
        tmp = tempfile.NamedTemporaryFile(prefix="voxflow-pw-", suffix=".wav", delete=False)
        path = Path(tmp.name)
        tmp.close()

        process = subprocess.Popen(
            [
                "pw-record",
                "--rate",
                str(self.config.sample_rate),
                "--channels",
                "1",
                str(path),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        deadline = time.monotonic() + self.config.max_utterance_s
        try:
            while not stop_event.is_set() and time.monotonic() < deadline:
                time.sleep(0.03)
        finally:
            process.terminate()
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()

        if not path.exists() or path.stat().st_size <= 44:
            path.unlink(missing_ok=True)
            raise NoSpeechDetected("没有录到有效音频，请检查 PipeWire 默认输入源。")
        return path


def sleep_before_next_utterance() -> None:
    time.sleep(0.05)
