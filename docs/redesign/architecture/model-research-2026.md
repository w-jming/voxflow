# 2026 语音识别模型调研

> **编号** VF-ARCH-09 · **版本** 0.1 · **状态** 调研报告(随选型演进更新) · **最后更新** 2026-06-11

应所有者要求的全网调研:2025 下半年至 2026 年中的最新 ASR 模型,重点 Qwen、字节跳动、MiniMax 系列;按 VoxFlow 约束评估(本地离线、真 token 级流式、中英双语;**GPU 优先运行,D-21**,本机 RTX 5070 Ti 16GB)。

## 1. 重点结论

1. **Qwen3-ASR 0.6B/1.7B 已于 2026-01-29 开源(Apache-2.0)**,支持流式/离线统一推理——这推翻了 D-9 当时"官方无稳定 streaming API"的前提。注意:**真流式目前只在官方 vLLM 后端**(GPU);sherpa-onnx 已有 `sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25` 模型包,但在 sherpa 里是**离线识别器 + VAD 切段**,不是 token 级流式。
2. 字节跳动 **Seed-ASR/豆包系仍全部闭源**(火山引擎 API);**MiniMax 只有 TTS(Speech-02)开源生态,无开源 ASR**。两家暂不构成本地候选。
3. 真 token 级流式 + 本地开源的选项 2026 年仍然很少:zipformer/Paraformer 系(sherpa-onnx)与 Qwen3-ASR(vLLM)是仅有的成熟路线;其余新模型(FireRedASR2S、Kimi-Audio 等)均为离线/VAD 切段形态。

## 2. 模型清单(2025H2-2026)

| 模型 | 机构/时间 | 开源 | 流式 | 中英 | 体积/算力 | VoxFlow 适配评估 |
| --- | --- | --- | --- | --- | --- | --- |
| **Qwen3-ASR-1.7B** | 阿里 Qwen,2026-01 | ✅ Apache-2.0 | ✅(仅 vLLM 后端) | ✅ 30 语言+22 中文方言;zh WER 2.41(Fleurs)/2.71(AISHELL-2),en 3.38(LibriSpeech) | GPU(16GB 充裕);vLLM Python 服务栈 | **高准确率流式候选(P1 POC)**:作为本地 sidecar 服务,经 IPC 接入;开源 SOTA,对标商用 API |
| **Qwen3-ASR-0.6B** | 同上 | ✅ | vLLM 流式 / sherpa 离线+VAD | ✅ zh 2.88/3.15,en 4.55 | int8 ONNX 包(sherpa 2026-03-25);GGUF/纯 C 实现(qwen3-asr.cpp、antirez/qwen-asr)CPU 可跑 | **final/refine 首选(D-9 升级)**:sherpa-onnx 离线识别器接到现有 backend crate,停顿后精修 final segment;CUDA EP 或 CPU 均可 |
| Qwen3-ForcedAligner-0.6B | 同上 | ✅ | — | ✅ | 0.6B | 字级时间戳对齐,可服务语义撤销账本的字符级映射(候补工具) |
| **FireRedASR2S** | 小红书,2026-02 | ✅ | VAD 流式,ASR 本体离线 [待验证] | ✅ 普通话+20 方言+英语+中英混读+歌词,字级时间戳/置信度 | AED/LLM 两形态,GPU 建议 | refine 备选;一体化 VAD/LID/Punc 工具链可借鉴;字级置信度对账本友好 |
| Seed-ASR / Doubao-Seed-2.0 | 字节跳动 | ❌ 闭源 API | API 流式 | ✅ 强(2000 万小时) | 云端 | 不适用(本地优先红线) |
| MiniMax Speech-02 | MiniMax,2025-05 | TTS,非 ASR | — | — | — | 不适用;MiniMax 无开源 ASR |
| Kimi-Audio-7B | 月之暗面,2025-04 | ✅ | ❌ 离线 | ✅ | 7B,GPU 必需 | 过重,通用音频理解定位;不建议 |
| Step-Audio(2) | 阶跃星辰 | 部分开源 | 对话流式 | ✅ | 130B/大模型级 | 体量不适配桌面输入法 |
| GLM-4-Voice | 智谱 | ✅ | 编码器流式化 | ✅ | 9B 级 | 语音对话模型,非转写产品形态;不建议 |
| streaming zipformer zh 2025-06-30(现用) | k2-fsa | ✅ | ✅ 真 token 级 | zh(en 单独模型) | int8 167MB-771MB,CPU 即实时 | **P0 现行路线,已验证** |
| streaming zipformer 双语 2023-02-20(现用) | k2-fsa | ✅ | ✅ | ✅ 中英混说 | int8 198MB | **P0 现行路线,已通过门控** |
| Kyutai STT | Kyutai,2025 | ✅ | ✅ | ❌ 仅 en/fr | 1B/2.6B | 无中文,不适用 |
| NVIDIA Parakeet/Canary v3 | NVIDIA,2025 | ✅ | 部分 | zh 覆盖弱 | GPU | 中文非重心,观察 |

