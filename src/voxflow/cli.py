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
from .config import AppConfig, load_config, save_user_asr_settings
from .model_registry import (
    download_model_profile,
    get_model_profile,
    import_model_profile,
    list_model_profiles,
    model_cache_dir,
    validate_model_profile,
)
from .runtime import DictationRunner, process_audio_file


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.command == "doctor":
        return doctor()
    if args.command == "models":
        return handle_models(args)
    if args.command == "gui":
        config = load_config(args.config)
        apply_overrides(config, args)
        from .gui import serve_gui

        serve_gui(config, host=args.host, port=args.port, dry_run=args.dry_run)
        return 0
    if args.command == "native-gui":
        from .native_gui import main as native_gui_main

        argv = ["--no-start-daemon"] if args.no_start_daemon else []
        return native_gui_main(argv)
    if args.command == "daemon":
        config = load_config(args.config)
        apply_overrides(config, args)
        if args.hotkey:
            config.daemon.hotkey = args.hotkey
        if args.hotkey_mode:
            config.daemon.hotkey_mode = args.hotkey_mode
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
    if args.command == "ibus-engine":
        from .ibus_engine import run_ibus_engine

        config = load_config(args.config)
        return run_ibus_engine(config, dry_run=args.dry_run)

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
    parser = argparse.ArgumentParser(prog="voxflow", description="Linux 中文/英文语音输入法")
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

    native_gui = subparsers.add_parser("native-gui", help="启动 GTK 原生控制中心")
    native_gui.add_argument("--no-start-daemon", action="store_true", help="打开窗口时不自动启动后台输入")

    daemon = subparsers.add_parser("daemon", help="启动后台语音输入服务和全局快捷键")
    add_common_options(daemon)
    daemon.add_argument("--hotkey", help="全局快捷键，默认 ctrl+space")
    daemon.add_argument("--hotkey-mode", choices=["toggle", "hold"], help="toggle=按一次开始再按一次停止；hold=按住录音")
    daemon.add_argument("--dry-run", action="store_true", help="只打印注入动作，不实际输入")

    subparsers.add_parser("tray", help="启动桌面右上角控制图标")
    ibus_engine = subparsers.add_parser("ibus-engine", help="启动 IBus 输入法引擎")
    ibus_engine.add_argument("--config", help="配置文件路径")
    ibus_engine.add_argument("--dry-run", action="store_true")
    subparsers.add_parser("doctor", help="检查系统依赖")
    models = subparsers.add_parser("models", help="列出、下载或选择 ASR 模型")
    models.add_argument("--download", choices=[profile.id for profile in list_model_profiles()], help="下载模型到本地缓存")
    models.add_argument("--select", choices=[profile.id for profile in list_model_profiles()], help="把模型档位写入用户配置")
    models.add_argument("--import-model", type=Path, help="导入已有本地模型目录，避免重复下载")
    models.add_argument("--validate-model", type=Path, help="校验已有本地模型目录，不导入也不切换")
    models.add_argument("--symlink", action="store_true", help="配合 --import-model 使用，创建符号链接而不是复制模型")
    models.add_argument("--dir", type=Path, default=model_cache_dir(), help="模型下载目录")
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
    parser.add_argument("--script", choices=["simplified", "traditional", "original"], help="输出字形")


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
    if getattr(args, "script", None):
        config.text.script = args.script


def doctor() -> int:
    print(f"Python: {platform.python_version()} ({sys.executable})")
    print(f"系统: {platform.platform()}")
    for module in ["opencc", "faster_whisper", "qwen_asr", "sounddevice"]:
        print(f"Python 模块 {module}: {_module_status(module)}")
    print(f"X11 全局快捷键: {_x11_hotkey_status()}")
    print(f"IBus 输入法: {_ibus_status()}")
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


def _ibus_status() -> str:
    if not shutil.which("ibus"):
        return "缺少 ibus"
    python = shutil.which("python3")
    if not python:
        return "缺少 python3"
    script = "import gi; gi.require_version('IBus', '1.0'); from gi.repository import IBus"
    result = subprocess.run([python, "-c", script], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    if result.returncode == 0:
        return "OK"
    return f"不可用：{result.stderr.strip() or '缺少 gir1.2-ibus-1.0'}"


def handle_models(args: argparse.Namespace) -> int:
    if args.validate_model:
        if not args.select:
            raise SystemExit("--validate-model 需要同时指定 --select 模型档位")
        profile = get_model_profile(args.select)
        print("正在校验模型身份、格式和 SHA256...")
        result = validate_model_profile(profile.id, args.validate_model)
        print(f"校验通过：{profile.label}")
        print(f"路径：{result.path}")
        print(f"官方 revision：{result.revision}")
        print(f"已检查文件数：{len(result.checked_files)}")
        for warning in result.warnings:
            print(f"警告：{warning}")
        return 0

    if args.import_model:
        if not args.select:
            raise SystemExit("--import-model 需要同时指定 --select 模型档位")
        profile = get_model_profile(args.select)
        print("正在校验模型身份、格式和 SHA256...")
        path = import_model_profile(profile.id, args.import_model, args.dir, symlink=args.symlink)
        config_path = save_user_asr_settings(backend=profile.backend, model=str(path))
        print(f"已导入：{profile.label}")
        print("校验通过：官方 SHA256 与模型结构匹配")
        print(path)
        print(f"配置：{config_path}")
        return 0

    if args.download:
        profile = get_model_profile(args.download)
        path = download_model_profile(profile.id, args.dir)
        print(f"已下载：{profile.label}")
        print(path)
        return 0

    if args.select:
        profile = get_model_profile(args.select)
        local_path = args.dir / profile.model.split("/")[-1]
        model = str(local_path) if local_path.exists() else profile.model
        config_path = save_user_asr_settings(backend=profile.backend, model=model)
        print(f"已选择：{profile.label}")
        print(f"配置：{config_path}")
        return 0

    for profile in list_model_profiles():
        marker = []
        if profile.source_default:
            marker.append("源码默认")
        if profile.package_default:
            marker.append("deb 默认")
        suffix = f" ({'，'.join(marker)})" if marker else ""
        print(f"{profile.id}: {profile.label}{suffix}")
        print(f"  后端/模型: {profile.backend} / {profile.model}")
        print(f"  规模/语言: {profile.size} / {profile.languages}")
        print(f"  许可证: {profile.license} - {profile.license_url}")
        print(f"  官方地址: {profile.source}")
    print("下载示例：voxflow models --download qwen3-asr-0.6b")
    print("选择示例：voxflow models --select qwen3-asr-0.6b")
    print("校验示例：voxflow models --select qwen3-asr-1.7b --validate-model /path/to/Qwen3-ASR-1.7B")
    print("导入示例：voxflow models --select qwen3-asr-1.7b --import-model /path/to/Qwen3-ASR-1.7B --symlink")
    return 0
if __name__ == "__main__":
    raise SystemExit(main())
