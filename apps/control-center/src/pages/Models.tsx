import { useEffect, useState } from "react";
import { PageHead } from "../App";
import {
  useControlStore,
  type DownloadProgress,
  type ModelItem,
} from "../store";

function formatBytes(bytes?: number): string {
  if (!bytes) return "—";
  if (bytes >= 1024 * 1024 * 1024)
    return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  return `${Math.round(bytes / 1024 / 1024)} MB`;
}

function phaseLabel(progress: DownloadProgress): string {
  switch (progress.phase) {
    case "downloading": {
      const pct =
        progress.total && progress.downloaded
          ? Math.floor((progress.downloaded / progress.total) * 100)
          : 0;
      const speed = progress.speed_bps
        ? `${(progress.speed_bps / 1024 / 1024).toFixed(1)} MB/s`
        : "";
      return `下载中 ${pct}% ${speed} ${progress.eta_s ? `剩余 ${progress.eta_s}s` : ""}`;
    }
    case "verifying":
      return "校验中…";
    case "installing":
      return "安装中…";
    case "done":
      return "完成";
    case "failed":
      return `失败:${progress.code ?? ""}`;
  }
}

function ZipformerCard({ model }: { model: ModelItem }) {
  const progress = useControlStore((state) => state.progress[model.profile.id]);
  const download = useControlStore((state) => state.download);
  const pause = useControlStore((state) => state.pause);
  const cancel = useControlStore((state) => state.cancel);
  const activate = useControlStore((state) => state.activate);
  const remove = useControlStore((state) => state.remove);

  const id = model.profile.id;
  const state = model.local.state;
  const busy =
    progress &&
    ["downloading", "verifying", "installing"].includes(progress.phase);
  const pct =
    progress?.total && progress.downloaded
      ? Math.min(100, (progress.downloaded / progress.total) * 100)
      : 0;

  return (
    <section className="panel">
      <div className="row">
        <div className="grow">
          <strong>{model.profile.label}</strong>{" "}
          {model.profile.recommended ? (
            <span className="badge accent">推荐</span>
          ) : null}
          <div className="muted mono" style={{ marginTop: 2 }}>
            {id} · {model.profile.languages.join("/")} ·{" "}
            {formatBytes(model.source.size_bytes)} · {model.profile.license}
          </div>
        </div>
        <span className={`badge ${busy ? progress.phase : state}`}>
          {busy ? progress.phase : state}
        </span>
      </div>

      {busy ? (
        <div style={{ marginTop: 10 }}>
          <div className="progress-track">
            <div className="progress-fill" style={{ width: `${pct}%` }} />
          </div>
          <div className="muted mono" style={{ marginTop: 4 }}>
            {phaseLabel(progress)}
          </div>
        </div>
      ) : null}
      {progress?.phase === "failed" ? (
        <div className="muted" style={{ color: "var(--vf-danger)" }}>
          {phaseLabel(progress)} {progress.message ?? ""}
        </div>
      ) : null}
      {model.local.issues.length > 0 ? (
        <div className="muted" style={{ color: "var(--vf-danger)" }}>
          {model.local.issues.join("; ")}
        </div>
      ) : null}

      <div className="row" style={{ marginTop: 12 }}>
        {state === "not_installed" && !busy ? (
          <button className="vf" onClick={() => void download(id)}>
            {progress?.phase === "failed" ? "重试下载" : "一键下载"}
          </button>
        ) : null}
        {busy && progress.phase === "downloading" ? (
          <>
            <button className="vf ghost" onClick={() => void pause(id)}>
              暂停
            </button>
            <button className="vf danger" onClick={() => void cancel(id)}>
              取消
            </button>
          </>
        ) : null}
        {state === "ready" ? (
          <button className="vf" onClick={() => void activate(id)}>
            设为 Zipformer 当前模型
          </button>
        ) : null}
        {state === "ready" || state === "broken" ? (
          <button className="vf danger" onClick={() => void remove(id)}>
            删除
          </button>
        ) : null}
      </div>
    </section>
  );
}

