# 架构说明

## 模块

- `local_speak_input.asr`：ASR 抽象和后端实现。当前包含 `faster-whisper`、`qwen-asr`、OpenAI 兼容 HTTP API。
- `local_speak_input.postprocess`：口语助词清理、自动标点、撤回/修正命令和文本历史。
- `local_speak_input.input`：Linux 文本注入，自动选择 `wtype`、`xdotool`、`ydotool`。
- `local_speak_input.audio`：轻量能量阈值 VAD 录音，适合第一版听写。
- `local_speak_input.gui`：无额外后端依赖的本地 Web 控制台。
- `local_speak_input.hotkey` / `local_speak_input.daemon`：X11 全局快捷键和后台语音输入服务。
- `local_speak_input.tray` / `local_speak_input.service_control`：右上角 AppIndicator 控制图标和 GUI/daemon 进程管理。

## 口语修正算法

后处理采用单次线性扫描：

1. 正则识别高置信命令：“哦不”“不对”“错了”“句首不是”“撤回刚才”等。
2. 命令前已有文本时，优先在当前识别片段内局部修正，避免不必要退格。
3. 命令出现在片段开头时，从会话历史中删除最近一个语义单元，再插入后续文本。
4. 历史以已注入文本 chunk 保存，撤回只检查最后一个 chunk，常规操作复杂度接近 O(n)。

局部修正规则优先删除最近的时间/数量表达，其次删除最近英文 token 或最后一个空格后的片段；中文整句则退回到最近标点边界。

## 可视化控制台

`local-speak gui` 使用 Python 标准库 HTTP server 提供页面：

- 浏览器端使用 Web Audio API 绘制实时波形和音量带。
- 浏览器端使用 MediaRecorder 录音，上传到 `/api/transcribe`。
- 后端懒加载 ASR 模型，避免打开页面时立即加载大模型。
- `/api/process-text` 可在没有 ASR 模型时测试口语修正逻辑。

## 输入注入边界

Web 控制台的“系统输入”由后端调用系统输入工具实现。浏览器录音时焦点通常在控制台页面，因此正式长时间听写更适合使用 `local-speak dictate` 后台模式；控制台主要用于观察、调试、短句录入和模型验证。

后台 daemon 使用 X11 `XGrabKey` 注册全局快捷键，触发时通过 `xdotool getactivewindow` 记录目标窗口，识别结束后再激活该窗口并输入文本。Wayland 下不能保证全局快捷键和窗口激活，后续需要接入桌面门户或输入法框架。

控制台提供快捷键设置接口，只更新用户级 `~/.config/local-speak-input/config.toml` 中的 `[daemon] hotkey`，避免覆盖系统默认配置或其它用户配置。右上角图标使用 GTK AppIndicator，负责启动/停止/重启后台输入和打开控制台；在 GNOME 上依赖 AppIndicator/KStatusNotifier 扩展。
