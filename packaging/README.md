# 打包目录

新版打包以 `docs/redesign/engineering/packaging-release.md` 为准。

阶段 0 已删除旧 Python/venv 打包路径。后续阶段 7 需要在本目录落地:

- Linux deb:单包包含 `voxflow-core`、IBus 前端、Fcitx5 addon、Tauri 控制台、systemd user unit。
- portable tar:可解压任意目录运行,附 `install-desktop`、`install-ibus`、`install-fcitx5`、`uninstall`。
- 包测试矩阵:Ubuntu 22.04、Ubuntu 24.04、Debian 12。

大模型、日志、缓存和用户配置不得放入 `/usr` 或 `/opt`,必须进入 `~/.voxflow/` 或 `$VOXFLOW_HOME`。
