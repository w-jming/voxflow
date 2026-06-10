# AGENTS.md

## 项目目标

实现 Linux 下轻量级中文/英文语音输入法，优先保证准确率、速度、易用性和可维护性。必须支持口语助词清理、自然撤回/修正、自动标点、可插拔模型、本地部署路径和中文文档。

## 当前决策

- 开发分支：按任务创建 `feature/*` 或 `fix/*` 分支，不要直接在 `main` 上开发。
- 核心语言：Python 3.11+。
- 源码内置 fallback：`Systran/faster-whisper-tiny`，MIT 许可证，提交权重以保证源码 checkout 后有轻量模型可用。
- 高准确率模型：Qwen3-ASR 0.6B/1.7B，Apache-2.0 许可证；deb 默认不打包大模型，下载模型放入用户数据目录。
- 用户数据目录：由 `VOXFLOW_HOME` 控制，默认 `~/.voxflow/`；模型、日志、pid、缓存和用户配置都应放在该目录下。
- 复用本地 Qwen 权重必须先校验必需文件、`config.json`、safetensors 和官方 SHA256；校验失败不得写入用户配置。
- 工业化本地候选：FunASR/SenseVoice。
- 可视化：`voxflow gui` 本地 Web 控制台，保持轻量，不引入前端构建链。

## 本机诊断记录

- 2026-06-06：`OpenFit by Shokz` 蓝牙耳机已连接，但默认是 `a2dp-sink`，只提供输出，没有输入源。
- 同日用 `wpctl set-profile 73 261` 切到 `headset-head-unit-msbc` 后，PipeWire 出现 `bluez_input.A8_F5_E1_AF_A0_C1.0`，并成功录制 `/tmp/openfit-test.wav`：16-bit mono 16000 Hz，约 3 秒。
- 2026-06-06：已用 `uv` 创建 `.venv`，安装 `faster-whisper`、`sounddevice`、`pytest` 等依赖；`qwen_asr` 仍未安装，因为它是可选高准确率后端。
- 当前系统有 `xdotool`、`xclip`、`pw-record`；没有 `libportaudio2`，所以 `sounddevice` 已安装但不可用。代码已增加 `pw-record` fallback，`voxflow dictate` 可在 PipeWire 默认输入源上固定窗口录音。
- 2026-06-10：`0.2.0` 统一英文名、命令、包名为 `voxflow`；默认快捷键改为 `Ctrl+Space`；支持 `toggle` 与 `hold` 两种录音模式、简体/繁体输出设置、模型下载/切换、托盘退出停止后台。
- 2026-06-10：控制台 logo 改为内联 SVG，并为 `/logo.svg` 明确返回 `image/svg+xml` 和支持 HEAD 请求，避免浏览器出现图片加载失败占位。
- 2026-06-10：本机 `.venv` 已安装 `qwen-asr` 和 `hf_transfer`，用于 Qwen3-ASR 1.7B 测试和下载；最终 deb 不打包大模型，大模型下载目录 `downloads/` 不提交。
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
voxflow doctor
```
