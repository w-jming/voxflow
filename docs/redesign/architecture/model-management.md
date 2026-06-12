# 模型管理设计

> **编号** VF-ARCH-07 · **版本** 0.2 · **状态** 评审中 · **最后更新** 2026-06-10

## 1. 目标

模型管理必须从用户角度可理解:当前用什么模型、模型在哪里、占多少空间、是否可用、下载进度如何、许可证是什么、能否删除或迁移(FR-MDL-01~07)。模型状态机见[系统架构 §7.2](system-architecture.md)。

## 2. 目录

```text
~/.voxflow/                      (VOXFLOW_HOME 可重定向,见 D-8)
  config.toml
  models/
    <model_id>/                  每模型一目录,含 manifest.lock
  cache/
    downloads/<task_id>/         下载临时文件与断点元数据
  logs/  run/  ledger/
```

大模型、缓存、日志和用户配置不得默认放入 `/usr` 或 `/opt`(PRD 非目标)。

## 3. 模型描述档(Profile)

软件内置 profile 目录(随版本更新),每个模型:

```toml
[profile]
id = "streaming-zh-en-small"
label = "VoxFlow Streaming Small"
kind = "asr-streaming"            # asr-streaming | asr-refiner | intent-classifier
backend = "sherpa-onnx"
version = "2026.06"
license = "Apache-2.0"
languages = ["zh", "en"]
streaming = true
recommended = true
min_ram_mb = 1024

[source]
url = "https://…(官方来源)"
size_bytes = 104857600

[[files]]                         # 逐文件校验
path = "encoder.int8.onnx"
sha256 = "…"

[[files]]
path = "tokens.txt"
sha256 = "…"
```

约束:

- `url` 必须是 profile 声明的官方来源,下载时不得重定向到未知域(NFR-PRV-03)。
- 安装完成后在模型目录写入 `manifest.lock`(实际文件清单 + checksum + 安装时间),供 `model.verify` 复检。

## 4. 模型类型

| 类型 | 角色 | P0 要求 |
| --- | --- | --- |
| 轻量流式 ASR | 默认即用,低延迟 partial | 随包或首启自动可用(FR-MDL-01) |
| 高准确率 ASR | final 精修 / 高质量模式 | 一键下载(P1 启用精修) |
| 语义意图分类器 | 智能撤销 | 至少一个轻量包:embedding runtime + 分类头 + 标签表 + 版本 + 许可证 + smoke 样本 |

## 5. 下载

能力清单(FR-MDL-02):来源与许可证展示、总大小、进度、速度、剩余时间、暂停、继续、取消、断点续传、完成后校验。

实现要求:

- **预检**:下载前检查目标磁盘剩余空间 ≥ size × 2.2(临时 + 解压);不足返回 `model.disk_full`。
- **续传**:HTTP Range;断点元数据(已下载字节、ETag)存 `cache/downloads/<task_id>/`,Core 重启后可恢复。
- **重试**:网络错误指数退避自动重试 3 次,之后转为可手动重试的失败态。
- **并发**:同时进行的下载任务默认 1 个,其余排队(`core.busy`)。
- **进度事件**:`model.progress` 节流 ≤ 4 次/秒。
- 临时文件全程在 `cache/downloads/`,逐文件 sha256 校验通过后**原子改名**进入 `models/<id>/`。

## 6. 本地导入

模式:复制 / 软链接(FR-MDL-03)。导入前依序校验,任一失败即终止且不修改当前模型配置(FR-MDL-04):

1. 必需文件存在(按 profile/用户 manifest)。
2. config / tokenizer / processor 可解析。
3. 权重格式可读(safetensors/onnx 头部检查)。
4. checksum 匹配(manifest 提供时强制;缺失时显著提示"未校验来源")。
5. 最小 smoke inference:固定样本输入,输出非空且形状正确。

软链接模式额外要求:目标路径存在性纳入 doctor 检查;目标失效时模型转入 `Broken` 态并提示。

## 7. 模型切换与回滚

`model.activate` 流程(FR-MDL-05):

```text
校验目标 Ready -> 后台加载新模型 -> smoke test
  -> 原子切换会话引用(无进行中会话时立即,否则等会话边界)
  -> 旧模型转 Ready 保留
失败任一步 -> 继续使用旧模型,UI 显示具体失败原因
```

删除限制:Active 模型不可删除(`model.active_locked`);删除前显示释放空间。

## 8. 许可证与分发

- 控制台和文档必须显示每个模型许可证;无许可证信息的模型不得进入稳定版推荐列表(FR-MDL-06)。
- 源码仓库只提交许可证允许、体积合理的轻量资产;大模型一律经官方渠道下载,不进 git(NFR-PRV-05)。
- 语义意图模型同样遵守许可证与体积约束;使用外部 embedding 时在 profile 中标注来源、许可证、参数规模与校验值。
