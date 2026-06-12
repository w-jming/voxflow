# 语义意图数据集

本目录对应 `docs/redesign/architecture/semantic-intent-classifier.md` 的 P0 起步数据集。

当前版本:`semantic-intent-synthetic-v0.1`

状态:确定性 synthetic seed,全部 `review_status=unreviewed`。它满足 P0 数据量和切分门槛,但还不能作为轻量 ONNX 分类器发布成绩;发布前必须完成人工复核、纳入真实误触发反馈样本,并按发布门记录 macro F1、混淆矩阵和误删率。

## 文件

| 文件 | 数量 | 说明 |
| --- | ---: | --- |
| `label_schema.json` | 1 | 标签、字段、阈值和冻结 test 标记 |
| `train.jsonl` | 1080 | 训练切分 |
| `dev.jsonl` | 360 | 调参与阈值切分 |
| `test.jsonl` | 360 | 当前 synthetic 冻结测试切分 |

## 标签分布

| 标签 | 数量 |
| --- | ---: |
| `literal` | 600 |
| `undo_last` | 240 |
| `undo_target` | 240 |
| `replace_entity` | 240 |
| `repair_previous` | 240 |
| `uncertain` | 240 |

`literal` 中 600 条包含"不是"误触发负例,占总量 33.33%,满足 P0 不低于 30% 的安全要求。

## 生成与校验

生成器和校验器位于 `crates/voxflow-semantic`:

```bash
cargo run -p voxflow-semantic -- generate data/semantic-intent
cargo run -p voxflow-semantic -- validate data/semantic-intent
```

`validate` 当前必须通过:

- 总量不少于 1500。
- 每标签不少于 150。
- test 集不少于 300。
- "不是"误触发类 `literal` 负例占比不少于 30%。
- 样本 ID 不重复。
- `undo_target`、`replace_entity`、`repair_previous` 的 `target`/`replacement` 字段符合标签约束。

## 维护规则

- 不手工改 JSONL;新增模板、字段或切分逻辑应修改 `voxflow-semantic` 后重新生成。
- 每次新增规则或标签必须同步补充生成器和校验器测试。
- 人工复核后的版本应更新 `dataset_version`,保留当前 synthetic 版本用于回归对比。
