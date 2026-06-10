# 架构说明

## 模块

- `voxflow.asr`：ASR 抽象和后端实现。当前包含 `faster-whisper`、`qwen-asr`、OpenAI 兼容 HTTP API。
- `voxflow.postprocess`：口语助词清理、自动标点、撤回/修正命令和文本历史。
- `voxflow.input`：Linux 文本注入，自动选择 `wtype`、`xdotool`、`ydotool`。
- `voxflow.audio`：轻量能量阈值 VAD 录音，适合第一版听写。
- `voxflow.composition`：VoxFlow 自己提交文本的账本和 IBus composition/commit 命令转换。
- `voxflow.semantic_intent`：语义撤销后端注册表；默认只向软件界面暴露已实现的规则后端，计划后端仅保留在源码/文档中。
- `voxflow.ibus_engine`：IBus 系统输入法引擎，负责 preedit、稳定 commit 和连续听写控制。
- `voxflow.native_gui`：GTK 原生控制中心，负责后台服务控制、快捷键、模型下载/暂停、本地模型导入和数据目录设置。
- `voxflow.gui`：无额外后端依赖的本地 Web 调试控制台。
- `voxflow.hotkey` / `voxflow.daemon`：X11 全局快捷键和后台语音输入服务。
- `voxflow.tray` / `voxflow.service_control`：右上角 AppIndicator 控制图标和 GUI/daemon 进程管理。
- `voxflow.model_registry`：模型档位、许可证、官方来源、下载/选择逻辑，以及本地模型导入前的必需文件、`config.json`、safetensors 和官方 SHA256 校验。
- `voxflow.paths`：统一用户数据目录，默认 `~/.voxflow`，可通过 `VOXFLOW_HOME` 自定义。

## 口语修正算法

后处理采用单次线性扫描：

1. 正则识别高置信命令：“哦不”“不对”“错了”“句首不是”“撤回刚才”等。
2. 命令前已有文本时，优先在当前识别片段内局部修正，避免不必要退格。
3. 命令出现在片段开头时，从会话历史中删除最近一个语义单元，再插入后续文本。
4. 历史以已注入文本 chunk 保存，撤回只检查最后一个 chunk，常规操作复杂度接近 O(n)。

局部修正规则优先删除最近的时间/数量表达，其次删除最近英文 token 或最后一个空格后的片段；中文整句则退回到最近标点边界。

简体/繁体转换使用 OpenCC。默认输出简体，用户可切换为繁体或模型原文。

语义撤销识别由 `[text] semantic_correction_enabled` 控制。默认可用路径是规则状态机 + 注入账本：规则只产生撤销动作，实际删除数量由账本限制。`[text] semantic_intent_backend` 当前只能保存可用的 `rules` 后端；MiniLM/SetFit、Qwen3-Embedding 分类头和 LLM 低置信仲裁被设计为可插拔后端，在没有训练分类头、安装模型和回归集前不会作为可选运行能力暴露。

## 原生控制中心

`voxflow-gui` 默认启动 GTK 原生控制中心：

- 后台输入服务启动、停止、重启和日志目录打开。
- 快捷键、录音模式、输出字形、智能撤销和模型选择。
- 用户数据目录设置；无 `VOXFLOW_HOME` 环境变量时写入 `~/.config/voxflow/home` 指针文件。
- Qwen3-ASR 下载进度、速度、暂停/继续。
- 已有本地 Qwen 权重校验、复制导入和软链接导入。

## Web 调试控制台

`voxflow gui` 使用 Python 标准库 HTTP server 提供调试页面：

- 浏览器端使用 Web Audio API 绘制实时波形和音量带。
- 浏览器端使用 MediaRecorder 录音，上传到 `/api/transcribe`。
- 后端懒加载 ASR 模型，避免打开页面时立即加载大模型。
- `/api/process-text` 可在没有 ASR 模型时测试口语修正逻辑。
- `/api/models`、`/api/models/download`、`/api/models/download/status`、`/api/models/validate-local` 和 `/api/models/import-local` 提供模型列表、后台下载状态、本地校验和导入。

## 输入注入边界

Web 调试控制台的“系统输入”由后端调用系统输入工具实现。浏览器录音时焦点通常在调试页面，因此正式长时间听写更适合使用 `voxflow dictate`、后台快捷键或 IBus 模式；Web 页面主要用于观察、调试、短句录入和模型验证。

后台 daemon 使用 X11 `XGrabKey` 注册全局快捷键，触发时通过 `xdotool getactivewindow` 记录目标窗口，识别结束后再激活该窗口并输入文本。快捷键支持 `toggle` 和 `hold` 两种状态机：`toggle` 按一次开始、再按一次停止；`hold` 按住超过阈值后录音、松开停止。Wayland 下不能保证全局快捷键和窗口激活。

deb 包同时注册 IBus 引擎 `VoxFlow Input`。IBus 模式不依赖剪贴板或窗口重激活：引擎直接把临时识别显示为 preedit，把稳定分句 commit 到当前输入上下文；撤销命令经过 `InjectionLedger` 限制，只删除 VoxFlow 在当前会话提交过的字符。当前实现是分块稳定提交，不宣称 token 级实时流式 ASR。

控制中心提供设置接口，只更新用户级 `$VOXFLOW_HOME/config.toml`，默认是 `~/.voxflow/config.toml`；旧版 `~/.config/voxflow/config.toml` 仍会被读取以兼容升级。下载模型位于 `$VOXFLOW_HOME/models`，日志位于 `$VOXFLOW_HOME/logs`，pid 等运行状态位于 `$VOXFLOW_HOME/run`。本地 Qwen 权重导入或软链接导入时，VoxFlow 会先做模型身份和格式校验，通过后才写入用户配置。右上角图标使用 GTK AppIndicator，负责启动/停止/重启后台输入、打开控制中心和退出 VoxFlow；退出 VoxFlow 会停止后台 daemon。
