from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol
import os
import shutil
import subprocess
import time

from local_speak_input.config import InputConfig
from local_speak_input.postprocess import EditAction


class Injector(Protocol):
    def type_text(self, text: str) -> None:
        ...

    def backspace(self, count: int) -> None:
        ...


@dataclass(slots=True)
class InjectionEvent:
    kind: str
    value: str | int


class DryRunInjector:
    def __init__(self) -> None:
        self.events: list[InjectionEvent] = []

    def type_text(self, text: str) -> None:
        self.events.append(InjectionEvent("type", text))
        print(text)

    def backspace(self, count: int) -> None:
        self.events.append(InjectionEvent("backspace", count))
        print(f"[backspace x{count}]")


class SystemInjector:
    def __init__(self, method: str = "auto", backspace_delay: float = 0.0) -> None:
        self.method = _resolve_method(method)
        self.backspace_delay = backspace_delay

    def type_text(self, text: str) -> None:
        if not text:
            return
        if self.method == "wtype":
            subprocess.run(["wtype", text], check=True)
        elif self.method == "wayland-clipboard":
            subprocess.run(["wl-copy"], input=text.encode("utf-8"), check=True)
            subprocess.run(["wtype", "-M", "ctrl", "-k", "v", "-m", "ctrl"], check=True)
        elif self.method == "xclip":
            subprocess.run(["xclip", "-selection", "clipboard"], input=text.encode("utf-8"), check=True)
            subprocess.run(["xdotool", "key", "--clearmodifiers", "ctrl+v"], check=True)
        elif self.method == "xsel":
            subprocess.run(["xsel", "--clipboard", "--input"], input=text.encode("utf-8"), check=True)
            subprocess.run(["xdotool", "key", "--clearmodifiers", "ctrl+v"], check=True)
        elif self.method == "xdotool":
            subprocess.run(["xdotool", "type", "--clearmodifiers", "--delay", "0", "--", text], check=True)
        elif self.method == "ydotool":
            subprocess.run(["ydotool", "type", "--delay", "0", text], check=True)
        else:
            raise RuntimeError(f"不支持的输入注入方式：{self.method}")

    def backspace(self, count: int) -> None:
        if count <= 0:
            return
        if self.method in {"xdotool", "xclip", "xsel"}:
            subprocess.run(["xdotool", "key", "--repeat", str(count), "BackSpace"], check=True)
            return
        for _ in range(count):
            if self.method in {"wtype", "wayland-clipboard"}:
                subprocess.run(["wtype", "-k", "BackSpace"], check=True)
            elif self.method == "ydotool":
                subprocess.run(["ydotool", "key", "14:1", "14:0"], check=True)
            else:
                raise RuntimeError(f"不支持的输入注入方式：{self.method}")
            if self.backspace_delay:
                time.sleep(self.backspace_delay)


def build_injector(config: InputConfig, force_dry_run: bool = False) -> Injector:
    if force_dry_run or config.dry_run:
        return DryRunInjector()
    return SystemInjector(config.injector, config.backspace_delay)


def apply_actions(injector: Injector, actions: list[EditAction]) -> None:
    for action in actions:
        if action.backspace:
            injector.backspace(action.backspace)
        if action.insert:
            injector.type_text(action.insert)


def _resolve_method(method: str) -> str:
    normalized = method.strip().lower()
    if normalized != "auto":
        _validate_method(normalized)
        return normalized

    session = os.getenv("XDG_SESSION_TYPE", "").lower()
    if session == "wayland" and shutil.which("wl-copy") and shutil.which("wtype"):
        return "wayland-clipboard"
    if session == "wayland" and shutil.which("wtype"):
        return "wtype"
    if os.getenv("WAYLAND_DISPLAY") and shutil.which("wl-copy") and shutil.which("wtype"):
        return "wayland-clipboard"
    if os.getenv("WAYLAND_DISPLAY") and shutil.which("wtype"):
        return "wtype"
    if os.getenv("DISPLAY") and shutil.which("xclip") and shutil.which("xdotool"):
        return "xclip"
    if os.getenv("DISPLAY") and shutil.which("xsel") and shutil.which("xdotool"):
        return "xsel"
    if os.getenv("DISPLAY") and shutil.which("xdotool"):
        return "xdotool"
    if shutil.which("ydotool"):
        return "ydotool"
    raise RuntimeError("未找到 wtype/xdotool/ydotool 或剪贴板输入工具，请安装至少一个 Linux 文本注入工具。")


def _validate_method(method: str) -> None:
    requirements = {
        "wtype": ["wtype"],
        "wayland-clipboard": ["wl-copy", "wtype"],
        "xclip": ["xclip", "xdotool"],
        "xsel": ["xsel", "xdotool"],
        "xdotool": ["xdotool"],
        "ydotool": ["ydotool"],
    }
    required = requirements.get(method)
    if not required:
        raise RuntimeError(f"不支持的输入注入方式：{method}")
    missing = [binary for binary in required if not shutil.which(binary)]
    if missing:
        raise RuntimeError(f"输入方式 {method} 缺少命令：{', '.join(missing)}")