function LocalImport() {
  const models = useControlStore((state) => state.models);
  const importLocal = useControlStore((state) => state.importLocal);
  const candidates = models.filter(
    (model) => model.local.state === "not_installed",
  );
  const [modelId, setModelId] = useState("");
  const [path, setPath] = useState("");
  const [mode, setMode] = useState<"copy" | "symlink">("copy");

  return (
    <section className="panel">
      <div className="panel-label">导入本地模型</div>
      <p className="muted" style={{ marginTop: 0 }}>
        按 profile 的 sha256 清单逐文件校验,不通过即拒绝导入。
      </p>
      <div className="row">
        <select
          className="vf"
          value={modelId}
          onChange={(event) => setModelId(event.target.value)}
        >
          <option value="">选择 profile…</option>
          {candidates.map((model) => (
            <option key={model.profile.id} value={model.profile.id}>
              {model.profile.label}
            </option>
          ))}
        </select>
        <input
          className="vf grow"
          placeholder="本地模型目录绝对路径"
          value={path}
          onChange={(event) => setPath(event.target.value)}
        />
        <select
          className="vf"
          value={mode}
          onChange={(event) =>
            setMode(event.target.value as "copy" | "symlink")
          }
        >
          <option value="copy">复制</option>
          <option value="symlink">软链接</option>
        </select>
        <button
          className="vf"
          disabled={!modelId || !path}
          onClick={() => void importLocal(modelId, path, mode)}
        >
          校验并导入
        </button>
      </div>
    </section>
  );
}

type Tab = "streaming" | "cloud" | "refine";

export default function Models() {
  const models = useControlStore((state) => state.models);
  const lastError = useControlStore((state) => state.lastError);
  const status = useControlStore((state) => state.status);
  const refreshModels = useControlStore((state) => state.refreshModels);
  const refreshStatus = useControlStore((state) => state.refreshStatus);
  const [tab, setTab] = useState<Tab>("streaming");

  useEffect(() => {
    void refreshModels();
    void refreshStatus();
  }, [refreshModels, refreshStatus]);

  const backend = status?.models?.asr_backend ?? "";
  const engine = status?.models?.engine_state ?? "—";

  return (
    <div>
      <PageHead
        index="03"
        title="模型"
        desc="按用途分组管理;「当前使用」由输入页/托盘选择的后端决定。"
      />
      {lastError ? <div className="error-banner">{lastError}</div> : null}

      <div className="tabs">
        <button
          className={`tab ${tab === "streaming" ? "active" : ""}`}
          onClick={() => setTab("streaming")}
        >
          实时流式
        </button>
        <button
          className={`tab ${tab === "cloud" ? "active" : ""}`}
          onClick={() => setTab("cloud")}
        >
          云端 API
        </button>
        <button
          className={`tab ${tab === "refine" ? "active" : ""}`}
          onClick={() => setTab("refine")}
        >
          精修(规划)
        </button>
      </div>

      {tab === "streaming" ? (
        <>
          <section className="panel">
            <div className="row">
              <div className="grow">
                <strong>Qwen3-ASR-1.7B</strong>{" "}
                <span className="badge accent">默认后端</span>{" "}
                {backend === "qwen3_vllm" ? (
                  <span className="badge ok">当前使用</span>
                ) : null}
                <div className="muted mono" style={{ marginTop: 2 }}>
                  vLLM · GPU 常驻 · zh/en + 30 语言 · Apache-2.0 · ~3.4 GB
                </div>
                <div className="muted" style={{ marginTop: 4 }}>
                  权重由部署脚本经 Hugging Face 缓存管理(纳入统一下载链路在
                  todo);驻留状态:
                  <span className={`badge ${engine}`}>{engine}</span>
                </div>
              </div>
            </div>
          </section>
          {models.map((model) => (
            <ZipformerCard key={model.profile.id} model={model} />
          ))}
          <LocalImport />
        </>
      ) : null}

      {tab === "cloud" ? (
        <section className="panel">
          <div className="row">
            <div className="grow">
              <strong>火山引擎 大模型流式语音识别</strong>{" "}
              {backend === "volcano_api" ? (
                <span className="badge ok">当前使用</span>
              ) : null}
              <div className="muted" style={{ marginTop: 4 }}>
                无本地权重;在「输入」页配置 APP ID / Access Token 后切换。
                <br />
                <span style={{ color: "var(--vf-warn)" }}>
                  ⚠ 该路径未经真实服务验证(暂无测试密钥)。
                </span>
              </div>
            </div>
          </div>
        </section>
      ) : null}

      {tab === "refine" ? (
        <section className="panel">
          <div className="row">
            <div className="grow">
              <strong>Qwen3-ASR-0.6B(final 精修层)</strong>{" "}
              <span className="badge">规划中</span>
              <div className="muted" style={{ marginTop: 4 }}>
                停顿后异步精修已上屏文本,经账本安全门替换(D-9/D-21,
                sherpa-onnx 离线包 2026-03-25);下一批次接入。
              </div>
            </div>
          </div>
        </section>
      ) : null}
    </div>
  );
}
