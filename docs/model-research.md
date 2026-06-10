# 模型调研与建议

调研日期：2026-06-06。

## 结论

如果以准确率和速度优先，建议第一阶段按下面顺序验证：

1. `Qwen/Qwen3-ASR-1.7B`：中英混输、中文方言、流式/离线统一能力强，适合有 NVIDIA GPU 的本地部署或 vLLM 服务。
2. Qwen3-ASR-Flash / DashScope：云端实时 API 候选，适合先追求准确率和延迟，但要评估费用、网络和隐私。
3. 火山引擎豆包 ASR：字节相关 API 候选，中文场景值得实测；第一版代码通过 OpenAI 兼容接口抽象，后续可单独接 WebSocket 协议。
4. FunASR/SenseVoice：本地工业化服务候选，提供 ASR、VAD、标点、ITN 等能力，适合长期自托管。
5. `faster-whisper` + `large-v3`/`large-v3-turbo`：成熟、资料多、部署简单，适合作为第一版默认后端。
6. NVIDIA Parakeet CTC 0.6B Mandarin-English：中英混输和 Linux 部署友好，但生态和输入法场景还需实测。

“准确率 98%+”需要明确测试集和口径。ASR 常用 WER/CER；单个公开 benchmark 达到 2% 左右错误率不等于所有真实输入都稳定 98%+。建议用你的日常麦克风、常用软件名、人名、技术词、中文夹英文句子建立 200-500 句回归集，再决定默认模型和热词策略。

## Qwen3-ASR

Hugging Face 官方模型卡说明 Qwen3-ASR-1.7B 和 0.6B 支持 30 种语言和 22 种中文方言，并支持语言识别和 ASR。模型卡还列出 0.6B 版本在并发场景下的高吞吐，以及流式/离线统一推理能力。

推荐用法：

```bash
pip install -U qwen-asr
local-speak transcribe sample.wav --backend qwen --model Qwen/Qwen3-ASR-0.6B
```

服务化建议：

```bash
vllm serve Qwen/Qwen3-ASR-1.7B
local-speak gui --backend openai-compatible --api-base http://127.0.0.1:8000/v1 --api-model Qwen/Qwen3-ASR-1.7B
```

适用：准确率优先、中英混输、GPU 可用、后续需要上下文 bias 或热词。

风险：依赖较新，环境比 Whisper 重；1.7B 推荐 GPU，CPU 不适合实时输入。

## FunASR / SenseVoice

FunASR 官方介绍覆盖 ASR、VAD、标点恢复、说话人分离、情感和音频事件检测，并提供 OpenAI 兼容服务形态。SenseVoice-Small 模型卡说明它支持中文、英文、粤语、日语、韩语，并提供 LID、SER、AED、ITN 等能力。

适用：长期本地部署、需要 VAD/标点/ITN 一体化、希望以后扩展会议转写或字幕。

风险：实时服务安装路径比文件识别更复杂；不同版本依赖较重，需要固定版本和回归测试。

## Whisper / faster-whisper

OpenAI Whisper large-v3/turbo 覆盖语言广，`faster-whisper` 使用 CTranslate2 提升推理效率和降低显存占用。中文标点可以考虑 Belle Whisper 中文标点微调版的 CTranslate2 转换模型。

适用：第一版快速可用、离线部署、显卡或 CPU 均可跑。

风险：中文标点和中英混输专有词不一定优于 Qwen/FunASR；实时输入需要较好的 VAD 和窗口策略。

## 字节/火山引擎

火山引擎提供语音识别相关 WebSocket/实时接口文档，豆包 ASR 2.0 属于值得实测的云端候选。建议后续在确认账号、区域、费用、隐私要求后实现独立 `volcengine` 后端。

需要你决策：

- 是否接受音频出网。
- 是否有火山引擎账号、AppID、Token 或 API Key。
- 是否优先支持实时 WebSocket，还是先支持录音文件转写。

## NVIDIA Parakeet

NVIDIA NIM 模型卡介绍 Parakeet CTC 0.6B Mandarin-English 面向普通话/英文转写，支持 Linux，输出包含中英文文本和常见标点。

适用：NVIDIA 生态、GPU 服务化、普通话/英文混输。

风险：具体输入法延迟、中文真实场景和成本需要实测。

## 资料链接

- Qwen3-ASR Hugging Face：https://huggingface.co/Qwen/Qwen3-ASR-1.7B
- qwen-asr PyPI：https://pypi.org/project/qwen-asr/
- Qwen3-ASR 技术报告：https://arxiv.org/abs/2601.21337
- FunASR 官方站：https://www.funasr.com/en/
- SenseVoiceSmall Hugging Face：https://huggingface.co/FunAudioLLM/SenseVoiceSmall
- Belle Whisper 中文标点 CTranslate2：https://huggingface.co/k1nto/Belle-whisper-large-v3-zh-punct-ct2
- NVIDIA Parakeet NIM：https://build.nvidia.com/nvidia/parakeet-ctc-0_6b-zh-cn/modelcard
- 火山引擎语音识别文档：https://www.volcengine.com/docs/6561/113644
