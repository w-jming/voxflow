# IPC API 合同

> **编号** VF-ARCH-08 · **版本** 0.4 · **状态** 评审中(阶段 1-6 骨架实现反馈已并入) · **最后更新** 2026-06-10

实现状态:本合同已由 `crates/voxflow-ipc`、`crates/voxflow-core` 落地并经集成测试覆盖;实现期新增的 schema(`model.*` 详式、`audio.list_devices`、`correction.applied.input_events`)与扩充错误码均以本文档为准。`schemas/ipc/` 维护机器可读 envelope schema。

Core 与 UI/输入法前端/CLI 之间的本地协议。传输与连接模型见 [Core-UI 分离 §3](core-ui-separation.md)。本文档是实现与测试的唯一合同;新增或修改消息必须先改本文档并提升 `proto` 小版本。

## 1. 封皮(Envelope)

每行一条 UTF-8 JSON 消息。四种 `kind`:

```json
{ "version": 1, "id": "c-42", "kind": "command",  "name": "model.download", "payload": {} }
{ "version": 1, "id": "c-42", "kind": "response", "name": "model.download", "payload": {} }
{ "version": 1, "kind": "event", "name": "dictation.partial", "payload": {} }
{ "version": 1, "id": "c-42", "kind": "error", "code": "model.not_found",
  "message": "未找到模型", "recoverable": true, "details": {} }
```

规则:

- `version`:协议主版本。不兼容变更才提升;字段新增属兼容变更。
- `id`:命令由客户端生成(建议自增前缀),response/error 原样回带;事件无 `id`。
- 每条命令必有且仅有一条 response 或 error;长任务的进度经事件推送。
- 未知字段必须忽略(向前兼容);未知 `name` 返回 `core.unknown_command`。

## 2. 命令目录

| 命令 | 方向 | 说明 |
| --- | --- | --- |
| `core.hello` | 任意客户端 | 握手,声明客户端类型与协议版本 |
| `core.status` | UI/CLI | 全量状态快照 |
| `core.subscribe` | 任意客户端 | 订阅事件组:`dictation` `audio_level` `model` `correction` `state` |
| `core.shutdown` | UI/CLI | 优雅退出 Core |
| `config.get` | UI/CLI | 读取当前配置(脱敏后) |
| `config.update` | UI/CLI | 局部更新配置,Core 校验并持久化 |
| `dictation.start` | UI/前端/CLI | 开始听写会话 |
| `dictation.stop` | UI/前端/CLI | 停止并 flush |
| `dictation.pause` / `dictation.resume` | UI/前端 | 暂停/恢复 |
| `model.list` | UI/CLI | 列出全部 profile 与本地状态 |
| `model.download` | UI/CLI | 开始/继续下载 |
| `model.pause` / `model.resume` / `model.cancel` | UI/CLI | 下载控制 |
| `model.import` | UI/CLI | 本地导入(复制/软链接) |
| `model.verify` | UI/CLI | 重新校验本地模型 |
| `model.activate` | UI/CLI | 切换当前模型(失败自动回滚) |
| `model.delete` | UI/CLI | 删除非 Active 模型 |
| `audio.list_devices` | UI/CLI | 枚举输入设备及蓝牙 profile 信息 |
| `audio.select_device` | UI/CLI | 选择输入设备 |
| `audio.test_start` / `audio.test_stop` | UI | 录音测试(电平经事件推送) |
| `correction.list_recent` | UI | 最近修正记录 |
| `correction.revert` | UI | 恢复一条修正(经安全门) |
| `correction.feedback` | UI | 标记误触发/漏触发样本 |
| `frontend.register` | 输入法前端 | 前端上线,声明能力 |
| `frontend.report` | 输入法前端 | 焦点/能力/surrounding text 变化上报 |
| `diagnostics.run` | UI/CLI | 执行 doctor,返回结构化结果 |

## 3. 关键命令 schema

