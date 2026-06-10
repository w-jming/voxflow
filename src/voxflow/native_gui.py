from __future__ import annotations

from dataclasses import asdict
from pathlib import Path
import os
import shutil
import sys

try:
    import gi

    gi.require_version("Gtk", "3.0")
    from gi.repository import GLib, Gtk
except Exception as exc:  # pragma: no cover - exercised by runtime doctor
    print(f"无法启动 VoxFlow 图形界面：缺少 GTK 运行时：{exc}", file=sys.stderr)
    raise SystemExit(1) from exc

from .config import (
    load_config,
    normalize_hotkey_mode,
    normalize_script,
    save_user_asr_settings,
    save_user_daemon_settings,
    save_user_text_semantic_correction,
    save_user_text_script,
)
from .gui import ModelDownloadManager
from .hotkey import parse_hotkey
from .model_registry import (
    get_model_profile,
    import_model_profile,
    list_model_profiles,
    model_cache_dir,
    model_local_dir,
    validate_model_profile,
)
from .paths import app_home, app_home_source, set_app_home, user_config_path
from .service_control import daemon_status, open_state_dir, restart_daemon, start_daemon, stop_daemon


class VoxFlowWindow(Gtk.Window):
    def __init__(self, *, start_daemon_on_open: bool = True) -> None:
        super().__init__(title="声流输入法 VoxFlow")
        self.set_default_size(840, 640)
        self.set_border_width(0)
        self.config = load_config()
        self.downloads = ModelDownloadManager()
        self.models = list_model_profiles()
        self._apply_style()
        self._build_ui()
        self._load_config()
        if start_daemon_on_open:
            try:
                start_daemon()
            except Exception as exc:
                self._set_message(f"后台启动失败：{exc}", error=True)
        self._refresh_daemon_status()
        GLib.timeout_add_seconds(1, self._tick)

    def _apply_style(self) -> None:
        css = b"""
        window { background: #f2f8fc; color: #1f2833; }
        .header { background: #dff4ff; border-bottom: 1px solid #bedbeb; }
        .title { font-size: 24px; font-weight: 700; }
        .muted { color: #5d6a73; }
        .section-title { font-size: 13px; font-weight: 700; color: #52616b; }
        button.suggested-action { background: #1da7e8; border-color: #1da7e8; color: white; }
        button.destructive-action { background: #b4323b; border-color: #b4323b; color: white; }
        progressbar trough { min-height: 10px; border-radius: 999px; background: #e9f4fb; }
        progressbar progress { min-height: 10px; border-radius: 999px; background: #1da7e8; }
        entry, combobox, switch { min-height: 34px; }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css)
        Gtk.StyleContext.add_provider_for_screen(
            self.get_screen(),
            provider,
            Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION,
        )

    def _build_ui(self) -> None:
        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.add(root)

        header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=16)
        header.get_style_context().add_class("header")
        header.set_border_width(18)
        root.pack_start(header, False, False, 0)

        title_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        title = Gtk.Label(label="声流输入法")
        title.set_xalign(0)
        title.get_style_context().add_class("title")
        subtitle = Gtk.Label(label="VoxFlow Input")
        subtitle.set_xalign(0)
        subtitle.get_style_context().add_class("muted")
        title_box.pack_start(title, False, False, 0)
        title_box.pack_start(subtitle, False, False, 0)
        header.pack_start(title_box, True, True, 0)

        self.daemon_label = Gtk.Label(label="后台状态：读取中")
        self.daemon_label.get_style_context().add_class("muted")
        header.pack_start(self.daemon_label, False, False, 0)

        content = Gtk.ScrolledWindow()
        content.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        root.pack_start(content, True, True, 0)

        body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=18)
        body.set_border_width(18)
        content.add(body)

        body.pack_start(self._daemon_section(), False, False, 0)
        body.pack_start(self._settings_section(), False, False, 0)
        body.pack_start(self._model_section(), False, False, 0)

        self.message = Gtk.Label(label=" ")
        self.message.set_xalign(0)
        self.message.set_line_wrap(True)
        self.message.get_style_context().add_class("muted")
        body.pack_start(self.message, False, False, 0)

    def _daemon_section(self) -> Gtk.Widget:
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        box.pack_start(_section_title("后台输入"), False, False, 0)
        row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        self.start_btn = Gtk.Button(label="启动")
        self.stop_btn = Gtk.Button(label="停止")
        self.restart_btn = Gtk.Button(label="重启")
        self.logs_btn = Gtk.Button(label="打开日志目录")
        self.start_btn.get_style_context().add_class("suggested-action")
        self.stop_btn.get_style_context().add_class("destructive-action")
        self.start_btn.connect("clicked", lambda _btn: self._run_daemon_action(start_daemon, "后台输入已启动"))
        self.stop_btn.connect("clicked", lambda _btn: self._run_daemon_action(stop_daemon, "后台输入已停止"))
        self.restart_btn.connect("clicked", lambda _btn: self._run_daemon_action(restart_daemon, "后台输入已重启"))
        self.logs_btn.connect("clicked", lambda _btn: open_state_dir())
        for button in (self.start_btn, self.stop_btn, self.restart_btn, self.logs_btn):
            row.pack_start(button, False, False, 0)
        box.pack_start(row, False, False, 0)
        return box

    def _settings_section(self) -> Gtk.Widget:
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        box.pack_start(_section_title("输入设置"), False, False, 0)
        grid = Gtk.Grid(column_spacing=12, row_spacing=10)
        box.pack_start(grid, False, False, 0)

        self.hotkey_entry = Gtk.Entry()
        self.mode_combo = _combo([("toggle", "按一次开始，再按一次停止"), ("hold", "按住录音，松开输入")])
        self.script_combo = _combo([("simplified", "简体中文"), ("traditional", "繁体中文"), ("original", "模型原文")])
        self.app_home_entry = Gtk.Entry()
        self.semantic_switch = Gtk.Switch()
        self.save_btn = Gtk.Button(label="保存设置")
        self.save_btn.get_style_context().add_class("suggested-action")
        self.save_btn.connect("clicked", lambda _btn: self._save_settings())

        _attach_labeled(grid, 0, 0, "快捷键", self.hotkey_entry)
        _attach_labeled(grid, 1, 0, "录音模式", self.mode_combo)
        _attach_labeled(grid, 0, 1, "输出字形", self.script_combo)
        _attach_labeled(grid, 1, 1, "数据目录", self.app_home_entry)
        _attach_labeled(grid, 0, 2, "智能撤销（规则+账本）", self.semantic_switch)
        grid.attach(self.save_btn, 1, 2, 1, 1)
        return box

    def _model_section(self) -> Gtk.Widget:
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        box.pack_start(_section_title("模型管理"), False, False, 0)

        row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        self.model_combo = Gtk.ComboBoxText()
        for profile in self.models:
            self.model_combo.append(profile.id, profile.label)
        self.model_combo.connect("changed", lambda _combo: self._refresh_model_summary())
        row.pack_start(self.model_combo, True, True, 0)
        self.download_btn = Gtk.Button(label="下载/继续")
        self.pause_btn = Gtk.Button(label="暂停")
        self.download_btn.get_style_context().add_class("suggested-action")
        self.download_btn.connect("clicked", lambda _btn: self._start_download())
        self.pause_btn.connect("clicked", lambda _btn: self._pause_download())
        row.pack_start(self.download_btn, False, False, 0)
        row.pack_start(self.pause_btn, False, False, 0)
        box.pack_start(row, False, False, 0)

        self.model_summary = Gtk.Label(label=" ")
        self.model_summary.set_xalign(0)
        self.model_summary.set_line_wrap(True)
        self.model_summary.get_style_context().add_class("muted")
        box.pack_start(self.model_summary, False, False, 0)

        self.progress = Gtk.ProgressBar()
        self.progress.set_show_text(False)
        self.progress_label = Gtk.Label(label="未下载")
        self.progress_label.set_xalign(0)
        self.progress_label.get_style_context().add_class("muted")
        box.pack_start(self.progress, False, False, 0)
        box.pack_start(self.progress_label, False, False, 0)

        local_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        self.local_path_entry = Gtk.Entry()
        self.local_path_entry.set_placeholder_text("/path/to/Qwen3-ASR-1.7B")
        validate_btn = Gtk.Button(label="校验本地模型")
        copy_btn = Gtk.Button(label="复制导入")
        link_btn = Gtk.Button(label="软链接导入")
        validate_btn.connect("clicked", lambda _btn: self._validate_local_model())
        copy_btn.connect("clicked", lambda _btn: self._import_local_model(symlink=False))
        link_btn.connect("clicked", lambda _btn: self._import_local_model(symlink=True))
        local_row.pack_start(self.local_path_entry, True, True, 0)
        for button in (validate_btn, copy_btn, link_btn):
            local_row.pack_start(button, False, False, 0)
        box.pack_start(local_row, False, False, 0)
        return box

    def _load_config(self) -> None:
        self.hotkey_entry.set_text(self.config.daemon.hotkey)
        _set_combo(self.mode_combo, self.config.daemon.hotkey_mode)
        _set_combo(self.script_combo, self.config.text.script)
        self.semantic_switch.set_active(self.config.text.semantic_correction_enabled)
        self.app_home_entry.set_text(str(app_home()))
        self.app_home_entry.set_sensitive(os.environ.get("VOXFLOW_HOME") is None)
        selected = next((profile for profile in self.models if _model_matches(profile, self.config.asr.backend, self.config.asr.model)), self.models[0])
        self.model_combo.set_active_id(selected.id)
        self._refresh_model_summary()

    def _save_settings(self) -> None:
        try:
            hotkey = self.hotkey_entry.get_text().strip().lower()
            parse_hotkey(hotkey)
            if os.environ.get("VOXFLOW_HOME") is None:
                old_config_path = user_config_path()
                new_home = set_app_home(self.app_home_entry.get_text().strip())
                new_config_path = user_config_path()
                if old_config_path.exists() and old_config_path != new_config_path and not new_config_path.exists():
                    new_config_path.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(old_config_path, new_config_path)
                self.app_home_entry.set_text(str(new_home))
            profile = get_model_profile(self._selected_model_id())
            local_path = model_local_dir(profile.id, model_cache_dir())
            model = str(local_path) if local_path.exists() else profile.model
            if profile.model.startswith("bundled:"):
                model = profile.model
            save_user_asr_settings(backend=profile.backend, model=model)
            save_user_daemon_settings(hotkey=hotkey, hotkey_mode=self._combo_value(self.mode_combo))
            save_user_text_script(self._combo_value(self.script_combo))
            save_user_text_semantic_correction(self.semantic_switch.get_active())
            self.config = load_config()
            if daemon_status().running:
                restart_daemon()
            self._set_message("已保存设置。")
            self._refresh_model_summary()
            self._refresh_daemon_status()
        except Exception as exc:
            self._set_message(str(exc), error=True)

    def _selected_model_id(self) -> str:
        return self.model_combo.get_active_id() or self.models[0].id

    def _combo_value(self, combo: Gtk.ComboBoxText) -> str:
        return combo.get_active_id() or ""

    def _refresh_model_summary(self) -> None:
        profile = get_model_profile(self._selected_model_id())
        local_path = model_local_dir(profile.id, model_cache_dir())
        local_state = "已在本机缓存" if local_path.exists() else "未下载"
        if profile.model.startswith("bundled:"):
            local_state = "内置可用"
        self.model_summary.set_text(
            f"{profile.label} · {profile.size} · {profile.languages}\n"
            f"许可证：{profile.license} · 状态：{local_state}\n"
            f"模型目录：{local_path}"
        )

    def _start_download(self) -> None:
        try:
            status = self.downloads.start(self._selected_model_id(), model_cache_dir())
            self._render_download_status(status)
            self._set_message(f"正在下载：{status.get('label', '')}")
        except Exception as exc:
            self._set_message(str(exc), error=True)

    def _pause_download(self) -> None:
        status = self.downloads.pause()
        self._render_download_status(status)
        self._set_message("已暂停下载，点击“下载/继续”可恢复。")

    def _validate_local_model(self) -> None:
        try:
            result = validate_model_profile(self._selected_model_id(), Path(self.local_path_entry.get_text()).expanduser())
            self._set_message(f"校验通过：{result.path}，已检查 {len(result.checked_files)} 个文件。")
        except Exception as exc:
            self._set_message(str(exc), error=True)

    def _import_local_model(self, *, symlink: bool) -> None:
        try:
            profile = get_model_profile(self._selected_model_id())
            imported = import_model_profile(profile.id, Path(self.local_path_entry.get_text()).expanduser(), model_cache_dir(), symlink=symlink)
            save_user_asr_settings(backend=profile.backend, model=str(imported))
            self.config = load_config()
            self._set_message(("已软链接导入：" if symlink else "已复制导入：") + str(imported))
            self._refresh_model_summary()
        except Exception as exc:
            self._set_message(str(exc), error=True)

    def _run_daemon_action(self, action: object, message: str) -> None:
        try:
            action()
            self._set_message(message)
            self._refresh_daemon_status()
        except Exception as exc:
            self._set_message(str(exc), error=True)

    def _refresh_daemon_status(self) -> None:
        status = daemon_status()
        if status.running:
            self.daemon_label.set_text(f"后台状态：已启动 ({status.pid})")
            self.start_btn.set_sensitive(False)
            self.stop_btn.set_sensitive(True)
        else:
            self.daemon_label.set_text("后台状态：未启动")
            self.start_btn.set_sensitive(True)
            self.stop_btn.set_sensitive(False)

    def _tick(self) -> bool:
        self._refresh_daemon_status()
        self._render_download_status(self.downloads.status())
        return True

    def _render_download_status(self, status: dict[str, object]) -> None:
        state = str(status.get("status", "idle"))
        done = int(status.get("bytes", 0) or 0)
        total = int(status.get("total_bytes", 0) or 0)
        fraction = min(1.0, done / total) if total else 0.0
        self.progress.set_fraction(fraction)
        self.pause_btn.set_sensitive(state == "downloading")
        self.download_btn.set_sensitive(state not in {"downloading", "pausing"})
        if state == "idle":
            self.progress_label.set_text("未下载")
            return
        speed = float(status.get("speed_bps", 0.0) or 0.0)
        text = f"{_status_label(state)} · {_format_bytes(done)} / {_format_bytes(total)}"
        if state == "downloading":
            text += f" · {_format_bytes(speed)}/s"
        error = str(status.get("error", "") or "")
        if error:
            text += f" · {error}"
        self.progress_label.set_text(text)

    def _set_message(self, text: str, *, error: bool = False) -> None:
        self.message.set_text(text)
        context = self.message.get_style_context()
        if error:
            context.add_class("error")
        else:
            context.remove_class("error")


def _section_title(text: str) -> Gtk.Label:
    label = Gtk.Label(label=text)
    label.set_xalign(0)
    label.get_style_context().add_class("section-title")
    return label


def _attach_labeled(grid: Gtk.Grid, left: int, top: int, label: str, widget: Gtk.Widget) -> None:
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
    title = Gtk.Label(label=label)
    title.set_xalign(0)
    title.get_style_context().add_class("muted")
    box.pack_start(title, False, False, 0)
    box.pack_start(widget, False, False, 0)
    grid.attach(box, left, top, 1, 1)


def _combo(items: list[tuple[str, str]]) -> Gtk.ComboBoxText:
    combo = Gtk.ComboBoxText()
    for value, label in items:
        combo.append(value, label)
    return combo


def _set_combo(combo: Gtk.ComboBoxText, value: str) -> None:
    combo.set_active_id(value)
    if combo.get_active_id() is None:
        combo.set_active(0)


def _model_matches(profile: object, backend: str, model: str) -> bool:
    profile_backend = getattr(profile, "backend")
    profile_model = getattr(profile, "model")
    if profile_backend != backend:
        return False
    if profile_model == model:
        return True
    name = profile_model.split("/")[-1]
    return bool(name and model.endswith(f"/{name}"))


def _format_bytes(value: float) -> str:
    units = ["B", "KB", "MB", "GB", "TB"]
    amount = float(value)
    for unit in units:
        if amount < 1024 or unit == units[-1]:
            return f"{amount:.1f} {unit}" if unit != "B" else f"{amount:.0f} B"
        amount /= 1024
    return f"{amount:.1f} TB"


def _status_label(value: str) -> str:
    return {
        "downloading": "下载中",
        "pausing": "暂停中",
        "paused": "已暂停",
        "completed": "已完成",
        "failed": "失败",
    }.get(value, value)


def main(argv: list[str] | None = None) -> int:
    args = argv or sys.argv[1:]
    smoke_test = "--smoke-test" in args
    start_daemon_on_open = "--no-start-daemon" not in args and not smoke_test
    window = VoxFlowWindow(start_daemon_on_open=start_daemon_on_open)
    destroy_handler = window.connect("destroy", Gtk.main_quit)
    window.show_all()
    if smoke_test:
        GLib.timeout_add(500, Gtk.main_quit)
    Gtk.main()
    if smoke_test:
        window.disconnect(destroy_handler)
        window.destroy()
        print("native_gui_window_smoke_ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
