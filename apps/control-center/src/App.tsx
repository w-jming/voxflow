import { useEffect, useState } from "react";
import { onConnectionChanged, onCoreEvent, onSnapshot, resync } from "./bridge";
import About from "./pages/About";
import Appearance from "./pages/Appearance";
import Audio from "./pages/Audio";
import Correction from "./pages/Correction";
import Diagnostics from "./pages/Diagnostics";
import Input from "./pages/Input";
import Models from "./pages/Models";
import Overview from "./pages/Overview";
import { useControlStore, type ConnectionState } from "./store";
import { applyTheme } from "./pages/Appearance";
import { coreCommand } from "./bridge";
import logoLight from "./assets/voxflow-symbol.svg";
import logoDark from "./assets/voxflow-symbol-dark.svg";

const PAGES = [
  { key: "overview", label: "总览", node: <Overview /> },
  { key: "input", label: "输入", node: <Input /> },
  { key: "models", label: "模型", node: <Models /> },
  { key: "correction", label: "语义修正", node: <Correction /> },
  { key: "audio", label: "音频", node: <Audio /> },
  { key: "appearance", label: "外观", node: <Appearance /> },
  { key: "diagnostics", label: "诊断", node: <Diagnostics /> },
  { key: "about", label: "关于", node: <About /> },
] as const;

type PageKey = (typeof PAGES)[number]["key"];

export function WaveRule() {
  return (
    <svg className="wave-rule" viewBox="0 0 600 14" preserveAspectRatio="none">
      <path
        d="M0 7 H180 L188 2 L196 12 L204 4 L212 10 L220 7 H600"
        fill="none"
        stroke="currentColor"
        strokeWidth="1"
      />
    </svg>
  );
}

export function PageHead({
  index,
  title,
  desc,
}: {
  index: string;
  title: string;
  desc: string;
}) {
  return (
    <header className="page-head">
      <h2 className="page-title">
        <span className="index">{index}</span>
        {title}
      </h2>
      <p className="page-desc">{desc}</p>
      <WaveRule />
    </header>
  );
}

export default function App() {
  const [page, setPage] = useState<PageKey>("overview");
  const setConnection = useControlStore((state) => state.setConnection);
  const setSnapshot = useControlStore((state) => state.setSnapshot);
  const applyCoreEvent = useControlStore((state) => state.applyCoreEvent);
  const refreshModels = useControlStore((state) => state.refreshModels);
  const refreshStatus = useControlStore((state) => state.refreshStatus);
  const connection = useControlStore((state) => state.connection);

  useEffect(() => {
    const subscriptions = [
      onConnectionChanged((payload) => {
        const state = (payload.state as ConnectionState) ?? "disconnected";
        setConnection(state, (payload.error as string) ?? null);
        if (state === "connected") {
          void refreshModels();
          void refreshStatus();
        }
      }),
      onSnapshot((payload) => setSnapshot(payload)),
      onCoreEvent((name, payload) => applyCoreEvent(name, payload)),
    ];
    void Promise.all(subscriptions).then(() => resync());
    // 启动即应用主题(默认跟随系统;config.ui.theme 覆盖)
    applyTheme("system");
    void coreCommand("config.get").then((reply) => {
      if (reply.kind === "response" && reply.payload) {
        const ui = (reply.payload.config as Record<string, unknown>)?.ui as
          | Record<string, unknown>
          | undefined;
        applyTheme(((ui?.theme as string) ?? "system") as Parameters<typeof applyTheme>[0]);
      }
    });
    return () => {
      for (const subscription of subscriptions) {
        void subscription.then((unlisten) => unlisten());
      }
    };
  }, [setConnection, setSnapshot, applyCoreEvent, refreshModels, refreshStatus]);

  return (
    <div className="layout">
      <nav className="rail">
        <div className="brand">
          <img className="brand-logo" src={logoDark} alt="" data-logo-dark />
          <img
            className="brand-logo"
            src={logoLight}
            alt=""
            data-logo-light
            style={{ display: "none" }}
          />
          <span className="brand-name">VoxFlow</span>
          <span className="brand-sub">声流输入法</span>
        </div>
        {PAGES.map(({ key, label }, index) => (
          <button
            key={key}
            className={`nav-item ${page === key ? "active" : ""}`}
            onClick={() => setPage(key)}
          >
            <span className="nav-index">
              {String(index + 1).padStart(2, "0")}
            </span>
            {label}
          </button>
        ))}
        <div className="rail-foot">
          <span className={`led ${connection}`} />
          {connection}
        </div>
      </nav>
      <main className="content">
        <div className="page-enter" key={page}>
          {PAGES.find((item) => item.key === page)!.node}
        </div>
      </main>
    </div>
  );
}
