# 流式 ASR 设计

> **编号** VF-ARCH-03 · **版本** 0.5 · **状态** 评审中(D-1 POC 实测已回填,待 go/no-go 人工签字) · **最后更新** 2026-06-11

## 1. 为什么必须重构

当前 0.2.0 架构是 `record wav -> transcribe(file) -> final text`,只能在录音结束后输出文本。即便把录音切成 1 秒小块,也只是分块识别,不是 token 级流式。token 级流式必须让 ASR runtime 接收连续音频帧,并在解码过程中输出 partial token:

```text
audio frame -> streaming decoder -> partial token -> stable token -> final segment
```

## 2. ASR 抽象接口

```rust
pub trait StreamingRecognizer: Send {
    fn start_session(&mut self, config: RecognitionConfig) -> Result<SessionId>;
    fn push_audio(&mut self, session: SessionId, frame: AudioFrame) -> Result<()>;
    fn poll_events(&mut self, session: SessionId) -> Result<Vec<AsrEvent>>;
    fn finish_session(&mut self, session: SessionId) -> Result<Vec<AsrEvent>>;
}

pub enum AsrEvent {
    Partial { revision: u64, text: String, tokens: Vec<Token> },
    Stable  { revision: u64, text: String, token_range: TokenRange },
    Final   { revision: u64, text: String, segment_id: SegmentId },
}
```

实现要求:

- `push_audio` 不得阻塞音频线程;内部用有界队列,满则丢帧并计数上报。
- 必须提供 `MockRecognizer`:按脚本回放 token 序列,供 IPC/前端/账本测试使用(FR-INP-02)。
- 事件语义与 [IPC 听写事件](ipc-api.md) 一一对应。

## 3. partial / stable / final

| 级别 | 含义 | 前端处理 |
| --- | --- | --- |
| partial | 模型当前猜测,可变 | `update_preedit` |
| stable | 稳定判定通过,不再回改 | `commit_text` + 账本 append |
| final | segment 结束的最终结果 | 可选触发二阶段精修/修正 |

## 4. token 稳定策略

组合策略,参数可配置(默认值如下):

| 策略 | 默认参数 | 说明 |
| --- | --- | --- |
| LCP | 连续 2 次 partial 公共前缀 | 重复出现的最长公共前缀进入 stable 候选 |
| 时间窗 | token 产生后 600 ms 未变化 | 有 token 时间戳时启用 |
| 边界提升 | 标点 / 停顿 > 300 ms | 在短语边界整体 commit,避免半个词 |
| 修正窗口 | 最近 1 个未冻结 segment | final/refine 只允许修改该窗口,超出即冻结 |

默认行为:partial 留在 preedit;stable 按短语边界 commit;final 只允许修改最近一个未冻结 segment。冻结策略详见[语义撤销 §2](semantic-correction.md)。

## 5. 模型路线

### 5.1 默认低延迟流式模型(P0)

准入条件:中英双语、本地离线、真 streaming partial、许可证允许分发或引导下载、基准机上实时(RTF < 1)。

候选(最终选型见 [D-1](../review-and-decisions.md);2026-06-11 POC 实测值见下,报告全文见 [stage-3-asr-poc.md](../../release/stage-3-asr-poc.md)):

| 候选 | 运行时 | 量化后体积 | 许可证 | 备注 |
| --- | --- | --- | --- | --- |
| streaming Zipformer 双语(zh-en,2023-02-20) | sherpa-onnx | **198 MB(int8 实测:encoder 182M + decoder 13M + joiner 3M)** | Apache-2.0 | 首选,**POC 已通过**:4 核限核 RTF 0.053、首 partial p90 471 ms、首 stable p90 953 ms、10 分钟稳定回放峰值 RSS 587 MB、加载 1.5 s;profile 已回填真实 URL/sha256 |
| streaming Paraformer 双语 | sherpa-onnx / FunASR | 约 100-200 MB [待验证] | Apache-2.0/模型自带 | 备选;中文表现好;Zipformer 已达标,仅在准确率主观验收不通过时再 POC |

POC 实测项:首 partial 延迟、RTF、内存峰值已回填;**中英混说 CER/WER 与真实麦克风主观体验属人工验证点(go/no-go),待所有者执行**。回放样本目前为模型自带 test_wavs(4 条中英混说),自录样本集见测试策略。

### 5.2 高准确率模型(P1)

**2026-06-11 更新(详见[模型调研 2026](model-research-2026.md),D-21 GPU 优先)**:Qwen3-ASR 0.6B/1.7B 已于 2026-01-29 开源(Apache-2.0,30 语言 + 22 中文方言,开源 SOTA;1.7B zh WER 2.41/2.71),原"无稳定 streaming API"前提失效。分两条路接入:

- **final/refine(立即可做)**:Qwen3-ASR-0.6B int8 经 sherpa-onnx **离线识别器 + VAD 切段**(官方模型包 2026-03-25),停顿后异步精修 final segment,经账本安全门替换;CUDA EP 优先,CPU 兜底。
- **高准确率流式模式(P1 POC)**:Qwen3-ASR-1.7B 的真流式仅在官方 vLLM 后端(GPU);作为本地 sidecar 服务 POC,实测延迟/显存/启动成本后决定是否作为设置页可选模式。**不替代** zipformer 默认链路(轻量即用分层,PRD §3)。

备选 refine:FireRedASR2S(小红书 2026-02 开源,中英混读 + 字级时间戳/置信度)。

### 5.3 双模型级联(P1)

```text
small streaming ASR  -> 低延迟 partial/stable
large ASR refiner    -> final segment 异步精修
semantic ledger      -> 精修结果经安全门安全替换
```

精修只能改写修正窗口内、且账本可定位的 segment;窗口外文本一律不动。

## 6. VAD 与音频要求

- VAD:silero-vad(ONNX,约 2 MB,MIT)经 sherpa-onnx 集成;判定窗 ≤ 96 ms。
- 流式链路禁止以 `pw-record` 子进程 + 文件作为主链路;Core 必须直接采集音频帧(Linux:PipeWire native,fallback ALSA/Pulse;macOS:CoreAudio;Windows:WASAPI,见 [D-3](../review-and-decisions.md))。

统一帧格式:

| 项 | 值 |
| --- | --- |
| 采样率 | 16 kHz(或模型要求值,Core 内重采样) |
| 声道 | mono |
| 编码 | PCM int16 或 float32 |
| 帧长 | 10-30 ms |

## 7. 延迟预算

基准机、默认流式模型;测量方法见[测试策略 §5](../engineering/testing-strategy.md)。

| 阶段 | 预算 |
| --- | --- |
| 音频采集帧 | 10-30 ms |
| VAD 判定 | < 20 ms |
| 首 partial(NFR-PRF-01) | < 500 ms (p90) |
| stable commit(NFR-PRF-02) | < 1000 ms (p90) |
| final refine | 后台异步,不计入输入延迟 |

延迟通过 tracing span 埋点测量:`frame_captured → vad_speech_start → first_partial → stable_commit`,基准测试用预录音频经 mock 设备注入,保证可重复。

## 8. 验收标准

1. `MockRecognizer` 按脚本产生 partial/stable/final,驱动全链路集成测试。
2. 真实 streaming 模型在说话过程中产生 partial(非分块伪流式)。
3. IBus preedit 实时显示 partial;stable 在用户仍在说话时即可 commit。
4. final/refine 不修改账本外文本、不修改冻结 segment。
5. 延迟预算各项在基准机达标并出具测量报告。

## 9. 实现注记(2026-06-10,阶段 3 骨架)

已落地(`crates/voxflow-asr`、`crates/voxflow-audio`、`voxflow-core::pipeline`,详见 [docs/release/stage-3-asr-poc.md](../../release/stage-3-asr-poc.md)):

- `StreamingRecognizer`/`AsrEvent`/`MockRecognizer` 抽象;`StablePrefixStabilizer`(连续 partial LCP);`EnergyVad` 基线与 `Vad` trait。
- `ReplayBenchmark`/`ReplaySuiteReport`:固定 `vad_speech_start → first_partial/first_stable` 埋点与 p90 门控(500/1000 ms)JSON 形状;`asr-suite-mock` 为 go/no-go 报告骨架。
- 音频侧:有界队列 + 丢帧计数、synthetic source、RMS/peak 电平、PipeWire runtime probe;设备枚举暂经 `wpctl status`(仅枚举,不作采集链路)。

已接入(2026-06-11,真实链路,详见 [stage-3-asr-poc.md](../../release/stage-3-asr-poc.md)):

- `crates/voxflow-asr-sherpa`(D-17 隔离 backend crate):经 `sherpa-rs-sys` 0.6.8 预编译库直接封装 sherpa-onnx **online** C API(注:`sherpa-rs` 高层 API 截至 0.6.8 只有离线识别器,故自写薄封装),实现 `StreamingRecognizer`,内部复用 `StablePrefixStabilizer` 产出 partial/stable,端点检测产出 final。
- `voxflow_asr_sherpa::vad::SileroVad`:silero VAD(ONNX 644 KB)实现 `Vad` trait,为默认;`EnergyVad` 降为兜底。
- `voxflow-audio` `pipewire-native` feature:pipewire-rs 0.8 原生帧采集(`PipeWireAudioSource`,S16LE mono 16 kHz,有界队列),已对真实 PipeWire 1.0.5 守护进程 smoke 通过;feature 非默认,缺 `libpipewire-0.3-dev` 的环境仍可构建工作区。
- `sherpa-poc` 基准工具:离线 RTF / 实时墙钟延迟 / `--loop-minutes` 稳定性三种模式,输出 JSON 报告。

待完成(阶段 3 收尾):真实模型接入 `voxflow-core` 流水线与模型管理激活链路(替换 `pending_runtime_integration`);自录中文/英文/混说回放样本集;🧑 真实麦克风 go/no-go 人工签字。
