# AGENTS.md

## 项目目标

VoxFlow / 声流输入法下一代版本是 Linux 优先、后续跨平台的本地流式桌面语音输入法。核心目标不是语音文件转写,而是在任意可输入文本位置提供输入法级体验:token 级 partial/stable/final 流式输出、光标处 preedit、自动 commit、语义撤销、模型管理、可诊断安装和本地优先隐私边界。

`docs/redesign/` 是当前唯一产品、架构、设计、测试和发布依据。用户已确认该文档库完成;后续实现按该文档库执行。若实现需要偏离文档,必须先更新对应文档、提升版本号并在变更说明中标明影响。

## 当前裁决

- 旧 Python/GTK 0.2.0 版本按 D-7 裁决直接废弃;不保留运行版,不迁入 `legacy/`,历史由 git 追溯。
- 仓库保持 monorepo;阶段 0 建立 Rust workspace 并删除旧 Python 实现。
- 新架构:Rust `voxflow-core` + Tauri Control Center + 输入法前端。
- Core 是唯一核心状态源,负责音频、VAD、streaming ASR、文本后处理、语义撤销、注入账本、模型管理、配置、日志和诊断。
- Tauri 控制台只展示状态和发起命令,不采集音频、不加载模型、不向目标应用写文本。
- 输入法前端只做协议翻译和 preedit/commit/delete;业务逻辑不得下沉到 IBus/Fcitx5 前端。
- Linux P0 必须同时交付 IBus 与 Fcitx5 前端;fallback 注入只能作为兼容模式并明确标注风险。
- IPC 采用本地 UDS + JSON Lines;协议以 `docs/redesign/architecture/ipc-api.md` 为准,新增/修改消息必须同步 schema 和测试。
- 默认流式 ASR 路线:首选 sherpa-onnx + streaming Zipformer 双语 int8;Paraformer 并行 POC 备选。
- 音频主路径:Linux PipeWire native,cpal ALSA/Pulse fallback;`pw-record` 只能用于诊断或最后兼容 fallback,不能作为流式主链路。
- 默认快捷键为 `Alt+S`,支持 toggle 与 hold 两种模式。
- 听写状态使用全局状态指示器/HUD 或托盘降级;preedit 全局禁止放入"听写中"等非正文占位文案,只能承载真实 partial 文本。
- 用户数据目录继续由 `VOXFLOW_HOME` 控制,默认 `~/.voxflow/`;模型、日志、pid、缓存、配置和可选账本都应在该目录下。
- 大模型不进入 git,也不默认安装到 `/usr` 或 `/opt`;模型通过 profile 声明来源、许可证和 sha256。
- Qwen3-ASR 0.6B/1.7B 定位为 P1 final/refine 或高质量离线模式,不承诺实时主模型。
- 语义撤销 P0 由规则状态机 + 轻量意图分类器 + 注入账本安全门组成;分类器只提供建议,任何删除/替换必须过安全门。

## 文档索引

- 总入口:`docs/redesign/README.md`。
- 术语和写作规范:`docs/redesign/glossary.md`。
- 产品与验收:`docs/redesign/product/prd.md`,`docs/redesign/product/requirements.md`,`docs/redesign/product/user-journeys.md`。
- 架构合同:`docs/redesign/architecture/system-architecture.md`,`core-ui-separation.md`,`ipc-api.md`,`streaming-asr.md`,`input-method.md`,`semantic-correction.md`,`semantic-intent-classifier.md`,`model-management.md`。
- 设计与前端:`docs/redesign/design/*`,`docs/redesign/frontend/*`。
- 平台:`docs/redesign/platforms/linux.md`,`cross-platform-strategy.md`;macOS/Windows 文档是 P1/P2 spike,不是当前 P0 实现承诺。
- 工程:`docs/redesign/engineering/migration-plan.md`,`testing-strategy.md`,`packaging-release.md`,`security-privacy.md`,`git-workflow.md`。
- 审查裁决:`docs/redesign/review-and-decisions.md`,D-1~D-16 为已回填裁决。

## 实施阶段

按 `docs/redesign/engineering/migration-plan.md` 执行,不得把后续阶段的完整 MVP 伪装成当前阶段完成。

1. 阶段 0:文档冻结与仓库重组。建立 Rust workspace,删除旧 Python 实现,更新 AGENTS.md 与目录结构。
2. 阶段 1:Rust Core 原型。daemon 启动、单实例、优雅退出、IPC 服务端、配置日志、MockRecognizer partial/stable/final。
3. 阶段 2:IBus 原型。zbus 优先,受阻切 C/GLib shim;preedit/commit/delete 跑通。
4. 阶段 3:真实流式 ASR go/no-go。PipeWire、VAD、sherpa-onnx 候选 POC、延迟埋点和基准报告。
5. 阶段 4:Tauri 控制台与状态指示器。
6. 阶段 5:语义撤销与分类器。
7. 阶段 6:Fcitx5 前端。
8. 阶段 7:deb/portable MVP 发布。
9. 阶段 8:macOS/Windows 预研。

