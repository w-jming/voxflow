# Linux 使用说明

## 输入工具

Wayland：

```bash
sudo apt install wtype wl-clipboard
```

X11：

```bash
sudo apt install xdotool xclip
```

通用但配置较重：

```bash
sudo apt install ydotool
```

## 麦克风录音

安装 `sounddevice` extra：

```bash
pip install -e '.[mic]'
```

如果录音设备不可用，先检查 PipeWire/PulseAudio 和浏览器麦克风权限。可视化控制台通过浏览器录音，`dictate` 命令通过 Python 本地录音。

`dictate` 优先使用 `sounddevice + PortAudio` 做能量阈值 VAD。如果系统没有 PortAudio，但安装了 `pw-record`，项目会自动降级为 PipeWire 固定窗口录音。Ubuntu 上安装 PortAudio：

```bash
sudo apt install libportaudio2
```

如果暂时不能安装这个系统库，仍可使用：

```bash
local-speak dictate --once
```

但每次会按 `audio.max_utterance_s` 录满一段，默认 15 秒。

## 蓝牙耳机麦克风

蓝牙耳机常见失败原因是只连接了 `A2DP` 高音质播放模式。该模式只提供输出，不提供麦克风输入；需要切到 `HSP/HFP` Headset 模式。

检查当前设备：

```bash
wpctl status
bluetoothctl devices Connected
```

如果耳机只出现在 `Sinks`，没有出现在 `Sources`，说明麦克风没有暴露给系统。查看可用 profile：

```bash
pw-cli enum-params <device-id> EnumProfile
```

切换到带麦克风的 profile，例如：

```bash
wpctl set-profile <device-id> <profile-index>
```

本机 `OpenFit by Shokz` 的一次实测中，设备 id 是 `73`，`headset-head-unit-msbc` profile index 是 `261`：

```bash
wpctl set-profile 73 261
```

切换后 `wpctl status` 应该能看到 `OpenFit by Shokz` 同时出现在 `Sinks` 和 `Sources`。注意：HSP/HFP 会降低耳机播放音质，这是蓝牙协议限制。

## 常用命令

启动控制台。安装 deb 后也可以从应用菜单打开“声流输入法”；应用菜单入口会自动拉起后台快捷键服务和右上角控制图标：

```bash
local-speak-gui
```

只录一段并输入：

```bash
local-speak dictate --once
```

只测试口语修正，不加载 ASR：

```bash
local-speak gui --dry-run
```

打开页面后使用“口语修正测试”文本框。

## 后台快捷键输入

X11 桌面下可以启动后台服务：

```bash
local-speak-daemon
```

如果是从应用菜单或 `local-speak-gui` 打开的，后台服务会自动启动，不需要再手动执行上面的命令。日志位置：

```bash
~/.local/state/local-speak-input/daemon.log
```

默认快捷键：

```text
Ctrl+Alt+Space
```

使用流程：

1. 把光标放到任意普通文本输入框。
2. 按 `Ctrl+Alt+Space`。
3. 说完一句话后等待识别和输入。

后台服务会在录音前记录当前活动窗口，识别后恢复该窗口并输入文本，避免浏览器控制台抢焦点。通过 systemd 用户服务常驻：

```bash
systemctl --user enable --now local-speak-input.service
```

后台服务会显示桌面通知：就绪、开始录音、正在识别、已输入、未检测到语音或失败。如果按快捷键后 8 秒内没有声音超过阈值，本次录音会自动结束并提示检查默认麦克风，不会一直卡在“开始录音”。安装 deb 时会自动安装 `libnotify-bin` 来提供通知命令。

快捷键可以在控制台的“快捷键”输入框中修改，例如：

```text
ctrl+shift+space
ctrl+alt+return
super+space
```

保存后会写入 `~/.config/local-speak-input/config.toml`，并重启正在运行的后台输入服务。右上角图标可以打开控制台、启动/停止/重启后台输入、打开日志目录或只退出图标本身。

Wayland 对全局快捷键和模拟输入限制较多；当前版本优先保证 X11 可用。

## 中文注入说明

Wayland 下优先使用 `wl-copy + wtype` 粘贴文本，X11 下优先使用 `xclip/xsel + xdotool` 粘贴文本，这比逐键模拟更适合中文。该方式会临时覆盖系统剪贴板；如果你更看重不动剪贴板，可以在配置里改成 `wtype` 或 `xdotool` 直接键入，但 X11 中文可靠性可能下降。