### 3.1 core.status → response

```json
{
  "core": { "version": "0.3.0", "state": "running", "uptime_ms": 123456 },
  "dictation": { "state": "idle", "session_id": null },
  "frontend": { "kind": "ibus", "state": "connected", "capabilities": ["preedit", "surrounding_text"] },
  "audio": { "device_id": "alsa:...", "label": "内置麦克风", "available": true,
             "bluetooth_profile": null },
  "models": { "active_asr": "streaming-zh-en-small", "active_refiner": null,
              "intent_classifier": { "state": "ready", "version": "0.1.0" } },
  "paths": { "home": "/home/u/.voxflow", "logs": "...", "models": "...", "cache": "..." },
  "config_revision": 17
}
```

### 3.2 dictation.start

```json
{ "frontend": "ibus", "mode": "continuous" }
```

`mode`:`continuous` | `hold`。response 返回 `{ "session_id": "s-20260610-001" }`。

### 3.3 config.update

```json
{ "patch": { "ui": { "theme": "dark" }, "correction": { "enabled": false } } }
```

- `patch` 为深合并;Core 校验失败返回 `config.invalid` 且整体不生效。
- 成功后广播 `config.changed`(含新 `config_revision`),所有客户端据此刷新。

### 3.4 model.list / model.verify

`model.list` response:

```json
{ "models": [
  { "profile": { "id": "streaming-zh-en-small", "kind": "asr-streaming",
                 "backend": "sherpa-onnx", "license": "Apache-2.0" },
    "source": { "url": "https://…", "size_bytes": 104857600 },
    "local": { "state": "not_installed", "path": "~/.voxflow/models/streaming-zh-en-small",
               "manifest_present": false, "total_size_bytes": 0, "issues": [] },
    "profile_issues": [] }
] }
```

`model.verify` request/response:

```json
{ "model_id": "streaming-zh-en-small" }
```

```json
{ "model": { "profile": { "id": "streaming-zh-en-small" },
             "local": { "state": "ready", "manifest_present": true,
                        "issues": [] } } }
```

`local.state`:`not_installed` | `ready` | `active` | `broken`。`broken` 必须带 `issues`,例如 `model.file_missing:encoder.int8.onnx`、`model.checksum_failed:tokens.txt`。profile 自身的来源或 checksum 占位问题放入 `profile_issues`,不得把占位 profile 判定为可用模型。

### 3.5 model.activate / model.delete

`model.activate` request/response:

```json
{ "model_id": "streaming-zh-en-small" }
```

```json
{ "previous_active_asr": "old-model", "active_asr": "streaming-zh-en-small",
  "runtime_smoke": "pending_runtime_integration",
  "model": { "profile": { "id": "streaming-zh-en-small" },
             "local": { "state": "active", "issues": [] } } }
```

当前实现只允许 `ready`/`active` 本地模型进入 active 配置;保存配置失败必须回滚。真实 runtime load 与 smoke inference 接入后,`runtime_smoke` 必须从 `pending_runtime_integration` 升级为实际结果。

`model.delete` request/response:

```json
{ "model_id": "streaming-zh-en-small" }
```

```json
{ "delete": { "model_id": "streaming-zh-en-small", "deleted": true,
              "released_bytes": 104857600, "path": "~/.voxflow/models/streaming-zh-en-small" } }
```

Active 模型不可删除,必须返回 `model.active_locked`。

### 3.6 model.download / model.import

```json
{ "model_id": "qwen3-asr-1.7b" }
```

```json
{ "model_id": "qwen3-asr-1.7b", "path": "/data/models/Qwen3-ASR-1.7B", "mode": "symlink" }
```

`model.download` response 立即返回 `{ "task_id": "t-7" }`;进度走 `model.progress` 事件。`model.import` 可同步完成并返回 `{ "task_id": "import-…", "import": { … } }`,后续大目录复制可升级为后台任务但必须保留 `task_id`。`mode`:`copy` | `symlink`。

