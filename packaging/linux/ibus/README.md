# IBus 前端打包占位

阶段 2 的 IBus POC 以 `crates/voxflow-ibus` 为实现入口。当前目录保存 component XML 模板,用于后续 deb 与 portable 安装脚本。

安装目标按 `docs/redesign/platforms/linux.md`:

- deb: `/usr/share/ibus/component/voxflow.xml`,引擎可执行 `/usr/lib/voxflow/voxflow-ibus`
- portable: `~/.local/share/ibus/component/voxflow.xml`,引擎可执行为 portable 包内路径

安装后需要执行:

```bash
ibus write-cache
ibus restart
```

用户级安装 smoke:

```bash
packaging/linux/ibus/install-ibus-user.sh /absolute/path/to/voxflow-ibus
packaging/linux/ibus/uninstall-ibus-user.sh
packaging/linux/ibus/smoke-private-ibus.sh /absolute/path/to/voxflow-ibus
```

当前 POC 已覆盖 component XML 生成、前端能力注册 JSON、dictation partial/stable/final 到 IBus 操作的无桌面自动测试、Core UDS 订阅式 focus smoke、session D-Bus Factory 注册 probe、Factory `CreateEngine("voxflow")` 动态 Engine smoke、IBus preedit/commit/delete signal 暴露、临时用户数据目录下的 install/uninstall smoke、私有 `dbus-run-session` + 临时 `ibus-daemon` component cache/拉起 smoke。真实 GNOME Wayland/X11 应用内 preedit/commit/delete 行为仍是阶段 2 人工验证点。
