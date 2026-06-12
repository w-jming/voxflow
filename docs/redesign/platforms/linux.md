# Linux 实现设计

> **编号** VF-PLT-02 · **版本** 0.3 · **状态** 评审中(决策已回填,待冻结) · **最后更新** 2026-06-10

Linux 是 P0 平台。本文档给出落地细节;输入法抽象见[输入法架构](../architecture/input-method.md)。

## 1. 输入法

P0:IBus **与** Fcitx5(D-15 裁决,二者均为 MVP 验收项)。

IBus 实现要点:

- 引擎注册名 `voxflow`,显示名"VoxFlow / 声流输入法"。
- focus in 启动/恢复会话,focus out 暂停(可配置)。
- partial → `update_preedit_text`;stable → `commit_text`;安全门通过的撤销 → `delete_surrounding_text`。
- 实现方式:zbus 纯 Rust 实现 IBus D-Bus 协议(D-2),组件 XML + 引擎进程由 ibus-daemon 拉起。

组件安装位置:

| 安装方式 | component XML | 引擎可执行 |
| --- | --- | --- |
| deb | `/usr/share/ibus/component/voxflow.xml` | `/usr/lib/voxflow/voxflow-ibus` |
| portable | `~/.local/share/ibus/component/voxflow.xml`(由 `install-ibus` 写入) | 包内路径 |

安装后需要 `ibus restart` 或重新登录;doctor 必须能检测"已安装未注册"状态。

Fcitx5 实现要点(薄 C++ addon,路线见[输入法架构 §3.2](../architecture/input-method.md)):

| 安装方式 | 文件 |
| --- | --- |
| deb | addon 动态库 → fcitx5 addon 目录(随发行版路径,构建时探测);`/usr/share/fcitx5/addon/voxflow.conf`;`/usr/share/fcitx5/inputmethod/voxflow.conf` |
| portable | `install-fcitx5` 脚本写入 `~/.local/share/fcitx5/`(addon 动态库需 ABI 匹配,脚本校验 fcitx5 版本) |

## 2. 音频

主路径:PipeWire native(pipewire-rs);fallback:PulseAudio、ALSA(经 cpal,见 [D-3](../review-and-decisions.md))。

- `pw-record` 子进程只能作为诊断工具或最后兼容 fallback,不允许作为流式主链路(对应[流式 ASR §6](../architecture/streaming-asr.md))。
- 设备事件:监听默认输入源变化、蓝牙 profile 变化(A2DP↔HFP),经 `audio.device_changed` 上报。
- 蓝牙修复:检测到仅输出 profile 时,提供切换到 headset profile 的操作(WirePlumber API;不可行时引导系统设置)。

## 3. Wayland 与 X11

| 环境 | IBus | Fcitx5 | 状态指示器 HUD |
| --- | --- | --- | --- |
| GNOME(Wayland) | 好(一等公民) | 可用 | 无 layer-shell → 降级托盘(AppIndicator 扩展) |
| GNOME(X11) | 好 | 可用 | 完整 |
| KDE Plasma(Wayland) | 受限 [待验证] | 好(官方推荐) | layer-shell 可用 [待验证] |
| wlroots 系(Sway 等) | 受限 | 好 | layer-shell 可用 |
| X11 通用 | 好 | 好 | 完整 |

结论:IBus 覆盖 GNOME 系,Fcitx5 覆盖 KDE/wlroots 系,双前端 P0(D-15 裁决)消除桌面覆盖缺口;模拟按键/剪贴板 fallback 在 Wayland 下受安全模型限制,不能作为主路径。指示器降级矩阵见[交互设计 §1.1](../frontend/interaction-design.md)。

## 4. 打包与目录

支持 deb、portable tar、AppImage(P1),细节见[打包与发布](../engineering/packaging-release.md)。

| 内容 | 位置 |
| --- | --- |
| 程序与静态资源 | `/usr/bin`、`/usr/lib/voxflow`、`/usr/share`(deb)或任意解压目录(portable) |
| 大模型 | `~/.voxflow/models/`(`$VOXFLOW_HOME` 可重定向) |
| 日志 / 配置 | `~/.voxflow/logs/`、`~/.voxflow/config.toml` |
| systemd user unit | `/usr/lib/systemd/user/voxflow-core.service`(deb;portable 提供安装脚本) |
| IPC socket | `$XDG_RUNTIME_DIR/voxflow/core.sock` |

## 5. 系统依赖基线

| 依赖 | 用途 | 最低版本(基线) |
| --- | --- | --- |
| ibus | 输入法框架 | 1.5.x(Ubuntu 22.04 自带)[待验证] |
| PipeWire | 音频 | 0.3.x;无 PipeWire 时走 ALSA/Pulse fallback |
| webkit2gtk-4.1 | Tauri WebView | Ubuntu 22.04+ 仓库版本 |
| glibc | 运行时 | 以 Ubuntu 22.04 为构建基线 |

支持承诺:Ubuntu 22.04/24.04、Debian 12 为一级测试目标;Fedora/Arch 为尽力支持。

## 6. 诊断(doctor)

Linux doctor 检查项(输出结构化结果,UI 与 CLI 共用):

1. ibus / fcitx5 是否安装、版本。
2. voxflow 组件是否注册:IBus(`ibus list-engine`)与 Fcitx5(addon 配置存在且版本匹配)。
3. 当前会话输入法框架(IBus/Fcitx5/无)与显示协议(Wayland/X11),及状态指示器档位(HUD/托盘)。
4. PipeWire/Pulse/ALSA 可用性;默认输入源;蓝牙 profile 状态。
5. 模型目录与 manifest.lock 校验。
6. Core socket 存在性与可连接性。
7. systemd user service 状态(如适用)。

每项输出:通过/警告/失败 + 用户可执行的修复建议(UJ-01 的"下一步"数据来源)。
