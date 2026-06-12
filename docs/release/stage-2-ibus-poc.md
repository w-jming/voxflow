# 阶段 2 IBus POC 记录

日期:2026-06-10

## 已完成

- 新增 `voxflow-ipc`,把 IPC envelope 与状态快照类型从 Core 中抽成共享 crate。
- 新增 `voxflow-input`,定义平台无关 `InputEvent`、`FrontendEvent`、前端能力与 dictation 事件投影。
- `DictationProjector` 将 `dictation.partial` 投影为 `SetPreedit`,将 `dictation.stable/final` 投影为未提交后缀的 `Commit`。
- 自动测试覆盖 FR-INP-10: `"听写中"` 等状态占位文案不得进入 preedit。
- `voxflow-core` 接入 `frontend.register` 与 `frontend.report`,状态快照可反映 IBus/Fcitx5 能力。
- 新增 `voxflow-ibus`,覆盖 component XML 生成、前端注册 JSON、IBus 操作适配层和无桌面 self-test。
- 新增 zbus 4.4 `org.freedesktop.IBus.Engine` 接口骨架;在 Rust 1.75 下通过编译与接口名测试。
- 新增 `org.freedesktop.IBus.Factory` 最小实现;常驻 `voxflow-ibus --ibus-engine` 入口由 Factory 的 `CreateEngine("voxflow")` 动态注册 Engine object。
- 新增 `voxflow-ibus core-roundtrip`,可连接真实 Core UDS,发送 `core.hello`、`frontend.register`、`dictation.start`,并输出 IBus 操作。
- 新增 `voxflow-ibus --ibus-engine --probe-once`,可连接当前 session D-Bus 并注册 `org.freedesktop.IBus.Factory` 后退出。
- 新增 `voxflow-ibus engine-focus-smoke`,通过 zbus engine 的 `FocusIn/FocusOut` handler 驱动 Core bridge。
- 新增用户级 IBus component 安装/卸载脚本,用于 portable POC 的 ibus-daemon 拉起流程。
- `voxflow-core` 的 UDS server 支持 `core.subscribe` 事件广播;订阅连接可接收后续命令触发的 `dictation`/`state` 等事件组。
- `voxflow-ibus` 可将操作发射为 IBus D-Bus signals:`UpdatePreeditText(vub)`、`CommitText(v)`、`DeleteSurroundingText(iu)`;`IBusText` payload 按本机 `IBus.Text.serialize_object()` 形状构造。
- 已用临时 Core + 常驻 `voxflow-ibus --ibus-engine` 完成 D-Bus `CreateEngine("voxflow")`、动态 Engine introspection、`FocusIn`、`FocusOut` smoke。
- 新增 `packaging/linux/ibus/smoke-private-ibus.sh`,在私有 `dbus-run-session` + 临时 `ibus-daemon` 下验证 VoxFlow component cache 可见、daemon 可拉起 VoxFlow bus name、Factory/Engine D-Bus 路径可用。
- 用户级 IBus component 安装/卸载脚本已用临时 `XDG_DATA_HOME` 与 `XDG_CACHE_HOME` smoke,未写真实用户 IBus component 目录。

## 当前限制

- `voxflow-ibus --ibus-engine` 已按 IBus Factory 路径注册,但尚未完成真实 `ibus-daemon` + 桌面应用输入上下文中的 preedit/commit/delete 人工验证。
- 私有 headless `ibus-daemon` 中 `ibus engine voxflow` 会因没有真实桌面输入上下文而返回 `SetGlobalEngine` timeout;脚本将其记录为 warning,但已确认 component 被拉起并可通过 D-Bus `CreateEngine`。
- Core UDS 客户端当前可保持同一 socket session 处理 focus start/stop,并通过 `core.subscribe` 接收事件;尚未接入真实 ASR 连续流。
- 尚未在 GNOME Wayland/X11 中执行真实应用人工 smoke。
- zbus 5.x 当前依赖 Cargo edition2024,不满足项目 Rust 1.75 约束;本阶段固定 zbus 4.4,并将 `async-lock` 精确固定到 3.3.0 以避开 rustc 1.85 MSRV。

## 验证

```bash
cargo run -p voxflow-ibus -- component-xml
cargo run -p voxflow-ibus -- register-json
cargo run -p voxflow-ibus -- self-test
VOXFLOW_HOME=/tmp/voxflow-core-stage2-roundtrip XDG_RUNTIME_DIR=/tmp/voxflow-runtime-stage2-roundtrip cargo run -p voxflow-core -- serve
cargo run -p voxflow-ibus -- core-roundtrip /tmp/voxflow-runtime-stage2-roundtrip/voxflow/core.sock
cargo run -p voxflow-ibus -- engine-focus-smoke /tmp/voxflow-runtime-stage2-roundtrip/voxflow/core.sock
cargo run -p voxflow-ibus -- --ibus-engine --probe-once --core-socket /tmp/voxflow-runtime-stage2-roundtrip/voxflow/core.sock
cargo run -p voxflow-ibus -- --ibus-engine --core-socket /tmp/voxflow-runtime-stage2-roundtrip/voxflow/core.sock
busctl --user call org.freedesktop.IBus.VoxFlow /org/freedesktop/IBus/Factory org.freedesktop.IBus.Factory CreateEngine s voxflow
busctl --user introspect org.freedesktop.IBus.VoxFlow /org/freedesktop/IBus/Engine/VoxFlow/voxflow/1 org.freedesktop.IBus.Engine
busctl --user call org.freedesktop.IBus.VoxFlow /org/freedesktop/IBus/Engine/VoxFlow/voxflow/1 org.freedesktop.IBus.Engine FocusIn
busctl --user call org.freedesktop.IBus.VoxFlow /org/freedesktop/IBus/Engine/VoxFlow/voxflow/1 org.freedesktop.IBus.Engine FocusOut
XDG_DATA_HOME=/tmp/voxflow-ibus-install-smoke XDG_CACHE_HOME=/tmp/voxflow-ibus-cache-smoke \
  packaging/linux/ibus/install-ibus-user.sh /home/terry/workplace/voxflow/target/debug/voxflow-ibus
XDG_DATA_HOME=/tmp/voxflow-ibus-install-smoke XDG_CACHE_HOME=/tmp/voxflow-ibus-cache-smoke \
  packaging/linux/ibus/uninstall-ibus-user.sh
packaging/linux/ibus/smoke-private-ibus.sh /home/terry/workplace/voxflow/target/debug/voxflow-ibus
scripts/dev-check.sh
```

注:`voxflow-core serve` 与常驻 `voxflow-ibus --ibus-engine` 需在 smoke 期间保持运行;当前 Codex 沙箱禁止普通进程创建/连接 UDS 和 session D-Bus,上述 UDS/D-Bus smoke 在本机提权执行,路径全部位于 `/tmp` 或当前用户会话。