## 3. 对 VoxFlow 的选型建议(GPU 优先,D-21)

```text
P0 实时层(已验证,不动):streaming zipformer(双语 2023 / 中文 2025)
  └ GPU 工程项:sherpa-onnx CUDA EP(onnxruntime-gpu)加速,降首 partial 延迟
P1 精修层(立即可做,D-9 升级):Qwen3-ASR-0.6B int8(sherpa-onnx 离线+VAD)
  └ 停顿后异步精修 final segment,经账本安全门替换;CER 显著优于 zipformer
P1 POC(高准确率流式模式):Qwen3-ASR-1.7B + vLLM 本地 sidecar
  └ 真流式、开源 SOTA 准确率;代价:Python/vLLM 服务栈 + 常驻显存;
    作为可选模式(设置页切换),不替代 P0 默认链路
```

维持 zipformer 为 P0 默认的理由:体积/启动/延迟已实测达标,纯 Rust 链路无 Python 依赖;Qwen3-ASR 的流式被 vLLM 绑定,作为"可选高配模式"而非默认,符合"轻量即用 + 高配可选"的产品分层(PRD §3)。

## 4. 行动项(已挂 todo.md)

1. D-9/D-21 决策更新与回填(本文档 + review-and-decisions + streaming-asr §5.2)。
2. `voxflow-asr-sherpa` 增加离线识别器封装(Qwen3-ASR-0.6B int8 包),接 refine 流水线;新增模型 profile。
3. sherpa-onnx CUDA EP 构建链调研(sherpa-rs `cuda` feature 与预编译 GPU 库分发)。
4. vLLM sidecar POC(Qwen3-ASR-1.7B 流式,RTX 5070 Ti):延迟/显存/启动时间实测后决定是否进产品。

## 5. 来源

- [QwenLM/Qwen3-ASR](https://github.com/QwenLM/Qwen3-ASR) · [Qwen3-ASR-1.7B (HF)](https://huggingface.co/Qwen/Qwen3-ASR-1.7B) · [开源公告](https://www.alibabacloud.com/blog/602843) · [技术报告](https://arxiv.org/html/2601.21337v2)
- [sherpa-onnx Qwen3-ASR 文档](https://k2-fsa.github.io/sherpa/onnx/qwen3-asr/index.html) · [sherpa-onnx qwen3-asr-0.6b-int8 包](https://huggingface.co/pantinor/sherpa-onnx-qwen3-asr-0.6b-int8)
- CPU 实现:[qwen3-asr.cpp](https://github.com/predict-woo/qwen3-asr.cpp) · [antirez/qwen-asr](https://github.com/antirez/qwen-asr) · [GGUF](https://huggingface.co/ggml-org/Qwen3-ASR-0.6B-GGUF)
- [Seed-ASR 技术页](https://bytedancespeech.github.io/seedasr_tech_report/) · [豆包大模型(火山引擎)](https://www.volcengine.com/product/doubao)
- [MiniMax Speech](https://www.minimax.io/models/speech) · [MiniMax-Speech 论文](https://arxiv.org/pdf/2505.07916)
- [FireRedASR2S 介绍](https://www.aipuzi.cn/ai-news/fireredasr2s.html) · [FireRedTeam](https://github.com/FireRedTeam)
- [Kimi-Audio 报道](https://news.qq.com/rain/a/20250428A0448900) · [语音大模型概述(2026.02)](https://zhuanlan.zhihu.com/p/14831605089)
