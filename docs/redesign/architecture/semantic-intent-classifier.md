# P0 语义意图分类器规范

> **编号** VF-ARCH-06 · **版本** 0.4 · **状态** 评审中(数据集骨架已建立) · **最后更新** 2026-06-10

实现状态:`crates/voxflow-semantic` 已生成合成起步数据集 `semantic-intent-synthetic-v0.1`(train/dev/test = 1080/360/360,位于 `data/semantic-intent/`)。**合成数据仅为起步线**:训练前必须经所有者抽样复核(≥ 10%),并逐步用真实听写与 `correction.feedback` 样本替换;训练工具链选型见 [D-18](../review-and-decisions.md)。

## 1. 定位

判断 ASR 输出片段表达撤销、修正、替换还是字面文本。分类器**只提供建议**,不直接删除;所有删除/替换必须经过[账本安全门](semantic-correction.md)。它是 P0 能力,但允许按 [PRD §8](../product/prd.md) 的裁剪规则以"规则状态机先行、分类器 P0.5 紧随"的方式交付。

## 2. 资源预算

| 项 | 预算 | 说明 |
| --- | --- | --- |
| 分类器包体积 | ≤ 50 MB 目标,≤ 150 MB 硬上限 | 含 embedding 模型 + 分类头 + 标签表 |
| 冷加载时间 | ≤ 2 s(基准机) | 异步加载,不阻塞输入法启动 |
| 单次推理 | ≤ 30 ms (p95, 基准机 CPU 单线程) | 在 stable→commit 路径之外异步执行亦可 |
| 运行内存 | ≤ 300 MB | |

## 3. 输入

输入是带上下文的结构,不是裸句子:

```json
{
  "current_text": "不对,四点",
  "previous_segments": [
    { "id": "seg-1", "text": "今天下午三点开会", "age_ms": 1800 }
  ],
  "language_hint": "zh",
  "pause_before_ms": 320,
  "pause_after_ms": 480
}
```

必须包含:当前 stable/final 文本、最近 1-3 个账本片段、语言提示、停顿信息、可选 token 时间戳。
不得包含:大量用户上下文、非 VoxFlow 输入的完整文本、敏感日志内容(NFR-PRV-02)。

## 4. 标签

P0 标签集合固定六类(与[语义撤销 §3](semantic-correction.md)一致):

| 标签 | 含义 | 示例 |
| --- | --- | --- |
| `literal` | 原样输入 | "这不是问题" |
| `undo_last` | 删除上一段 | "刚才那句删掉" |
| `undo_target` | 删除指定内容 | "把前面的三点删掉" |
| `replace_entity` | 替换实体 | "三点不对,四点" |
| `repair_previous` | 修正上一短语 | "不对,应该是四点" |
| `uncertain` | 不确定 | "不是不对" |

## 5. 模型路线

```text
multilingual embedding (ONNX, int8)
  -> feature builder(文本 embedding + 结构特征拼接)
  -> lightweight classifier head (LR / Linear SVM / 小型 MLP)
  -> calibrated confidence (Platt/温度标定)
```

候选 embedding(最终选型见 [D-5](../review-and-decisions.md)):

| 候选 | 量化后体积 | 许可证 | 备注 |
| --- | --- | --- | --- |
| multilingual MiniLM(SetFit 风格) | 约 120 MB(int8)[待验证] | Apache-2.0 | 中英混合稳妥,体积偏上限 |
| bge-small-zh + 英文规则特征 | 约 25 MB(int8)[待验证] | MIT | 体积优,英文召回靠规则补 |
| 项目内字符 n-gram + 线性模型 | < 5 MB | 自有 | 退路;与规则状态机叠加 |

不可接受:只用关键词硬删;让 LLM 直接输出删除操作;无置信度的黑盒判断。

运行时:Rust `ort`(ONNX Runtime)加载;分类头与标定参数随分类器包分发,带版本号与许可证(见[模型管理 §4](model-management.md))。

## 6. 特征

文本特征:当前片段;上一片段;两者拼接;是否含修正标记("不对""错了""改成");是否含否定但非撤销表达("不是问题")。

结构特征:当前片段长度;距上一 commit 时间;是否有明显停顿;候选 target 是否存在于账本;replacement 是否非空。

## 7. 输出

```json
{
  "intent": "replace_entity",
  "confidence": 0.86,
  "target_hint": "三点",
  "replacement_hint": "四点",
  "reason_code": "repair_marker_and_entity_pair"
}
```

五个字段全部必填(`*_hint` 可为 null),保证每次判断可解释、可入修正记录。

## 8. 置信度阈值

| 置信度 | 行为 |
| --- | --- |
| `< 0.70` | 不执行撤销,按 literal/uncertain 处理 |
| `0.70 – 0.85` | 只允许低风险替换:target 在最近 segment 中完全匹配 |
| `>= 0.85` | 进入账本安全门(安全门仍可拒绝) |

用户阈值模式:保守(各档 +0.05)/ 标准(默认)/ 积极(各档 -0.05,但任何档位都不能绕过安全门)。

## 9. 训练数据

```text
data/semantic-intent/
  train.jsonl  dev.jsonl  test.jsonl
  label_schema.json
  README.md          # 数据集版本、采集与标注规范
```

样本字段:

```json
{ "locale": "zh-CN", "current_text": "不对,四点",
  "previous_text": "今天下午三点开会",
  "label": "replace_entity", "target": "三点", "replacement": "四点" }
```

规模与构成要求(P0 起步线):

- 总量 ≥ 1500 条,每标签 ≥ 150 条;test 集 ≥ 300 条且冻结版本。
- 负例("不是"误触发类)占比 ≥ 30%。
- 覆盖:中文、英文、中英混合、口语停顿、指代性撤销、实体替换。
- `correction.feedback` 上报的误触发样本经人工复核后进入下一版数据集。
- 每次新增规则或标签必须同步补充测试样本。

## 10. 发布门(Release Gate)

每次发布分类器包必须记录,且达标才可发布:

| 指标 | 门槛 |
| --- | --- |
| macro F1(冻结 test 集) | ≥ 0.85 |
| `literal` 被判为删除类(分类器层) | ≤ 1% |
| 安全门后端到端误删率 | ≤ 0.1% |
| accuracy / 每类 P-R / 混淆矩阵 / 低置信降级率 | 记录留档 |

原则:**误删率优先于召回率**;`literal` 被误判为删除类是最高优先级缺陷;`uncertain` 可以偏多,但不能激进删除。

## 11. 集成与降级

```text
ASR stable/final -> rule precheck -> intent classifier
  -> confidence policy -> ledger safety gate -> input method operation
```

分类器加载/推理失败时(FR-SEM-07):记录 warning → UI 显示"已降级为规则模式" → 自动使用规则状态机 → 普通输入完全不受影响。冷启动期间同样按降级模式运行。

## 12. UI 集成

控制台显示:分类器状态(可用/加载中/降级/缺失/失败)、版本、当前阈值模式、最近分类结果、恢复上一修正。普通用户不暴露模型内部参数;开发者模式可显示标签分布、阈值和测试集版本。
