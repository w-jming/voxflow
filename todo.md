# VoxFlow 下一步开发意见

更新:2026-06-11 · 依据:对 `feature/redesign-rust-core-20260610` 分支的整体审视 + 主线 A 实际开发(`docs/release/stage-3-real-asr-poc.md`)。
权威规格:`docs/redesign/` 与 `AGENTS.md`(工具链约定已更新)。

---

## 1. 现状评估

**已验证的进展**(本次实测:`cargo test` 100 通过 0 失败,fmt/clippy 干净):

- 阶段 0/1 完成:Rust workspace(9 crate)、旧 Python 删除、Core daemon(单实例/UDS JSONL/订阅广播/配置日志)、MockRecognizer 三级事件流。
- 阶段 2-6 的**离线可测骨架**全部建立:IBus zbus 引擎(私有 ibus-daemon smoke 通过)、ASR 抽象 + LCP 稳定器 + p90 门控基准骨架、模型 profile/manifest/导入/激活/删除 IPC、账本 + 规则状态机 + 安全门 + InputEvent 映射(UT 覆盖负例)、合成数据集 v0.1(1800 条)、控制台 bridge/shell 适配层 + 静态原型、Fcitx5 适配骨架。
- 文档纪律好:实现偏离处(安全门第 7 项、`input_events`、扩充错误码、wpctl 枚举)已同步回 `docs/redesign/`。

**真实差距**(所有"已勾选"都是 mock/离线层面,以下尚无一项被真实验证):

1. 没有任何真实模型跑过——sherpa-onnx 未接入,延迟门控只在 mock 上空转。
2. 没有真实音频采集——PipeWire native capture 未接入,VAD 还是能量基线。
3. 没有在真实桌面输入过一个字——GNOME 会话人工验证(阶段 2 退出条件)未做。
4. 控制台没有真实 Tauri 壳,HUD 指示器窗口未建。
5. 分类器未训练;Fcitx5 真实 C++ addon 未实现;打包未开始。

**结论**:广度铺垫已经饱和,继续横向铺骨架收益递减。**当前最大风险仍是阶段 3 go/no-go 未被检验**——如果 sherpa-onnx 在基准机上延迟不达标,下游大量工作需要返工。下一步必须转入纵向打穿。

---

## 2. 下一步主线(按优先级)

### 主线 A(最高优先):打穿真实听写垂直切片 → 阶段 3 go/no-go

目标:真实麦克风 → PipeWire native → VAD → sherpa-onnx → partial/stable → IBus preedit,在真实 GNOME 会话输入第一句话。

**2026-06-11 进展(报告:`docs/release/stage-3-real-asr-poc.md`)——自动化部分已全部打穿:**

- [x] **确认 D-17/D-18**(所有者 2026-06-11 批复,按建议执行)。
- [x] `crates/voxflow-asr-sherpa` 建成:经 sherpa-rs-sys 封装 online C API(高层 sherpa-rs 无流式封装,见报告 §6),实现 `StreamingRecognizer`;`sherpa-poc` 基准工具(离线 RTF/实时延迟/`--loop-minutes` 稳定性)。
- [x] streaming Zipformer 双语 int8 实测:**4 核限核 RTF 0.053、首 partial p90 471ms ✅、首 stable p90 953ms ✅、10 分钟稳定回放峰值 RSS 587MB**;候选表与 `model-profiles/streaming-zh-en-small.toml`(真实 URL/sha256)已回填。
- [x] PipeWire native 采集:`voxflow-audio` `pipewire-native` feature(非默认),真实守护进程 smoke 通过;无 sudo 环境用 `scripts/dev/pipewire-env.sh`。
- [x] silero VAD 接入(`voxflow_asr_sherpa::vad::SileroVad`,默认;EnergyVad 兜底);VAD 起点埋点进基准报告。
- [x] `live-poc` bin(`--features live-poc`):一条命令跑通"麦克风→PipeWire→silero→sherpa→终端三级事件",供 go/no-go 主观验收。
- [ ] 真实模型接入 `voxflow-core` 流水线与模型激活链路(替换 `pending_runtime_integration`;recognizer 工厂按 profile 选 Mock/Sherpa)。
- [ ] 自录中文/英文/中英混说回放样本集(不入 git);现有结论基于模型自带 4 条混说样本,**首 stable p90 距预算仅 4.7%,样本扩充后必须复测**。
- [ ] 🧑 **人工验证点(go/no-go)**:跑 `live-poc`(命令见报告 §7),中文/英文/混说各一段,主观评价延迟与可读性;通过则签字关闭阶段 3 复审点。
- [ ] 🟡 **确认 D-20**(工具链政策:rustup stable 取代 MSRV 1.75 承诺;已先行实施,见 review-and-decisions §3.1)。

