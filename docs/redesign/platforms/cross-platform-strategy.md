# 跨平台策略

> **编号** VF-PLT-01 · **版本** 0.3 · **状态** 评审中(决策已回填,待冻结) · **最后更新** 2026-06-10

## 1. 目标

VoxFlow 下一代从 Linux 起步,但必须提前为 macOS 和 Windows 设计抽象。核心业务不能绑定 IBus、PipeWire 或 X11(NFR-MNT-01)。

## 2. 平台抽象

Core 定义 trait,平台实现以独立 crate 注入:

| Trait | 契约(一句话) |
| --- | --- |
| `AudioBackend` | 枚举输入设备、打开 16 kHz mono 帧流、上报设备/profile 变化 |
| `InputFrontendChannel` | 与输入法前端的会话通道(preedit/commit/delete 指令,焦点/能力上报) |
| `NotificationBackend` | 发送分级系统通知 |
| `TrayBackend` | 托盘图标状态与菜单(MVP 由 Tauri 承担,trait 预留无 UI 场景) |
| `PermissionBackend` | 查询/引导麦克风等权限(Linux 基本为空实现) |
| `ModelRuntime` | 加载/卸载模型,执行 streaming/offline 推理 |

实现矩阵:

| 能力 | Linux | macOS | Windows |
| --- | --- | --- | --- |
| 音频 | PipeWire(fallback ALSA/Pulse) | CoreAudio | WASAPI |
| 输入法 | IBus / Fcitx5 | InputMethodKit | TSF |
| IPC | Unix socket | Unix socket | Named pipe |
| 托盘 | StatusNotifier/AppIndicator | NSStatusItem | Shell NotifyIcon |
| 打包 | deb/AppImage/tar | dmg/pkg | MSIX/NSIS |

工程实现:

- crate 划分:`voxflow-core`(纯业务)+ `voxflow-platform-linux/...`(trait 实现)+ `voxflow-frontend-ibus/...`。
- 条件编译:`#[cfg(target_os)]` 只出现在 platform crate;core 内禁止。
- CI 至少保持 `cargo check` 三平台编译矩阵,防止抽象腐化(P1 起)。

## 3. 迁移顺序

1. Linux Rust Core + IBus(P0)
2. Linux Tauri 控制台 + 状态指示器(P0)
3. Linux Fcitx5(P0,D-15 裁决)
4. macOS 音频 + 控制台(P1 spike)
5. macOS InputMethodKit(P1 spike)
6. Windows 音频 + 控制台(P2)
7. Windows TSF(P2)

"trait 抽象演练"在 P0 内即完成:Fcitx5 作为第二个输入法前端实现,直接检验抽象的真实可移植性,之后才投入 macOS/Windows。

## 4. 禁止的跨平台假设

Core 业务逻辑禁止直接假设以下内容存在(必须在平台层处理):

- `/usr/bin` 路径、`notify-send`、`ibus` 命令、`pw-record`。
- X11 window id。
- 文本删除能力一定可用(见[能力降级矩阵](../architecture/input-method.md))。
- 路径分隔符、可执行权限位等 POSIX 细节。

违反项通过代码评审 + `voxflow-core` crate 的依赖白名单(不得依赖平台 crate)约束。
