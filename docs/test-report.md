# 测试报告

测试日期：2026-06-10
测试系统：Ubuntu 24.04 / Linux 6.17 / amd64 / X11 / PipeWire

## 结果摘要

- Python 编译检查通过：`python -m compileall src tests`。
- Python 单元测试通过：`17 passed, 1 warning`，覆盖后处理、快捷键、用户快捷键配置写入、版本一致性、输入注入 dry-run、录音未检测到语音超时、桌面通知超时容错。
- deb 构建通过：生成 `dist/local-speak-input_0.1.1_amd64.deb`，约 212 MB。
- deb 包声明依赖可由 apt 解析；当前机器模拟安装只升级 `local-speak-input`。
- deb 包内包含 Python 运行环境、默认 `faster-whisper-base` 模型、GUI 内联 logo、系统 SVG 图标、桌面入口、右上角 AppIndicator 控制入口、AppStream 元数据、systemd 用户服务、系统默认配置。
- staged 包内 GUI HTTP 首页、`/logo.svg`、`/api/config`、`/api/process-text`、快捷键设置接口通过。
- 右上角图标运行时检查通过：`local-speak doctor` 显示 `右上角图标: OK`；真实桌面会话中托盘进程可启动并拉起 daemon。
- X11 daemon 快捷键触发通过：`xdotool key ctrl+alt+space` 可触发录音流程。
- 原问题已回归验证：未检测到语音时不再卡在“开始录音”，8 秒后输出明确提示。
- `notify-send` 超时时不再导致 daemon 崩溃。
- 控制台图片修复已验证：主页面使用内联 SVG logo，`/logo.svg` GET/HEAD 返回 `Content-Type: image/svg+xml`。
- X11 中文注入既往真实测试通过：`zenity` 临时输入框收到 `语音输入测试。`。

## 执行项目

### 单元测试

```bash
.venv/bin/python -m compileall src tests
.venv/bin/pytest -q
```

结果：

```text
17 passed, 1 warning in 0.04s
```

覆盖：

- 口语修正
- 自动标点
- 输入注入 dry-run
- X11 快捷键解析
- 用户级快捷键配置写入和非法快捷键拒绝
- 录音未检测到语音时超时退出
- `notify-send` 超时容错
- `pyproject.toml` 与 CLI 包版本一致

### 环境诊断

```bash
.venv/bin/local-speak doctor
```

结果摘要：

```text
Python 模块 faster_whisper: OK
Python 模块 qwen_asr: 未安装
Python 模块 sounddevice: OK
X11 全局快捷键: OK
右上角图标: OK
命令 notify-send: /usr/bin/notify-send
命令 xdotool: /usr/bin/xdotool
命令 xclip: /usr/bin/xclip
命令 pw-record: /usr/bin/pw-record
命令 ffmpeg: /usr/bin/ffmpeg
```

### 打包

```bash
scripts/build_deb.sh
```

结果：

```text
Built /home/terry/workplace/local_speak_input/dist/local-speak-input_0.1.1_amd64.deb
```

### 包元数据

```bash
dpkg-deb --info dist/local-speak-input_0.1.1_amd64.deb
desktop-file-validate build/deb/rootfs/usr/share/applications/local-speak-input.desktop
```

结果：通过。`Depends` 包含：

```text
python3 (>= 3.11), python3-gi, gir1.2-gtk-3.0, gir1.2-appindicator3-0.1, gnome-shell-extension-appindicator, libportaudio2, libx11-6, libnotify-bin, ffmpeg, pipewire-bin, wireplumber, xdotool, xclip, xdg-utils, ca-certificates
```

`appstreamcli validate build/deb/rootfs/usr/share/metainfo/local-speak-input.metainfo.xml` 只剩一个非运行时警告：当前仓库没有公开 homepage URL，因此没有伪造链接写入元数据。

### apt 模拟安装

```bash
apt-get -s install ./dist/local-speak-input_0.1.1_amd64.deb
```

当前机器结果：

```text
The following packages will be upgraded:
  local-speak-input
1 upgraded, 0 newly installed, 0 to remove and 9 not upgraded.
```

