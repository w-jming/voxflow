# 模型调研与决策

调研日期：2026-06-10。

## 当前决策

VoxFlow 采用“源码轻量可用 + 高准确率模型一键下载到用户目录”的策略：

- 源码仓库内置 `Systran/faster-whisper-tiny`，许可证 MIT，支持 99 种语言，用作下载即用 fallback。
- Qwen3-ASR 0.6B 作为轻量高准确率下载档位，许可证 Apache-2.0。
- Qwen3-ASR 1.7B 作为高准确率档位，许可证 Apache-2.0；deb 默认不打包该模型，用户可一键下载到 `$VOXFLOW_HOME/models`，默认 `~/.voxflow/models`。
- 复用已有 Qwen3-ASR 权重时，导入前必须做模型结构和官方 safetensors SHA256 校验；通过后才允许写入用户配置。

Qwen3-ASR 官方模型卡说明 0.6B 和 1.7B 支持 30 种语言、22 种中文方言和离线/流式统一推理；1.7B 在开源 ASR 中属于高准确率优先，0.6B 是准确率和效率折中。

## 用户可见命令

```bash
voxflow models
voxflow models --download qwen3-asr-0.6b
voxflow models --select qwen3-asr-0.6b
voxflow models --download qwen3-asr-1.7b
voxflow models --select qwen3-asr-1.7b
```

控制中心提供模型下拉选择、下载/继续、暂停、本地模型校验、复制导入和软链接导入。

## 为什么源码内置 tiny

直接把 Qwen3-ASR 0.6B/1.7B 权重提交到 GitHub 或安装到 `/opt` 不现实，也会让源码 clone 和系统分区占用过重。`Systran/faster-whisper-tiny` 单个权重文件低于 GitHub 单文件限制，许可证明确，能保证源码 checkout 后不联网也有一个可运行模型。大模型通过 GUI/CLI 下载到用户目录，便于迁移、清理和改放到大容量磁盘。

## 风险

- Whisper tiny 只作为 fallback，不代表高准确率体验。
- Qwen3-ASR 本地推理依赖 PyTorch/Transformers，包体和显存/内存要求明显更高。
- 长时间实时输入的最终体验取决于麦克风、VAD 分块、模型延迟和 IBus composition 兼容性，需要真实桌面回归测试。

## 资料链接

- Systran faster-whisper-tiny：https://huggingface.co/Systran/faster-whisper-tiny
- Qwen3-ASR 0.6B：https://huggingface.co/Qwen/Qwen3-ASR-0.6B
- Qwen3-ASR 1.7B：https://huggingface.co/Qwen/Qwen3-ASR-1.7B
- qwen-asr PyPI：https://pypi.org/project/qwen-asr/
