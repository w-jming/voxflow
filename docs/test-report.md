# 测试报告

测试日期：2026-06-10
测试系统：Ubuntu 24.04 / Linux 6.17 / amd64 / X11 / PipeWire

## 结果摘要

- Python 编译检查通过：`python3 -m compileall src tests`。
- Python 单元测试通过：`49 passed, 1 warning`。
- Web 调试页 JavaScript 语法检查通过：`node --check src/voxflow/web/app.js`。
- GTK 原生控制中心 smoke test 通过：窗口可在当前 X11 会话打开并自动关闭。
- 本机已有 Qwen3-ASR 1.7B 权重通过 VoxFlow 导入前校验：官方 revision `7278e1e70fe206f11671096ffdd38061171dd6e5`，检查 10 个关键文件，并对两个 safetensors 分片做 SHA256 比对。
- deb 构建通过：`dist/voxflow_0.2.0_amd64.deb`，约 152 MB；默认不包含 Qwen3-ASR 大模型或 `/opt/voxflow/models`。
- portable tar 构建通过：`dist/voxflow-0.2.0_amd64.tar.gz`，约 187 MB；可解压到任意目录运行。
- apt 模拟安装通过：会移除旧 `local-speak-input 0.1.1` 并安装 `voxflow 0.2.0`。
- staged rootfs 验证通过：`voxflow doctor`、`voxflow models`、`voxflow ibus-engine --dry-run`。
- `VOXFLOW_HOME` 验证通过：模型、配置、日志、pid、缓存路径默认在 `~/.voxflow`，测试中可改到 `/tmp/...`。
- 本地 Qwen 权重复用验证通过：`--import-model ... --symlink` 写入用户配置，实际权重仍指向既有目录，没有复制或删除已有下载。
- portable 解压包验证通过：`doctor`、本地 Qwen 软链接导入、IBus dry-run。
- Web 调试控制台 smoke test 通过：`/logo.svg` 返回 `image/svg+xml`，`/api/config` 暴露新的用户数据目录。

## 自动化测试

```bash
python3 -m compileall src tests
.venv/bin/python -m pytest -q
```

结果：

```text
49 passed, 1 warning in 0.10s
```

覆盖重点：

- 默认简体中文输出、OpenCC 简繁转换和繁体设置。
- `toggle` / `hold` 快捷键状态机和用户快捷键配置。
- 语义撤销开关、“不是不对”等误触发边界。
- VoxFlow 注入账本只删除自己提交过的文本。
- 模型档位、模型下载进度统计、模型导入校验、语义意图后端注册和许可证元数据。
- 录音 fallback、通知容错、输入注入 dry-run、版本一致性。
- `VOXFLOW_HOME` 下的配置、模型、日志和运行状态路径。
- `~/.config/voxflow/home` 数据目录指针。

## 模型校验

本机已有 Qwen3-ASR 1.7B 权重未删除、未重新下载。校验命令：

```bash
.venv/bin/python -m voxflow.cli models \
  --select qwen3-asr-1.7b \
  --validate-model downloads/models/Qwen3-ASR-1.7B
```

结果摘要：

```text
校验通过：Qwen3-ASR 1.7B 高准确率
官方 revision：7278e1e70fe206f11671096ffdd38061171dd6e5
已检查文件数：10
```

内置官方 SHA256：

```text
model-00001-of-00002.safetensors
a4cd1f1a04d90b757dc7f7dd26254e69a013b19e80efe590a83c6a3bde8608d6

model-00002-of-00002.safetensors
6e0b9d9e09e2e0238e7ef3cc8a484ab387e91b90f1900bedf88bc92d7929ccfc
```

复用既有权重的 staged rootfs 验证：

```bash
VOXFLOW_HOME=/tmp/voxflow-import-test \
  build/deb/rootfs/opt/voxflow/venv/bin/python -m voxflow models \
  --select qwen3-asr-1.7b \
  --import-model downloads/models/Qwen3-ASR-1.7B \
  --symlink
```

结果摘要：

```text
校验通过：官方 SHA256 与模型结构匹配
路径：/tmp/voxflow-import-test/models/Qwen3-ASR-1.7B
配置：/tmp/voxflow-import-test/config.toml
```

`readlink -f /tmp/voxflow-import-test/models/Qwen3-ASR-1.7B` 指向既有下载目录，说明没有再次复制 4GB+ 权重。

## 打包验证

构建命令：