### 包内容检查

```bash
dpkg-deb --contents dist/local-speak-input_0.1.1_amd64.deb | rg 'local-speak-input.desktop|local-speak-input.metainfo.xml|local-speak-input.svg|logo.svg|local-speak-daemon|local-speak-tray|model.bin'
```

结果确认包含：

- `/usr/share/applications/local-speak-input.desktop`
- `/usr/share/icons/hicolor/scalable/apps/local-speak-input.svg`
- `/usr/share/metainfo/local-speak-input.metainfo.xml`
- `/usr/bin/local-speak-daemon`
- `/usr/bin/local-speak-tray`
- `/opt/local-speak-input/venv/lib/python3.12/site-packages/local_speak_input/web/logo.svg`
- `/opt/local-speak-input/models/faster-whisper-base/model.bin`

### 路径污染检查

```bash
rg -a -n "/home/terry|workplace/local_speak_input|build/deb" \
  build/deb/rootfs/opt/local-speak-input/venv \
  build/deb/rootfs/usr/bin \
  build/deb/rootfs/etc/local-speak-input \
  build/deb/rootfs/usr/share/applications \
  build/deb/rootfs/usr/share/metainfo
```

结果：无匹配，包内运行入口没有引用开发目录。

### GUI 接口

```bash
build/deb/rootfs/opt/local-speak-input/venv/bin/python -m local_speak_input \
  gui --host 127.0.0.1 --port 8773 --dry-run \
  --config build/deb/rootfs/etc/local-speak-input/config.toml
curl -fsS http://127.0.0.1:8773/
curl -fsSI http://127.0.0.1:8773/logo.svg
curl -fsS http://127.0.0.1:8773/api/config
curl -fsS -X POST http://127.0.0.1:8773/api/process-text \
  -H 'Content-Type: application/json' \
  -d '{"text":"今天下午三点哦不四点","inject":false}'
curl -fsS -X POST http://127.0.0.1:8773/api/settings/hotkey \
  -H 'Content-Type: application/json' \
  -d '{"hotkey":"ctrl+shift+space","restart":false}'
```

关键结果：

```json
{"raw_text":"今天下午三点哦不四点","processed_text":"今天下午四点。"}
```

首页包含“声流输入法”“VoxFlow Input”“Ctrl+Alt+Space”“local-speak-daemon”，logo SVG 可正常返回。

快捷键设置结果：

```json
{"hotkey":"ctrl+shift+space","daemon_restarted":false}
```

非法快捷键会返回 400：

```json
{"error":"快捷键必须包含且只包含一个普通按键：ctrl+a+b"}
```

### 右上角图标

```bash
env PYTHONPATH=src LOCAL_SPEAK_PYTHON=.venv/bin/python python3 -m local_speak_input.tray
```

结果：AppIndicator 进程在真实 X11/GNOME 会话中保持运行，并自动启动后台 daemon。Ctrl+C 可干净退出托盘测试进程；测试后通过 `service_control.stop_daemon()` 停止自动拉起的 daemon。

### daemon 快捷键与无语音超时

```bash
.venv/bin/local-speak daemon \
  --model build/deb/rootfs/opt/local-speak-input/models/faster-whisper-base \
  --device cpu --compute-type int8 --dry-run
xdotool key ctrl+alt+space
```

结果：

```text
[local-speak-daemon] 后台语音输入已启动，快捷键：ctrl+alt+space
[local-speak-daemon] 把光标放在目标输入框，按快捷键开始一次语音输入。
[local-speak-daemon] 开始录音...
[local-speak-daemon] 8 秒内没有检测到语音，请检查默认麦克风或降低 energy_threshold。
```

Ctrl+C 停止结果：

```text
已停止后台语音输入。
```

## 未执行项

没有在当前系统执行真实 `sudo apt install ./dist/...deb`，因为 sudo 需要用户密码。已用 apt 模拟安装、staged rootfs 运行验证、真实 X11 快捷键触发和既往真实 X11 中文输入框注入覆盖主要链路。
