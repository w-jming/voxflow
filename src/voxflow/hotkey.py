from __future__ import annotations

from dataclasses import dataclass
from typing import Callable
import ctypes
import ctypes.util
import os
import re
import time


KEY_PRESS = 2
KEY_RELEASE = 3
GRAB_MODE_ASYNC = 1
SHIFT_MASK = 1 << 0
LOCK_MASK = 1 << 1
CONTROL_MASK = 1 << 2
MOD1_MASK = 1 << 3
MOD2_MASK = 1 << 4
MOD4_MASK = 1 << 6


MODIFIER_ALIASES = {
    "shift": SHIFT_MASK,
    "ctrl": CONTROL_MASK,
    "control": CONTROL_MASK,
    "alt": MOD1_MASK,
    "mod1": MOD1_MASK,
    "super": MOD4_MASK,
    "meta": MOD4_MASK,
    "win": MOD4_MASK,
    "mod4": MOD4_MASK,
}

KEY_ALIASES = {
    "space": "space",
    "enter": "Return",
    "return": "Return",
    "esc": "Escape",
    "escape": "Escape",
    "tab": "Tab",
}


@dataclass(frozen=True, slots=True)
class HotkeySpec:
    key: str
    modifiers: int


@dataclass(frozen=True, slots=True)
class HotkeyEvent:
    kind: str
    time: int


def parse_hotkey(value: str) -> HotkeySpec:
    normalized = value.strip().replace("-", "+")
    if "<" in normalized:
        tokens = [match.group(1) or match.group(2) for match in re.finditer(r"<([^>]+)>|([^+<>]+)", normalized)]
    else:
        tokens = normalized.split("+")
    parts = [part.strip().lower() for part in tokens]
    parts = [part for part in parts if part]
    if not parts:
        raise ValueError("快捷键不能为空")

    modifiers = 0
    key_parts: list[str] = []
    for part in parts:
        if part in MODIFIER_ALIASES:
            modifiers |= MODIFIER_ALIASES[part]
        else:
            key_parts.append(part)

    if len(key_parts) != 1:
        raise ValueError(f"快捷键必须包含且只包含一个普通按键：{value}")

    key = KEY_ALIASES.get(key_parts[0], key_parts[0])
    if len(key) == 1:
        key = key.upper()
    elif re.fullmatch(r"f\d{1,2}", key):
        key = key.upper()
    return HotkeySpec(key=key, modifiers=modifiers)


class XKeyEvent(ctypes.Structure):
    _fields_ = [
        ("type", ctypes.c_int),
        ("serial", ctypes.c_ulong),
        ("send_event", ctypes.c_int),
        ("display", ctypes.c_void_p),
        ("window", ctypes.c_ulong),
        ("root", ctypes.c_ulong),
        ("subwindow", ctypes.c_ulong),
        ("time", ctypes.c_ulong),
        ("x", ctypes.c_int),
        ("y", ctypes.c_int),
        ("x_root", ctypes.c_int),
        ("y_root", ctypes.c_int),
        ("state", ctypes.c_uint),
        ("keycode", ctypes.c_uint),
        ("same_screen", ctypes.c_int),
    ]


class XEvent(ctypes.Union):
    _fields_ = [
        ("type", ctypes.c_int),
        ("xkey", XKeyEvent),
        ("pad", ctypes.c_long * 24),
    ]


class XErrorEvent(ctypes.Structure):
    _fields_ = [
        ("type", ctypes.c_int),
        ("display", ctypes.c_void_p),
        ("resourceid", ctypes.c_ulong),
        ("serial", ctypes.c_ulong),
        ("error_code", ctypes.c_ubyte),
        ("request_code", ctypes.c_ubyte),
        ("minor_code", ctypes.c_ubyte),
    ]


XErrorHandler = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.POINTER(XErrorEvent))


