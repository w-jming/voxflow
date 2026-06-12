# 阶段 4 控制台基础记录

日期:2026-06-10

## 已完成

- 新增 `voxflow-control` crate,作为 Tauri 控制台前的离线可验证基础:
  - `ControlCenterSnapshot`:把 `core.status` 快照投影为控制台 store 形状。
  - 四张总览状态卡:输入服务、输入法前端、麦克风、模型。
  - 八页导航:总览、输入、模型、音频、语义修正、数据、诊断、外观。
  - 静态 bundle 写出命令:`voxflow-control write-web [output-dir]`。
  - 状态 JSON 输出命令:`voxflow-control snapshot-json`。
- 新增控制台 Rust 侧 IPC bridge 基础:
  - `CoreBridge::connect`:连接 Core UDS。
  - `hello`:按 `core.hello` 协议协商版本。
  - `command`:通用 JSONL 命令转发,保留 response 前到达的 event。
  - `status`:读取 `core.status` 并反序列化为 `StatusSnapshot`。
  - `subscribe`:订阅 state/model/correction/audio_level 等事件组。
  - `read_next`:读取后续 Core event。
  - `ReconnectPolicy`:0.5 s 起、10 s 封顶的指数退避策略。
  - CLI smoke:`voxflow-control bridge-status SOCKET`。
- 新增 Tauri shell 可复用 adapter 层:
  - `CoreCommandInvocation`:对应未来 `invoke("core_command", { name, payload })` 的入参形状。
  - `ShellEvent`:对应未来 Tauri `emit(...)` 的事件形状。
  - `ShellIpcSession`:完成连接、hello、默认订阅、status 全量同步、命令转发、Core event 转发。
  - 事件通道固定为 `connection-changed`、`control-snapshot`、`core-event`。
  - 默认 UI 订阅组:`state`、`model`、`correction`。
  - CLI smoke:`voxflow-control shell-status SOCKET`、`voxflow-control shell-command SOCKET NAME [PAYLOAD_JSON]`。
- 新增无 Node 构建依赖的静态控制台原型:
  - `index.html`
  - `app.css`
  - `app.js`
  - light/dark logo 与 symbol 资产
- UI 已按 `docs/redesign/design/ui-system.md` 使用控制台信息架构、状态卡、侧栏、主题 token、状态 badge、模型/音频/语义修正/数据/诊断/外观页基础控件。
- 响应式修复:
  - 桌面 1080x720 双列状态卡。
  - 窄屏 390x844 下侧栏转横向导航,内容不被挤压,长模型名可换行。

## 当前限制

- 这不是完整 Tauri 2 shell;尚未接入 Tauri invoke/event、tray、single-instance 或多窗口 HUD。
- 这不是 React/Vite 最终实现;当前是离线静态原型,用于固定状态契约和视觉首屏。
- 前端按钮只带 `data-command`,尚未通过真实 Tauri WebView invoke 接到 `ShellIpcSession`。
- 状态指示器只有页面内预览,尚未实现独立 Tauri HUD 窗口。

## 验证

```bash
cargo test -p voxflow-control
cargo run -p voxflow-control -- snapshot-json
cargo run -p voxflow-control -- write-web target/voxflow-control-web
VOXFLOW_HOME=/tmp/voxflow-control-shell-smoke XDG_RUNTIME_DIR=/tmp/voxflow-control-shell-smoke/runtime cargo run -p voxflow-core -- serve
cargo run -p voxflow-control -- shell-status /tmp/voxflow-control-shell-smoke/runtime/voxflow/core.sock
cargo run -p voxflow-control -- shell-command /tmp/voxflow-control-shell-smoke/runtime/voxflow/core.sock diagnostics.run
google-chrome --headless=new --disable-gpu --screenshot=/tmp/voxflow-control-desktop.png --window-size=1080,720 file:///home/terry/workplace/voxflow/target/voxflow-control-web/index.html
google-chrome --headless=new --disable-gpu --screenshot=/tmp/voxflow-control-narrow.png --window-size=390,844 file:///home/terry/workplace/voxflow/target/voxflow-control-web/index.html
```

自动化覆盖:

