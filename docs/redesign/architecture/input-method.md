# 输入法架构

> **编号** VF-ARCH-04 · **版本** 0.3 · **状态** 评审中(决策已回填,待冻结) · **最后更新** 2026-06-10

## 1. 原则

VoxFlow 的主输入路径必须是系统输入法能力(preedit/commit),而不是剪贴板、模拟按键或窗口焦点劫持。fallback 注入可以保留,但必须明确标注兼容性风险,不得宣称与输入法模式等价(FR-INP-07)。

## 2. 抽象接口

Core → 前端指令:

```text
InputEvent::SetPreedit { text, cursor_pos, underline }
InputEvent::Commit { text }
InputEvent::DeleteBeforeCursor { chars }
InputEvent::ClearPreedit
InputEvent::SessionStarted
InputEvent::SessionStopped
```

前端 → Core 上报(经 `frontend.register` / `frontend.report`,见 [IPC §3.5](ipc-api.md)):

```text
FrontendEvent::Focused { app_hint }
FrontendEvent::Blurred
FrontendEvent::Activated        # 被选为当前输入法
FrontendEvent::Deactivated
FrontendEvent::Capabilities { preedit, surrounding_text, delete_surrounding }
FrontendEvent::SurroundingTextChanged { before_cursor_tail }
```

Core 不出现任何 IBus/Fcitx5 具体类型;前端实现位于独立 crate,经 trait 接入(NFR-MNT-01)。

## 3. Linux

### 3.1 IBus(P0)

实现方式(选型见 [D-2](../review-and-decisions.md)):优先纯 Rust,用 zbus 实现 `org.freedesktop.IBus.Engine` D-Bus 接口并提供 component XML;若 POC 发现协议细节阻塞,退路是薄 C/GLib(libibus)shim 进程,业务仍走 Core IPC。

能力映射:

| VoxFlow 操作 | IBus API |
| --- | --- |
| partial 显示 | `update_preedit_text`(带下划线属性) |
| stable 提交 | `commit_text` |
| 安全门通过的删除 | `delete_surrounding_text` |
| 会话控制 | focus in/out 信号 |

组件注册:

- 系统安装:`/usr/share/ibus/component/voxflow.xml`。
- 用户/portable 安装:`~/.local/share/ibus/component/voxflow.xml` + `ibus write-cache`,由 `install-ibus` 脚本完成。

已知限制(必须在 doctor 与文档中可见):

- 不同应用 surrounding text 支持不一(终端、Electron 常缺失)→ 触发降级矩阵(§7)。
- 部分应用在焦点切换时会把 preedit 残留文本 commit → preedit 中不放非正文占位文案,见 [D-12](../review-and-decisions.md)。
- KDE/wlroots 系 Wayland 对 IBus 支持弱于 GNOME → 由 Fcitx5 前端覆盖(§3.2)。

### 3.2 Fcitx5(P0,D-15 裁决)

与 IBus 能力对等(FR-INP-09)。实现路线:**薄 C++ addon**(Fcitx5 引擎只能以 C++ addon 形式加载),只做协议翻译,经 UDS 连 Core,业务逻辑零下沉——与 IBus 前端同构。

- 安装文件:addon 动态库(fcitx5 addon 目录)+ `addon/voxflow.conf` + `inputmethod/voxflow.conf`。
- 验收口径与 IBus 相同:preedit/commit/delete、焦点与能力上报、KDE Plasma(Wayland)会话真实应用 smoke。
- Fcitx5 同时是平台抽象层的第二实现,用于检验 trait 设计(见[跨平台策略 §3](../platforms/cross-platform-strategy.md))。

## 4. macOS(P1 spike)

目标 InputMethodKit:

```text
IMK input method bundle -> UDS -> voxflow-core -> streaming ASR
```

bundle 负责 composition(对应 preedit)、commit、删除最近 VoxFlow 文本、输入源激活状态。需处理麦克风权限、输入法安装(`~/Library/Input Methods`)、签名与 notarization。不依赖辅助功能模拟键盘作为主路径。详见 [macOS 迁移设计](../platforms/macos.md)。

## 5. Windows(P2 spike)

目标 TSF Text Service:

```text
TSF text service -> named pipe -> voxflow-core -> streaming ASR
```

负责 composition、commit、surrounding text、账本范围内的删除替换。详见 [Windows 迁移设计](../platforms/windows.md)。

## 6. fallback 注入

仅用于:用户未安装输入法前端、平台早期原型、调试兼容模式。UI 中必须显示"兼容模式,部分应用可能异常"。Wayland 下模拟按键/剪贴板受安全模型限制,永远不作为主路径。

## 7. 能力降级矩阵

Core 根据 `frontend.register` 声明的能力 + 运行期探测,逐应用选择档位并经 `frontend.state_changed` 上报 UI(FR-INP-08):

| 档位 | 条件 | 行为 |
| --- | --- | --- |
| 完整 | preedit + surrounding + delete | partial 上屏、stable commit、安全门撤销可用 |
| 仅提交 | 无可靠 preedit | partial 不上屏(状态指示器仍显示听写中与电平),stable 直接 commit |
| 受限撤销 | 无 surrounding text | 撤销仅允许 `undo_last` 且窗口缩短(光标上下文校验降级,见[语义撤销 §5](semantic-correction.md)) |
| 兼容注入 | 无输入法通道 | fallback 注入,撤销完全禁用 |

## 8. 光标处内容与状态通道(D-12 裁决)

preedit/composition **只承载真实识别文本**(partial、修正中的短暂样式),全局禁止"听写中"等非正文占位文案(FR-INP-10)——占位文案存在被目标应用在焦点变化时意外 commit 成脏文本的风险,且无法在所有应用上验证安全。

听写状态(空闲/听写中/处理中/错误)与电平统一由**全局状态指示器**承载,要求实时(< 100 ms),设计见[交互设计 §1.1](../frontend/interaction-design.md)。

系统通知只用于:Core 崩溃、麦克风权限缺失、模型不可用、下载完成或失败(与[核心错误分级](core-ui-separation.md)一致)。
