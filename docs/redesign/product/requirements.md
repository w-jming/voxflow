# 需求规格

> **编号** VF-PRD-02 · **版本** 0.3 · **状态** 评审中(决策已回填,待冻结) · **最后更新** 2026-06-10

本文档把 [PRD](prd.md) 的目标拆解为可验证的需求条目。每条需求有唯一编号、优先级和验证方式;其他文档引用需求时使用编号。验证方式详见[测试策略](../engineering/testing-strategy.md)。

验证方式缩写:**UT** 单元测试 / **IT** 集成测试 / **PT** 包测试 / **MT** 人工验收 / **BM** 基准测量。

## 1. 功能需求

### FR-INP 输入

| 编号 | 需求 | 优先级 | 验证 |
| --- | --- | --- | --- |
| FR-INP-01 | 支持连续听写,会话期间音频采集不中断 | P0 | IT/BM |
| FR-INP-02 | 支持 token 级 partial 输出,partial→stable→final 事件齐全 | P0 | UT/IT |
| FR-INP-03 | stable 文本自动 commit,无需用户确认 | P0 | IT |
| FR-INP-04 | partial 通过输入法 preedit 显示在光标处 | P0 | IT/MT |
| FR-INP-05 | 支持快捷键开始/停止听写,保存后热生效 | P0 | IT/MT |
| FR-INP-06 | 支持"按住说话"和"按一次开始再按一次停止"两种模式 | P0 | MT |
| FR-INP-07 | 输入法模式为主路径;注入 fallback 仅用于兼容并明确标注风险 | P0 | MT |
| FR-INP-08 | 目标应用不支持 preedit 时自动降级为仅 commit 模式并上报状态 | P0 | IT |
| FR-INP-09 | Fcitx5 前端与 IBus 能力对等:preedit/commit/delete、焦点与能力上报(D-15) | P0 | IT/MT |
| FR-INP-10 | preedit 仅承载真实 partial 文本,全局禁止非正文占位文案(D-12) | P0 | IT |

### FR-TXT 文本处理

| 编号 | 需求 | 优先级 | 验证 |
| --- | --- | --- | --- |
| FR-TXT-01 | 默认输出简体中文;支持繁体和模型原文 | P0 | UT |
| FR-TXT-02 | 简繁转换不得以简单字表替换为主要策略 | P0 | UT |
| FR-TXT-03 | 口语助词清理(嗯、啊、那个等),可关闭 | P0 | UT |
| FR-TXT-04 | 自动标点 | P0 | UT |
| FR-TXT-05 | 中英文混合输出,词间空格规则正确 | P0 | UT |

### FR-SEM 语义撤销与修正

| 编号 | 需求 | 优先级 | 验证 |
| --- | --- | --- | --- |
| FR-SEM-01 | 每次 commit 写入注入账本;账本只记录 VoxFlow 自己提交的文本 | P0 | UT |
| FR-SEM-02 | 支持 literal / undo_last / undo_target / replace_entity / repair_previous / uncertain 六类意图 | P0 | UT |
| FR-SEM-03 | 规则状态机处理高确定性短句,并显式排除"这不是问题"类负例 | P0 | UT |
| FR-SEM-04 | 本地轻量意图分类器输出意图+置信度+可解释字段,只作建议 | P0 | UT/IT |
| FR-SEM-05 | 任何删除/替换必须通过账本安全门全部校验项 | P0 | UT |
| FR-SEM-06 | 智能撤销可在控制台关闭,关闭后所有修正词按普通文本处理,立即生效 | P0 | IT |
| FR-SEM-07 | 分类器加载失败时自动降级为规则状态机,不影响普通输入 | P0 | IT |
| FR-SEM-08 | 最近修正记录可查看、可恢复;恢复同样经过账本 | P0 | IT/MT |
| FR-SEM-09 | LLM 仲裁后端(实验)不出现在普通用户设置中 | P1 | MT |

### FR-MDL 模型

| 编号 | 需求 | 优先级 | 验证 |
| --- | --- | --- | --- |
| FR-MDL-01 | 随包或首启即提供可用的轻量流式模型 | P0 | PT |
| FR-MDL-02 | 高准确率模型一键下载:进度、速度、剩余时间、暂停、继续、取消、断点续传 | P0 | IT |
| FR-MDL-03 | 本地模型导入,支持复制与软链接两种模式 | P0 | IT |
| FR-MDL-04 | 模型下载/导入后强制 checksum 校验 + smoke test,失败不激活 | P0 | UT/IT |
| FR-MDL-05 | 模型切换失败自动回滚到旧模型 | P0 | IT |
| FR-MDL-06 | 每个模型展示版本、参数规模、许可证、来源、占用空间 | P0 | MT |
| FR-MDL-07 | 按机器性能推荐模型档位 | P1 | MT |

### FR-CC 控制台

