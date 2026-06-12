# VoxFlow / 声流输入法

VoxFlow 下一代版本是 Linux 优先、后续跨平台的本地流式桌面语音输入法。当前仓库正在按 `docs/redesign/` 从旧 Python/GTK 0.2.0 直接重写为 Rust Core + Tauri 控制台 + IBus/Fcitx5 输入法前端。

当前阶段:阶段 2 IBus 原型自动化链路已基本成形,真实桌面输入上下文仍待人工验证;阶段 3 真实流式 ASR 的抽象、stable 判定、mock replay 基准、能量 VAD 基线、音频探测和模型管理骨架已开始;阶段 5 的语义撤销账本、规则状态机、安全门和平台无关输入操作映射已进入 Core 单元测试。阶段 0/1 已建立 Rust workspace 和 `voxflow-core` 原型;旧版 Python 实现和旧打包路径已移除,历史可由 git 追溯。

## 规格来源

实现依据只来自:

- `AGENTS.md`
- `docs/redesign/README.md`
- `docs/redesign/product/requirements.md`
- `docs/redesign/engineering/migration-plan.md`
- `docs/redesign/architecture/*.md`
- `docs/redesign/engineering/testing-strategy.md`

如实现与文档冲突,先更新文档并说明影响。

## 当前可用命令

```bash
cargo run -p voxflow-core -- --help
cargo run -p voxflow-core -- status
cargo run -p voxflow-core -- mock-session
cargo run -p voxflow-core -- models
cargo run -p voxflow-core -- model-import MODEL_ID PATH [copy|symlink]
cargo run -p voxflow-core -- model-activate MODEL_ID
cargo run -p voxflow-core -- model-delete MODEL_ID
cargo run -p voxflow-core -- asr-benchmark-mock
cargo run -p voxflow-core -- asr-suite-mock
cargo run -p voxflow-core -- audio-probe
cargo run -p voxflow-core -- pipeline-smoke
cargo run -p voxflow-core -- doctor
cargo run -p voxflow-core -- serve
cargo run -p voxflow-ibus -- component-xml
cargo run -p voxflow-ibus -- register-json
cargo run -p voxflow-ibus -- self-test
cargo run -p voxflow-fcitx5 -- addon-conf
cargo run -p voxflow-fcitx5 -- inputmethod-conf
cargo run -p voxflow-fcitx5 -- register-json
cargo run -p voxflow-fcitx5 -- self-test
cargo run -p voxflow-fcitx5 -- probe
```

`mock-session` 使用 `MockRecognizer` 输出 `dictation.partial`、`dictation.stable`、`dictation.final` JSONL 事件,用于阶段 1 IPC 和前端联调。

`models` 读取 `model-profiles/` 并核对 `$VOXFLOW_HOME/models/<model_id>/manifest.lock` 与逐文件 checksum,输出 `not_installed`、`ready`、`active` 或 `broken`。当前 `streaming-zh-en-small` profile 仍含 D-1 POC 占位来源/checksum,因此不会被误判为可用真实模型。

`model-import` 支持本地模型目录 copy/symlink 导入。导入会先按 profile 校验必需文件和 SHA256,通过后写入 `$VOXFLOW_HOME/cache/imports/` 临时目录,生成 `manifest.lock`,再原子安装到 `$VOXFLOW_HOME/models/<model_id>/`。symlink 模式只在 VoxFlow 模型目录内建立逐文件链接,不向源目录写入 manifest。

`model-activate` 只允许切换到 `ready` 或已 `active` 的本地模型,成功后持久化 `config.toml` 并广播 `config.changed`/`model.state_changed`。当前 runtime smoke 字段为 `pending_runtime_integration`,表示真实模型加载与 smoke inference 要等 sherpa-onnx/silero runtime 接入后补齐。

`model-delete` 只能删除非 Active 模型目录;Active 模型会返回 `model.active_locked`。

语义撤销基础目前在 Core 内部实现并测试:注入账本、冻结策略、规则状态机、安全门、修正记录、`correction.list_recent` IPC,以及安全门通过后的 delete/replace 到平台无关 `InputEvent` 的映射。`replace_entity` 会删除账本尾部完整 segment 后提交修正后的完整文本,避免把段内目标误映射成光标前局部删除。Core mock 听写路径可发出 `correction.applied.input_events`,IBus 投影层可翻译为 delete/commit 操作。ONNX 轻量分类器、真实 streaming ASR 流水线、Fcitx5、真实桌面删除验证和控制台修正页仍未完成。

