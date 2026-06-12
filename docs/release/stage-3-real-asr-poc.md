# 阶段 3 真实流式 ASR POC 报告(D-1 / D-17)

日期:2026-06-11 · 执行:AI agent(Claude)· 状态:**自动化门控全部通过,待 🧑 真实麦克风人工验证(go/no-go 签字)**

## 1. 范围

按 todo.md 主线 A 打穿真实垂直切片的模型侧与音频侧:

- sherpa-onnx streaming Zipformer 双语 int8 接入(`crates/voxflow-asr-sherpa`,D-17 A 路线)。
- silero VAD 接入(默认 VAD,`EnergyVad` 降为兜底)。
- PipeWire native 帧采集(`voxflow-audio` `pipewire-native` feature,D-3 主路径)。
- 延迟/RTF/稳定性基准与 `model-profiles/streaming-zh-en-small.toml` 真实数据回填。

尚未包含:真实模型接入 `voxflow-core` 流水线(`pending_runtime_integration` 仍为占位)、自录回放样本集、真实麦克风主观验收。

## 2. 测试环境

| 项 | 值 |
| --- | --- |
| 机器 | 20 核 x86_64,Ubuntu 24.04(基准测试经 `taskset -c 0-3` 限制为 4 核,贴近规格基准机) |
| 工具链 | rustup stable 1.96.0(用户级安装;原因见 §6) |
| 模型 | sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20,int8(encoder 182 MB + decoder 13 MB + joiner 3.2 MB + tokens 56 KB ≈ 198 MB) |
| VAD | silero_vad.onnx(644 KB) |
| 样本 | 模型自带 test_wavs 4 条 16 kHz 中英混说(0-3.wav,4.7-10.1 s) |
| 线程 | num_threads = 2,provider = cpu |

## 3. 结果

### 3.1 延迟门控(4 核限核,实时回放,墙钟测量)

| 指标 | 预算(p90) | EnergyVad 起点 | silero VAD 起点 | 判定 |
| --- | --- | --- | --- | --- |
| 首 partial | < 500 ms | **471 ms** (233/312/333/471) | 236 ms (36/76/116/236) | ✅ |
| 首 stable | < 1000 ms | **953 ms** (553/652/796/953) | 714 ms (357/435/556/714) | ✅ |

注:p90 取 nearest-rank(4 样本即最大值);silero 的 speech-start 判定晚于能量阈值(更接近真实语音起点),两组口径都记录,门控以较保守的 EnergyVad 口径判定。首 stable 953 ms 距预算仅 4.7%,样本量小,**不能视为富余**——自录样本集扩充后须复测。

### 3.2 RTF 与加载

| 指标 | 值 |
| --- | --- |
| RTF(20 核不限) | 0.034-0.036 |
| RTF(4 核限核) | 0.052-0.058 |
| 模型加载 | 1.5 s |

### 3.3 10 分钟稳定性(4 核限核,循环回放 + 500 ms 静音间隔,单会话)

| 指标 | 值 |
| --- | --- |
| 音频时长 | 10.0 min |
| 处理耗时 | 19.4 s(RTF 0.032) |
| 事件 | 957 partial / 78 final(端点切分正常) |
| 峰值 RSS | 587 MB |

### 3.4 PipeWire native 采集 smoke(真实守护进程 1.0.5)

3 秒采集:138 帧(20 ms/帧)、音频时钟 2740 ms、覆盖率 92%(差额为流连接协商耗时,属预期);峰值电平 0(本机麦克风静音,帧链路本身通)。

### 3.5 识别文本抽样(int8,greedy_search)

`0.wav` → "昨天是 MONDAY TODAY IS LIBR THE DAY AFTER TOMORROW是星期三"(LIBR 为已知的该模型对 "Tuesday" 的误识,与上游 README 一致)。中英混说整体可读;CER/WER 与主观验收留给人工验证点。

## 4. 自动化门控结论

