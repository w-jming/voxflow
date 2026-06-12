# 阶段 3 真实流式 ASR POC 记录

日期:2026-06-10

## 已完成

- 新增 `voxflow-asr` crate,把阶段 1 的 ASR 基础类型从 Core 中抽出:
  - `AudioFrame`
  - `TimestampedAudioFrame`
  - `Token`
  - `AsrEvent`
  - `StreamingRecognizer`
  - `MockRecognizer`
- `voxflow-core::recognizer` 现在 re-export `voxflow-asr`,现有 Core IPC 与测试继续使用同一套类型。
- 新增 `StablePrefixStabilizer`,按 [流式 ASR §4](../redesign/architecture/streaming-asr.md) 的连续 partial LCP 思路,在连续 partial 公共 token 前缀增长时产出 stable 事件。
- 新增 `ReplayBenchmark` 与 `ReplayReport`,固定阶段 3 基准报告字段:
  - `frame_count`
  - `audio_ms`
  - `event_count`
  - `vad_speech_start_ms`
  - `first_partial_ms`
  - `first_stable_ms`
  - `final_ms`
  - `first_partial_latency_ms`
  - `first_stable_latency_ms`
- 新增 `voxflow-core asr-benchmark-mock`,使用 20 ms/16 kHz silence frame 回放 mock recognizer,验证基准链路与 JSON 输出形状。
- 新增 `Vad` trait 与 `EnergyVad` 基线实现,用于阶段 3 自动化测试中的语音起点检测、迟滞起止判断和 VAD 起点延迟埋点。
- 新增 `ReplayBenchmark::run_with_vad`,在 replay 中同步喂入 VAD,报告 `vad_speech_start_ms`、首 partial 相对 VAD 起点延迟和首 stable 相对 VAD 起点延迟。
- 新增 `voxflow-core::pipeline`,把 `AudioSource`、`Vad`、`StreamingRecognizer` 和 `StablePrefixStabilizer` 串成可测 Core 内部流式管线。
- 新增 `voxflow-core pipeline-smoke`,用 synthetic 音频源、Energy VAD 和 mock recognizer 输出事件列表与延迟报告,覆盖 `frame_captured → vad_speech_start → first_partial/stable` 的自动化 smoke 形状。
- 新增 `ReplaySuiteReport` / `LatencyBudget` / `LatencyGate`,用于汇总多个 replay case 的 p90 延迟门控:
  - `first_partial_p90_ms`,默认预算 500 ms。
  - `first_stable_p90_ms`,默认预算 1000 ms。
  - 缺少任一延迟样本时 gate 失败,避免空报告被误判通过。
- 新增 `voxflow-core asr-suite-mock`,用多组 mock replay case 输出阶段 3 go/no-go 报告 JSON 形状。
- 新增模型 profile/manifest 校验骨架:
  - `model.list` 读取内置 profile 并返回本地状态。
  - `model.verify` 按 model id 复检本地目录、`manifest.lock` 和逐文件 checksum。
  - `model.import` 支持本地目录 copy/symlink 导入,先校验源目录必需文件和 SHA256,通过后写入 cache staging,生成 `manifest.lock`,再原子安装到 `models/<id>/`。
  - `model.activate` 只允许切换到 ready/active 模型,持久化 `config.toml`,失败时回滚内存配置并返回稳定错误码。
  - `model.delete` 删除非 Active 模型目录,Active 模型返回 `model.active_locked`。
  - 当前占位 profile 的来源/checksum 会被报告为 profile issue 或 broken,避免把未完成的 D-1 POC 模型误判为可用。
- 新增 `voxflow-audio` crate,为真实采集链路建立不依赖具体后端的基础:
  - `CaptureConfig`
  - `AudioSource`
  - 有界音频队列与 dropped frame 计数
  - `SyntheticAudioSource`
  - RMS/peak 电平测量
  - PipeWire runtime probe