### 主线 B(可立即并行,不依赖 A):阶段 2 收尾——真实桌面 IBus 验证

mock 链路已就绪,这一步只差真实环境,且能提前暴露 zbus 信号序列化与各应用 preedit 兼容性问题:

- [ ] 用户级安装 component 到真实会话,`ibus restart` 后在 GNOME Text Editor + Chrome 上跑 mock 听写链路(preedit/commit/delete)。
- [ ] 🧑 **人工验证点**:GNOME Wayland 与 X11 各走查一次;确认 preedit 无占位文案、commit 无残留;记录到 `docs/release/stage-2-ibus-poc.md` 并关闭阶段 2。

### 主线 C(进行中):阶段 4 真实化

**2026-06-11 批次 2 完成(记录:`docs/release/stage-4-control-center-poc.md` 批次 2)**:

- [x] 真实 Tauri 2 app(`apps/control-center`,crate `voxflow-control-center`):泵任务独占 `ShellIpcSession`,`core_command` + 三事件通道契约落地,断线退避重连;对真实 daemon 10s smoke + headless 截图视觉冒烟通过。
- [x] React 18 + Vite + Zustand 前端:总览页 + **模型管理页**(多模型一键下载 + 实时进度/暂停/取消、激活、删除、本地导入校验);其余 6 页骨架。
- [x] core 侧 `model.download/pause/resume/cancel` + `model.progress`(HF 按文件下载、Range 续传、sha256、原子安装);3 个 HF profile(双语 2023 推荐 / 中文 2025 SOTA / 中文 xlarge 2025);真网端到端实测通过(streaming-zh-2025 167MB)。
- [ ] 🆕 **所有者体验反馈(2026-06-12,优先)**:① 控制中心更新/重启不得"闪退"——需要平滑升级路径(single-instance + 窗口状态保留;开发期 agent 替换二进制时改为提示用户重启,而非直接 kill);② 连接体验优化——启动即渲染骨架屏,连接中给出进度反馈,缩短首连/重连等待(当前退避 500ms 起步,可对首次连接做更密集重试),断线横幅含重试倒计时。
- [ ] 🆕 **所有者反馈(2026-06-12):模型管理页纳入 Qwen 系列模型并优化展示形式**。① 内容:把 Qwen3-ASR-1.7B(vLLM 默认后端,权重现走 HF 缓存、对模型管理器不可见)与 Qwen3-ASR-0.6B(精修层,D-21)纳入统一 profile 体系——扩展 profile schema 支持 `backend = qwen3_vllm` 类目(HF safetensors 文件清单 + sha256),使下载/校验/删除/占用显示与 zipformer 同一套管理链路;显示驻留状态(已下载/已加载/显存占用)。② 展示:单页平铺改为分组/多页形式——按用途分组(实时流式 / 精修 / 云端 API)或分页签,并与「输入」页后端选择联动标注"当前后端使用中";体积/语言/推荐标签做视觉分层(规格按 control-center-spec 演进)。
- [ ] 其余 5 页实化(语义修正/音频/外观/诊断/关于,规格见 control-center-spec;输入页已含后端切换)。
- [ ] tray、single-instance、退出语义(UJ-09 三层)、原生文件选择器(tauri-plugin-dialog)。
- [ ] HUD 状态指示器窗口:X11 定位/置顶 → gtk-layer-shell POC(KDE/wlroots)→ GNOME Wayland 托盘降级;🧑 三种环境各人工走查一次。
- [ ] 🧑 所有者走查批次 2:总览连接 + 模型页全流程(命令见阶段记录);确认后删除批次 1 静态原型。

### D-22 后端切换(2026-06-11 实现)

