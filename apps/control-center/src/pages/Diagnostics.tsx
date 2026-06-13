import { useState } from "react";
import { PageHead } from "../App";
import { coreCommand } from "../bridge";

interface Check {
  id?: string;
  name?: string;
  status?: string;
  detail?: string;
  message?: string;
}

export default function Diagnostics() {
  const [checks, setChecks] = useState<Check[] | null>(null);
  const [running, setRunning] = useState(false);

  const run = async () => {
    setRunning(true);
    const reply = await coreCommand("diagnostics.run");
    setRunning(false);
    if (reply.kind === "response" && reply.payload) {
      setChecks((reply.payload.checks as Check[]) ?? []);
    }
  };

  return (
    <div>
      <PageHead
        index="07"
        title="诊断"
        desc="一键体检:运行环境、音频链路、模型与输入法前端。"
      />
      <section className="panel">
        <div className="row">
          <button className="vf" disabled={running} onClick={() => void run()}>
            {running ? "检查中…" : "运行诊断"}
          </button>
          <span className="muted">
            等价于命令行 <span className="mono">voxflow-core doctor</span>
          </span>
        </div>
      </section>
      {checks ? (
        <section className="panel">
          <div className="panel-label">检查结果</div>
          <table className="vf">
            <thead>
              <tr>
                <th>检查项</th>
                <th>状态</th>
                <th>详情</th>
              </tr>
            </thead>
            <tbody>
              {checks.map((check, index) => (
                <tr key={check.id ?? index}>
                  <td>{check.name ?? check.id ?? `check-${index}`}</td>
                  <td>
                    <span
                      className={`badge ${
                        (check.status ?? "").includes("ok") ? "ok" : "warn"
                      }`}
                    >
                      {check.status ?? "—"}
                    </span>
                  </td>
                  <td className="muted">
                    {check.detail ?? check.message ?? ""}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      ) : null}
    </div>
  );
}
