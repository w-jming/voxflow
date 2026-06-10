# Debian 打包说明

## 目标

`声流输入法 / VoxFlow Input` 的 deb 包面向 Ubuntu/Debian amd64 桌面环境，提供完整可运行的软件安装：

- `/usr/bin/local-speak` 命令行入口。
- `/usr/bin/local-speak-gui` 桌面控制台入口。
- `/usr/bin/local-speak-daemon` 后台快捷键输入入口。
- `/usr/bin/local-speak-tray` 右上角控制图标入口。
- `/opt/local-speak-input/venv` 内置 Python 运行环境和 Python 依赖。
- `/opt/local-speak-input/models/faster-whisper-base` 内置默认 ASR 模型。
- `/etc/local-speak-input/config.toml` 系统默认配置。
- `/usr/share/applications/local-speak-input.desktop` 应用菜单入口。

系统级依赖由 deb `Depends` 声明，用户通过 apt 安装 deb 时会自动处理，不需要逐个手动安装：

- `python3 >= 3.11`
- `python3-gi`
- `gir1.2-gtk-3.0`
- `gir1.2-appindicator3-0.1`
- `gnome-shell-extension-appindicator`
- `libportaudio2`
- `libx11-6`
- `ffmpeg`
- `pipewire-bin`
- `wireplumber`
- `xdotool`
- `xclip`
- `libnotify-bin`
- `xdg-utils`
- `ca-certificates`

## 构建

```bash
scripts/build_deb.sh
```

输出：

```text
dist/local-speak-input_0.1.1_amd64.deb
```

默认内置 `Systran/faster-whisper-base`。可通过环境变量替换内置模型：

```bash
LOCAL_SPEAK_BUNDLE_MODEL_REPO=Systran/faster-whisper-large-v3-turbo \
LOCAL_SPEAK_BUNDLE_MODEL_NAME=faster-whisper-large-v3-turbo \
scripts/build_deb.sh
```

## 安装

```bash
sudo apt install ./dist/local-speak-input_0.1.1_amd64.deb
```

安装后：

```bash
local-speak doctor
local-speak-gui
local-speak-daemon
```

也可以从桌面应用菜单启动“声流输入法”。应用菜单入口会自动启动 GUI、后台 daemon 和右上角控制图标。后台 daemon 会通过 `notify-send` 显示“开始录音”“正在识别”“已输入”和错误提示，用户不需要盯着终端判断快捷键是否生效。

启用后台服务：

```bash
systemctl --user enable --now local-speak-input.service
```

## 配置

系统默认配置在：

```text
/etc/local-speak-input/config.toml
```

用户级覆盖配置在：

```text
~/.config/local-speak-input/config.toml
```

配置加载顺序：内置默认值 -> 系统配置 -> 用户配置 -> CLI 参数。
