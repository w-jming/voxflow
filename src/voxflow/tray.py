from __future__ import annotations

import subprocess
import sys

from . import service_control


def main() -> int:
    try:
        import gi

        gi.require_version("Gtk", "3.0")
        try:
            gi.require_version("AyatanaAppIndicator3", "0.1")
            from gi.repository import AyatanaAppIndicator3 as AppIndicator
        except (ImportError, ValueError):
            gi.require_version("AppIndicator3", "0.1")
            from gi.repository import AppIndicator3 as AppIndicator
        from gi.repository import GLib, Gtk
    except Exception as exc:
        print(f"无法启动右上角图标：缺少 GTK/AppIndicator 运行时：{exc}", file=sys.stderr)
        return 1

    indicator = AppIndicator.Indicator.new(
        "voxflow",
        "voxflow",
        AppIndicator.IndicatorCategory.APPLICATION_STATUS,
    )
    indicator.set_status(AppIndicator.IndicatorStatus.ACTIVE)
    indicator.set_title("声流输入法")

    menu = Gtk.Menu()
    status_item = Gtk.MenuItem(label="状态：读取中")
    status_item.set_sensitive(False)
    open_item = Gtk.MenuItem(label="打开控制台")
    start_item = Gtk.MenuItem(label="启动后台输入")
    stop_item = Gtk.MenuItem(label="停止后台输入")
    restart_item = Gtk.MenuItem(label="重启后台输入")
    log_item = Gtk.MenuItem(label="打开日志目录")
    quit_item = Gtk.MenuItem(label="退出 VoxFlow")

    for item in [status_item, open_item, start_item, stop_item, restart_item, log_item, quit_item]:
        menu.append(item)

    def refresh() -> bool:
        status = service_control.daemon_status()
        if status.running:
            status_item.set_label(f"状态：后台输入已启动 ({status.pid})")
            start_item.set_sensitive(False)
            stop_item.set_sensitive(True)
        else:
            status_item.set_label("状态：后台输入未启动")
            start_item.set_sensitive(True)
            stop_item.set_sensitive(False)
        return True

    def open_console(_item: object) -> None:
        subprocess.Popen(["voxflow-gui"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        refresh()

    def start_daemon(_item: object) -> None:
        service_control.start_daemon()
        refresh()

    def stop_daemon(_item: object) -> None:
        service_control.stop_daemon()
        refresh()

    def restart_daemon(_item: object) -> None:
        service_control.restart_daemon()
        refresh()

    def open_logs(_item: object) -> None:
        service_control.open_state_dir()

    def quit_tray(_item: object) -> None:
        service_control.stop_daemon()
        service_control.stop_gui()
        Gtk.main_quit()

    open_item.connect("activate", open_console)
    start_item.connect("activate", start_daemon)
    stop_item.connect("activate", stop_daemon)
    restart_item.connect("activate", restart_daemon)
    log_item.connect("activate", open_logs)
    quit_item.connect("activate", quit_tray)

    menu.show_all()
    indicator.set_menu(menu)
    service_control.start_daemon()
    refresh()
    GLib.timeout_add_seconds(2, refresh)
    try:
        Gtk.main()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
