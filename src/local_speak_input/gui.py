from __future__ import annotations

from dataclasses import asdict
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from importlib import resources
from pathlib import Path
from typing import Any
import cgi
import json
import mimetypes
import tempfile
import threading

from .asr import Recognizer, build_recognizer
from .config import AppConfig, save_user_daemon_hotkey
from .hotkey import parse_hotkey
from .input import apply_actions, build_injector
from .postprocess import DictationSession, EditAction
from .service_control import daemon_status, restart_daemon


class GuiState:
    def __init__(self, config: AppConfig, dry_run: bool = False) -> None:
        self.config = config
        self.session = DictationSession(
            remove_spoken_fillers=config.text.remove_fillers,
            auto_punctuation=config.text.auto_punctuation,
        )
        self.dry_run = dry_run
        self.injector = None
        self.recognizer: Recognizer | None = None
        self.lock = threading.Lock()

    def get_recognizer(self) -> Recognizer:
        with self.lock:
            if self.recognizer is None:
                self.recognizer = build_recognizer(self.config.asr)
            return self.recognizer

    def process_text(self, text: str, inject: bool) -> list[EditAction]:
        with self.lock:
            actions = self.session.process(text)
            if inject:
                if self.injector is None:
                    self.injector = build_injector(self.config.input, force_dry_run=self.dry_run)
                apply_actions(self.injector, actions)
            return actions


class GuiHandler(BaseHTTPRequestHandler):
    server: "GuiServer"

    def do_GET(self) -> None:
        if self.path == "/" or self.path.startswith("/?"):
            self._send_static("index.html")
            return
        if self.path in {"/app.css", "/app.js", "/logo.svg"}:
            self._send_static(self.path.removeprefix("/"))
            return
        if self.path == "/api/config":
            self._send_json(
                {
                    "asr": asdict(self.server.state.config.asr),
                    "text": asdict(self.server.state.config.text),
                    "input": asdict(self.server.state.config.input),
                    "audio": asdict(self.server.state.config.audio),
                    "daemon": asdict(self.server.state.config.daemon),
                }
            )
            return
        self.send_error(HTTPStatus.NOT_FOUND)

    def do_HEAD(self) -> None:
        if self.path == "/" or self.path.startswith("/?"):
            self._send_static("index.html", include_body=False)
            return
        if self.path in {"/app.css", "/app.js", "/logo.svg"}:
            self._send_static(self.path.removeprefix("/"), include_body=False)
            return
        self.send_error(HTTPStatus.NOT_FOUND)

    def do_POST(self) -> None:
        if self.path == "/api/process-text":
            payload = self._read_json()
            text = str(payload.get("text", ""))
            inject = bool(payload.get("inject"))
            actions = self.server.state.process_text(text, inject=inject)
            self._send_json(_actions_payload(text, actions))
            return

        if self.path == "/api/settings/hotkey":
            payload = self._read_json()
            hotkey = str(payload.get("hotkey", "")).strip().lower()
            restart_requested = bool(payload.get("restart", True))
            try:
                parse_hotkey(hotkey)
                config_path = save_user_daemon_hotkey(hotkey)
                self.server.state.config.daemon.hotkey = hotkey
                restarted = False
                if restart_requested and daemon_status().running:
                    restart_daemon()
                    restarted = True
                self._send_json(
                    {
                        "hotkey": hotkey,
                        "config_path": str(config_path),
                        "daemon_restarted": restarted,
                    }
                )
            except Exception as exc:
                self._send_json({"error": str(exc)}, status=HTTPStatus.BAD_REQUEST)
            return

        if self.path == "/api/transcribe":
            fields = cgi.FieldStorage(
                fp=self.rfile,
                headers=self.headers,
                environ={
                    "REQUEST_METHOD": "POST",
                    "CONTENT_TYPE": self.headers.get("Content-Type", ""),
                    "CONTENT_LENGTH": self.headers.get("Content-Length", "0"),
                },
            )
            audio_field = fields["audio"] if "audio" in fields else None
            if audio_field is None or not getattr(audio_field, "file", None):
                self._send_json({"error": "缺少 audio 文件字段"}, status=HTTPStatus.BAD_REQUEST)
                return
            inject = _field_bool(fields["inject"]) if "inject" in fields else False
            suffix = _suffix_from_upload(audio_field)

            with tempfile.NamedTemporaryFile(prefix="local-speak-ui-", suffix=suffix, delete=False) as fh:
                path = Path(fh.name)
                while chunk := audio_field.file.read(1024 * 1024):
                    fh.write(chunk)

            try:
                result = self.server.state.get_recognizer().transcribe(path)
                actions = self.server.state.process_text(result.text, inject=inject)
                payload = _actions_payload(result.text, actions)
                payload["language"] = result.language
                payload["duration"] = result.duration
                self._send_json(payload)
            except Exception as exc:
                self._send_json({"error": str(exc)}, status=HTTPStatus.INTERNAL_SERVER_ERROR)
            finally:
                path.unlink(missing_ok=True)
            return

        self.send_error(HTTPStatus.NOT_FOUND)

    def log_message(self, format: str, *args: Any) -> None:
        print(f"[gui] {self.address_string()} {format % args}")

    def _send_static(self, name: str, *, include_body: bool = True) -> None:
        package_files = resources.files("local_speak_input").joinpath("web")
        data = package_files.joinpath(name).read_bytes()
        content_type = "image/svg+xml" if name.endswith(".svg") else mimetypes.guess_type(name)[0]
        content_type = content_type or "application/octet-stream"
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        if include_body:
            self.wfile.write(data)

    def _send_json(self, payload: dict[str, Any], status: HTTPStatus = HTTPStatus.OK) -> None:
        data = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def _read_json(self) -> dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0"))
        if length <= 0:
            return {}
        return json.loads(self.rfile.read(length).decode("utf-8"))


class GuiServer(ThreadingHTTPServer):
    def __init__(self, server_address: tuple[str, int], state: GuiState) -> None:
        super().__init__(server_address, GuiHandler)
        self.state = state


def serve_gui(config: AppConfig, host: str = "127.0.0.1", port: int = 8765, dry_run: bool = False) -> None:
    server = GuiServer((host, port), GuiState(config, dry_run=dry_run))
    print(f"可视化控制台：http://{host}:{port}")
    print("按 Ctrl+C 停止。")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n已停止可视化控制台。")
    finally:
        server.server_close()


def _actions_payload(raw_text: str, actions: list[EditAction]) -> dict[str, Any]:
    return {
        "raw_text": raw_text,
        "processed_text": "".join(action.insert for action in actions if action.insert),
        "actions": [
            {"insert": action.insert, "backspace": action.backspace, "reason": action.reason}
            for action in actions
        ],
    }


def _field_bool(field: Any) -> bool:
    value = getattr(field, "value", field)
    return str(value).lower() in {"1", "true", "yes", "on"}


def _suffix_from_upload(field: Any) -> str:
    filename = getattr(field, "filename", "") or ""
    suffix = Path(filename).suffix
    if suffix:
        return suffix
    mime = getattr(field, "type", "") or ""
    return {
        "audio/wav": ".wav",
        "audio/wave": ".wav",
        "audio/x-wav": ".wav",
        "audio/webm": ".webm",
        "audio/ogg": ".ogg",
        "audio/mpeg": ".mp3",
        "audio/mp4": ".m4a",
    }.get(mime, ".webm")
