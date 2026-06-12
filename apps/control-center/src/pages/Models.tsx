import { useState } from "react";
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
      const eta = progress.eta_s ? `剩余 ${progress.eta_s}s` : "";
      return `下载中 ${pct}% ${speed} ${eta}`;
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

function ModelCard({ model }: { model: ModelItem }) {
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
  const badge = busy ? progress.phase : state;
  const pct =
    progress?.total && progress.downloaded
      ? Math.min(100, (progress.downloaded / progress.total) * 100)
      : 0;

  return (
    <div className="card">
      <div className="row">
        <div className="grow">
          <strong>{model.profile.label}</strong>{" "}
          {model.profile.recommended ? (
            <span className="badge active">推荐</span>
          ) : null}
          <div className="muted">
            {id} · {model.profile.languages.join("/")} ·{" "}
            {formatBytes(model.source.size_bytes)} · {model.profile.license} ·
            最低内存 {model.profile.min_ram_mb} MB
          </div>
        </div>
        <span className={`badge ${badge}`}>{badge}</span>
      </div>

      {busy ? (
        <div style={{ marginTop: 10 }}>
          <div className="progress-track">
            <div className="progress-fill" style={{ width: `${pct}%` }} />
          </div>
          <div className="muted" style={{ marginTop: 4 }}>
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
            <button className="vf secondary" onClick={() => void pause(id)}>
              暂停
            </button>
            <button className="vf danger" onClick={() => void cancel(id)}>
              取消
            </button>
          </>
        ) : null}
        {state === "ready" ? (
          <button className="vf" onClick={() => void activate(id)}>
            设为当前模型
          </button>
        ) : null}
        {state === "ready" || state === "broken" ? (
          <button className="vf danger" onClick={() => void remove(id)}>
            删除
          </button>
        ) : null}
      </div>
    </div>
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
    <div className="card">
      <strong>导入本地模型</strong>
      <div className="muted" style={{ margin: "4px 0 10px" }}>
        选择目标 profile 与本地模型目录;导入时按 profile 的 sha256
        清单逐文件校验,校验不通过会拒绝导入。
      </div>
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
          placeholder="本地模型目录绝对路径,如 /home/you/models/zipformer"
          value={path}
          onChange={(event) => setPath(event.target.value)}
        />
        <select
          className="vf"
          value={mode}
          onChange={(event) => setMode(event.target.value as "copy" | "symlink")}
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
    </div>
  );
}

export default function Models() {
  const models = useControlStore((state) => state.models);
  const lastError = useControlStore((state) => state.lastError);

  return (
    <div>
      <h2>模型管理</h2>
      <p className="muted">
        模型仅安装到用户数据目录(VOXFLOW_HOME,默认 ~/.voxflow/models),
        不写入系统目录;下载支持断点续传与 sha256 校验。
      </p>
      {lastError ? <div className="error-banner">{lastError}</div> : null}
      {models.map((model) => (
        <ModelCard key={model.profile.id} model={model} />
      ))}
      {models.length === 0 ? (
        <div className="card muted">
          未读取到模型列表(Core 未连接或 profile 目录为空)。
        </div>
      ) : null}
      <LocalImport />
    </div>
  );
}
