# 阶段 5 语义意图数据集记录

日期:2026-06-10

## 已完成

- 新增 `voxflow-semantic` crate,提供:
  - `generate` 命令:确定性生成 `label_schema.json`、`train.jsonl`、`dev.jsonl`、`test.jsonl`。
  - `validate` 命令:从磁盘读取数据集并输出 JSON 校验报告。
  - 单元测试:覆盖 P0 阈值和按标签分层切分。
- 当前数据集版本:`semantic-intent-synthetic-v0.1`。
- 当前切分:
  - train:1080
  - dev:360
  - test:360
- 当前标签分布:
  - `literal`:600
  - `undo_last`:240
  - `undo_target`:240
  - `replace_entity`:240
  - `repair_previous`:240
  - `uncertain`:240
- "不是"误触发类 `literal` 负例:600/1800,占比 33.33%。

## 当前限制

- 所有样本均为 `source=synthetic_rule_seed` 且 `review_status=unreviewed`。
- test 集当前只是在 synthetic 版本内冻结;人工复核版本需要新的 `dataset_version`。
- 尚未训练、量化或发布轻量 ONNX 分类器。
- 尚未记录 macro F1、混淆矩阵、literal→删除类误判率和端到端误删率。

## 验证

```bash
cargo test -p voxflow-semantic
cargo run -p voxflow-semantic -- validate data/semantic-intent
```

最近校验结果:

```json
{
  "dataset_version": "semantic-intent-synthetic-v0.1",
  "total_samples": 1800,
  "split_counts": { "train": 1080, "dev": 360, "test": 360 },
  "test_samples": 360,
  "literal_false_trigger_negatives": 600,
  "literal_false_trigger_share": 0.3333333333333333,
  "errors": [],
  "passed": true
}
```
