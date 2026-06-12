# 整体审查报告与决策记录

> **编号** VF-REV-01 · **版本** 0.5 · **状态** D-1~19 已裁决(D-17/D-18 于 2026-06-11 批复);D-20 待确认 · **最后更新** 2026-06-11 · **审查人** Claude(应改版要求)

本报告是对 `docs/redesign/` 文档库的整体审查结论与决策记录。0.2 版提出 D-1~D-16 待决策清单;所有者已于 2026-06-10 批复,本版(0.3)记录裁决并已将其回填全部受影响文档。

---

## 1. 总体评价(首轮审查结论,保留备查)

原文档库(0.1 版)**方向正确、覆盖面完整**,核心架构判断(Core-UI 分离、输入法 preedit 为主反馈、账本安全门、本地优先)均成立。主要缺口:

| 维度 | 原状态 | 主要缺口 |
| --- | --- | --- |
| 可行性 | 中 | 无工期与阶段出入口;关键技术未落选型;无风险登记;指标无测量口径 |
| 明确性 | 中 | 需求无编号无验收方式;IPC 合同残缺;"状态机"缺转移条件 |
| 规范性 | 弱 | 无文档元数据、术语表、编号体系;"冻结"无判定标准 |
| 设计完备度 | 中 | 缺字号/阴影/动效/层级/焦点态;白字在 #0EA5E9 上 ≈2.8:1 不达 AA 未被发现 |

## 2. 修订摘要

### 2.1 第一轮(0.2,体系化重写)

27 篇文档全部重写并新增术语表:需求编号化(FR/NFR)、IPC 全量合同(26 命令/15 事件/16 错误码)、状态机转移表、技术选型基线、设计 token 补全(字号/阴影/动效/层级/对比度)、阶段化实施计划与风险登记。逐文件明细见本文档 0.2 版(git 历史)。

### 2.2 第二轮(0.3,裁决回填)

| 裁决 | 回填文档 |
| --- | --- |
| D-7 旧版直接废弃 | migration-plan(更名"实施计划",删除旧版维护/数据迁移章节,新增 §3 旧版处置)、prd §3 |
| D-10 个人项目 + AI agent | migration-plan(§0 协作模型与工程纪律、批次/人工验证点制、风险表更新) |
| D-12 全局状态指示器 | interaction-design §1.1(指示器规格与降级矩阵)、input-method §8、ui-system §4.7(HUD 组件)、tauri-ui §3.1(指示器窗口)、requirements FR-CC-11/FR-INP-10、prd、user-journeys UJ-02、control-center-spec 输入页、linux §3、testing-strategy、glossary |
| D-14 默认快捷键 Alt+S | interaction-design §2、user-journeys UJ-02 |
| D-15 Fcitx5 进 P0 | prd(P0 清单、MVP 验收 9 项、裁剪规则)、requirements FR-INP-09、input-method §3.2(薄 C++ addon 路线)、linux §1/§3/§6、cross-platform §3、migration-plan 阶段 6、packaging(deb 布局、install-fcitx5)、testing-strategy(双会话 smoke) |
| D-16 引入主色阶 | ui-system §2.1(`--vf-primary-50…900` 阶梯 + 按钮三态全 AA)、theme-system §5/§7、brand-visual §2、control-center-spec 选中态 token |

---

## 3. 决策记录(D-1 ~ D-16)

未附批注的决策项,所有者未提出异议,按 0.2 版建议方案执行;如需变更,在阶段 0 结束前提出并按冻结流程修订。

