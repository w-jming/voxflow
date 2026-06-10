# 声流输入法 / VoxFlow Input

VoxFlow 是 Linux 桌面中文/英文语音输入法。它提供桌面启动器、右上角控制图标、本地 Web 控制台、全局快捷键、自动标点、口语修正、简体/繁体输出设置和可切换 ASR 模型。

## 安装

Debian/Ubuntu 用户安装本机打包产物：

```bash
sudo apt install ./dist/voxflow_0.2.0_amd64.deb
```

安装后从应用菜单打开“声流输入法”，或运行：

```bash
voxflow-gui
```

这个入口会启动后台输入服务、右上角图标和本地控制台。

也可以使用解压版安装到任意目录，例如 `/data/apps/voxflow`：

```bash
mkdir -p /data/apps
tar -xzf dist/voxflow-0.2.0_amd64.tar.gz -C /data/apps
/data/apps/voxflow-0.2.0/bin/voxflow doctor
/data/apps/voxflow-0.2.0/bin/voxflow-gui
```

可选安装用户级桌面入口和 IBus component：

```bash
/data/apps/voxflow-0.2.0/bin/voxflow-install-desktop
/data/apps/voxflow-0.2.0/bin/voxflow-install-ibus
```

## 使用

默认快捷键是 `Ctrl+Space`，默认模式是“按一次开始录音，再按一次停止并输入”。把光标放到终端、浏览器搜索框、编辑器或聊天窗口后按快捷键即可输入。

控制台可以设置：

- 快捷键。
- 录音模式：按键切换，或按住录音。
- 输出字形：简体中文、繁体中文、模型原文。
- 语义撤销识别：可开启或关闭，并显示当前可用/计划中的语义意图后端。
- 语音模型：内置轻量模型、Qwen3-ASR 0.6B、Qwen3-ASR 1.7B。

右上角图标可以打开控制台、启动/停止/重启后台输入、打开日志目录，以及退出 VoxFlow。退出 VoxFlow 会停止后台输入服务。

## 安装位置与数据目录

`apt install` 会把程序本体安装到系统目录，包括 `/usr/bin`、`/usr/share`、`/etc/voxflow` 和 `/opt/voxflow/venv`，因此会占用 `/` 所在分区的少量软件运行空间。大体积和会增长的数据不放进系统目录。

VoxFlow 的用户数据根目录由 `VOXFLOW_HOME` 控制，默认是：

```text
~/.voxflow/
```

其中：

- `~/.voxflow/config.toml`：用户设置。
- `~/.voxflow/models/`：下载的 Qwen3-ASR 等大模型。
- `~/.voxflow/logs/`：GUI、daemon、tray 日志。
- `~/.voxflow/run/`：pid 等运行状态文件。
- `~/.voxflow/cache/`：预留缓存目录。

自定义目录示例：

```bash
export VOXFLOW_HOME=/data/voxflow
voxflow-gui
```

## 系统输入法模式

安装 deb 后会注册 IBus 引擎 `VoxFlow Input`。在 GNOME/KDE 的输入源设置里添加 VoxFlow 后，VoxFlow 可以作为系统输入法工作：

- 临时识别文本显示为 IBus preedit composition。
- 稳定文本通过 IBus commit 写入当前光标处。
- “不对 / 错了 / 不是”等修正会根据 VoxFlow 自己的提交账本撤销相关片段，不删除用户手动输入内容。
- 选中 VoxFlow 输入法后可持续听写；切走输入法或焦点离开时停止。

旧的快捷键 daemon 仍保留，适合不想修改系统输入源时使用；系统输入法模式更适合长时间连续输入。

## 语义撤销

VoxFlow 默认启用规则状态机 + 注入账本的语义撤销。设置里可以关闭该功能，关闭后“不是”“不对”“撤回”等都会按普通文本处理。

语义意图后端采用可插拔设计：当前可用默认档是规则引擎；MiniLM/SetFit、Qwen3-Embedding 0.6B 分类头和低置信 LLM 仲裁在界面中作为计划后端展示，训练/安装前不可选择。任何后端都只能提出撤销建议，真正删除必须经过 VoxFlow 注入账本验证。

## 模型

源码仓库内置 `Systran/faster-whisper-tiny` 作为轻量 fallback，许可证为 MIT，支持中文和英文在内的多语言输入。更高准确率模型可在控制台点击“下载”，也可以用命令行下载并切换：

```bash
voxflow models
voxflow models --download qwen3-asr-0.6b
voxflow models --select qwen3-asr-0.6b
voxflow models --download qwen3-asr-1.7b
voxflow models --select qwen3-asr-1.7b
```

Qwen3-ASR 0.6B/1.7B 使用官方 Hugging Face 模型仓库，许可证为 Apache-2.0。deb 默认不打包这些大模型；点击控制台“下载”或运行 `voxflow models --download ...` 后，模型会进入 `~/.voxflow/models/` 或 `$VOXFLOW_HOME/models/`。

如果本机已经有 Qwen3-ASR 权重，可以先校验它是否为 VoxFlow 支持的官方权重：

```bash
voxflow models --select qwen3-asr-1.7b --validate-model /path/to/Qwen3-ASR-1.7B
```

校验会检查必需文件、`config.json` 架构、safetensors 索引/头部，并对官方 safetensors 权重文件做 SHA256 比对。校验失败时 VoxFlow 不会写入配置。

如果本机已经有 Qwen3-ASR 权重，不需要重复下载。可以把已有目录导入到 VoxFlow 模型目录：

```bash
voxflow models --select qwen3-asr-1.7b --import-model /path/to/Qwen3-ASR-1.7B
```

如果不想复制 4GB+ 权重，可以创建符号链接：

```bash
voxflow models --select qwen3-asr-1.7b --import-model /path/to/Qwen3-ASR-1.7B --symlink
```

本机当前下载目录也可以这样复用：

```bash
voxflow models --select qwen3-asr-1.7b --import-model downloads/models/Qwen3-ASR-1.7B --symlink
```

导入或软链接导入时会自动执行同样的 SHA256 和格式校验；通过后才会把模型路径写入 `~/.voxflow/config.toml`。

## 诊断

检查依赖和运行环境：

```bash
voxflow doctor
```

查看后台日志：

```bash
tail -n 80 ~/.voxflow/logs/daemon.log
```

命令行一次性听写：

```bash
voxflow dictate --once
```

如果使用蓝牙耳机但没有输入，请在系统声音设置里确认蓝牙设备切到 headset / hands-free profile，而不是仅输出的 A2DP profile。

## 开发与测试

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -e '.[mic,dev]'
pytest -q
python3 -m compileall src tests
```

构建 deb：

```bash
scripts/build_deb.sh
```

构建解压版：

```bash
scripts/build_portable.sh
```

默认构建轻量可用包，不把大模型写入 `/opt`。如果只想构建并测试：

```bash
scripts/build_deb.sh
```

## 许可证

VoxFlow 代码使用 MIT 许可证。内置 `Systran/faster-whisper-tiny` 模型使用 MIT 许可证。Qwen3-ASR 模型使用 Apache-2.0 许可证，下载来源为 Qwen 官方 Hugging Face 仓库。
