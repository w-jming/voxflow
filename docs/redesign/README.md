# VoxFlow 下一代改版文档库

> **编号** VF-IDX-00 · **版本** 0.4 · **状态** 实施中(阶段 1 完成,阶段 2-6 骨架并行推进) · **最后更新** 2026-06-10

本目录定义 VoxFlow 下一代版本:跨平台、本地优先、token 级流式输出的桌面语音输入法(Rust Core + Tauri 控制台 + 输入法前端)。按 D-7 裁决,旧 Python/GTK 0.2.0 直接废弃,新版从零按本文档库实现,验收标准逐项落地。项目为个人项目,完全由 AI agent 开发(D-10),协作模型见[实施计划 §0](engineering/migration-plan.md)。

**先读**:[术语表与写作规范](glossary.md) → [PRD](product/prd.md) → [审查与决策记录](review-and-decisions.md)。

## 改版核心主张

1. Rust Core 负责音频、流式 ASR、语义撤销、账本、模型管理和本地 IPC;Tauri 负责控制台、托盘与状态指示器;两者严格分离。
2. Linux P0 同时交付 IBus 与 Fcitx5 前端(D-15),抽象层提前为 macOS/Windows 设计。
3. 临时文本显示在光标处(preedit 只含真实识别文本);听写状态由全局状态指示器实时显示(D-12),不依赖系统通知。
4. 真正 token 级流式输出(partial/stable/final),不是"短录音 + 一次性转写"的伪实时。
5. 本地模型默认路线:轻量即用模型 + 高准确率模型下载 + 本地权重导入校验。
6. 语义撤销纳入 P0:规则状态机 + 轻量意图分类器 + 注入账本安全门,默认可用、可关闭;所有删除必须过安全门。

## 文档地图

| 编号 | 文档 | 状态 | 摘要 |
| --- | --- | --- | --- |
| VF-GLS-01 | [术语表与写作规范](glossary.md) | 评审中(待冻结) | 统一术语、文档头、编号与用语约束 |
| **产品** | | | |
| VF-PRD-01 | [PRD](product/prd.md) | 评审中(待冻结) | 定位、目标、指标(含测量口径)、MVP 验收、裁剪规则 |
| VF-PRD-02 | [需求规格](product/requirements.md) | 评审中(待冻结) | FR/NFR 编号化需求与验证方式 |
| VF-PRD-03 | [用户旅程](product/user-journeys.md) | 评审中(待冻结) | UJ-01~09,含前置条件与成功标准 |
| **架构** | | | |
| VF-ARCH-01 | [系统架构](architecture/system-architecture.md) | 评审中(待冻结) | 进程模型、并发模型、技术选型基线、状态机转移表 |
| VF-ARCH-02 | [Core-UI 分离](architecture/core-ui-separation.md) | 评审中(待冻结) | 职责边界、连接模型、握手、多客户端、错误分级 |
| VF-ARCH-03 | [流式 ASR](architecture/streaming-asr.md) | 评审中(待冻结) | 接口、稳定策略参数、候选模型、延迟预算与测量 |
| VF-ARCH-04 | [输入法架构](architecture/input-method.md) | 评审中(待冻结) | 抽象接口、IBus 实现路线、能力降级矩阵 |
| VF-ARCH-05 | [语义撤销](architecture/semantic-correction.md) | 评审中(待冻结) | 账本、冻结策略、三层判断、安全门 |
| VF-ARCH-06 | [语义意图分类器](architecture/semantic-intent-classifier.md) | 评审中(待冻结) | 资源预算、标签、数据集、发布门 |
| VF-ARCH-07 | [模型管理](architecture/model-management.md) | 评审中(待冻结) | profile/manifest、下载续传、导入校验、切换回滚 |
| VF-ARCH-08 | [IPC API 合同](architecture/ipc-api.md) | 评审中(待冻结) | 封皮、命令/事件目录、错误码表、兼容性规则 |
| **设计** | | | |
| VF-DSN-01 | [品牌视觉](design/brand-visual.md) | 评审中(待冻结) | 品牌、色板(主题色不变)、Logo 规则与尺寸 |
| VF-DSN-02 | [UI 系统](design/ui-system.md) | 评审中(待冻结) | 全量设计 token(色彩/字号/阴影/动效/层级)、组件、可访问性 |
| VF-DSN-03 | [主题系统](design/theme-system.md) | 评审中(待冻结) | 三主题切换机制、FOUC 防护、对比度对照表 |
| VF-DSN-04 | [控制台规格](design/control-center-spec.md) | 评审中(待冻结) | 信息架构、各页规格、空态与错误一览 |
| **前端** | | | |
| VF-UI-01 | [Tauri 控制台](frontend/tauri-ui.md) | 评审中(待冻结) | 技术栈、IPC 桥、状态管理、错误展示 |
| VF-UI-02 | [交互设计](frontend/interaction-design.md) | 评审中(待冻结) | 全局状态指示器、快捷键(默认 Alt+S)、托盘、迁移向导、撤销交互 |
| **平台** | | | |
| VF-PLT-01 | [跨平台策略](platforms/cross-platform-strategy.md) | 评审中(待冻结) | trait 抽象、实现矩阵、迁移顺序、禁止假设 |
| VF-PLT-02 | [Linux 实现](platforms/linux.md) | 评审中(待冻结) | IBus 落地、音频、Wayland 矩阵、依赖基线、doctor |
| VF-PLT-03 | [macOS 迁移](platforms/macos.md) | 草案(预研) | IMK 路线与预研问题清单 |
| VF-PLT-04 | [Windows 迁移](platforms/windows.md) | 草案(预研) | TSF 路线与预研问题清单 |
| **工程** | | | |
| VF-ENG-01 | [测试策略](engineering/testing-strategy.md) | 评审中(待冻结) | 测试分层、覆盖目标、性能基准、CI 流水线 |
| VF-ENG-02 | [实施计划](engineering/migration-plan.md) | 评审中(待冻结) | AI agent 协作模型、阶段批次与人工验证点、出入口条件、风险登记 |
| VF-ENG-03 | [打包发布](engineering/packaging-release.md) | 评审中(待冻结) | deb/portable 布局、发布检查单、版本渠道 |
| VF-ENG-04 | [安全与隐私](engineering/security-privacy.md) | 评审中(待冻结) | 威胁模型、日志脱敏、模型供应链、secret 管理 |
| VF-ENG-05 | [Git 工作流](engineering/git-workflow.md) | 评审中(待冻结) | 分支、Conventional Commits、PR 模板 |
| **审查** | | | |
| VF-REV-01 | [审查与决策记录](review-and-decisions.md) | 已裁决 | 文档库审查结论 + D-1~D-16 决策记录与回填明细 |

