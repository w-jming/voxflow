# 测试报告

测试日期：2026-06-10
测试系统：Ubuntu 24.04 / Linux 6.17 / amd64 / X11 / PipeWire

## 结果摘要

- Python 编译检查通过：`python3 -m compileall src tests`。
- Python 单元测试通过：`37 passed, 1 warning`。
- 默认高准确率 deb 构建通过：`dist/voxflow_0.2.0_amd64.deb`，约 5.9 GB。
- deb 包内包含 Qwen3-ASR 1.7B 两个模型分片、内置 `faster-whisper-tiny` fallback、IBus component、桌面入口、右上角图标入口、systemd 用户服务和旧 `local-speak-*` 兼容入口。
- apt 模拟安装通过：会移除旧 `local-speak-input 0.1.1` 并安装 `voxflow 0.2.0`。
- staged rootfs 运行验证通过：`voxflow doctor`、`voxflow models`、`voxflow ibus-engine --dry-run`。
- staged GUI 验证通过：首页、`/logo.svg` HEAD、`/api/models`、`/api/semantic-intent`、`/api/process-text`。
- 语义撤销开关和后端状态已覆盖：当前可用 `rules`，未训练/未安装的 MiniLM/SetFit 后端会被 GUI API 拒绝。
- 包内路径污染扫描通过：VoxFlow 运行入口、配置、模型目录和 package data 未包含开发目录、`downloads/models` 或 Hugging Face `.cache` 路径。

## 执行项目

### 自动化测试

```bash
python3 -m compileall src tests
.venv/bin/python -m pytest -q
```

结果：

```text
37 passed, 1 warning in 0.09s
```

覆盖重点：

- 默认简体中文输出、OpenCC 简繁转换和繁体设置。
- `toggle` / `hold` 快捷键状态机和用户快捷键配置。
- 语义撤销开关、“不是不对”等误触发边界。
- VoxFlow 注入账本只删除自己提交过的文本。
- 模型档位、语义意图后端注册和许可证元数据。
- 录音 fallback、通知容错、输入注入 dry-run、版本一致性。

### 模型缓存

Qwen3-ASR 1.7B 本机缓存分片：

```text
4220320824 downloads/models/Qwen3-ASR-1.7B/model-00001-of-00002.safetensors
478200688 downloads/models/Qwen3-ASR-1.7B/model-00002-of-00002.safetensors
```

`huggingface-cli download Qwen/Qwen3-ASR-1.7B model-00001-of-00002.safetensors --local-dir downloads/models/Qwen3-ASR-1.7B --max-workers 1` 返回本地文件路径，未重新下载。

### 打包

```bash
scripts/build_deb.sh
```

结果：

```text
Built dist/voxflow_0.2.0_amd64.deb
```

包元数据摘要：

```text
Package: voxflow
Version: 0.2.0
Architecture: amd64
Maintainer: Jiaming Wang <w_jming@outlook.com>
Provides: local-speak-input
Conflicts: local-speak-input
Replaces: local-speak-input
```

关键内容检查确认包含：

- `/opt/voxflow/models/Qwen3-ASR-1.7B/model-00001-of-00002.safetensors`
- `/opt/voxflow/models/Qwen3-ASR-1.7B/model-00002-of-00002.safetensors`
- `/opt/voxflow/venv/lib/python3.12/site-packages/voxflow/bundled/faster-whisper-tiny/model.bin`
- `/usr/share/ibus/component/voxflow.xml`
- `/usr/bin/voxflow-ibus-engine`
- `/usr/share/applications/voxflow.desktop`
- `/usr/share/icons/hicolor/scalable/apps/voxflow.svg`
- `/usr/bin/local-speak-daemon`

### apt 模拟安装

```bash
apt-get -s install ./dist/voxflow_0.2.0_amd64.deb
```

结果摘要：

```text
The following packages will be REMOVED:
  local-speak-input
The following NEW packages will be installed:
  voxflow
Remv local-speak-input [0.1.1]
Inst voxflow (0.2.0 local-deb [amd64])
Conf voxflow (0.2.0 local-deb [amd64])
```

### staged rootfs

```bash
build/deb/rootfs/opt/voxflow/venv/bin/python -m voxflow doctor
build/deb/rootfs/opt/voxflow/venv/bin/python -m voxflow models
build/deb/rootfs/opt/voxflow/venv/bin/python -m voxflow ibus-engine --dry-run --config build/deb/rootfs/etc/voxflow/config.toml
```

结果摘要：

```text
Python 模块 opencc: OK
Python 模块 faster_whisper: OK
Python 模块 qwen_asr: OK
Python 模块 sounddevice: OK
X11 全局快捷键: OK
IBus 输入法: OK
右上角图标: OK
```

IBus dry-run：

```text
VoxFlow IBus dry-run engine is available.
COMMIT 这是语音输入测试。
```

### GUI 接口

```bash
build/deb/rootfs/opt/voxflow/venv/bin/python -m voxflow gui --dry-run --host 127.0.0.1 --port 8877 --config build/deb/rootfs/etc/voxflow/config.toml
curl -fsSI http://127.0.0.1:8877/logo.svg
curl -fsS http://127.0.0.1:8877/api/models
curl -fsS http://127.0.0.1:8877/api/semantic-intent
curl -fsS -X POST http://127.0.0.1:8877/api/process-text -H 'Content-Type: application/json' -d '{"text":"今天下午三点哦不四点","inject":false}'
```

关键结果：

```text
Content-Type: image/svg+xml
```

```json
{"raw_text":"今天下午三点哦不四点","processed_text":"今天下午四点。","actions":[{"insert":"今天下午四点。","backspace":0,"reason":""}]}
```

未启用语义后端拒绝结果：

```json
{"error":"该语义意图后端尚未在本机启用，需要先训练或安装对应模型"}
```

### 包内路径检查

```bash
rg -a -n "/home/<user>|<repo-absolute-path>|downloads/models|\\.cache/huggingface" \
  build/deb/rootfs/usr/bin \
  build/deb/rootfs/etc/voxflow \
  build/deb/rootfs/usr/share/applications \
  build/deb/rootfs/usr/share/ibus \
  build/deb/rootfs/opt/voxflow/venv/lib/python3.12/site-packages/voxflow \
  build/deb/rootfs/opt/voxflow/venv/lib/python3.12/site-packages/voxflow-0.2.0.dist-info \
  build/deb/rootfs/opt/voxflow/models
```

结果：无匹配。

## 未执行项

没有执行真实 `sudo apt install ./dist/voxflow_0.2.0_amd64.deb`，避免在开发过程中直接替换当前桌面会话已安装包。当前验收覆盖了 apt 依赖解析、staged rootfs 运行、GUI 接口、IBus dry-run、模型文件和包内容。
