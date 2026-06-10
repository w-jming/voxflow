# Debian 打包说明

VoxFlow deb 包面向 Ubuntu/Debian amd64 桌面环境，安装后提供：

- `/usr/bin/voxflow` 命令行入口。
- `/usr/bin/voxflow-gui` 桌面控制台入口。
- `/usr/bin/voxflow-daemon` 后台快捷键输入入口。
- `/usr/bin/voxflow-tray` 右上角控制图标入口。
- `/opt/voxflow/venv` 内置 Python 运行环境和 Python 依赖。
- `/opt/voxflow/models/<model>` 打包模型。
- `/etc/voxflow/config.toml` 系统默认配置。
- `/usr/share/applications/voxflow.desktop` 应用菜单入口。

系统级依赖由 deb `Depends` 声明，apt 会自动处理。

## 构建

默认构建本机高准确率包，下载并打包 Qwen3-ASR 1.7B：

```bash
scripts/build_deb.sh
```

输出：

```text
dist/voxflow_0.2.0_amd64.deb
```

构建轻量包：

```bash
VOXFLOW_BUNDLE_PROFILE=bundled-faster-whisper-tiny scripts/build_deb.sh
```

构建 Qwen3-ASR 0.6B 包：

```bash
VOXFLOW_BUNDLE_PROFILE=qwen3-asr-0.6b scripts/build_deb.sh
```

## 安装与启动

```bash
sudo apt install ./dist/voxflow_0.2.0_amd64.deb
voxflow doctor
voxflow-gui
```

应用菜单入口会自动启动 GUI、后台 daemon 和右上角控制图标。默认快捷键是 `Ctrl+Space`，默认录音模式是按一次开始、再按一次停止。

## 配置

系统默认配置：

```text
/etc/voxflow/config.toml
```

用户级覆盖配置：

```text
~/.config/voxflow/config.toml
```

配置加载顺序：内置默认值 -> 系统配置 -> 用户配置 -> CLI 参数。

控制台会写入用户级配置，保存快捷键、录音模式、输出字形和模型选择，并在后台服务运行时自动重启 daemon。