实现语义(2026-06-11 落地):profile 的 `source.url` 为以 `/` 结尾的按文件下载基址(Hugging Face `resolve/main/`),文件经 `<models>/.staging-<id>/` 暂存(`.part` 后缀 + HTTP Range 断点续传)→ 全量 sha256 校验 → 原子改名安装 + manifest.lock。`model.pause` 停止 worker 并保留 `.part`;`model.resume` 等价于重新 `model.download`(自动续传);`model.cancel` 停止并删除暂存目录。模型只落在用户数据目录(`VOXFLOW_HOME`,默认 `~/.voxflow`),不写系统目录。

### 3.7 frontend.register

```json
{ "kind": "ibus", "frontend_version": "0.3.0",
  "capabilities": ["preedit", "surrounding_text", "delete_surrounding"] }
```

Core 据 `capabilities` 决定降级策略(见[输入法架构 §7](input-method.md))。

### 3.8 audio.list_devices

response:

```json
{
  "devices": [
    { "id": "pipewire:55", "label": "Built-in Audio Analog Stereo",
      "backend": "pipe_wire", "is_default": true, "available": true,
      "bluetooth_profile": null, "sample_rate_hz": null, "channels": null }
  ],
  "default_device_id": "pipewire:55",
  "warnings": [],
  "probe": { "pipewire_command": true, "pw_cli_command": true,
             "wpctl_command": true, "pw_record_command": true,
             "libpipewire_runtime": true,
             "pkg_config_development_files": false,
             "version": "1.0.5" }
}
```

规则:

- P0 Linux 先用 `wpctl status` 枚举输入 source,后续 PipeWire native capture 接入后复用同一 ID。
- 如果当前用户会话无法连接 PipeWire,命令仍返回 response,`devices=[]` 并在 `warnings` 中说明原因,不得把控制台拖入 fatal。
- `pw-record` 只在 probe 中报告可用性,不得因此被视为主采集链路。

## 4. 事件目录

| 事件 | 订阅组 | 触发 |
| --- | --- | --- |
| `core.state_changed` | state | Core 生命周期变化 |
| `dictation.state_changed` | state | 会话状态机转移 |
| `dictation.partial` | dictation | 新 partial(高频,需订阅) |
| `dictation.stable` | dictation | stable 判定,已 commit 并入账 |
| `dictation.final` | dictation | segment 结束 |
| `correction.applied` | correction | 修正已执行 |
| `correction.rejected` | correction | 建议被安全门拒绝(含原因) |
| `correction.reverted` | correction | 修正被恢复 |
| `model.progress` | model | 下载/校验进度(节流 ≤ 4 次/秒) |
| `model.state_changed` | model | 模型状态机转移 |
| `audio.level` | audio_level | 输入电平(节流 ≤ 20 次/秒,需订阅) |
| `audio.device_changed` | state | 默认/选定设备变化 |
| `frontend.state_changed` | state | 前端状态机转移 |
| `config.changed` | state | 配置生效 |
| `core.notice` | state | 分级通知(info/warning/error/fatal) |

### 4.1 听写事件 schema

```json
{ "session_id": "s", "revision": 12, "text": "今天下午",
  "stable_prefix_chars": 4 }
```

```json
{ "session_id": "s", "segment_id": "seg-3", "text": "今天下午",
  "tokens": [{ "t": "今天", "ms": [120, 360] }] }
```

```json
{ "session_id": "s", "segment_id": "seg-3", "text": "今天下午三点开会",
  "refined": false }
```

约定:`revision` 单调递增;`partial` 整体替换上一次 partial;`stable` 表示该文本已 commit,前端不得再修改,除非收到经安全门的 correction 指令。

### 4.2 correction.applied