- [x] `config.asr.backend`:qwen3_vllm(默认)/ volcano_api / zipformer_local / mock;dictation.start 时按配置构建识别器。
- [x] `voxflow-asr-qwen3`:官方 qwen-asr[vllm] 流式 API 经 Python sidecar(JSONL stdio);`voxflow-asr-volcano`:火山 wss 二进制协议(协议帧/映射均有离线 UT)。
- [x] 控制中心「输入」页:后端切换 + 火山密钥配置(仅存本机)。
- [x] `scripts/deploy-local.sh` → `~/software/voxflow`(uv venv + qwen-asr[vllm] + 1.7B 权重预下载)。
- [ ] 🧑 真机测试:Qwen3 sidecar 首次加载与流式听写。
- [ ] ⚠️ **火山 API 完全未经真实服务测试(2026-06-12 标注:暂不具备测试条件,无可用密钥)**——协议按公开文档+参考实现编写,仅离线单测;获得密钥后必须先跑 `asr-live --backend volcano_api` 实测再视为可用,并同步更新 review-and-decisions D-22 与部署 README 的标注。
- [ ] core 流水线真实音频泵接入(dictation.start 后 PipeWire→VAD→recognizer 持续推流;当前 IPC 层已按后端构建识别器,但听写音频循环仍待接)。
- [ ] 🆕 **所有者硬性验收(2026-06-12):按下快捷键必须即刻进入听写,不得有加载/预热等待**。实现要求:① daemon 启动时后台**预载 + 预热** Qwen3 sidecar(一次性 1-2 分钟,登录时无感完成),模型常驻显存(~9.4GB,D-22 取舍);② sidecar 跨会话复用已实现(有 UT),Alt+S → start_session 为毫秒级;③ 可选:空闲 N 分钟自动卸载省显存、再按键时重载(默认关闭,作为设置项);④ 验收口径:热路径"按键→可听写"< 200ms。注:asr-live 每次冷启动是测试工具行为,与产品形态无关。

### 输入法链路遗留(2026-06-12 本批次发现)

- [ ] IBus 引擎**免 sudo 自注册**:IBus 1.5.29 不扫描用户组件目录;改为引擎进程连 IBus 私有总线(地址经 `ibus address`)调用 `RegisterComponent` 运行时注册 + 在该连接上服务 factory;部署脚本随 core 一起常驻启动引擎。当前过渡:一条 sudo cp 安装组件 XML。
- [ ] 真实听写流水线接语义修正:pump 的 Final 暂未过账本/安全门(runtime.rs 注记);需把 project_final_event 路径接入 pump 事件流。
- [ ] 控制中心前端已用 frontend-design 插件重构(暗色“声学仪器”系统);后续页面迭代继续使用该插件。

### GPU 优先与 2026 模型(D-21,2026-06-11 新增;调研见 `docs/redesign/architecture/model-research-2026.md`)

- [ ] sherpa-onnx CUDA EP 接入调研(sherpa-rs `cuda` feature 与 GPU 预编译库分发),P0 zipformer 链路 GPU 加速实测。
- [ ] Qwen3-ASR-0.6B int8(sherpa-onnx 离线+VAD 包 2026-03-25)接入 `voxflow-asr-sherpa` 离线识别器封装 → final/refine 流水线(D-9 升级);新增模型 profile。
- [ ] Qwen3-ASR-1.7B + vLLM 本地 sidecar POC(真流式,RTX 5070 Ti):延迟/显存/启动实测,决定是否进"高准确率流式模式"。

### 后续(依赖前三条主线的产出)

- [ ] 阶段 5 收尾:🧑 合成数据集抽样复核(≥10%)→ 按 D-18 训练 LR/SVM 头 + ONNX 导出 → ort 接入 → 发布门指标记录;修正事件接入真实 ASR 流水线;控制台语义修正页。
- [ ] 阶段 6:真实 Fcitx5 薄 C++ addon(需本机安装 fcitx5 开发环境,目前 probe 显示缺失);🧑 KDE Plasma Wayland 会话 smoke。
- [ ] 阶段 7:单 deb + portable、systemd user unit、容器包测试矩阵、发布检查单。
- [ ] 模型下载链路(reqwest Range 续传 + 磁盘预检)——`model.download/pause/resume/cancel` 目前仅有 IPC 占位,建议放在 A 之后、阶段 7 之前实现。

