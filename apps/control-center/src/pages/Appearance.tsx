import { useEffect, useState } from "react";
import { PageHead } from "../App";
import { coreCommand } from "../bridge";

type Theme = "system" | "light" | "dark";

export function applyTheme(theme: Theme) {
  const dark =
    theme === "dark" ||
    (theme === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.setAttribute(
    "data-theme",
    dark ? "dark" : "light",
  );
  for (const img of document.querySelectorAll<HTMLElement>("[data-logo-dark]")) {
    img.style.display = dark ? "" : "none";
  }
  for (const img of document.querySelectorAll<HTMLElement>("[data-logo-light]")) {
    img.style.display = dark ? "none" : "";
  }
}

const THEMES: { key: Theme; label: string; description: string }[] = [
  { key: "system", label: "跟随系统", description: "随桌面环境自动切换明暗。" },
  { key: "dark", label: "仪表盘(暗)", description: "默认身份;深墨蓝面板 + 天蓝辉光。" },
  { key: "light", label: "工作台(亮)", description: "高环境光场景;同一色阶的亮面映射。" },
];

export default function Appearance() {
  const [theme, setTheme] = useState<Theme>("system");

  useEffect(() => {
    void (async () => {
      const reply = await coreCommand("config.get");
      if (reply.kind === "response" && reply.payload) {
        const ui = (reply.payload.config as Record<string, unknown>)?.ui as
          | Record<string, unknown>
          | undefined;
        const saved = (ui?.theme as Theme) ?? "system";
        setTheme(saved);
        applyTheme(saved);
      }
    })();
  }, []);

  const choose = (next: Theme) => {
    setTheme(next);
    applyTheme(next);
    void coreCommand("config.update", { patch: { ui: { theme: next } } });
  };

  return (
    <div>
      <PageHead
        index="06"
        title="外观"
        desc="主题切换;品牌色阶(#0EA5E9)在两套主题下保持一致。"
      />
      <section className="panel">
        <div className="panel-label">Theme</div>
        {THEMES.map(({ key, label, description }) => (
          <label
            key={key}
            className={`choice ${theme === key ? "selected" : ""}`}
          >
            <input
              type="radio"
              name="theme"
              checked={theme === key}
              onChange={() => choose(key)}
            />
            <div className="grow">
              <div>{label}</div>
              <div className="muted">{description}</div>
            </div>
          </label>
        ))}
      </section>
      <p className="muted">
        全局状态指示器(HUD)样式将在指示器窗口落地后并入本页设置。
      </p>
    </div>
  );
}
