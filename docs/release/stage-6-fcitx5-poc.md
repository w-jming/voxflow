# 阶段 6 Fcitx5 前端骨架记录

日期:2026-06-10

## 已完成

- 新增 `voxflow-fcitx5` crate,建立 Fcitx5 前端的离线可测骨架:
  - `Fcitx5EngineAdapter`,把平台无关 `InputEvent` 翻译为 Fcitx5 操作枚举。
  - `addon-conf` / `inputmethod-conf` 配置生成。
  - `register-json`,声明 `kind=fcitx5` 与 `preedit` / `surrounding_text` / `delete_surrounding` 能力。
  - `self-test`,验证 mock dictation 与 correction 事件可翻译为 preedit/commit/delete 操作。
  - `probe`,输出本机 `fcitx5` 命令与 pkg-config 开发文件状态。
- 新增用户级 Fcitx5 metadata 安装/卸载脚本骨架:
  - `packaging/linux/fcitx5/install-fcitx5-user.sh`
  - `packaging/linux/fcitx5/uninstall-fcitx5-user.sh`

## 当前限制

- 本机当前没有 `fcitx5` 命令,也没有 `fcitx5` pkg-config 开发文件。
- 真实 Fcitx5 薄 C++ addon 动态库尚未实现,因此还不能完成 KDE Plasma Wayland 真实应用 smoke。
- `install-fcitx5-user.sh` 需要真实 `voxflow.so` addon 动态库路径;没有动态库时会拒绝安装,避免注册不可用输入法。

## 验证

```bash
cargo test -p voxflow-fcitx5
cargo run -p voxflow-fcitx5 -- self-test
cargo run -p voxflow-fcitx5 -- probe
scripts/dev-check.sh
```
