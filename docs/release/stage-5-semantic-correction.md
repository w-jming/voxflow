# 阶段 5 语义撤销基础记录

日期:2026-06-10

## 已完成

- 新增 `voxflow-core::correction` 模块:
  - `InjectionLedger`
  - `LedgerSegment`
  - `RuleIntentClassifier`
  - `SafetyGate`
  - `CorrectionHistory`
- 注入账本默认保留最近 50 个 segment,支持:
  - append 时归一化文本。
  - 超过后续 10 个 segment 的旧 segment 自动冻结。
  - 全量冻结和按时间冻结。
- 规则状态机覆盖第一批高确定性场景:
  - `刚才那句删掉` / `删掉刚才那句` / `撤销上一句` → `undo_last`
  - `三点不对,四点` → `replace_entity`
  - `不对,四点` → `repair_previous`
  - `这不是问题` / `不是不对` / `不是说要删除` → `literal`
- 安全门基础校验:
  - 智能撤销开关。
  - 阈值模式。
  - delete 能力。
  - 目标存在于未冻结账本。
  - surrounding text 与账本尾部匹配。
  - 删除范围不超过账本记录范围。
  - 替换目标可解析为真实 segment 文本范围。
  - 修正记录可写。
- 新增 `correction.list_recent` IPC,返回 Core 内存中的最近修正记录。
- 新增安全门通过后的输入操作映射:
  - `undo_last`/`undo_target` 生成 `DeleteBeforeCursor`。
  - `replace_entity`/`repair_previous` 删除账本尾部完整 segment,再提交修正后的完整文本。
  - 如果替换目标只存在于归一化文本、无法映射回真实 segment 字符范围,安全门拒绝执行。
- Core mock 听写路径已能维护最小注入账本和前端 surrounding tail,并在修正意图命中时发出:
  - `correction.applied`,包含可投影的 `input_events`。
  - `correction.rejected`,同时按原文 `dictation.final` 降级输入。
- IBus bridge 订阅 `correction` 组,并把 `correction.applied.input_events` 投影为 IBus delete/commit 操作。
- 新增 `voxflow-semantic` 数据工具:
  - 生成 `data/semantic-intent/label_schema.json`、`train.jsonl`、`dev.jsonl`、`test.jsonl`。
  - 固定六类标签:`literal`、`undo_last`、`undo_target`、`replace_entity`、`repair_previous`、`uncertain`。
  - 校验 P0 起步线:总量、每标签数量、test 集规模、"不是"误触发负例占比、ID 唯一性和标签字段约束。

## 当前限制

- 规则状态机只是 P0 基础,不是完整轻量 ONNX 分类器。
- 目前 correction 事件化仍基于 mock 听写路径;真实 streaming ASR 流水线、真实桌面输入上下文和 Fcitx5 前端尚未完成。
- 当前语义意图数据集是 synthetic seed,尚未人工复核,也没有轻量 ONNX 分类器 macro F1、误删率等发布门结果。
- `correction.list_recent` 当前只暴露记录容器;真实听写链路执行修正后才会产生应用/拒绝记录。

## 验证

```bash
cargo test -p voxflow-core correction
cargo test -p voxflow-core
cargo test -p voxflow-semantic
cargo run -p voxflow-semantic -- validate data/semantic-intent
scripts/dev-check.sh
```
