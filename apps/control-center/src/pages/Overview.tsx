import { useControlStore } from "../store";

interface StatusCard {
  id: string;
  title: string;
  tone: string;
  badge: string;
  description: string;
}

const TONE_CLASS: Record<string, string> = {
  ready: "ready",
  warning: "downloading",
  error: "broken",
  degraded: "downloading",
  loading: "not_installed",
  empty: "not_installed",
};

export default function Overview() {
  const connection = useControlStore((state) => state.connection);
  const connectionError = useControlStore((state) => state.connectionError);
  const snapshot = useControlStore((state) => state.snapshot);

  const cards = (snapshot?.overview_cards as StatusCard[] | undefined) ?? [];
  const currentModel = (snapshot?.current_model as string) ?? "—";
  const appVersion = (snapshot?.app_version as string) ?? "—";

  return (
    <div>
      <h2>总览</h2>
      <div className="card">
        <div className="row">
          <span className={`dot ${connection}`} />
          <strong>Core 连接:{connection}</strong>
          <span className="muted">
            Core 版本 {appVersion} · 当前模型 {currentModel}
          </span>
          {connectionError ? (
            <span className="muted">{connectionError}</span>
          ) : null}
        </div>
      </div>
      {cards.map((card) => (
        <div className="card" key={card.id}>
          <div className="row">
            <div className="grow">
              <strong>{card.title}</strong>
              <div className="muted">{card.description}</div>
            </div>
            <span className={`badge ${TONE_CLASS[card.tone] ?? "not_installed"}`}>
              {card.badge}
            </span>
          </div>
        </div>
      ))}
      {cards.length === 0 ? (
        <div className="card muted">等待 Core 连接后显示状态卡片。</div>
      ) : null}
    </div>
  );
}
