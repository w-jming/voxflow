# Debian 打包说明

VoxFlow deb 包面向 Ubuntu/Debian amd64 桌面环境，安装后提供：

- `/usr/bin/voxflow` 命令行入口。
- `/usr/bin/voxflow-gui` 桌面控制台入口。
- `/usr/bin/voxflow-daemon` 后台快捷键输入入口。
- `/usr/bin/voxflow-tray` 右上角控制图标入口。
- `/opt/voxflow/venv` 内置 Python 运行环境和 Python 依赖。
- `/etc/voxflow/config.toml` 系统默认配置。
- `/usr/share/applications/voxflow.desktop` 应用菜单入口。

系统级依赖由 deb `Depends` 声明，apt 会自动处理。`apt install` 会占用 `/` 所在分区的软件本体空间，但不会把 Qwen3-ASR 这类大模型安装到 `/opt`。

用户数据根目录由 `VOXFLOW_HOME` 控制，默认是 `~/.voxflow/`：

- `config.toml`：用户设置。
- `models/`：下载的大模型。
- `logs/`：GUI、daemon、tray 日志。
- `run/`：pid 等运行状态文件。
- `cache/`：预留缓存目录。

## 构建

默认构建轻量可用包，系统包内只包含程序运行时和源码内置 `faster-whisper-tiny` fallback：

```bash
scripts/build_deb.sh
```

输出：

```text
dist/voxflow_0.2.0_amd64.deb
```

构建解压版：

```bash
scripts/build_portable.sh
```

输出：

```text
dist/voxflow-0.2.0_amd64.tar.gz
```

如果确实需要在系统包内预装 Qwen 运行时依赖，可显式开启，但仍不会把模型参数打入 `/opt`：

```bash
VOXFLOW_INCLUDE_QWEN_RUNTIME=1 scripts/build_deb.sh
```

## 安装与启动

deb 安装：

```bash
sudo apt install ./dist/voxflow_0.2.0_amd64.deb
voxflow doctor
voxflow-gui
```

应用菜单入口会自动启动 GUI、后台 daemon 和右上角控制图标。默认快捷键是 `Ctrl+Space`，默认录音模式是按一次开始、再按一次停止。

解压版安装到任意目录：

```bash
mkdir -p /data/apps
tar -xzf dist/voxflow-0.2.0_amd64.tar.gz -C /data/apps
/data/apps/voxflow-0.2.0/bin/voxflow doctor
/data/apps/voxflow-0.2.0/bin/voxflow-gui
```

解压版可选安装用户级桌面入口和 IBus component：

```bash
/data/apps/voxflow-0.2.0/bin/voxflow-install-desktop
/data/apps/voxflow-0.2.0/bin/voxflow-install-ibus
```

这两个脚本只写入用户目录下的 `~/.local/share/applications`、`~/.local/share/icons` 和 `~/.local/share/ibus/component`。

## 配置

系统默认配置：

```text
/etc/voxflow/config.toml
```

用户级覆盖配置：

```text
~/.voxflow/config.toml
```

也可以自定义：

```bash
export VOXFLOW_HOME=/data/voxflow
```

配置加载顺序：内置默认值 -> 系统配置 -> 旧版 `~/.config/voxflow/config.toml` -> 新版 `$VOXFLOW_HOME/config.toml` -> CLI 参数。

控制台会写入用户级配置，保存快捷键、录音模式、输出字形和模型选择，并在后台服务运行时自动重启 daemon。

## 复用已有模型

如果本机已有 Qwen3-ASR 权重，可以先校验：

```bash
voxflow models --select qwen3-asr-1.7b --validate-model /path/to/Qwen3-ASR-1.7B
```

校验内容包括必需文件、`config.json` 架构、safetensors 索引/头部，以及官方 safetensors 权重的 SHA256。校验失败时不会写入用户配置。

如果本机已有 Qwen3-ASR 权重，不要重复下载。可以复制导入：

```bash
voxflow models --select qwen3-asr-1.7b --import-model /path/to/Qwen3-ASR-1.7B
```

如果希望避免再复制一份大模型，可以软链接导入：

```bash
voxflow models --select qwen3-asr-1.7b --import-model /path/to/Qwen3-ASR-1.7B --symlink
```

导入或软链接导入会先执行同样的 SHA256 和格式校验，通过后才会写入 `$VOXFLOW_HOME/config.toml`。导入目标默认是 `$VOXFLOW_HOME/models`，也就是 `~/.voxflow/models`。已有的开发下载缓存 `downloads/models/Qwen3-ASR-1.7B` 不会被构建脚本删除。
