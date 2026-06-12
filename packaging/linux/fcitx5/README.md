# Fcitx5 前端 POC

本目录保存阶段 6 的 Fcitx5 安装骨架。Fcitx5 是 Linux P0 输入法前端之一,用于覆盖 KDE/wlroots 系桌面,并验证输入法抽象不是 IBus 单实现。

当前状态:

- `voxflow-fcitx5` Rust crate 已覆盖平台无关适配层:
  - `InputEvent` → Fcitx5 操作枚举。
  - `frontend.register` 能力声明为 `kind=fcitx5`。
  - addon 与 inputmethod 配置生成。
  - 本机 `fcitx5` 命令与 pkg-config 探测。
- 本机当前没有 `fcitx5` 命令,也没有 `fcitx5` pkg-config 开发文件;真实 C++ addon 尚未构建。
- 薄 C++ addon 仍是阶段 6 未完成项。它必须只做协议翻译,经 UDS 连接 Core,不得下沉 ASR、账本或修正业务逻辑。

验证命令:

```bash
cargo run -p voxflow-fcitx5 -- addon-conf
cargo run -p voxflow-fcitx5 -- inputmethod-conf
cargo run -p voxflow-fcitx5 -- register-json
cargo run -p voxflow-fcitx5 -- self-test
cargo run -p voxflow-fcitx5 -- probe
```

portable 安装脚本需要真实 `voxflow.so` addon 动态库路径。当前没有动态库时脚本会拒绝安装,避免把不可用输入法注册进用户环境。
