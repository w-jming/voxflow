# 系统架构

> **编号** VF-ARCH-01 · **版本** 0.4 · **状态** 评审中(实现反馈已并入) · **最后更新** 2026-06-10

## 1. 架构总览

```text
┌───────────────────────────┐
│ Tauri Control Center       │
│ settings / models / status │
└─────────────┬─────────────┘
              │ local IPC (UDS, JSONL)
┌─────────────▼─────────────┐
│ voxflow-core               │
│ Rust daemon                │
│ - audio capture            │
│ - VAD                      │
│ - streaming ASR            │
│ - text postprocess         │
│ - semantic correction      │
│ - intent classifier        │
│ - injection ledger         │
│ - model manager            │
│ - config/log/state         │
└───────┬───────────┬───────┘
        │ IPC       │ platform traits
┌───────▼──────┐ ┌──▼────────────────┐
│ IBus/Fcitx5  │ │ audio/model/runtime │
│ frontend     │ │ platform services   │
└───────┬──────┘ └───────────────────┘
        │
┌───────▼───────────────┐
│ target application     │
│ cursor preedit/commit  │
└───────────────────────┘
```

架构图:[core-ui-architecture.svg](assets/core-ui-architecture.svg)。

## 2. 进程模型

### 2.1 voxflow-core

常驻 Rust daemon,唯一核心状态源。负责:听写会话、当前模型、当前麦克风、流式 ASR、账本、配置写入、模型下载任务、日志与诊断。

生命周期:

- 推荐以 systemd user service(`voxflow-core.service`,挂接 `graphical-session.target`)启动;portable 模式由托盘/控制台按需拉起。
- **单实例**:启动时抢占 `$XDG_RUNTIME_DIR/voxflow/core.sock`;socket 已被存活进程持有则退出并提示。
- 崩溃恢复:systemd `Restart=on-failure`;UI 检测断连后显示状态并提供手动重启(NFR-REL-01)。
- 优雅退出:收到 `core.shutdown` 或 SIGTERM 后,停止听写会话 → flush 账本与日志 → 持久化下载任务断点 → 退出。

### 2.2 Tauri 控制台

用户界面进程,管理两个窗口:控制台主窗口与全局状态指示器(HUD,见[交互设计 §1.1](../frontend/interaction-design.md))。WebView 不直接访问 UDS;由 Tauri Rust 壳层连接 Core 并转发(见 [Tauri 控制台设计 §3](../frontend/tauri-ui.md))。控制台退出不停止 Core,除非用户选择"退出 VoxFlow"。

### 2.3 输入法前端

Linux 上为 IBus 引擎(进程由 ibus-daemon 拉起)与 Fcitx5 addon(D-15 裁决,二者同属 P0)。前端不跑 ASR、不采集音频,只从 Core 接收会话事件,把 preedit/commit/delete 应用到当前输入上下文,并回报焦点与能力信息。

## 3. Core 内部并发模型

基于 tokio 多任务 + 专用线程:

| 执行单元 | 类型 | 职责 |
| --- | --- | --- |
| audio thread | 专用 OS 线程 | 实时音频回调,只做帧拷贝入 ring buffer,不阻塞 |
| asr worker | 专用 OS 线程 | 消费音频帧,驱动 streaming decoder,产出 AsrEvent |
| pipeline task | tokio task | VAD→稳定判定→后处理→意图分类→安全门→分发 |
| ipc server | tokio task | UDS 监听,每客户端一组读写 task |
| model manager | tokio task | 下载(reqwest)、校验、导入、激活 |
| config/state | tokio task + watch channel | 配置读写、状态广播 |

跨单元通信使用有界 channel;音频路径上禁止无界队列和锁竞争。

## 4. 技术选型基线

具体取舍见[审查与待决策](../review-and-decisions.md) D-1~D-6。

| 能力 | 选型(基线) | 备注 |
| --- | --- | --- |
| 异步运行时 | tokio | |
| IPC 序列化 | serde_json(JSON Lines) | 预留 MessagePack 演进 |
| 音频采集 | pipewire-rs 为主,cpal 兼容 fallback | D-3;过渡期设备**枚举**经 `wpctl status`,采集链路必须 native |
| 流式 ASR 运行时 | sherpa-onnx(C API / sherpa-rs 绑定) | D-1;集成与链接方式见 D-17 |
| VAD | silero-vad(ONNX,经 sherpa-onnx) | 阶段 3 骨架期以 `EnergyVad` 为基线,接入后降为兜底 |
| 意图分类器推理 | ort(ONNX Runtime) | D-5;训练工具链见 D-18 |
| IBus 引擎 | zbus 实现 IBus D-Bus 接口 | D-2;**zbus 固定 =4.4**(zbus 5.x 需 edition2024,与 MSRV 1.75 冲突,见 D-19) |
| 配置 | toml + serde | |
| 日志 | tracing + tracing-appender | |
| 下载/校验 | reqwest(Range 续传)+ sha2 | |