### 风险与纪律提醒

- 阶段退出**串行**:骨架先行可以,但每阶段人工验证点(🧑)未签字不得勾选阶段完成(已写入 `migration-plan.md` 0.4 执行方式注记)。
- `model-profiles/streaming-zh-en-small.toml` 是占位(example.invalid + TO_BE_FILLED),已被实现正确判为 profile_issue;**在 D-1 POC 回填前不得让它进入"可下载/推荐"路径**。
- `runtime_smoke: pending_runtime_integration`(model.activate)必须在真实 runtime 接入后升级为实际 smoke 结果,不得长期保留占位值。
- 合成数据集不可直接当训练真值;误删率指标只有在真实样本混入后才有意义。
- zbus 固定 =4.4(D-19),不要动;工具链已切 rustup stable(D-20 待确认),`Cargo.lock` 的 `idna_adapter`/`tempfile`/`home` pin 勿整体 `cargo update`。
- sherpa 预编译库供应链风险(D-17 已知):正式发布前评估切官方 C API 源码构建;`voxflow-asr-sherpa` 首次构建需联网。

---

## 3. 本轮改版文档更改说明(0.4)

本轮(2026-06-10 第三轮)把实现反馈并入 `docs/redesign/`,共改 9 篇:

| 文档 | 更改 |
| --- | --- |
| architecture/ipc-api.md | 版本 0.2→0.4;头部加实现状态注记(实现期新增的 model.* 详式 schema、audio.list_devices、`correction.applied.input_events`、扩充错误码均已是合同的一部分,机器可读 schema 在 `schemas/ipc/`) |
| architecture/semantic-correction.md | 版本 0.2→0.4;头部注记:安全门第 7 项与"完整 segment 删除 + 整段重提交"映射规则已落地并有 UT;分类器未训练,指向 D-18 |
| architecture/streaming-asr.md | 版本→0.4;新增 §9 实现注记:已落地的稳定器/基准/门控/音频骨架清单 + 待接入真实链路清单;候选表回填义务挂到 D-17 POC |
| architecture/system-architecture.md | 版本→0.4;技术选型表更新:zbus 固定 =4.4(D-19)、wpctl 仅作过渡期枚举、EnergyVad 为基线;补实际 9-crate 划分对照 |
| architecture/semantic-intent-classifier.md | 版本→0.4;注记合成数据集 v0.1(1080/360/360)已存在,训练前必须所有者抽样复核,指向 D-18 |
| frontend/tauri-ui.md | 版本→0.4;ipc-bridge 契约与实现对齐:固定三事件通道(`connection-changed`/`control-snapshot`/`core-event`)、CLI smoke 入口;注明静态原型为过渡物,React 版可用后删除 |
| engineering/migration-plan.md | 版本→0.4;新增进度跟踪指针(docs/release + todo.md)与"广度优先铺骨架允许、阶段退出必须串行"的执行方式注记 |
| review-and-decisions.md | 版本→0.4;新增 §3.1 实现期决策:**D-17 sherpa-onnx 集成方式(建议先 sherpa-rs POC,独立 backend crate,待确认)**、**D-18 分类器训练工具链(建议允许 Python dev-time 工具 + skl2onnx,待确认)**、D-19 zbus =4.4 + MSRV 1.75(已按此实现,作记录) |
| README.md(redesign) | 版本→0.4;状态改"实施中",指向进度记录与 D-17/18 待确认项 |

未改动:产品/设计/平台/其余工程文档(0.3 仍有效);开发侧此前对 ipc-api 与 semantic-correction 的内容修改全部保留,仅补版本头与注记。

**需要所有者动作的三件事**:① 确认 D-17、D-18;② 主线 B 的 GNOME 真实会话人工验证;③ 主线 A 完成后的 go/no-go 签字。

---

## 4. 开发规则(不变)

- 不直接在 `main` 开发;当前分支 `feature/redesign-rust-core-20260610`。
- 任何偏离 `docs/redesign/` 的行为变更必须先更新文档并提升版本号。
- 包产物、大模型、日志、音频样本、secret 不提交;推送前执行 `scripts/secret-scan.sh`。
- 阶段执行记录写入 `docs/release/stage-*.md`,人工验证点结果须留痕。