```bash
UV_CACHE_DIR=/tmp/voxflow-uv-cache scripts/build_deb.sh
UV_CACHE_DIR=/tmp/voxflow-uv-cache scripts/build_portable.sh
```

产物：

```text
dist/voxflow_0.2.0_amd64.deb
dist/voxflow-0.2.0_amd64.tar.gz
```

deb 元数据重点：

```text
Package: voxflow
Version: 0.2.0
Maintainer: Jiaming Wang <w_jming@outlook.com>
Provides: local-speak-input
Conflicts: local-speak-input
Replaces: local-speak-input
```

包内容检查确认：

- 包含 `/usr/bin/voxflow`、`voxflow-gui`、`voxflow-daemon`、`voxflow-tray`、`voxflow-ibus-engine`。
- `voxflow-gui` 默认打开 GTK 原生控制中心；`voxflow gui` 保留为 Web 调试控制台。
- 包含 IBus component、桌面入口、SVG 图标和内置 `faster-whisper-tiny` fallback。
- 不包含 Qwen3-ASR safetensors 分片。
- 不包含 `/opt/voxflow/models`。
- 大模型、日志、pid、缓存和用户配置放在 `$VOXFLOW_HOME`，默认 `~/.voxflow`。

apt 模拟安装：

```bash
apt-get -s install ./dist/voxflow_0.2.0_amd64.deb
```

结果摘要：

```text
Remv local-speak-input [0.1.1]
Inst voxflow (0.2.0 local-deb [amd64])
Conf voxflow (0.2.0 local-deb [amd64])
```

## 运行验证

GTK 原生控制中心：

```bash
PYTHONPATH=src python3 - <<'PY'
from voxflow.native_gui import GLib, Gtk, VoxFlowWindow
window = VoxFlowWindow(start_daemon_on_open=False)
window.show_all()
GLib.timeout_add(500, Gtk.main_quit)
Gtk.main()
window.destroy()
print('native_gui_window_smoke_ok')
PY
```

结果：

```text
native_gui_window_smoke_ok
```

Web 调试页：

```bash
node --check src/voxflow/web/app.js
```

staged rootfs：

```bash
build/deb/rootfs/opt/voxflow/venv/bin/python -m voxflow doctor
build/deb/rootfs/opt/voxflow/venv/bin/python -m voxflow models
build/deb/rootfs/opt/voxflow/venv/bin/python -m voxflow ibus-engine --dry-run --config build/deb/rootfs/etc/voxflow/config.toml
```

`doctor` 显示 `opencc`、`faster_whisper`、`sounddevice`、X11、IBus、右上角图标检查通过。`qwen_asr` 在默认轻量包中显示未安装，这是预期结果，因为默认包不把 Qwen 运行时和大模型塞进系统目录。

IBus dry-run 输出：

```text
VoxFlow IBus dry-run engine is available.
COMMIT 这是语音输入测试。
```

portable 解压包：

```bash
tar -xzf dist/voxflow-0.2.0_amd64.tar.gz -C /tmp/voxflow-portable-test
/tmp/voxflow-portable-test/voxflow-0.2.0/bin/voxflow doctor
VOXFLOW_HOME=/tmp/voxflow-portable-test/home \
  /tmp/voxflow-portable-test/voxflow-0.2.0/bin/voxflow models \
  --select qwen3-asr-1.7b \
  --import-model /path/to/Qwen3-ASR-1.7B \
  --symlink
/tmp/voxflow-portable-test/voxflow-0.2.0/bin/voxflow ibus-engine --dry-run
```

以上命令通过，portable 包可以在解压目录运行，并可把用户数据放到自定义 `VOXFLOW_HOME`。

Web 调试控制台 smoke test：

```bash
VOXFLOW_HOME=/tmp/voxflow-gui-smoke \
  build/deb/rootfs/opt/voxflow/venv/bin/python -m voxflow gui \
  --host 127.0.0.1 --port 18765 --dry-run
```

检查项：

- `HEAD /logo.svg` 返回 `200 OK` 和 `Content-Type: image/svg+xml`。
- `GET /api/config` 返回 `paths.home=/tmp/voxflow-gui-smoke`，并包含 `models`、`logs`、`run`、`cache` 路径。

## 剩余边界

- 默认 deb/portable 不内置 `qwen_asr`，因此选择 Qwen 模型后若需要本地 Qwen 推理，应安装带 Qwen runtime 的变体或在本机 Python 环境中补充该运行时；默认包的设计目标是避免把大运行时和大模型占用根分区。
- 当前 IBus 实现是分块稳定提交，不宣称 token 级实时流式 ASR。