实际 crate 划分(workspace `crates/`):`voxflow-core`(daemon 与业务)、`voxflow-ipc`(envelope 与状态类型)、`voxflow-input`(平台无关 InputEvent/投影层)、`voxflow-asr`、`voxflow-audio`、`voxflow-semantic`(数据集工具)、`voxflow-ibus`、`voxflow-fcitx5`、`voxflow-control`(控制台 bridge/shell 适配与静态原型)。

## 5. 数据流

### 5.1 听写路径

```text
audio device
  -> audio frame (16 kHz mono)
  -> VAD
  -> streaming ASR
  -> partial token
  -> stable detector
  -> postprocess (简繁/标点/助词)
  -> rule precheck + intent classifier
  -> ledger safety gate
  -> input method frontend
  -> cursor preedit/commit/delete
```

### 5.2 控制路径

```text
Tauri UI -> IPC command -> core command handler
  -> config/state/model/audio service
  -> IPC event -> Tauri state update
```

### 5.3 模型下载路径

```text
UI select model -> core model manager -> profile 声明的官方来源
  -> 临时目录分块下载(断点续传)
  -> sha256 校验 -> smoke test -> 原子改名激活
```

## 6. 关键设计原则

1. Core 不依赖 Tauri;可在无 UI 环境运行、测试、诊断。
2. UI 不持有模型运行时,不采集音频,不向目标应用写文本。
3. 输入法前端不采集音频,不跑模型。
4. 平台能力一律通过 trait 注入(见[跨平台策略](../platforms/cross-platform-strategy.md))。
5. 所有跨进程消息版本化(见 [IPC API](ipc-api.md))。
6. 所有删除输入的动作必须经过账本安全门;分类器只能建议。
7. 所有模型切换具备回滚能力。

## 7. 状态机

### 7.1 听写会话

状态:`Idle` `Starting` `Listening` `Paused` `Stopping` `Error`

| 当前状态 | 事件 | 下一状态 | 动作 |
| --- | --- | --- | --- |
| Idle | `dictation.start` | Starting | 加载/复用模型,打开音频流 |
| Starting | 就绪 | Listening | 广播 `dictation.state_changed` |
| Starting | 初始化失败 | Error | 上报 error 事件 |
| Listening | `dictation.pause` / 焦点丢失(可配) | Paused | 暂停解码,保留会话 |
| Paused | `dictation.resume` / 焦点恢复 | Listening | 恢复解码 |
| Listening/Paused | `dictation.stop` | Stopping | flush 解码器,产出 final |
| Stopping | flush 完成 | Idle | 关闭音频流 |
| 任意 | 不可恢复故障 | Error | 记录原因,保留诊断信息 |
| Error | `dictation.start` | Starting | 重新初始化 |

### 7.2 模型

状态:`NotInstalled` `Downloading` `DownloadPaused` `Verifying` `Ready` `Active` `Broken`

| 当前状态 | 事件 | 下一状态 |
| --- | --- | --- |
| NotInstalled | `model.download` | Downloading |
| Downloading | `model.pause` | DownloadPaused |
| DownloadPaused | `model.resume` | Downloading |
| Downloading | 下载完成 | Verifying |
| Downloading/DownloadPaused | `model.cancel` | NotInstalled(清理临时文件) |
| Verifying | 校验+smoke 通过 | Ready |
| Verifying | 校验失败 | Broken(临时文件不进入模型目录) |
| Ready | `model.activate` 成功 | Active(旧 Active → Ready) |
| Ready/Active | 文件损坏被检测 | Broken |
| Broken | `model.verify` 重新通过 / 重新下载 | Ready / Downloading |

### 7.3 输入法前端

状态:`NotInstalled` `Installed` `Registered` `Connected` `Active` `Disconnected`

| 当前状态 | 事件 | 下一状态 |
| --- | --- | --- |
| NotInstalled | 安装组件 | Installed |
| Installed | ibus 注册成功 | Registered |
| Registered | 前端连上 Core socket | Connected |
| Connected | 获得输入焦点且被选为当前输入法 | Active |
| Active | 焦点丢失/切走输入法 | Connected |
| Connected/Active | socket 断开 | Disconnected |
| Disconnected | 重连成功 | Connected |

状态变化通过 `frontend.state_changed` 事件广播给 UI(总览页状态卡片数据来源)。

## 8. 数据与目录

```text
~/.voxflow/                  (可经 VOXFLOW_HOME 重定向, D-8)
  config.toml                配置(Core 唯一写入方)
  models/                    模型目录(按 model_id 分目录)
  cache/                     下载临时文件、可重建数据
  logs/                      滚动日志
  run/                       pid、断点元数据等运行期状态
  ledger/                    账本会话文件(可选持久化,默认仅内存)
```

`$XDG_RUNTIME_DIR/voxflow/core.sock` 为 IPC 端点,目录权限 0700(NFR-PRV-04)。
