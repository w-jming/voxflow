# 语义撤销与修正设计

> **编号** VF-ARCH-05 · **版本** 0.4 · **状态** 评审中(阶段 5 基础实现反馈已并入) · **最后更新** 2026-06-10

实现状态:账本、冻结策略、规则状态机、安全门(含实现期新增的第 7 项与"完整 segment 删除 + 整段重提交"映射规则)已在 `voxflow-core::correction` 落地并有 UT 覆盖;意图分类器尚未训练(见 [D-18](../review-and-decisions.md))。

## 1. 目标

支持自然语言修正,但不能因为用户说了"不是""不对"就粗暴删除文本。撤销系统必须**可解释、可关闭、可回滚**,且只能操作 VoxFlow 自己输入的内容(FR-SEM-01~08)。

## 2. 注入账本

每次 commit 写入账本:

```rust
LedgerSegment {
    id: SegmentId,
    session_id: SessionId,
    committed_text: String,      // 实际提交文本
    normalized_text: String,     // 归一化文本(匹配用)
    token_range: TokenRange,
    source: Source,              // asr_stable | refine | correction
    timestamp: Instant,
    cursor_context_hash: u64,    // commit 时光标前 M 字符哈希(M=16)
    frozen: bool,
}
```

规则:

- 账本只记录 VoxFlow 自己 commit 的文本,不记录用户手动输入的完整上下文。
- **存储**:内存环形缓冲,默认保留最近 50 个 segment;持久化默认关闭,开启时写入 `~/.voxflow/ledger/`(仅调试用途,遵守 NFR-PRV-02 脱敏规则)。
- **冻结策略**(任一满足即 `frozen = true`,冻结后不可再被自动修改):
  - segment 提交超过 30 s;
  - 其后已有 ≥ 10 个新 segment;
  - 焦点切换/会话停止;
  - 检测到用户手动编辑(surrounding text 与账本尾部不匹配)→ **冻结全部**未冻结 segment。

## 3. 意图类型

| 意图 | 含义 |
| --- | --- |
| `literal` | 原样输入 |
| `undo_last` | 撤销上一段 |
| `undo_target` | 撤销指定片段 |
| `replace_entity` | 替换实体 |
| `repair_previous` | 修正上一短语 |
| `uncertain` | 不确定 |

低置信度必须归为 `literal` 或 `uncertain`,不允许默认删除。

## 4. 三层判断

```text
ASR stable/final
  -> 第一层 规则状态机(高确定性,毫秒级)
  -> 第二层 轻量意图分类器(P0,本地 ONNX)
  -> 第三层 LLM 仲裁(P1/P2 实验,默认关闭)
  -> 账本安全门
  -> 输入法操作
```

### 第一层:规则状态机

处理高确定性短句:"删掉刚才那句""撤销上一句""不对,四点"。同时显式排除:"这不是问题""不是不对""不是说要删除"。规则命中时仍走安全门,不直接删除。

### 第二层:轻量语义分类器(P0)

与规则状态机共同组成默认智能撤销能力,只输出候选意图与置信度,不直接删除。完整规范见[语义意图分类器](semantic-intent-classifier.md)。

### 第三层:低置信仲裁(P1/P2,非 P0)

可选小 LLM 只在歧义时输出结构化建议(JSON:intent/target/replacement/confidence),输出同样必须经过安全门,且不出现在普通用户设置中(FR-SEM-09)。

## 5. 安全门

执行删除/替换前必须**全部**通过(FR-SEM-05);任一失败则原样输入,并发 `correction.rejected` 事件注明失败项:

| # | 校验项 | 降级行为 |
| --- | --- | --- |
| 1 | 目标文本存在于最近未冻结账本 segment | 失败 → 原样输入 |
| 2 | 光标前文本(surrounding text)与账本尾部匹配 | 前端无 surrounding 能力时,仅允许 `undo_last` 且修正窗口减半(见[降级矩阵](input-method.md)) |
| 3 | 删除范围不超过账本记录范围 | 失败 → 原样输入 |
| 4 | 用户未关闭智能撤销 | 关闭 → 一律 literal |
| 5 | 置信度高于当前阈值档 | 低于 → literal/uncertain |
| 6 | 操作可写入修正记录 | 写入失败 → 不执行 |
| 7 | 替换目标可映射回真实 segment 字符范围 | 失败 → 原样输入 |

安全门通过后,Core 生成平台无关 `InputEvent` 序列。段内替换不得被映射成"删除光标前目标词长度";必须删除账本确认的完整 segment,再提交修正后的完整 segment,以避免目标词不在光标尾部时误删。

## 6. 修正记录与恢复

每次 applied/rejected 都记录:原识别片段、意图、目标、替换、置信度、reason_code、各安全门结果。控制台展示最近 N 条(默认 20),支持"恢复上一条修正"——恢复操作同样经过安全门(目标文本必须仍在原位)。

## 7. 行为示例

| 输入 | 期望结果 |
| --- | --- |
| `今天下午三点开会,不对,四点` | `今天下午四点开会` |
| `这不是问题` | 原样输入 |
| `刚才那句删掉` | 删除最近一个 VoxFlow segment |
| `我说的不是不对` | 原样输入,不触发撤销 |
| (用户手动改过文本后)`删掉刚才那句` | 安全门 #2 拦截,原样输入并提示 |

以上样例全部进入固定回归测试集(见[测试策略 §4.1](../engineering/testing-strategy.md))。

## 8. UI 要求

控制台语义修正页提供:

- 智能撤销开关(FR-SEM-06)。
- 分类器状态:可用 / 加载中 / 降级到规则 / 缺少模型 / 加载失败(FR-SEM-07)。
- 阈值模式:保守 / 标准 / 积极。
- 最近修正记录 + 恢复上一条。
- 误触发反馈入口(`correction.feedback`,样本进入数据集迭代)。

未实现的高阶后端不得显示在普通用户设置中,可在开发者页面或文档 TODO 中说明。
