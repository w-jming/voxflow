# Tauri 控制台设计

> **编号** VF-UI-01 · **版本** 0.4 · **状态** 评审中(阶段 4 骨架实现反馈已并入) · **最后更新** 2026-06-10

## 1. 定位

Tauri 控制台是 VoxFlow 的桌面控制中心,不是网页 demo,也不是普通设置页。它帮助用户判断软件是否可用、为什么不可用、如何修复,以及当前输入体验处于什么状态。页面信息架构见[控制台规格](../design/control-center-spec.md)。

## 2. 技术栈

| 层 | 选型 | 备注 |
| --- | --- | --- |
| 壳层 | Tauri 2.x(多窗口:控制台 + 状态指示器) | 插件:tray、single-instance;autostart 可选 |
| 语言 | TypeScript(strict) | |
| UI 框架 | React 18 + Vite | 选型理由与备选见 [D-4](../review-and-decisions.md) |
| 状态 | Zustand(或等价轻量 store) | 单一 store,事件驱动 |
| 样式 | CSS variables + CSS Modules | token 见 [UI 系统](../design/ui-system.md),不引入重型 UI 库 |
| 图标 | Lucide | |

原则:不引入超出 Vite 构建体系的复杂依赖;包体与启动时间纳入性能预算(首屏可交互 < 1.5 s,基准机)。

## 3. 进程内架构:IPC 桥

WebView **不能**直接访问 Unix socket。连接 Core 的是 Tauri Rust 壳层:

```text
React (WebView)
  ⇅ tauri invoke / event
Tauri Rust shell (ipc-bridge)
  ⇅ UDS + JSON Lines
voxflow-core
```

ipc-bridge 职责(已在 `crates/voxflow-control` 落地为 `CoreBridge` + `ShellIpcSession`,React 端必须按以下契约消费):

- 维护与 Core 的连接、握手、订阅(`core.subscribe`,UI 默认订阅 state/model/correction;打开音频页时追加 audio_level)。
- 命令转发:前端 `invoke("core_command", { name, payload })` → 写 socket → response 以 Promise 返回。
- 事件通道固定三个:`connection-changed`(连接状态)、`control-snapshot`(`core.status` 全量快照投影)、`core-event`(其余 Core 事件)。
- 断线指数退避重连(0.5 s 起,上限 10 s);重连成功后自动 `core.status` 全量同步并重发 `control-snapshot`。
- CLI smoke:`voxflow-control bridge-status / shell-status / shell-command`,用于无 WebView 验证。

实现注记:当前存在一个**无构建链静态控制台原型**(`voxflow-control write-web`,八页布局 + 主题 token),仅作过渡验证;最终控制台按 §2 用 React 18 + Vite 重建,静态原型在 React 版可用后删除,不进入发布物。

### 3.1 状态指示器窗口(FR-CC-11)

第二个 Tauri 窗口:无边框、置顶、不进任务栏、不可聚焦抢键盘;渲染 [UI 系统 §4.7](../design/ui-system.md) 的 HUD 组件,订阅 `state` + `audio_level` 事件组,状态更新 < 100 ms。

平台定位能力(降级矩阵见[交互设计 §1.1](interaction-design.md)):

| 环境 | 实现 |
| --- | --- |
| X11 | 标准窗口定位 + always-on-top,拖动后记忆坐标 |
| Wayland KDE/wlroots | 经 gtk-layer-shell 把 tao 的 GTK 窗口转为 layer-shell surface,锚定屏幕角 [待验证,阶段 4 POC] |
| Wayland GNOME | 不创建 HUD 窗口,托盘图标承载四态;设置页显示降级说明 |

实现要求:启动时探测合成器能力(`XDG_SESSION_TYPE` + layer-shell 协议可用性)决定档位;指示器崩溃不影响控制台与 Core(独立 WebView,异常时自动重建一次,失败则降级托盘)。

## 4. 页面结构

总览 / 输入 / 模型 / 音频 / 语义修正 / 数据 / 诊断 / 外观 —— 各页内容、状态与空态规格以[控制台规格](../design/control-center-spec.md)为准,本文档不重复。

## 5. 状态管理

单向数据流,UI 不主动推断 Core 状态:

```text
core event -> ipc-bridge -> store action -> selectors -> components
user action -> invoke command -> (await response) -> 等待 Core 事件回流更新 store
```

Store 形状(节选):

```ts
interface AppState {
  connection: "connecting" | "connected" | "disconnected";
  core: { version: string; state: CoreState } | null;
  dictation: { state: DictationState; sessionId: string | null };
  frontend: { kind: string; state: FrontendState; capabilities: string[] };
  audio: { device: AudioDevice | null; level?: number };
  models: { profiles: ModelProfile[]; tasks: Record<string, DownloadTask> };
  corrections: CorrectionRecord[];
  config: ConfigSnapshot; // 以 config_revision 防止回退
}
```

规则:

- 命令成功不直接改 store(避免双写),一律等 Core 事件;只有纯 UI 状态(当前页、对话框开关)本地管理。
- `config.changed` 携带 `config_revision`,store 丢弃旧 revision 的迟到事件。
- 断连时整树进入 stale 态:页面置灰 + 顶部"Core 未连接"条 + 重启按钮(NFR-REL-01)。

## 6. 错误展示

| 错误层级 | 呈现 |
| --- | --- |
| 页面内错误 | 相关卡片内显示,附重试 |
| 全局错误 | 顶部状态条 |
| `fatal` | 系统通知 + 控制台醒目提示 |
| 输入中普通状态 | 光标处 preedit,控制台不抢焦点 |

错误码 → 本地化文案映射表与 [IPC §5 错误码](../architecture/ipc-api.md) 同步维护,未知错误码显示 `message` 兜底并上报日志。

## 7. 可用性原则

- 首页必须回答"现在能不能用"。
- 设置项必须解释当前值,不堆叠低级配置。
- 未实现的模型和后端不出现在普通用户选项中。
- 危险操作(删除模型、迁移目录覆盖)二次确认。
- 下载可暂停、恢复、取消。
- 主题切换即时生效,不重启 Core,不中断输入会话(FR-CC-09)。

## 8. 测试

- 组件层:状态卡片各状态渲染快照;快捷键录入组件交互。
- 集成层:mock ipc-bridge 回放事件流,验证 store 与页面(Core 断连、下载全周期、主题切换)。
- 端到端 smoke:真实 Core + mock recognizer,控制台启动 → 总览四卡就绪 → 触发下载 → 取消(见[测试策略 §4](../engineering/testing-strategy.md))。