- 新增 `voxflow-core audio-probe`,输出本机 PipeWire runtime、`pw-cli`/`pw-record`、`libpipewire-0.3` runtime 和 pkg-config 开发文件状态。
- `voxflow-core doctor` 新增 `audio.pipewire.runtime` 与 `audio.pipewire.development` 检查。
- 新增输入设备枚举基础:
  - `voxflow-audio::list_input_devices` 使用 `wpctl status` 枚举 PipeWire 输入 sources。
  - `parse_wpctl_sources` 覆盖默认输入源、蓝牙 headset 标签解析和无 source 降级。
  - `voxflow-core audio-devices` 输出设备列表、默认设备、warnings 和 PipeWire probe。
  - `audio.list_devices` IPC 返回同一份 inventory;即使当前会话无法连接 PipeWire,也返回可渲染的 warning。
  - `core.status.audio` 现在从设备 inventory 投影默认输入设备状态。

## 当前限制

- 本记录只完成阶段 3 自动化基础,不代表真实模型延迟、准确率或 go/no-go 结论。
- 当前 VAD 是能量阈值基线,只用于验证接口、迟滞行为和延迟报告形状;尚不能替代 silero VAD 或等价 ONNX VAD。
- `asr-suite-mock` 的 p90 结果来自 mock recognizer,只证明报告门控逻辑可用,不能作为真实模型达标证据。
- `model.activate` 当前完成的是配置级安全切换;真实 runtime load 和 smoke inference 仍需等 sherpa-onnx/silero 后端接入后补齐。
- 尚未接入 PipeWire native 音频帧采集;`pw-record` 仍只允许作为诊断/兜底。
- 本机 runtime 探测显示 PipeWire 运行时可用(`1.0.5`),但当前非桌面会话下 `wpctl status` 返回 `Could not connect to PipeWire`,且 `libpipewire-0.3` pkg-config 开发文件不可用;接入 `pipewire-rs` 前需要安装相应 dev 包或在构建脚本中明确处理缺失状态。
- 尚未接入 silero VAD 或 sherpa-onnx streaming Zipformer。
- 尚未准备中英/中英混说回放样本,也未执行 10 分钟稳定性基准。
- 阶段 2 的真实桌面输入上下文人工验证仍未完成;阶段 3 的自动化基础可并行推进,但不能宣布阶段 2 或阶段 3 完成。

## 验证

```bash
cargo test -p voxflow-asr -p voxflow-core
cargo run -p voxflow-core -- asr-benchmark-mock
cargo run -p voxflow-core -- asr-suite-mock
cargo run -p voxflow-core -- models
cargo run -p voxflow-core -- model-import MODEL_ID PATH [copy|symlink]
cargo run -p voxflow-core -- audio-probe
cargo run -p voxflow-core -- audio-devices
cargo run -p voxflow-core -- pipeline-smoke
scripts/dev-check.sh
```

示例 mock 报告:

```json
{
  "session_id": "mock-1",
  "frame_count": 50,
  "audio_ms": 1000,
  "event_count": 4,
  "vad_speech_start_ms": 0,
  "first_partial_ms": 0,
  "first_stable_ms": 0,
  "final_ms": 0,
  "first_partial_latency_ms": 0,
  "first_stable_latency_ms": 0
}
```

示例 mock suite:

```json
{
  "case_count": 3,
  "gates": [
    { "metric": "first_partial_p90_ms", "budget_ms": 500, "observed_p90_ms": 0, "passed": true },
    { "metric": "first_stable_p90_ms", "budget_ms": 1000, "observed_p90_ms": 0, "passed": true }
  ],
  "passed": true
}
```

示例本机 audio probe:

```json
{
  "pipewire_command": true,
  "pw_cli_command": true,
  "pw_record_command": true,
  "libpipewire_runtime": true,
  "pkg_config_development_files": false,
  "version": "1.0.5"
}
```

示例本机 audio-devices:

```json
{
  "devices": [],
  "default_device_id": null,
  "warnings": ["wpctl status failed: Could not connect to PipeWire"],
  "probe": {
    "pipewire_command": true,
    "pw_cli_command": true,
    "wpctl_command": true,
    "pw_record_command": true,
    "libpipewire_runtime": true,
    "pkg_config_development_files": false,
    "version": "1.0.5"
  }
}
```