| # | 议题 | 裁决(2026-06-10) | 状态 |
| --- | --- | --- | --- |
| D-1 | 默认流式 ASR | sherpa-onnx + streaming Zipformer 双语(int8)首选,Paraformer 并行 POC 备选;POC 验收:基准机首 partial < 500 ms (p90)、RTF < 1 | 采纳建议 |
| D-2 | IBus 引擎实现 | 纯 Rust(zbus)优先,阶段 2 首批次 POC 检查点,受阻切 C/GLib shim | 采纳建议 |
| D-3 | 音频采集库 | pipewire-rs 为主 + cpal(ALSA)fallback | 采纳建议 |
| D-4 | 前端框架 | React 18 + Vite + Zustand | 采纳建议 |
| D-5 | 分类器 embedding | 先 bge-small-zh int8 + 英文规则特征;英文 F1 不达标升 multilingual MiniLM;n-gram 线性模型为兜底 | 采纳建议 |
| D-6 | IPC 序列化 | JSON Lines;MessagePack 推迟 | 采纳建议 |
| D-7 | 旧 Python 版本 | **所有者裁决:不保留、不归档,直接重写新版;无旧版用户,不做任何用户/配置迁移** | 已裁决 ✦ |
| D-8 | 用户数据目录 | 沿用 `~/.voxflow` + `VOXFLOW_HOME`(注:D-7 后无兼容包袱,保留此选择因单目录实现最简) | 采纳建议 |
| D-9 | Qwen3-ASR 定位 | 仅 final 精修与高质量离线模式(P1),不承诺实时 | 采纳建议 |
| D-10 | 时间线与资源 | **所有者裁决:个人项目,完全由 AI agent 开发**;实施计划改为"阶段退出条件 + 人工验证点"制,不设硬日历承诺;参考周期 8-14 周(取决于验证带宽) | 已裁决 ✦ |
| D-11 | 仓库策略 | 现仓库 monorepo,新建 Rust workspace;结合 D-7:Python 实现直接删除(不入 legacy/),历史靠 git 追溯 | 采纳建议(按 D-7 修订) |
| D-12 | 听写状态提示方式 | **所有者裁决:preedit 占位文案全局禁用(含白名单方案);改用全局通用、实时的 GUI 提示** → 设计为全局状态指示器(HUD):X11 悬浮窗 / KDE·wlroots layer-shell / GNOME Wayland 托盘降级,更新 < 100 ms,详见[交互设计 §1.1](frontend/interaction-design.md) | 已裁决 ✦ |
| D-13 | deb 拆分 | MVP 单包(含 core + IBus + Fcitx5 + UI);P1 再评估拆分 | 采纳建议 |
| D-14 | 默认快捷键 | **所有者裁决:两键组合,默认 `Alt+S`**(toggle 与 hold 共用);录入组件保留冲突检测,Alt+字母 与应用菜单助记键的潜在冲突在 UI 中提示 | 已裁决 ✦ |
| D-15 | Fcitx5 优先级 | **所有者裁决:直接进 P0**;作为 MVP 验收项,覆盖 KDE/wlroots 桌面,同时充当平台抽象的第二实现 | 已裁决 ✦ |
| D-16 | 主按钮配色 | **所有者裁决:允许引入新色增强美观** → 按同色相补全主色阶 `50–900`(原色板全部落位、色相不变),主按钮三态(默认/hover/active)浅深模式全部满足 WCAG AA | 已裁决 ✦ |

✦ = 所有者明确批复项;其余为默认采纳项。

### 3.1 实现期新增决策(2026-06-10 提出,2026-06-11 所有者批复 D-17/D-18)