`asr-benchmark-mock` 使用 `voxflow-asr` 的 mock recognizer、20 ms/16 kHz frame 和 Energy VAD 回放,输出阶段 3 基准报告 JSON 形状,包括 VAD 起点、首 partial/stable 时间和相对 VAD 起点延迟。该命令只验证 replay/latency 记录链路,不代表真实模型延迟或准确率。

`asr-suite-mock` 在多个 mock replay case 上汇总 p90 延迟门控,输出 `first_partial_p90_ms` 与 `first_stable_p90_ms` 的预算、实测值和 pass/fail。该命令只固定阶段 3 go/no-go 报告口径;真实结论必须等 PipeWire native、silero/ONNX VAD、sherpa-onnx streaming 模型和回放样本接入后再出。

`audio-probe` 检查本机 PipeWire runtime、`pw-cli`/`pw-record` 命令、`libpipewire-0.3` runtime 和 `pkg-config` 开发文件状态。`pw-record` 只作为诊断/兜底信号,不代表主链路会用子进程录音。

`pipeline-smoke` 使用 synthetic 音频源、Energy VAD 和 mock recognizer 跑通 Core 内部流式管线,输出事件列表和 `frame_captured → vad_speech_start → first_partial/stable` 指标形状。该命令仍不代表真实模型或真实麦克风验收。

`voxflow-ibus self-test` 走无桌面的阶段 2 POC 链路:IPC 听写事件 → 平台无关 `InputEvent` → IBus preedit/commit/delete 操作。测试明确禁止把"听写中"等状态文案放入 preedit。

`voxflow-ibus core-roundtrip` 会连接真实 `voxflow-core` UDS,发送 `core.subscribe`、`frontend.register` 与 `dictation.start`,并把 Core 的 mock dictation 事件投影成 IBus 操作。`voxflow-ibus engine-focus-smoke` 走 zbus engine 的 `FocusIn/FocusOut` handler,用于验证引擎骨架到 Core 订阅流的桥接。`voxflow-ibus --ibus-engine --probe-once` 会连接当前 session D-Bus,注册 `org.freedesktop.IBus.Factory` 后立即退出,用于阶段 2 ibus-daemon 拉起路线 smoke;常驻 `--ibus-engine` 入口通过 Factory 的 `CreateEngine("voxflow")` 动态创建 `org.freedesktop.IBus.Engine` object,并暴露 `UpdatePreeditText(vub)`、`CommitText(v)`、`DeleteSurroundingText(iu)` signals。

`voxflow-fcitx5` 当前是阶段 6 的离线可测骨架:生成 Fcitx5 addon/inputmethod metadata、声明 `frontend.register kind=fcitx5` 能力、把 mock dictation 与 correction 事件翻译为 Fcitx5 preedit/commit/delete 操作,并探测本机 `fcitx5` 命令和 pkg-config 开发文件。真实薄 C++ addon 动态库、KDE Plasma Wayland 真实应用 smoke 仍未完成。

用户级 IBus component POC:

```bash
packaging/linux/ibus/install-ibus-user.sh /absolute/path/to/voxflow-ibus
packaging/linux/ibus/uninstall-ibus-user.sh
packaging/linux/ibus/smoke-private-ibus.sh /absolute/path/to/voxflow-ibus
```

用户级 Fcitx5 metadata 安装骨架:

```bash
packaging/linux/fcitx5/install-fcitx5-user.sh /absolute/path/to/voxflow.so /absolute/path/to/voxflow-fcitx5
packaging/linux/fcitx5/uninstall-fcitx5-user.sh
```

该脚本需要真实 `voxflow.so` addon 动态库;当前没有动态库时会拒绝安装,避免注册不可用输入法。

## 数据目录

默认用户数据目录:

```text
~/.voxflow/
```

可通过 `VOXFLOW_HOME` 重定向。目录规划:

```text
~/.voxflow/
  config.toml
  models/
  cache/
  logs/
  run/
  ledger/
```

IPC socket 默认位于:

```text
$XDG_RUNTIME_DIR/voxflow/core.sock
```

## 开发验证

Rust 工具链可用后运行:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

或:

```bash
scripts/dev-check.sh
```

推送前运行:

```bash
scripts/secret-scan.sh
```

## 后续阶段

按 `docs/redesign/engineering/migration-plan.md` 继续推进:

1. Rust Core 原型
2. IBus 原型(当前)
3. 真实流式 ASR go/no-go
4. Tauri 控制台与状态指示器
5. 语义撤销与分类器
6. Fcitx5 前端
7. deb/portable MVP 发布

## 许可证

VoxFlow 代码使用 MIT 许可证。模型许可证、来源和校验值由 `model-profiles/` 与控制台模型页展示。