## 开发约束

- 不直接在 `main` 开发。按任务使用 `feature/*`,`fix/*`,`docs/*`,`release/*`。
- 保持变更单一主题、可审查、可回滚;文档改动和代码改动尽量分开提交。
- Rust core 业务逻辑不得依赖平台 crate;平台能力通过 trait 注入。`#[cfg(target_os)]` 只应出现在 platform crate。
- Core 不依赖 Tauri;UI 和输入法前端都通过 IPC 与 Core 通信。
- 所有跨进程消息必须版本化并有机器可读 schema 或 Rust 类型导出校验。
- `MockRecognizer` 必须存在,并用于 IPC/前端/账本集成测试。
- 注入账本只记录 VoxFlow 自己 commit 的文本;默认不持久化用户文本。日志默认不得记录完整输入文本。
- 智能撤销关闭后,"不是""不对""删掉"等全部按普通文本处理。
- 语义修正映射到输入操作时只能删除账本确认的完整 segment;`replace_entity` 必须生成"删除完整 segment + commit 修正后完整文本",不得把段中目标词误当作光标前局部删除。
- 新模型后端必须接入模型 profile/manifest/checksum/smoke test 流程,不得绕过模型管理。
- 对准确率、延迟、内存等指标必须写明测试集、基准机和测量口径;不得用未经实测的宣传值。
- 包产物、大模型、日志、音频样本、虚拟环境、API key、token、私钥和真实用户数据不得提交。
- 提交到远程前必须执行 secret scan。提交身份使用 `Jiaming Wang <w_jming@outlook.com>`。

## 本机诊断记忆

- 2026-06-06:`OpenFit by Shokz` 蓝牙耳机默认 `a2dp-sink` 只提供输出;切到 `headset-head-unit-msbc` 后 PipeWire 出现 `bluez_input.A8_F5_E1_AF_A0_C1.0`,并成功录制 16-bit mono 16000 Hz。
- 当前系统经验:有 `xdotool`、`xclip`、`pw-record`;旧 Python 版曾因缺 `libportaudio2` 让 `sounddevice` 不可用。新版主链路必须用 PipeWire native,这些只作为诊断/兼容参考。
- 2026-06-10:旧版 `voxflow` 与 `local-speak-input` 已从本机卸载,用户数据、Qwen 缓存、旧 build/dist/downloads 已清理;源码仓库与 `.venv/` 保留。

## 工具链与构建环境(2026-06-11 起)

- 构建工具链:**用户级 rustup stable**(`~/.cargo/bin`,当前 1.96.0);系统 rustc 1.75 无法构建 `voxflow-asr-sherpa`(依赖要求 Cargo ≥ 1.77 与更新 rustc),见 D-20。命令前置 `PATH="$HOME/.cargo/bin:$PATH"`。
- `Cargo.lock` 中 `idna_adapter=1.1.0`、`tempfile=3.14.0`、`home=0.5.9` 为兼容性 pin,勿随意 `cargo update` 整体升级。
- `voxflow-asr-sherpa` 首次构建需联网(sherpa-rs-sys 下载预编译库);测试中真实模型用例由 `VOXFLOW_SHERPA_MODEL_DIR`、`VOXFLOW_SILERO_VAD_MODEL` 环境变量开启,未设置时自动跳过(模型在 `~/.voxflow/models/poc/`,不入 git)。
- `voxflow-audio` 的 `pipewire-native` feature 与 `voxflow-asr-sherpa` 的 `live-poc` feature 非默认;本机缺系统 dev 包时先 `source scripts/dev/pipewire-env.sh`。

## 验证命令

按变更类型运行。仓库重组前如果某些工具尚未建立,在交付说明中明确"尚未适用"并运行当前可用检查。

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

前端建立后:

```bash
npm run typecheck
npm run lint
npm test
```

发布或包相关变更:

```bash
# 具体脚本以新版 scripts/ 或 xtask 为准
cargo test --workspace
# deb / portable 安装级 smoke,覆盖 Ubuntu 22.04/24.04、Debian 12
# secret scan 必须为零泄漏
```

阶段 1 最低可验收项:

```bash
cargo run -p voxflow-core -- --help
cargo test -p voxflow-core ipc mock_recognizer config
```