| # | 议题 | 背景与选项 | 建议 | 状态 |
| --- | --- | --- | --- | --- |
| D-17 | sherpa-onnx 集成方式 | 阶段 3 需把 sherpa-onnx 接入 Rust:A. 社区 `sherpa-rs` 绑定(集成快,版本受制于人);B. 官方 C API + 自写 `build.rs`(源码构建,可控但构建链重);C. C API + 预编译库(快但有供应链与 glibc 兼容风险) | **所有者裁决(2026-06-11):按建议执行**——先 A 做 POC(隔离在独立 backend crate `voxflow-asr-sherpa`,trait 之后实现可替换);正式发布前若 A 的维护状况不可靠再切 B。POC 同时实测 int8 模型体积与延迟,回填 streaming-asr §5 候选表 | 已裁决 ✦ |
| D-18 | 分类器训练工具链 | 运行时已定 Rust + ort;训练侧需要 embedding + LR/SVM 头 + ONNX 导出,Rust 生态不成熟 | **所有者裁决(2026-06-11):按建议执行**——允许 Python 作为 dev-time 工具(`scripts/train/` 下,scikit-learn → skl2onnx 导出),不进入运行时与发布物;训练脚本、数据集版本、指标报告随仓库管理 | 已裁决 ✦ |
| D-19 | zbus 版本与 MSRV | zbus 5.x 要求 edition2024,与项目 MSRV(Rust 1.75)冲突 | **zbus 固定 `=4.4`、`async-lock =3.3.0`**,MSRV 保持 1.75;升级 MSRV 时再评估 zbus 5(纯依赖约束,已按此实现,作记录)。注:其中"MSRV 1.75"部分被 D-20 的现实推翻,见下行 | 已采纳(部分被 D-20 取代) |
| D-20 🟡 | 构建工具链与 MSRV 政策 | 阶段 3 接入 sherpa 时发现:`sherpa-rs-sys` build script 用 `cargo::` 语法(要求 Cargo ≥ 1.77),且多个传递依赖(idna_adapter/home/getrandom 链)超出 rustc 1.75;系统 rustc 无法升级(无 sudo) | **已实施**:用户级 rustup stable 1.96(`~/.cargo`,不动系统 rustc、不改默认 PATH),`Cargo.lock` pin `idna_adapter=1.1.0`/`tempfile=3.14.0`/`home=0.5.9`。**建议裁决**:放弃"MSRV 1.75"承诺,改为"以 rustup stable 为开发与 CI 工具链"(发布物为自包含二进制,MSRV 只影响构建环境,个人项目无下游依赖方);zbus 是否升 5.x 另行评估,默认维持 `=4.4` 不动 | 待确认 |
| D-21 | GPU 优先运行 | 原基准机假设为"4 核 CPU 无 dGPU";所有者 2026-06-11 明确:**VoxFlow 可以用 GPU 运行,且推荐优先用 GPU**;本机为 RTX 5070 Ti 16GB(驱动 580 / CUDA 13.0,无 nvcc) | 调整为 **GPU 优先、CPU 兜底**:① P0 zipformer 链路接 sherpa-onnx CUDA EP 加速;② Qwen3-ASR-0.6B(sherpa 离线+VAD)进 final/refine 层(D-9 升级,见 [模型调研 2026](architecture/model-research-2026.md));③ Qwen3-ASR-1.7B + vLLM 本地 sidecar 作"高准确率流式模式"POC。CPU-only 环境仍必须可用(延迟预算按 CPU 口径不放松) | **所有者裁决 ✦**(执行细化待 POC) |

---

## 4. 遗留事项(非决策类)

1. **线框图待更新**:`control-center-wireframe.svg` 侧栏缺"外观"项,未体现 badge 体系与状态指示器;建议阶段 4 前按 ui-system 0.3 重绘(可由 agent 生成)。
2. **`[待验证]` 数值**:候选模型体积、ibus 最低版本、KDE Wayland IBus 行为、gtk-layer-shell × Tauri 集成等,在对应阶段 POC 实测回填。
3. **机器可读 IPC schema**:阶段 1 落地(JSON Schema 或 Rust 类型导出 + CI 双向校验)。
4. **数据集标注流程**:`data/semantic-intent/` 的生成与复核流程,阶段 5 前补充(协作模型见实施计划 §0)。
5. **AGENTS.md 更新**:仓库重组(阶段 0)同批次改写为新架构约定;现文件中的 Python 决策(faster-whisper fallback、Ctrl+Space 默认键等)将随 D-7/D-14 失效。

---

## 5. 下一步

1. 所有者对本决策记录做最终确认(阶段 0 人工验证点)→ 全库文档状态转"已冻结"。
2. 仓库重组:建立 Rust workspace,删除 Python 实现,更新 AGENTS.md(D-7/D-11)。
3. 进入实施计划阶段 1(Rust Core 原型);阶段 2/3 的两个 POC(zbus、流式模型)结果回填 `[待验证]` 条目。
4. 阶段 4 前完成 §4.1 线框图重绘与指示器视觉稿。
