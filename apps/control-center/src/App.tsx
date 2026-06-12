import { useEffect, useState } from "react";
import { onConnectionChanged, onCoreEvent, onSnapshot } from "./bridge";
import Models from "./pages/Models";
import Overview from "./pages/Overview";
import { useControlStore, type ConnectionState } from "./store";

// control-center-spec.md 信息架构;未实现页为阶段 4 后续批次。
const PAGES = [
  { key: "overview", label: "总览", ready: true },
  { key: "input", label: "输入", ready: false },
  { key: "models", label: "模型", ready: true },
  { key: "correction", label: "语义修正", ready: false },
  { key: "audio", label: "音频", ready: false },
  { key: "appearance", label: "外观", ready: false },
  { key: "diagnostics", label: "诊断", ready: false },
  { key: "about", label: "关于", ready: false },
] as const;

type PageKey = (typeof PAGES)[number]["key"];

function Placeholder({ label }: { label: string }) {
  return (
    <div>
      <h2>{label}</h2>
      <div className="card muted">
        本页在阶段 4 后续批次实现(规格见
        docs/redesign/design/control-center-spec.md)。
      </div>
    </div>
  );
}

export default function App() {
  const [page, setPage] = useState<PageKey>("overview");
  const setConnection = useControlStore((state) => state.setConnection);
  const setSnapshot = useControlStore((state) => state.setSnapshot);
  const applyCoreEvent = useControlStore((state) => state.applyCoreEvent);
  const refreshModels = useControlStore((state) => state.refreshModels);
  const connection = useControlStore((state) => state.connection);

  useEffect(() => {
    const subscriptions = [
      onConnectionChanged((payload) => {
        const state = (payload.state as ConnectionState) ?? "disconnected";
        setConnection(state, (payload.error as string) ?? null);
        if (state === "connected") {
          void refreshModels();
        }
      }),
      onSnapshot((payload) => setSnapshot(payload)),
      onCoreEvent((name, payload) => applyCoreEvent(name, payload)),
    ];
    return () => {
      for (const subscription of subscriptions) {
        void subscription.then((unlisten) => unlisten());
      }
    };
  }, [setConnection, setSnapshot, applyCoreEvent, refreshModels]);

  return (
    <div className="layout">
      <nav className="sidebar">
        <h1>VoxFlow</h1>
        {PAGES.map(({ key, label }) => (
          <button
            key={key}
            className={`nav-item ${page === key ? "active" : ""}`}
            onClick={() => setPage(key)}
          >
            {label}
          </button>
        ))}
        <div style={{ marginTop: "auto", padding: "8px 12px" }}>
          <span className={`dot ${connection}`} />{" "}
          <span className="muted">{connection}</span>
        </div>
      </nav>
      <main className="content">
        {page === "overview" ? (
          <Overview />
        ) : page === "models" ? (
          <Models />
        ) : (
          <Placeholder label={PAGES.find((item) => item.key === page)!.label} />
        )}
      </main>
    </div>
  );
}