## 设计资产

- [VoxFlow Logo](design/assets/voxflow-logo.svg) / [深色版](design/assets/voxflow-logo-dark.svg)
- [Symbol](design/assets/voxflow-symbol.svg) / [深色版](design/assets/voxflow-symbol-dark.svg)
- [控制台线框图](design/assets/control-center-wireframe.svg) · [光标处输入反馈流程](design/assets/input-preedit-flow.svg)
- [Core-UI 架构图](architecture/assets/core-ui-architecture.svg)

## 阅读路径

| 角色 | 顺序 |
| --- | --- |
| 产品/验收 | PRD → 需求规格 → 用户旅程 → 测试策略 §8 |
| Core 工程 | 系统架构 → IPC API → 流式 ASR → 语义撤销/分类器 → 模型管理 |
| 前端工程 | 控制台规格 → UI 系统 → 主题系统 → Tauri 控制台 → 交互设计 |
| 输入法工程 | 输入法架构 → Linux 实现 → 跨平台策略 |
| 设计 | 品牌视觉 → UI 系统 → 主题系统 → 线框图 |

## 文档状态与流程

- 文档头格式、状态定义(草案/评审中/已冻结)、编号体系与用语约束见[术语表 §3](glossary.md)。
- 当前阶段:**实施中**——阶段 0/1 完成,阶段 2-6 骨架并行推进,主线 A(真实 ASR 垂直切片)自动化部分已打穿、待 go/no-go 人工签字;阶段执行记录见 `docs/release/stage-*.md`,任务清单见仓库根目录 `todo.md`,实现期新增决策见[审查与决策记录 §3.1](review-and-decisions.md)(D-17~19 已裁决,D-20 工具链政策待确认)。
- 实现期间任何与文档不一致的行为变更,必须同 PR 更新文档并提升版本号(见 [Git 工作流 §5](engineering/git-workflow.md));实现反馈已并入的文档标记为 0.4。