| 编号 | 需求 | 优先级 | 验证 |
| --- | --- | --- | --- |
| FR-CC-01 | 总览页显示输入服务、输入法注册、麦克风、模型四类状态及下一步操作 | P0 | MT |
| FR-CC-02 | Core 断连时显示明确状态并支持重连/重启 | P0 | IT |
| FR-CC-03 | 快捷键录入组件:监听、规范化、冲突提示 | P0 | MT |
| FR-CC-04 | 数据页:显示并可迁移 VoxFlow Home、模型、日志、缓存目录 | P0 | IT |
| FR-CC-05 | 模型下载管理页(对应 FR-MDL-02) | P0 | MT |
| FR-CC-06 | 日志查看入口与一键诊断(doctor) | P0 | MT |
| FR-CC-07 | 托盘:打开控制台、暂停听写、当前模型、退出 VoxFlow | P0 | MT |
| FR-CC-08 | "退出 VoxFlow"停止 Core、监听、模型任务和托盘 | P0 | IT/MT |
| FR-CC-09 | 浅色/深色/跟随系统三种主题,切换即时生效且不中断听写 | P0 | IT/MT |
| FR-CC-10 | 音频页:设备选择、实时电平、录音测试、蓝牙 profile 诊断 | P0 | MT |
| FR-CC-11 | 全局状态指示器:实时(< 100 ms)显示空闲/听写中/处理中/错误与电平,按环境降级(HUD → 托盘),可整体关闭(D-12) | P0 | IT/MT |

### FR-PKG 安装

| 编号 | 需求 | 优先级 | 验证 |
| --- | --- | --- | --- |
| FR-PKG-01 | Debian/Ubuntu deb 包,安装/升级/卸载干净 | P0 | PT |
| FR-PKG-02 | portable tar,解压任意目录可用,不要求 root | P0 | PT |
| FR-PKG-03 | AppImage | P1 | PT |
| FR-PKG-04 | macOS dmg/pkg | P1 | PT |
| FR-PKG-05 | Windows MSIX/NSIS | P2 | PT |

## 2. 非功能需求

### NFR-PRF 性能

| 编号 | 需求 | 目标 | 验证 |
| --- | --- | --- | --- |
| NFR-PRF-01 | 首 partial 延迟 | < 500 ms (p90, 基准机) | BM |
| NFR-PRF-02 | stable commit 延迟 | < 1000 ms (p90, 基准机) | BM |
| NFR-PRF-03 | Core 常驻内存 | 可观测,默认模型下 < 1.5 GB [待验证],10 分钟听写无持续增长 | BM |
| NFR-PRF-04 | 模型加载期间 UI 不阻塞 | 加载在 Core 异步执行,UI 显示进度 | IT |
| NFR-PRF-05 | Core 冷启动(不含模型) | < 1 s | BM |

### NFR-REL 稳定性

| 编号 | 需求 | 验证 |
| --- | --- | --- |
| NFR-REL-01 | Core 崩溃后 UI 显示状态并允许一键重启 | IT |
| NFR-REL-02 | 模型下载中断可恢复,临时文件不污染模型目录 | IT |
| NFR-REL-03 | 输入法前端断开不影响控制台;控制台退出不影响听写 | IT |
| NFR-REL-04 | 配置损坏时使用默认配置并保留 `.broken` 备份 | UT |
| NFR-REL-05 | 同一时刻只允许一个 Core 实例(锁文件/socket 抢占) | IT |

### NFR-PRV 隐私

| 编号 | 需求 | 验证 |
| --- | --- | --- |
| NFR-PRV-01 | 默认不上传音频、文本、日志、使用记录 | MT/代码审计 |
| NFR-PRV-02 | 默认日志不记录完整输入文本;调试模式须显式开启并提示风险 | UT |
| NFR-PRV-03 | 模型下载只访问 profile 声明的来源 | UT |
| NFR-PRV-04 | IPC socket 仅当前用户可访问(目录 0700) | IT |
| NFR-PRV-05 | 仓库与发布物不含 secret、token、私钥、用户数据 | secret scan |

### NFR-MNT 可维护性

| 编号 | 需求 | 验证 |
| --- | --- | --- |
| NFR-MNT-01 | Core 业务逻辑跨平台,平台能力经 trait 抽象 | 代码审查 |
| NFR-MNT-02 | UI 与 Core 通过版本化 IPC 合同交互 | IT |
| NFR-MNT-03 | 所有 P0 能力有自动测试或留档的人工验收记录 | 发布检查 |
| NFR-MNT-04 | 错误码稳定,UI 文案可本地化 | UT |

## 3. 优先级总览

| 优先级 | 范围 |
| --- | --- |
| P0 | Rust Core、Tauri 控制台、Linux IBus、Linux Fcitx5、token 级流式 ASR、光标处临时文本、全局状态指示器、模型管理、账本+安全门、轻量意图分类器、三主题、deb/portable |
| P1 | 高准确率二阶段精修、AppImage、macOS 输入原型、模型档位推荐 |
| P2 | Windows 输入原型、热词与自定义词典、多语言 UI、可选云端后端 |

P0 内部裁剪顺序见 [PRD §8](prd.md)。
