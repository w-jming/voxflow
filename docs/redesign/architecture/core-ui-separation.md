# Core-UI 分离设计

> **编号** VF-ARCH-02 · **版本** 0.2 · **状态** 评审中 · **最后更新** 2026-06-10

## 1. 分离目标

Rust Core 与 Tauri UI 必须形成清晰边界:Core 可以在无 UI 环境下运行、测试和诊断;UI 在 Core 重启或异常时保持可用并展示状态。验收口径:`voxflow-core` 单独启动后,所有 P0 能力可通过 CLI/IPC 驱动并通过集成测试,全程不加载任何 UI 组件。

## 2. 职责划分

### Core 负责

- 音频设备枚举和采集、VAD、流式 ASR、文本后处理。
- 语义撤销(规则状态机、意图分类器、安全门)与注入账本。
- 输入法前端会话协调。
- 模型下载、导入、校验、激活、回滚。
- 配置读写(唯一写入方)、日志、诊断。

### Core 不负责

- 渲染桌面页面、展示通知样式、管理前端路由、依赖 WebView。

### UI 负责

- 展示状态、发起命令、展示下载进度和诊断结果。
- 修改设置(经 `config.update` 命令,不直接写文件)。
- 打开文件夹或系统设置入口、托盘菜单。

### UI 不负责

- 采集麦克风、加载 ASR 模型、向目标应用写文本、绕过 Core 修改账本或配置文件。

## 3. 连接模型

### 3.1 端点与权限

| 平台 | 端点 |
| --- | --- |
| Linux | `$XDG_RUNTIME_DIR/voxflow/core.sock`(目录 0700) |
| macOS | `~/Library/Application Support/voxflow/run/core.sock` |
| Windows | `\\.\pipe\voxflow-core-<sid>` |

仅同用户进程可连接;不做额外鉴权(本地单用户威胁模型,见[安全与隐私](../engineering/security-privacy.md))。

### 3.2 握手

客户端连接后第一条消息必须是 `hello`:

```json
{ "version": 1, "id": "h-1", "kind": "command", "name": "core.hello",
  "payload": { "client": "ui", "client_version": "0.3.0", "proto_versions": [1] } }
```

Core 应答选定协议版本与自身版本;版本不兼容时返回 `core.proto_unsupported` 错误并断开。`client` 取值:`ui` / `frontend` / `cli`。

### 3.3 多客户端与订阅

- Core 支持多个并发客户端(UI + 输入法前端 + CLI)。
- 事件默认按客户端类型过滤:高频事件(`dictation.partial`、`audio.level`)只推送给声明订阅的客户端(`core.subscribe`),避免无谓唤醒。
- 命令的响应只回给发起方;状态变化事件广播给所有订阅者。

### 3.4 断连与重连

- 客户端断开:Core 清理其订阅;输入法前端断开时,若会话进行中则转入 Paused 并广播状态。
- Core 退出/崩溃:UI 与前端以指数退避重连(0.5 s 起,上限 10 s),期间 UI 显示"Core 未连接"与重启入口。
- 重连成功后客户端必须重新 `hello` + `core.status` 全量同步,不得假设旧状态有效。

## 4. 协议格式

MVP 采用 JSON Lines(UTF-8,每行一条消息,见 [D-6](../review-and-decisions.md));消息封皮与完整目录见 [IPC API 合同](ipc-api.md)。

## 5. 状态所有权

| 状态 | 所有者 | UI 可缓存 | 持久化位置 |
| --- | --- | --- | --- |
| 当前模型 | Core | 是 | `config.toml` |
| 下载进度 | Core | 是 | `run/`(断点元数据) |
| 听写状态 | Core | 是 | 不持久化 |
| preedit 文本 | 输入法前端 | 短暂缓存 | 不持久化 |
| 账本 | Core | 否(只读视图) | 内存,会话文件可选 |
| UI 主题 | Core 持久化,UI 应用 | 是 | `config.toml` `[ui]` |
| 窗口几何 | UI | — | UI 本地存储 |

原则:UI 缓存仅用于渲染,真值永远以 Core 事件为准;UI 不主动推断 Core 状态。

## 6. 错误分级

| 级别 | 含义 | 呈现位置 |
| --- | --- | --- |
| `info` | 普通状态变化 | 控制台状态区 |
| `warning` | 可恢复问题(麦克风暂不可用、分类器降级) | 状态卡片 + 托盘角标 |
| `error` | 当前操作失败(模型校验失败) | 相关页面卡片 + 全局状态条 |
| `fatal` | Core 无法继续运行 | 系统通知 + 控制台醒目提示 |

输入过程中的普通状态在光标处显示;后台异常在控制台和托盘显示;只有 `fatal` 与用户必须立即处理的事项才使用系统通知(对应[交互设计 §1](../frontend/interaction-design.md))。
