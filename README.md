# 声流输入法 / VoxFlow Input

用户友好的 Linux 中文/英文语音输入法。项目提供后台快捷键、命令行和本地可视化控制台，支持口语助词清理、“哦不/不是/撤回”等自然修正、自动标点、可插拔 ASR 后端和 Linux 系统文本注入。

## 当前能力

- 中文/英文 ASR 后端：`faster-whisper`、`qwen-asr`、OpenAI 兼容 `/v1/audio/transcriptions` 服务。
- 文本后处理：删除高置信口语助词，自动补句末标点，支持“哦不”“不对”“不是”“撤回刚才”等修正命令。
- Linux 输入：自动选择 `wtype`、`xdotool` 或 `ydotool`，也支持 dry-run 预览。
- 后台快捷键：X11 下支持 `Ctrl+Alt+Space` 触发一次录音识别，并把文本输入到当前光标所在窗口。
- 可视化控制台：浏览器录音、实时波形、识别结果、处理结果、退格/插入动作列表、文本修正测试。
- 轻量默认：核心包不强制安装大模型依赖，按实际后端安装 extras。

## 快速开始

### Debian/Ubuntu 安装包

构建完整 deb 包：

```bash
scripts/build_deb.sh
```

安装：

```bash
sudo apt install ./dist/local-speak-input_0.1.1_amd64.deb
```

deb 包内置 Python 依赖和默认 `faster-whisper-base` 模型；系统级依赖由 apt 自动安装。安装后启动：

```bash
local-speak-gui
```

也可以在应用菜单中打开“声流输入法”。这个入口会自动启动后台快捷键服务、右上角控制图标，并打开控制台。控制台会显示录音按钮、实时波形、识别结果、口语修正结果和快捷键设置，适合第一次确认麦克风、模型和文本处理是否正常。

后台快捷键语音输入：

```bash
local-speak-daemon
```

默认快捷键是 `Ctrl+Alt+Space`。把光标放到终端、浏览器搜索框、编辑器等目标输入框，按快捷键开始一次语音输入。录音、识别、完成和异常状态会通过桌面通知提示；如果 8 秒内没有检测到语音，会退出本次录音并提示检查默认麦克风或降低能量阈值。快捷键可以在控制台“快捷键”设置中修改，保存后会写入用户配置并重启正在运行的后台输入服务。

如果从应用菜单打开后快捷键没有反应，先检查后台日志：

```bash
tail -n 80 ~/.local/state/local-speak-input/daemon.log
```

右上角图标提供“打开控制台”“启动后台输入”“停止后台输入”“重启后台输入”“打开日志目录”和“退出图标”。

诊断和一次性听写：

```bash
local-speak doctor
local-speak dictate --once
```
后台服务也可以用 systemd 用户服务常驻：

```bash
systemctl --user enable --now local-speak-input.service
```

### 开发环境

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -e '.[whisper,mic,dev]'
```

检查环境：

```bash
local-speak doctor
```

启动可视化控制台：

```bash
local-speak gui --backend faster-whisper --model large-v3 --dry-run
```

打开终端输出的本地地址，默认是 `http://127.0.0.1:8765`。

识别文件：

```bash
local-speak transcribe sample.wav --backend faster-whisper --model large-v3
```

连续听写并输入到当前焦点窗口：

```bash
local-speak dictate --backend faster-whisper --model large-v3
```

## 配置

复制配置模板：

```bash
mkdir -p ~/.config/local-speak-input
cp config.example.toml ~/.config/local-speak-input/config.toml
```

OpenAI 兼容 ASR 服务示例：

```toml
[asr]
backend = "openai-compatible"
api_base = "http://127.0.0.1:8000/v1"
api_model = "Qwen/Qwen3-ASR-1.7B"
api_key_env = "OPENAI_API_KEY"
```

## Linux 输入依赖

Wayland 优先安装 `wtype`，X11 优先安装 `xdotool`。如果两者不可用，可以配置 `ydotool`，但通常需要守护进程和输入设备权限。

## 模型选择

第一版推荐：

- 准确率和中英混输优先：`Qwen/Qwen3-ASR-1.7B`，本地 GPU 或 vLLM/OpenAI 兼容服务。
- 轻量和低延迟优先：`Qwen/Qwen3-ASR-0.6B` 或 `faster-whisper` 的 `large-v3-turbo`。
- 中文标点优先且继续使用 Whisper：`k1nto/Belle-whisper-large-v3-zh-punct-ct2`。
- 工业化本地服务：FunASR/SenseVoice，适合 VAD、标点、ITN、长音频服务化。
- 云 API 候选：Qwen3-ASR-Flash/DashScope、火山引擎豆包 ASR、NVIDIA NIM Parakeet。正式采用前需要用你的口音、麦克风和常用术语集实测。

更完整调研见 [docs/model-research.md](docs/model-research.md)。

## 测试

```bash
pip install -e '.[dev]'
pytest -q
```

当前环境没有预装 `pytest` 时，可以先运行：

```bash
python3 -m compileall src tests
```

## 开发流程

项目已按功能分支开发，当前分支应保持在 `feature/initial-implementation`。不要直接在 `main` 上开发；完成本地验证后再由你决定提交到 GitHub。