class X11HotkeyListener:
    def __init__(self, hotkey: str) -> None:
        if not os.getenv("DISPLAY"):
            raise RuntimeError("全局快捷键当前只支持 X11，需要 DISPLAY 环境变量。")
        self.spec = parse_hotkey(hotkey)
        self.xlib = _load_xlib()
        self.display = self.xlib.XOpenDisplay(None)
        if not self.display:
            raise RuntimeError("无法连接 X11 display。")
        self.root = self.xlib.XDefaultRootWindow(self.display)
        self.keycode = self._keycode(self.spec.key)
        self._x_errors: list[int] = []
        self._error_handler = XErrorHandler(self._handle_x_error)
        self._grab()

    def run(self, callback: Callable[[], None]) -> None:
        self.run_events(lambda event: callback() if event.kind == "press" else None)

    def run_events(self, callback: Callable[[HotkeyEvent], None]) -> None:
        event = XEvent()
        while True:
            if self.xlib.XPending(self.display) <= 0:
                time.sleep(0.05)
                continue
            self.xlib.XNextEvent(self.display, ctypes.byref(event))
            if event.type not in {KEY_PRESS, KEY_RELEASE}:
                continue
            if event.xkey.keycode == self.keycode and _matches_state(event.xkey.state, self.spec.modifiers):
                if event.type == KEY_RELEASE and self._is_auto_repeat_release(event.xkey):
                    continue
                kind = "press" if event.type == KEY_PRESS else "release"
                callback(HotkeyEvent(kind=kind, time=int(event.xkey.time)))

    def close(self) -> None:
        if self.display:
            self.xlib.XCloseDisplay(self.display)
            self.display = None

    def _keycode(self, key: str) -> int:
        keysym = self.xlib.XStringToKeysym(key.encode("ascii"))
        if not keysym:
            raise ValueError(f"无法解析快捷键按键：{key}")
        keycode = self.xlib.XKeysymToKeycode(self.display, keysym)
        if not keycode:
            raise ValueError(f"无法获取快捷键 keycode：{key}")
        return int(keycode)

    def _grab(self) -> None:
        self.xlib.XSetErrorHandler(self._error_handler)
        for extra_mask in _lock_mask_variants():
            self.xlib.XGrabKey(
                self.display,
                self.keycode,
                self.spec.modifiers | extra_mask,
                self.root,
                False,
                GRAB_MODE_ASYNC,
                GRAB_MODE_ASYNC,
            )
        self.xlib.XSync(self.display, False)
        if self._x_errors:
            raise RuntimeError(f"快捷键 {self.spec.key} 已被占用或无法注册。")

    def _handle_x_error(self, _display: ctypes.c_void_p, event: ctypes.POINTER(XErrorEvent)) -> int:
        self._x_errors.append(int(event.contents.error_code))
        return 0

    def _is_auto_repeat_release(self, event: XKeyEvent) -> bool:
        if self.xlib.XPending(self.display) <= 0:
            return False
        next_event = XEvent()
        self.xlib.XPeekEvent(self.display, ctypes.byref(next_event))
        return bool(
            next_event.type == KEY_PRESS
            and next_event.xkey.keycode == event.keycode
            and _matches_state(next_event.xkey.state, self.spec.modifiers)
            and next_event.xkey.time == event.time
        )


def _matches_state(state: int, modifiers: int) -> bool:
    ignored = LOCK_MASK | MOD2_MASK
    return (state & ~ignored) == modifiers


def _lock_mask_variants() -> tuple[int, ...]:
    return (
        0,
        LOCK_MASK,
        MOD2_MASK,
        LOCK_MASK | MOD2_MASK,
    )


def _load_xlib() -> ctypes.CDLL:
    path = ctypes.util.find_library("X11")
    if not path:
        raise RuntimeError("找不到 libX11。请安装 libx11-6。")
    xlib = ctypes.CDLL(path)
    xlib.XOpenDisplay.argtypes = [ctypes.c_char_p]
    xlib.XOpenDisplay.restype = ctypes.c_void_p
    xlib.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
    xlib.XDefaultRootWindow.restype = ctypes.c_ulong
    xlib.XStringToKeysym.argtypes = [ctypes.c_char_p]
    xlib.XStringToKeysym.restype = ctypes.c_ulong
    xlib.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
    xlib.XKeysymToKeycode.restype = ctypes.c_uint
    xlib.XGrabKey.argtypes = [
        ctypes.c_void_p,
        ctypes.c_int,
        ctypes.c_uint,
        ctypes.c_ulong,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
    ]
    xlib.XGrabKey.restype = ctypes.c_int
    xlib.XNextEvent.argtypes = [ctypes.c_void_p, ctypes.POINTER(XEvent)]
    xlib.XNextEvent.restype = ctypes.c_int
    xlib.XPeekEvent.argtypes = [ctypes.c_void_p, ctypes.POINTER(XEvent)]
    xlib.XPeekEvent.restype = ctypes.c_int
    xlib.XPending.argtypes = [ctypes.c_void_p]
    xlib.XPending.restype = ctypes.c_int
    xlib.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
    xlib.XSync.restype = ctypes.c_int
    xlib.XCloseDisplay.argtypes = [ctypes.c_void_p]
    xlib.XCloseDisplay.restype = ctypes.c_int
    xlib.XSetErrorHandler.restype = ctypes.c_void_p
    return xlib