```json
{ "operation_id": "op-9", "intent": "replace_entity",
  "target": "三点", "replacement": "四点",
  "segments": ["seg-3"], "confidence": 0.86, "reason_code": "repair_marker_and_entity_pair",
  "input_events": [
    { "type": "delete_before_cursor", "chars": 8 },
    { "type": "commit", "text": "今天下午四点开会" }
  ] }
```

`input_events` 使用[输入法架构](input-method.md)定义的平台无关 `InputEvent` 序列。前端必须按顺序执行;如果事件缺失或无法解析,不得自行猜测删除范围。

### 4.3 model.progress

```json
{ "task_id": "t-7", "model_id": "qwen3-asr-1.7b", "phase": "downloading",
  "downloaded": 123456789, "total": 987654321, "speed_bps": 3456789, "eta_s": 250 }
```

`phase`:`downloading` | `verifying` | `installing` | `done` | `failed`。

### 4.4 core.notice

```json
{ "level": "warning", "code": "audio.bt_output_only",
  "message": "蓝牙耳机当前为仅输出模式", "action_hint": "open_audio_page" }
```

## 5. 错误码

错误码必须稳定;UI 据 code 本地化文案,`message` 仅为兜底。命名:`<域>.<原因>`。

| code | recoverable | 含义 |
| --- | --- | --- |
| `core.unknown_command` | true | 未知命令 |
| `core.proto_unsupported` | false | 协议版本不兼容 |
| `core.busy` | true | 互斥任务进行中(如同时两个下载激活) |
| `config.invalid` | true | 配置校验失败,`details.field` 指明字段 |
| `dictation.no_frontend` | true | 无可用输入法前端 |
| `dictation.audio_unavailable` | true | 无可用输入设备 |
| `dictation.model_unavailable` | true | 当前 ASR 模型不可用 |
| `model.not_found` | true | 未知 model_id |
| `model.profile_unavailable` | true | 内置 profile 目录不可用或 profile 解析失败 |
| `model.profile_invalid` | true | profile 缺少必需字段或 checksum 格式非法 |
| `model.not_ready` | true | 目标模型未安装或本地状态为 broken,不能激活 |
| `model.activate_failed` | true | 目标模型通过校验,但持久化 active 配置失败 |
| `model.import_source_invalid` | true | 本地导入源目录不存在或不可用 |
| `model.import_verify_failed` | true | 本地导入源目录未通过必需文件、格式或 checksum 校验 |
| `model.already_installed` | true | 目标模型目录已存在,当前导入不会覆盖 |
| `model.symlink_unsupported` | true | 当前平台不支持 symlink 导入 |
| `model.checksum_failed` | true | 校验失败 |
| `model.smoke_failed` | true | smoke test 失败 |
| `model.disk_full` | true | 磁盘空间不足(预检) |
| `model.source_unreachable` | true | 下载源不可达(含 profile 占位 URL/非 https) |
| `model.download_failed` | true | 下载任务失败兜底码(能归入上述稳定码的优先用稳定码,详情见 message) |
| `model.active_locked` | true | 不能删除 Active 模型 |
| `correction.gate_rejected` | true | 安全门拒绝,`details.gate` 指明失败项 |
| `correction.disabled` | true | 智能撤销已关闭 |
| `frontend.capability_missing` | true | 前端不支持所需能力 |

`correction.list_recent` 当前返回 `{ "records": [] }` 或最近修正记录数组。记录字段与[语义撤销 §6](semantic-correction.md)一致,包括 `operation_id`、`intent`、`target`、`replacement`、`confidence`、`reason_code` 和安全门结果。

## 6. 兼容性与测试

- 协议主版本变更必须提供迁移说明;Core 至少兼容上一个主版本一个发布周期。
- 仓库内维护机器可读 schema(JSON Schema 或 Rust 类型导出),CI 对消息样例做双向校验。
- 集成测试覆盖:握手、订阅过滤、断连重连全量同步、每个错误码至少一条用例(见[测试策略 §3](../engineering/testing-strategy.md))。
