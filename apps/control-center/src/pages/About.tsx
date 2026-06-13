import { invoke } from "@tauri-apps/api/core";
import { PageHead } from "../App";
import { useControlStore } from "../store";

export default function About() {
  const status = useControlStore((state) => state.status);
  return (
    <div>
      <PageHead
        index="08"
        title="关于"
        desc="VoxFlow / 声流输入法 — Linux 优先的本地流式语音输入法。"
      />
      <section className="panel">
        <div className="panel-grid">
          <div className="telemetry">
            <span className="k">Core</span>
            <span className="v">{status?.core?.version ?? "0.3.0"}</span>
          </div>
          <div className="telemetry">
            <span className="k">Architecture</span>
            <span className="v">Rust Core + Tauri 2</span>
          </div>
          <div className="telemetry">
            <span className="k">License</span>
            <span className="v">MIT</span>
          </div>
          <div className="telemetry">
            <span className="k">Data Dir</span>
            <span className="v">~/.voxflow</span>
          </div>
        </div>
      </section>
      <section className="panel">
        <div className="panel-label">产品原则</div>
        <p className="muted" style={{ margin: 0 }}>
          本地优先 · 数据不出本机(云端 API 为显式可选)· token 级真流式 ·
          所有删除经账本安全门 · 模型/日志/配置均存于用户目录。
        </p>
      </section>
      <section className="panel">
        <div className="panel-label">应用控制</div>
        <div className="row">
          <button
            className="vf"
            onClick={() => void invoke("restart_app_command")}
          >
            重启控制中心
          </button>
          <button
            className="vf danger"
            onClick={() => void invoke("quit_app_command")}
          >
            退出应用
          </button>
          <span className="hint">
            退出仅关闭控制中心与听写;Core 守护进程随之停止当前听写会话。
          </span>
        </div>
      </section>
      <p className="muted mono">github.com/w-jming/voxflow</p>
    </div>
  );
}