NFR-PRF-01(首 partial p90 < 500 ms)与 NFR-PRF-02(stable p90 < 1000 ms)在回放集上**通过**;RTF、内存、10 分钟稳定性无异常。**go/no-go 的人工部分(真实麦克风中文/英文/混说各一段 + 主观体验)待所有者执行**,执行方式见 §7。

## 5. 交付物

| 物件 | 位置 |
| --- | --- |
| sherpa backend crate | `crates/voxflow-asr-sherpa`(lib + `sherpa-poc` 基准 bin + `vad::SileroVad`) |
| PipeWire 采集 | `crates/voxflow-audio`(`pipewire-native` feature + `pipewire-smoke` bin) |
| profile 回填 | `model-profiles/streaming-zh-en-small.toml`(真实 URL/sha256/体积;`ModelSource.archive_sha256`、`ModelFileSpec.size_bytes` 可选字段同步加入) |
| 文档回填 | `streaming-asr.md` 0.5(§5 候选表 + §9 实现注记) |
| 模型存放 | `~/.voxflow/models/poc/`(不入 git) |

## 6. 工程注记与偏离

1. **sherpa-rs 高层 API 截至 0.6.8 只封装离线识别器**;本 crate 经 `sherpa-rs-sys` 直接封装 online C API(`SherpaOnnxCreateOnlineRecognizer` 等)。仍属 D-17 的 A 路线(社区绑定提供构建/链接),安全封装层自有。
2. **工具链被迫升级**:`sherpa-rs-sys` 0.6.8 的 build script 使用 `cargo::` 指令语法(要求 Cargo ≥ 1.77),系统 cargo 1.75 无法构建;另有多个传递依赖(`idna_adapter` 1.2.2 要求 edition2024、`home` 0.5.12 要求 rustc 1.81、`tempfile→getrandom 0.3→wit-bindgen` 链)超出 1.75。已用户级安装 rustup stable 1.96.0(`~/.cargo`,未动系统 rustc,未改默认 PATH),并在 Cargo.lock 中 pin `idna_adapter=1.1.0`、`tempfile=3.14.0`、`home=0.5.9`。**这与 D-19 的"MSRV 1.75"冲突,需所有者裁决 MSRV 政策**(建议:开发/CI 用 rustup stable,发布物为静态二进制故 MSRV 仅影响构建环境)。
3. `download-binaries` 模式使用 sherpa-rs 分发的预编译 `libsherpa-onnx-c-api.so` + `libonnxruntime.so`(D-17 已知供应链风险);首次构建需要网络。正式发布前按 D-17 评估切换官方 C API 源码构建。
4. 本机缺 `libpipewire-0.3-dev`(无 sudo),构建经 `apt-get download + dpkg -x` 提取头文件至 `~/.local/pipewire-dev` + `PKG_CONFIG_SYSROOT_DIR` 完成;owner 机器建议直接 `sudo apt install libpipewire-0.3-dev libspa-0.2-dev`。

## 7. 🧑 人工验证点操作单(go/no-go)

```bash
# 1) 实时基准(回放):
PATH="$HOME/.cargo/bin:$PATH" taskset -c 0-3 cargo run --release -p voxflow-asr-sherpa --bin sherpa-poc -- \
  --model-dir ~/.voxflow/models/poc/sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20 \
  --threads 2 --realtime --silero-model ~/.voxflow/models/poc/silero_vad.onnx

# 2) 麦克风链路(PipeWire 帧采集 smoke;先确认系统麦克风未静音):
PKG_CONFIG_SYSROOT_DIR=$HOME/.local/pipewire-dev/extracted \
PKG_CONFIG_PATH=$HOME/.local/pipewire-dev/extracted/usr/lib/x86_64-linux-gnu/pkgconfig \
PATH="$HOME/.cargo/bin:$PATH" cargo run --release -p voxflow-audio --features pipewire-native --bin pipewire-smoke -- 5

# 3) 主观评价:中文、英文、中英混说各说一段(live 麦克风→ASR 一体化 bin 见 todo.md 主线 A 收尾项)。
```

达标判定:延迟主观可接受、识别可读;不达标 → 按 D-1 启动 Paraformer 备选 POC 或调预算,复审后继续。
