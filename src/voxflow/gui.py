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
from .config import (
    AppConfig,
    normalize_hotkey_mode,
    normalize_semantic_intent_backend,
    normalize_script,
    save_user_asr_settings,
    save_user_daemon_hotkey,
    save_user_daemon_settings,
    save_user_text_semantic_correction,
    save_user_text_semantic_intent_backend,
    save_user_text_script,
)
from .hotkey import parse_hotkey
from .input import apply_actions, build_injector
from .postprocess import DictationSession, EditAction
from .model_registry import download_model_profile, get_model_profile, list_model_profiles, model_cache_dir
from .semantic_intent import list_semantic_intent_backends
from .service_control import daemon_status, restart_daemon


class GuiState:
    def __init__(self, config: AppConfig, dry_run: bool = False) -> None:
        self.config = config
        self.session = DictationSession(
            remove_spoken_fillers=config.text.remove_fillers,
            auto_punctuation=config.text.auto_punctuation,
            script=config.text.script,
            semantic_correction_enabled=config.text.semantic_correction_enabled,
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
        if self.path == "/api/models":
            self._send_json({"models": [profile.to_dict() for profile in list_model_profiles()]})
            return
        if self.path == "/api/semantic-intent":
            self._send_json({"backends": [backend.to_dict() for backend in list_semantic_intent_backends()]})
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

        if self.path == "/api/settings":
            payload = self._read_json()
            restart_requested = bool(payload.get("restart", True))
            try:
                hotkey = _optional_str(payload.get("hotkey"))
                hotkey_mode = _optional_str(payload.get("hotkey_mode"))
                script = _optional_str(payload.get("script"))
                semantic_correction = payload.get("semantic_correction_enabled")
                semantic_intent_backend = _optional_str(payload.get("semantic_intent_backend"))
                model_profile = _optional_str(payload.get("model_profile"))

                config_path = None
                if model_profile is not None:
                    profile = get_model_profile(model_profile)
                    local_path = model_cache_dir() / profile.model.split("/")[-1]
                    model = str(local_path) if local_path.exists() else profile.model
                    if profile.model.startswith("bundled:"):
                        model = profile.model
                    config_path = save_user_asr_settings(backend=profile.backend, model=model)
                    self.server.state.config.asr.backend = profile.backend
                    self.server.state.config.asr.model = model
                    self.server.state.recognizer = None
                if hotkey is not None or hotkey_mode is not None:
                    config_path = save_user_daemon_settings(hotkey=hotkey, hotkey_mode=hotkey_mode)
                    if hotkey is not None:
                        parse_hotkey(hotkey)
                        self.server.state.config.daemon.hotkey = hotkey
                    if hotkey_mode is not None:
                        self.server.state.config.daemon.hotkey_mode = normalize_hotkey_mode(hotkey_mode)
                if script is not None:
                    config_path = save_user_text_script(script)
                    self.server.state.config.text.script = normalize_script(script)
                    self.server.state.session.script = self.server.state.config.text.script
                if semantic_correction is not None:
                    config_path = save_user_text_semantic_correction(bool(semantic_correction))
                    self.server.state.config.text.semantic_correction_enabled = bool(semantic_correction)
                    self.server.state.session.semantic_correction_enabled = bool(semantic_correction)
                if semantic_intent_backend is not None:
                    normalized_backend = normalize_semantic_intent_backend(semantic_intent_backend)
                    backend_status = {
                        backend.id: backend.status for backend in list_semantic_intent_backends()
                    }.get(normalized_backend)
                    if backend_status != "available":
                        raise ValueError("该语义意图后端尚未在本机启用，需要先训练或安装对应模型")
                    config_path = save_user_text_semantic_intent_backend(normalized_backend)
                    self.server.state.config.text.semantic_intent_backend = normalized_backend

                restarted = False
                if restart_requested and daemon_status().running:
                    restart_daemon()
                    restarted = True
                self._send_json(
                    {
                        "hotkey": self.server.state.config.daemon.hotkey,
                        "hotkey_mode": self.server.state.config.daemon.hotkey_mode,
                        "script": self.server.state.config.text.script,
                        "semantic_correction_enabled": self.server.state.config.text.semantic_correction_enabled,
                        "semantic_intent_backend": self.server.state.config.text.semantic_intent_backend,
                        "asr": asdict(self.server.state.config.asr),
                        "config_path": str(config_path) if config_path else "",
                        "daemon_restarted": restarted,
                    }
                )
            except Exception as exc:
                self._send_json({"error": str(exc)}, status=HTTPStatus.BAD_REQUEST)
            return

        if self.path == "/api/models/download":
            payload = self._read_json()
            profile_id = str(payload.get("model_profile", "")).strip()
            try:
                path = download_model_profile(profile_id, model_cache_dir())
                self._send_json({"model_profile": profile_id, "path": str(path)})
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

            with tempfile.NamedTemporaryFile(prefix="voxflow-ui-", suffix=suffix, delete=False) as fh:
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
        package_files = resources.files("voxflow").joinpath("web")
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


def _optional_str(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value).strip().lower()
    return text if text else None
