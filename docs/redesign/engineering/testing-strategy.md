# 测试策略

> **编号** VF-ENG-01 · **版本** 0.3 · **状态** 评审中(决策已回填,待冻结) · **最后更新** 2026-06-10

验证方式缩写(UT/IT/PT/MT/BM)与需求编号见[需求规格](../product/requirements.md)。

## 1. 测试原则

- 不把"能启动"当作"可用"。
- 流式输出必须测 partial、stable、final 三级事件。
- 输入法行为必须测 preedit、commit、delete。
- 语义撤销必须测误触发边界,误删类缺陷优先级最高。
- 包必须经过安装级测试(容器内全新系统)。
- 性能指标必须在基准机口径下测量并留档。

## 2. Rust Core 测试(UT/IT)

单元测试(行覆盖目标:核心业务逻辑 ≥ 70%):

- 配置解析与损坏恢复(NFR-REL-04)、路径迁移。
- 模型 profile 解析、checksum、manifest.lock。
- token 稳定算法(LCP/时间窗/边界提升,含回退用例)。
- 账本匹配、冻结策略、安全门六项校验逐项的通过/拒绝用例。
- 分类器标签映射、阈值档位、降级逻辑。

集成测试(进程内或双进程):

- mock audio 注入 + MockRecognizer 全链路(帧→partial→stable→commit→账本)。
- IPC:握手、版本协商、订阅过滤、断连重连后全量同步、每个错误码至少一条用例。
- 下载:断点续传、暂停恢复、磁盘不足预检、校验失败不污染模型目录。
- 模型切换失败回滚(FR-MDL-05)。
- 单实例锁(NFR-REL-05)。

## 3. 输入法测试

引擎自动化测试(IBus 与 Fcitx5 同一套用例,容器/虚拟会话内运行对应 daemon,方案在阶段 2 验证可行性 [待验证]):

- 组件注册、引擎可被选中(IBus component / Fcitx5 addon)。
- preedit 更新、stable commit、删除操作、focus in/out。
- preedit 内容断言:仅真实 partial,无占位文案(FR-INP-10)。
- 能力降级:无 surrounding text 应用上的受限撤销行为(FR-INP-08)。

真实应用人工 smoke 矩阵(每次发布在 **GNOME(IBus)与 KDE Plasma(Fcitx5)两套会话**各执行一遍并留档):

| 应用 | 重点 |
| --- | --- |
| GNOME Text Editor / Kate | 基准行为 |
| Chrome/Chromium | Web 输入框、地址栏 |
| 终端(GNOME Terminal / Konsole) | preedit 受限场景 |
| VS Code(Electron) | surrounding text 缺失场景 |
| LibreOffice Writer | 长文本、修正窗口 |

## 4. UI 测试

- 控制台启动、Core 断连展示与恢复。
- 模型下载全周期状态、本地导入流程、缓存目录迁移向导。
- 设置保存与 `config.changed` 回流。
- 错误码 → 文案映射完整性(未知码兜底)。
- 分类器可用/降级/失败三态展示。
- 主题:三态初始加载、即时切换、system 跟随、切换不中断听写、对比度自动检查(见[主题系统 §8](../design/theme-system.md))。
- 状态指示器:四态切换 < 100 ms、X11 定位与置顶、GNOME Wayland 托盘降级、指示器崩溃自动重建且不影响控制台(FR-CC-11)。

## 4.1 语义分类器测试

固定冻结测试集(版本化,见[分类器规范 §9-10](../architecture/semantic-intent-classifier.md)),覆盖:literal/undo/repair/replace_entity/uncertain 正例、"不是"误触发负例、中英混合样本。

每次发布记录:测试集版本、accuracy、macro F1、每类混淆矩阵、误删率、低置信降级率。发布门:macro F1 ≥ 0.85;literal→删除类(分类器层)≤ 1%;安全门后端到端误删率 ≤ 0.1%。**误删率优先于召回率**。

## 5. 性能测试(BM)

基准机口径(见[术语表](../glossary.md)),预录音频经 mock 设备注入保证可重复;tracing span 埋点:`frame_captured → vad_speech_start → first_partial → stable_commit`。

| 指标 | 目标 | 对应 |
| --- | --- | --- |
| Core 冷启动(不含模型) | < 1 s | NFR-PRF-05 |
| 默认模型加载 | 记录留档 | NFR-PRF-04 |
| 首 partial 延迟 | < 500 ms (p90) | NFR-PRF-01 |
| stable commit 延迟 | < 1000 ms (p90) | NFR-PRF-02 |
| CPU / 内存 | 记录留档;10 分钟听写内存无持续增长 | NFR-PRF-03 |

性能报告随发布归档,回归超过 15% 视为阻塞缺陷。

## 6. 包测试(PT)

容器矩阵:Ubuntu 22.04、24.04、Debian 12(全新镜像)。

deb:安装 → doctor 全绿(或仅环境类警告)→ 输入法注册 → 升级(从上一发布版)→ 卸载干净(无残留文件/注册项)。

portable:解压任意目录 → 运行 doctor → `install-desktop`、`install-ibus` 用户级安装 → 卸载脚本清理。

## 7. CI 流水线

| 阶段 | 内容 | 门槛 |
| --- | --- | --- |
| lint | fmt + clippy + 前端 eslint/tsc | 零警告基线 |
| 单元测试 | cargo test + 前端组件测试 | 全绿 |
| 集成测试 | IPC/下载/账本/mock 链路 | 全绿 |
| secret scan | gitleaks(或等价) | 零泄漏 |
| 包构建 + smoke | 主分支与 release 分支 | 安装级通过 |
| 性能基准 | release 分支 | 达标见 §5 |

## 8. 人工验收(MT)

每次发布记录:系统版本、桌面环境(GNOME/KDE × Wayland/X11)、输入法框架、麦克风设备、模型版本、§3 应用矩阵结果、已知问题清单。模板存于仓库 `docs/release/` 下随版本归档。
