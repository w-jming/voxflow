# AGENTS.md

## 项目目标

实现 Linux 下轻量级中文/英文语音输入法，优先保证准确率、速度、易用性和可维护性。必须支持口语助词清理、自然撤回/修正、自动标点、可插拔模型、本地部署路径和中文文档。

## 当前决策

- 开发分支：`feature/initial-implementation`，不要直接在 `main` 上开发。
- 核心语言：Python 3.11+。
- 默认后端：`faster-whisper`，因为成熟、部署快、适合作为第一版 fallback。
- 高准确率候选：Qwen3-ASR，本地 GPU 或 OpenAI 兼容服务优先。
- 工业化本地候选：FunASR/SenseVoice。
- 可视化：`local-speak gui` 本地 Web 控制台，保持轻量，不引入前端构建链。

## 本机诊断记录

- 2026-06-06：`OpenFit by Shokz` 蓝牙耳机已连接，但默认是 `a2dp-sink`，只提供输出，没有输入源。
- 同日用 `wpctl set-profile 73 261` 切到 `headset-head-unit-msbc` 后，PipeWire 出现 `bluez_input.A8_F5_E1_AF_A0_C1.0`，并成功录制 `/tmp/openfit-test.wav`：16-bit mono 16000 Hz，约 3 秒。
- 2026-06-06：已用 `uv` 创建 `.venv`，安装 `faster-whisper`、`sounddevice`、`pytest` 等依赖；`qwen_asr` 仍未安装，因为它是可选高准确率后端。
- 当前系统有 `xdotool`、`xclip`、`pw-record`；没有 `libportaudio2`，所以 `sounddevice` 已安装但不可用。代码已增加 `pw-record` fallback，`local-speak dictate` 可在 PipeWire 默认输入源上固定窗口录音。
- 2026-06-06：已产出 deb 包 `dist/local-speak-input_0.1.0_amd64.deb`，内置 Python 依赖和 `Systran/faster-whisper-base` 模型。`apt-get -s install ./dist/local-speak-input_0.1.0_amd64.deb` 显示只需新增 `libportaudio2` 和本包，不会改动 NVIDIA 驱动。
- 2026-06-06：第二阶段新增 `local-speak daemon` / `local-speak-daemon`，X11 默认全局快捷键 `Ctrl+Alt+Space`。真实测试通过：XGrabKey 注册、xdotool 触发 daemon、pw-record 录音、ASR 静音处理、Ctrl+C 停止、zenity 输入框中文注入。
- 2026-06-10：`0.1.1` 新增 `local-speak-tray` 右上角 AppIndicator 控制图标，应用菜单入口会自动启动 GUI、后台 daemon 和托盘图标；控制台支持自定义快捷键，写入用户级 `~/.config/local-speak-input/config.toml` 并可重启 daemon 生效。
- 2026-06-10：控制台 logo 改为内联 SVG，并为 `/logo.svg` 明确返回 `image/svg+xml` 和支持 HEAD 请求，避免浏览器出现图片加载失败占位。
- 2026-06-10：提交到远程前必须执行 secret 扫描，禁止提交 `.venv/`、`build/`、`dist/`、模型、音频样本或任何 API key/private key/token；提交身份使用 `Jiaming Wang <w_jming@outlook.com>`。

## 开发约束

- 不把大模型、音频样本、虚拟环境提交到仓库。
- 新增模型后端必须接入 `Recognizer` 抽象，并补充中文文档。
- 文本后处理要有单元测试，尤其是撤回和“不是”误触发边界。
- 对 98% 准确率类指标必须写明测试集、口径和实测条件。

## 验证命令

```bash
python3 -m compileall src tests
pytest -q
local-speak doctor
```
