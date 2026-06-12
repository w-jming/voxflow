# Git 工作流

> **编号** VF-ENG-05 · **版本** 0.2 · **状态** 评审中 · **最后更新** 2026-06-10

## 1. 分支

不得直接在 `main` 开发。按任务创建:

| 前缀 | 用途 | 示例 |
| --- | --- | --- |
| `feature/*` | 新功能 | `feature/ibus-engine-poc` |
| `fix/*` | 缺陷修复 | `fix/download-resume-etag` |
| `docs/*` | 文档 | `docs/redesign-review` |
| `release/*` | 发布分支 | `release/0.3` |

`main` 始终可构建、测试全绿;release 分支只接受 cherry-pick 修复。

## 2. 提交

采用 Conventional Commits:`<type>(<scope>): <subject>`,type ∈ feat/fix/docs/refactor/test/build/chore。

```text
feat(core): 实现下载断点续传与磁盘空间预检

- Range 续传,断点元数据存 run/
- 预检阈值 size×2.2,不足返回 model.disk_full
```

- 文档改动与代码改动尽量分开提交。
- 包产物、大模型、日志、音频、虚拟环境不提交(`.gitignore` 维护,NFR-PRV-05)。
- 提交信息说明用户可见变化,而不只是实现细节。

## 3. 提交前检查

按变更类型运行(与 [CI 流水线](testing-strategy.md)一致,本地先行):

| 变更 | 检查 |
| --- | --- |
| 文档 | 链接有效性 + 基本拼写 |
| Rust | `cargo fmt --check`、`cargo clippy`、`cargo test` |
| 前端 | `tsc --noEmit`、eslint、组件测试 |
| 包 | 安装级 smoke |
| 发布 | [发布检查单](packaging-release.md)全项 |

## 4. 安全

推送前必须执行 secret scan;不得把本机路径、私密日志、token、私钥推送到远程。提交身份使用项目约定的真实身份(见 AGENTS.md)。

## 5. 评审

PR 描述模板:

```markdown
## 背景
## 变更(用户可见 / 内部)
## 测试(执行过什么,结果)
## 风险与回滚方式
关联文档/需求编号:FR-…, VF-…
```

要求:单一主题、可回滚、关联需求或决策编号;涉及"已冻结"文档的行为变更必须同 PR 更新文档并提升版本号。
