from __future__ import annotations

from pathlib import Path
from urllib.error import HTTPError
from urllib.parse import urljoin
from urllib.request import Request, urlopen
import json
import mimetypes
import os
import uuid

from .base import TranscriptionResult


class OpenAICompatibleRecognizer:
    def __init__(
        self,
        base_url: str,
        model: str,
        api_key_env: str = "OPENAI_API_KEY",
        language: str = "auto",
        timeout: float = 120.0,
    ) -> None:
        self.base_url = base_url.rstrip("/") + "/"
        self.model = model
        self.api_key_env = api_key_env
        self.language = None if language in {"", "auto", None} else language
        self.timeout = timeout

    def transcribe(self, audio_path: str | Path) -> TranscriptionResult:
        path = Path(audio_path)
        fields = {
            "model": self.model,
            "response_format": "json",
        }
        if self.language:
            fields["language"] = self.language

        body, content_type = _multipart_body(fields, path)
        request = Request(
            urljoin(self.base_url, "audio/transcriptions"),
            data=body,
            method="POST",
            headers={"Content-Type": content_type},
        )
        api_key = os.getenv(self.api_key_env)
        if api_key:
            request.add_header("Authorization", f"Bearer {api_key}")

        try:
            with urlopen(request, timeout=self.timeout) as response:
                payload = json.loads(response.read().decode("utf-8"))
        except HTTPError as exc:
            detail = exc.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"ASR API 请求失败：HTTP {exc.code}: {detail}") from exc

        text = payload.get("text")
        if not isinstance(text, str):
            raise RuntimeError(f"ASR API 返回中缺少 text 字段：{payload!r}")

        return TranscriptionResult(
            text=text.strip(),
            language=payload.get("language"),
            duration=payload.get("duration"),
            raw=payload,
        )


def _multipart_body(fields: dict[str, str], audio_path: Path) -> tuple[bytes, str]:
    boundary = f"voxflow-{uuid.uuid4().hex}"
    chunks: list[bytes] = []

    for key, value in fields.items():
        chunks.extend(
            [
                f"--{boundary}\r\n".encode(),
                f'Content-Disposition: form-data; name="{key}"\r\n\r\n'.encode(),
                str(value).encode("utf-8"),
                b"\r\n",
            ]
        )

    mime = mimetypes.guess_type(audio_path.name)[0] or "application/octet-stream"
    chunks.extend(
        [
            f"--{boundary}\r\n".encode(),
            (
                'Content-Disposition: form-data; name="file"; '
                f'filename="{audio_path.name}"\r\n'
            ).encode(),
            f"Content-Type: {mime}\r\n\r\n".encode(),
            audio_path.read_bytes(),
            b"\r\n",
            f"--{boundary}--\r\n".encode(),
        ]
    )

    return b"".join(chunks), f"multipart/form-data; boundary={boundary}"
