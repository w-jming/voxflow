# 术语表与文档写作规范

> **编号** VF-GLS-01 · **版本** 0.3 · **状态** 评审中(决策已回填,待冻结) · **最后更新** 2026-06-10

本文档定义改版文档库中的统一术语和写作规范。所有文档必须使用本表术语,不得自创同义词。

## 1. 核心术语

| 术语 | 英文 | 定义 |
| --- | --- | --- |
| Core | voxflow-core | 常驻 Rust 后台进程,唯一核心状态源,负责音频、ASR、账本、模型、配置 |
| 控制台 | Control Center | Tauri 桌面 UI 进程,只展示状态和发起命令 |
| 输入法前端 | Input Frontend | IBus/Fcitx5/IMK/TSF 引擎进程,负责 preedit/commit/delete |
| 状态指示器 | Status Indicator / HUD | 常驻小型悬浮窗,实时显示听写状态与电平;不可用环境降级为托盘图标 |
| 听写会话 | Dictation Session | 一次从开始听写到停止听写的完整过程 |
| 临时文本 | partial | ASR 当前猜测,可能变化,只显示为 preedit,不写入目标应用 |
| 稳定文本 | stable | 已通过稳定判定的文本,commit 到目标应用并写入账本 |
| 最终文本 | final | 一个 segment 结束后的最终结果,可触发二阶段精修 |
| 预编辑文本 | preedit / composition | 输入法在光标处显示的未提交文本 |
| 提交 | commit | 输入法把文本真正写入目标应用 |
| 光标周边文本 | surrounding text | 输入法可读取的光标前后文本,用于账本校验 |
| 注入账本 | Injection Ledger | Core 记录的"VoxFlow 自己提交过哪些文本"的结构化日志 |
| 账本片段 | LedgerSegment | 账本中一次 commit 对应的记录单元 |
| 安全门 | Safety Gate | 删除/替换执行前必须全部通过的账本校验规则集 |
| 意图分类器 | Intent Classifier | 判断片段是字面输入还是撤销/修正意图的本地轻量模型 |
| 修正窗口 | Repair Window | 允许被 final/refine/撤销修改的最近 token/segment 范围 |
| 冻结 | frozen | 账本片段超出修正窗口后不可再被自动修改的状态 |
| VAD | Voice Activity Detection | 语音活动检测,判断当前音频帧是否包含语音 |
| 首 token 延迟 | first-partial latency | 从 VAD 判定语音起点到第一个 partial 事件的耗时 |
| 模型描述档 | Model Profile | 描述一个模型的 id、来源、checksum、许可证等元数据 |
| 冒烟测试 | smoke test | 最小可用性验证,例如模型加载后跑一条固定样本 |
| 一键诊断 | doctor | 汇总环境、依赖、注册、权限检查结果的诊断命令 |
| 基准机 | Reference Machine | 性能指标的测量基线:4 核 x86_64(i5-8250U 级)、8 GB 内存、无独立显卡 |

## 2. 平台与技术缩写

| 缩写 | 全称 | 说明 |
| --- | --- | --- |
| IBus | Intelligent Input Bus | Linux 主流输入法框架,基于 D-Bus,GNOME 默认 |
| Fcitx5 | — | Linux 输入法框架,KDE/wlroots 系 Wayland 下支持更好 |
| IMK | InputMethodKit | macOS 输入法框架 |
| TSF | Text Services Framework | Windows 输入法框架 |
| UDS | Unix Domain Socket | 本地进程间通信通道,Linux/macOS 使用 |
| JSONL | JSON Lines | 每行一个 JSON 消息的流式协议格式 |
| LCP | Longest Common Prefix | 连续 partial 的最长公共前缀,token 稳定判定手段之一 |
| layer-shell | zwlr_layer_shell_v1 | Wayland 协议,允许客户端把窗口锚定到屏幕边角;KDE/wlroots 支持,GNOME 不支持 |

## 3. 文档写作规范

### 3.1 文档头

每篇文档第一行标题之后必须包含元数据行:

```markdown
> **编号** VF-XXX-NN · **版本** X.Y · **状态** 草案|评审中|已冻结 · **最后更新** YYYY-MM-DD
```

- **草案**:内容未完成,不可作为实现依据。
- **评审中**:内容完整,等待评审确认。
- **已冻结**:实现依据;修改必须提升版本号并在 PR 中说明影响。

### 3.2 编号体系

| 前缀 | 范围 |
| --- | --- |
| VF-PRD-* | 产品文档 |
| VF-ARCH-* | 架构文档 |
| VF-DSN-* | 设计文档 |
| VF-UI-* | 前端文档 |
| VF-PLT-* | 平台文档 |
| VF-ENG-* | 工程文档 |
| FR-* / NFR-* | 功能/非功能需求条目(见[需求规格](product/requirements.md)) |
| UJ-* | 用户旅程 |
| D-* | 待决策项(见[审查与待决策](review-and-decisions.md)) |

### 3.3 约束力用语

参照 RFC 2119 习惯:

- **必须**:不满足即验收失败。
- **应**:默认遵守,偏离需要在 PR 或决策记录中说明理由。
- **可**:可选项,实现者自行决定。

### 3.4 数值与待验证标注

- 所有性能指标必须注明测量条件(默认为基准机)。
- 来自外部资料、尚未在本项目实测的数值必须标注 `[待验证]`。
- 相对日期一律写为绝对日期(YYYY-MM-DD)。

### 3.5 跨文档引用

- 使用相对路径 Markdown 链接,链接文字使用文档标题或编号。
- 引用需求时直接使用 FR/NFR 编号,不复述需求原文。
