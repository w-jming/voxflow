import { useEffect, useState } from "react";
import { PageHead } from "../App";
import { coreCommand } from "../bridge";

interface CorrectionRecord {
  operation_id?: string;
  intent?: string;
  target?: string;
  replacement?: string;
  confidence?: number;
  reason_code?: string;
}

export default function Correction() {
  const [enabled, setEnabled] = useState(true);
  const [threshold, setThreshold] = useState("standard");
  const [records, setRecords] = useState<CorrectionRecord[]>([]);

  useEffect(() => {
    void (async () => {
      const config = await coreCommand("config.get");
      if (config.kind === "response" && config.payload) {
        const correction = (config.payload.config as Record<string, unknown>)
          ?.correction as Record<string, unknown> | undefined;
        setEnabled((correction?.enabled as boolean) ?? true);
        setThreshold((correction?.threshold_mode as string) ?? "standard");
      }
      const recent = await coreCommand("correction.list_recent");
      if (recent.kind === "response" && recent.payload) {
        setRecords((recent.payload.records as CorrectionRecord[]) ?? []);
      }
    })();
  }, []);

  const save = (patch: Record<string, unknown>) =>
    void coreCommand("config.update", { patch: { correction: patch } });

  return (
    <div>
      <PageHead
        index="04"
        title="语义修正"
        desc="“不对,改成…”类口语指令的识别与安全门;所有删除必须过账本安全门。"
      />

      <section className="panel">
        <div className="panel-label">开关</div>
        <label className={`choice ${enabled ? "selected" : ""}`}>
          <input
            type="checkbox"
            checked={enabled}
            onChange={(event) => {
              setEnabled(event.target.checked);
              save({ enabled: event.target.checked });
            }}
          />
          <div className="grow">
            <div>启用智能撤销 / 语义修正</div>
            <div className="muted">
              关闭后,“不对”“删掉”等一律按普通文本输入。
            </div>
          </div>
        </label>
        <div className="row" style={{ marginTop: 10 }}>
          <span className="muted">置信度档位</span>
          <select
            className="vf"
            value={threshold}
            onChange={(event) => {
              setThreshold(event.target.value);
              save({ threshold_mode: event.target.value });
            }}
          >
            <option value="conservative">保守</option>
            <option value="standard">标准</option>
            <option value="aggressive">积极</option>
          </select>
          <span className="muted">任何档位都不能绕过安全门。</span>
        </div>
      </section>

      <section className="panel">
        <div className="panel-label">最近修正</div>
        {records.length === 0 ? (
          <p className="muted" style={{ margin: 0 }}>
            暂无修正记录;真实听写产生的修正会在此留痕(可恢复)。
          </p>
        ) : (
          <table className="vf">
            <thead>
              <tr>
                <th>意图</th>
                <th>目标</th>
                <th>替换为</th>
                <th>置信度</th>
                <th>依据</th>
              </tr>
            </thead>
            <tbody>
              {records.map((record, index) => (
                <tr key={record.operation_id ?? index}>
                  <td className="mono">{record.intent ?? "—"}</td>
                  <td>{record.target ?? "—"}</td>
                  <td>{record.replacement ?? "—"}</td>
                  <td className="mono">
                    {record.confidence?.toFixed(2) ?? "—"}
                  </td>
                  <td className="mono">{record.reason_code ?? "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

      <p className="muted">
        分类器(轻量意图模型)训练中;当前由规则状态机驱动。修正接入真实听写流水线为下一批次。
      </p>
    </div>
  );
}
