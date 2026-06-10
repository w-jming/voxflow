from __future__ import annotations

from pathlib import Path
import argparse
import importlib.util
import ctypes.util
import platform
import shutil
import subprocess
import sys

from . import __version__
from .audio import NoSpeechDetected
from .config import AppConfig, load_config
from .runtime import DictationRunner, process_audio_file


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command == "doctor":
        return doctor()
    if args.command == "models":
        print_model_advice()
        return 0
    if args.command == "gui":
        config = load_config(args.config)
        apply_overrides(config, args)
        from .gui import serve_gui

        serve_gui(config, host=args.host, port=args.port, dry_run=args.dry_run)
        return 0
    if args.command == "daemon":
        config = load_config(args.config)
        apply_overrides(config, args)
        if args.hotkey:
            config.daemon.hotkey = args.hotkey
        from .daemon import VoiceInputDaemon

        try:
            VoiceInputDaemon(config, hotkey=args.hotkey, dry_run=args.dry_run).run()
        except KeyboardInterrupt:
            print("\n已停止后台语音输入。")
            return 0
        return 0
    if args.command == "tray":
        from .tray import main as tray_main

        return tray_main()

    config = load_config(args.config)
    apply_overrides(config, args)

    if args.command == "transcribe":
        result, actions = process_audio_file(
            args.audio,
            config,
            inject=args.inject,
            dry_run=args.dry_run,
        )
        print(f"原始识别：{result.text}")
        print("处理结果：" + "".join(action.insert for action in actions if action.insert))
        for action in actions:
            if action.backspace:
                print(f"退格：{action.backspace} ({action.reason})")
        return 0

    if args.command == "dictate":
        runner = DictationRunner(config, dry_run=args.dry_run)
        if args.once:
            try:
                result, _actions = runner.run_once()
            except NoSpeechDetected as exc:
                print(str(exc), file=sys.stderr)
                return 1
            print(f"识别：{result.text}")
            return 0
        try:
            runner.run_forever()
        except KeyboardInterrupt:
            print("\n已停止。")
            return 0

    parser.print_help()
    return 2


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="local-speak", description="Linux 中文/英文语音输入法")
    parser.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    subparsers = parser.add_subparsers(dest="command")

    transcribe = subparsers.add_parser("transcribe", help="识别音频文件")
    add_common_options(transcribe)
    transcribe.add_argument("audio", type=Path)
    transcribe.add_argument("--inject", action="store_true", help="把处理后的文本输入到当前焦点窗口")
    transcribe.add_argument("--dry-run", action="store_true", help="只打印注入动作，不实际输入")

    dictate = subparsers.add_parser("dictate", help="从麦克风连续听写并输入")
    add_common_options(dictate)
    dictate.add_argument("--once", action="store_true", help="只录一段话")
    dictate.add_argument("--dry-run", action="store_true", help="只打印注入动作，不实际输入")

    gui = subparsers.add_parser("gui", help="启动本地可视化控制台")
    add_common_options(gui)
    gui.add_argument("--host", default="127.0.0.1")
    gui.add_argument("--port", type=int, default=8765)
    gui.add_argument("--dry-run", action="store_true", help="后端只记录输入动作，不实际输入")

    daemon = subparsers.add_parser("daemon", help="启动后台语音输入服务和全局快捷键")
    add_common_options(daemon)
    daemon.add_argument("--hotkey", help="全局快捷键，默认 ctrl+alt+space")
    daemon.add_argument("--dry-run", action="store_true", help="只打印注入动作，不实际输入")

    subparsers.add_parser("tray", help="启动桌面右上角控制图标")
    subparsers.add_parser("doctor", help="检查系统依赖")
    subparsers.add_parser("models", help="显示模型选择建议")
    return parser


def add_common_options(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--config", help="配置文件路径")
    parser.add_argument("--backend", choices=["faster-whisper", "qwen", "openai-compatible"])
    parser.add_argument("--model", help="模型名或本地模型路径")
    parser.add_argument("--language", help="语言：auto/zh/en")
    parser.add_argument("--device", choices=["auto", "cpu", "cuda"])
    parser.add_argument("--compute-type", help="faster-whisper compute_type，例如 float16/int8")
    parser.add_argument("--api-base", help="OpenAI 兼容 ASR 服务地址，例如 http://127.0.0.1:8000/v1")
    parser.add_argument("--api-model", help="OpenAI 兼容 ASR 模型名")


def apply_overrides(config: AppConfig, args: argparse.Namespace) -> None:
    for attr in ("backend", "model", "language", "device"):
        value = getattr(args, attr, None)
        if value:
            setattr(config.asr, attr, value)
    if getattr(args, "compute_type", None):
        config.asr.compute_type = args.compute_type
    if getattr(args, "api_base", None):
        config.asr.api_base = args.api_base
    if getattr(args, "api_model", None):
        config.asr.api_model = args.api_model


def doctor() -> int:
    print(f"Python: {platform.python_version()} ({sys.executable})")
    print(f"系统: {platform.platform()}")
    for module in ["faster_whisper", "qwen_asr", "sounddevice"]:
        print(f"Python 模块 {module}: {_module_status(module)}")
    print(f"X11 全局快捷键: {_x11_hotkey_status()}")
    print(f"右上角图标: {_tray_status()}")
    for binary in ["notify-send", "wtype", "wl-copy", "xdotool", "xclip", "xsel", "ydotool", "pw-record", "ffmpeg", "nvidia-smi"]:
        print(f"命令 {binary}: {shutil.which(binary) or '未找到'}")
    return 0


def _module_status(module: str) -> str:
    if not importlib.util.find_spec(module):
        return "未安装"
    try:
        __import__(module)
    except Exception as exc:
        return f"已安装但不可用：{exc}"
    return "OK"


def _x11_hotkey_status() -> str:
    if not sys.platform.startswith("linux"):
        return "不适用"
    if not ctypes.util.find_library("X11"):
        return "缺少 libX11"
    return "OK" if shutil.which("xdotool") else "缺少 xdotool"


def _tray_status() -> str:
    python = shutil.which("python3")
    if not python:
        return "缺少 python3"
    script = (
        "import gi; "
        "gi.require_version('Gtk', '3.0'); "
        "gi.require_version('AppIndicator3', '0.1'); "
        "from gi.repository import Gtk, AppIndicator3"
    )
    result = subprocess.run([python, "-c", script], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    if result.returncode == 0:
        return "OK"
    return f"不可用：{result.stderr.strip() or '缺少 GTK/AppIndicator'}"


def print_model_advice() -> None:
    print(
        "\n".join(
            [
                "模型建议：",
                "1. 准确率/中英混输优先：Qwen/Qwen3-ASR-1.7B，本地 GPU 或 vLLM/OpenAI 兼容服务。",
                "2. 轻量和低延迟优先：Qwen/Qwen3-ASR-0.6B 或 faster-whisper large-v3-turbo。",
                "3. 中文标点优先且使用 Whisper：k1nto/Belle-whisper-large-v3-zh-punct-ct2。",
                "4. 工业化本地服务：FunASR/SenseVoice，可提供 VAD、标点、ITN 和 OpenAI 兼容服务。",
                "5. 云 API：优先实测 Qwen3-ASR-Flash/DashScope、火山引擎豆包 ASR、NVIDIA NIM Parakeet。",
            ]
        )
    )


if __name__ == "__main__":
    raise SystemExit(main())