- 控制台 store/status card 聚合。
- 静态 bundle 生成和品牌资产存在性。
- UDS JSONL bridge 的 hello/status/subscribe/event 读取。
- Core error envelope 回传。
- 重连退避策略。
- Shell adapter 的 connection/snapshot/core-event 发射。
- Shell adapter 的命令转发与 response 前 event 转发。
- 真实临时 Core UDS smoke:
  - `shell-status` 返回 `connection-changed` + `control-snapshot`。
  - `shell-command ... diagnostics.run` 返回 Core doctor checks。

最近截图检查:

- 桌面:首屏非空白,品牌资产、四张状态卡、侧栏和快速操作渲染正常。
- 窄屏:首屏非空白,内容全宽显示,无页面级横向滚动,状态卡文本无明显重叠。

---

# 批次 2(2026-06-11):真实 Tauri 2 壳 + 模型管理页

状态:壳与模型管理页可用,其余 6 页骨架;🧑 桌面视觉与交互走查待所有者。

## 交付物

| 物件 | 位置 |
| --- | --- |
| Tauri 2 壳 crate | `apps/control-center/src-tauri`(`voxflow-control-center`,workspace 成员,tauri 2.11) |
| React 前端 | `apps/control-center/src`(React 18 + Vite 5 + TS strict + Zustand,D-4) |
| 桥接 | 单 invoke 命令 `core_command` + 三事件通道(批次 1 固定的契约),复用 `ShellIpcSession`;泵任务独占会话,命令经 mpsc + oneshot 进泵,事件轮询用 100ms 超时(`next_line` 取消安全),断线按 `ReconnectPolicy` 指数退避重连 |
| 页面 | 总览(连接状态 + 状态卡片);**模型管理**:多模型卡片、一键下载(实时进度/速度/ETA)、暂停/取消、激活、删除、本地导入(目录 + copy/symlink,经 core 校验);其余 6 页占位 |
| 图标 | `icons/icon.png`(品牌色波形,程序化生成占位;正式图标待视觉批次) |

## 同批次 core 侧:模型下载管理器

- `model.download/pause/resume/cancel` + `model.progress`(≤4/s 节流)按 ipc-api §3.6/§4.3 实现(`voxflow-core/src/download.rs`):HTTP Range 断点续传(`.part`)、逐文件 sha256、`.staging-<id>` 原子安装 + manifest.lock。
- 3 个 Hugging Face profile:双语 2023-02-20(POC 已验,推荐)、streaming-zh-2025(k2-fsa 2025-06-30 中文 SOTA,167 MB)、streaming-zh-xlarge-2025(最高准确率,771 MB)。
- **真网端到端实测**:经守护进程 IPC 从 HF 下载 streaming-zh-2025 → 校验 → 安装 → 清单转 ready;`sherpa-poc` 加载解码正常(混合精度)。
- 模型仅落 `VOXFLOW_HOME`(默认 `~/.voxflow`),不写系统目录。

## 验证

- `npm run build`(tsc strict + vite)通过;`cargo clippy --workspace --all-targets -D warnings` 0 警告;全工作区 110 测试通过(含 download 3 项:完整安装/续传/校验失败)。
- 壳对真实守护进程运行 10 s 无崩溃、无警告日志。

## 已知限制与偏离

1. 工作区放宽 `toml`/`indexmap` 精确 pin(与 tauri 2.11 无法统一;原为 MSRV 1.75 防御,D-20 后无必要;可复现性由 Cargo.lock 保证)。zbus `=4.4`、`async-lock =3.3.0`(D-19)不变。
2. 托盘、single-instance、退出语义(UJ-09)、HUD 指示器窗口、主题三态、其余 6 页:后续批次。
3. 本地导入为路径输入框;原生文件选择器(tauri-plugin-dialog)后续批次。
4. 批次 1 的静态原型(`voxflow-control write-web`)按 tauri-ui.md 约定,React 版可用后删除——留待所有者视觉走查确认后执行。
5. 🧑 所有者走查:`cargo run -p voxflow-core -- serve` + `cargo run -p voxflow-control-center`,检查总览连接状态、模型页下载/暂停/取消/激活/导入全流程。
