import { useEffect } from "react";
import { PageHead } from "../App";
import { useControlStore } from "../store";

const BACKEND_LABEL: Record<string, string> = {
  qwen3_vllm: "Qwen3-ASR · 本地 GPU",
  volcano_api: "火山引擎 · 云端",
  zipformer_local: "Zipformer · 本地 CPU",
  mock: "Mock(测试)",
};

const STATE_LABEL: Record<string, string> = {
  idle: "待机",
  listening: "听写中",
  error: "异常",
};

export default function Overview() {
  const connection = useControlStore((state) => state.connection);
  const connectionError = useControlStore((state) => state.connectionError);
  const status = useControlStore((state) => state.status);
  const refreshStatus = useControlStore((state) => state.refreshStatus);

  useEffect(() => {
    if (connection === "connected") {
      void refreshStatus();
    }
  }, [connection, refreshStatus]);

  const dictation = status?.dictation?.state ?? "idle";
  const engine = status?.models?.engine_state ?? "—";
  const backend = status?.models?.asr_backend ?? "—";
  const uptime = status?.core?.uptime_ms
    ? `${Math.floor((status.core.uptime_ms as number) / 60000)} min`
    : "—";
  const meterMode =
    dictation === "listening" ? "listening" : engine === "ready" ? "ready" : "";

  return (
    <div>
      <PageHead
        index="01"
        title="总览"
        desc="守护进程、识别引擎与听写链路的实时状态。"
      />

      {connectionError ? (
        <div className="error-banner">{connectionError}</div>
      ) : null}
      {engine !== "ready" && connection === "connected" ? (
        <div
          className="error-banner"
          style={{
            borderColor: "rgba(251,191,36,0.5)",
            background: "rgba(251,191,36,0.08)",
            color: "var(--vf-warn)",
          }}
        >
          识别引擎{engine === "loading" ? "正在加载(约 1-2 分钟)" : `状态:${engine}`}
          ,此期间听写不可用。
        </div>
      ) : null}

      <section className="panel">
        <div className="panel-label">听写状态</div>
        <div className={`dictation-meter ${meterMode}`}>
          <div className="meter-bars">
            <i /> <i /> <i /> <i /> <i /> <i /> <i />
          </div>
          <div>
            <div className="meter-state">
              {STATE_LABEL[dictation] ?? dictation}
            </div>
            <div className="muted">
              Alt+S 开始/停止 · 当前后端 {BACKEND_LABEL[backend] ?? backend}
            </div>
          </div>
        </div>
      </section>

      <section className="panel">
        <div className="panel-label">运行遥测</div>
        <div className="panel-grid">
          <div className="telemetry">
            <span className="k">连接</span>
            <span className={`v ${connection === "connected" ? "glow" : ""}`}>
              {connection}
            </span>
          </div>
          <div className="telemetry">
            <span className="k">识别引擎</span>
            <span className={`v ${engine === "ready" ? "glow" : ""}`}>
              {engine}
            </span>
          </div>
          <div className="telemetry">
            <span className="k">后端</span>
            <span className="v">{backend}</span>
          </div>
          <div className="telemetry">
            <span className="k">运行时长</span>
            <span className="v">{uptime}</span>
          </div>
          <div className="telemetry">
            <span className="k">输入法前端</span>
            <span className="v">{status?.frontend?.state ?? "—"}</span>
          </div>
          <div className="telemetry">
            <span className="k">Zipformer 当前模型</span>
            <span className="v">{status?.models?.active_asr ?? "—"}</span>
          </div>
        </div>
      </section>

      <section className="panel">
        <div className="panel-label">快速上手</div>
        <p className="muted" style={{ margin: 0 }}>
          ① 引擎状态 ready 后,在任意输入框按 <b className="mono">Alt+S</b>{" "}
          开始听写;再按一次结束。 ② 输入源需切换到「VoxFlow / 声流输入法」
          (Super+Space)。 ③ 后端与模型可在托盘图标或「输入」「模型」页切换。
        </p>
      </section>
    </div>
  );
}
