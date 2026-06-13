import { useEffect, useState } from "react";
import { PageHead } from "../App";
import { coreCommand } from "../bridge";

interface AudioDevice {
  id: string;
  label: string;
  backend: string;
  is_default: boolean;
  bluetooth_profile?: string | null;
}

interface Inventory {
  devices: AudioDevice[];
  warnings: string[];
  probe: Record<string, unknown>;
}

export default function Audio() {
  const [inventory, setInventory] = useState<Inventory | null>(null);

  const refresh = async () => {
    const reply = await coreCommand("audio.list_devices");
    if (reply.kind === "response" && reply.payload) {
      setInventory(reply.payload as unknown as Inventory);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  return (
    <div>
      <PageHead
        index="05"
        title="音频"
        desc="输入设备与 PipeWire 链路;采集固定 16 kHz 单声道,20 ms 帧。"
      />

      <section className="panel">
        <div className="row" style={{ marginBottom: 10 }}>
          <div className="panel-label" style={{ margin: 0 }}>
            输入设备
          </div>
          <span className="grow" />
          <button className="vf ghost" onClick={() => void refresh()}>
            刷新
          </button>
        </div>
        {!inventory ? (
          <p className="muted">读取中…</p>
        ) : inventory.devices.length === 0 ? (
          <p className="muted">未发现输入设备。</p>
        ) : (
          <table className="vf">
            <thead>
              <tr>
                <th>设备</th>
                <th>ID</th>
                <th>蓝牙 Profile</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {inventory.devices.map((device) => (
                <tr key={device.id}>
                  <td>{device.label}</td>
                  <td className="mono">{device.id}</td>
                  <td className="mono">{device.bluetooth_profile ?? "—"}</td>
                  <td>
                    {device.is_default ? (
                      <span className="badge ok">默认</span>
                    ) : null}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {inventory?.warnings.length ? (
          <p className="muted" style={{ color: "var(--vf-warn)" }}>
            {inventory.warnings.join("; ")}
          </p>
        ) : null}
      </section>

      <section className="panel">
        <div className="panel-label">提示</div>
        <p className="muted" style={{ margin: 0 }}>
          蓝牙耳机请确认处于 headset(HFP)模式——A2DP 仅输出,无法采集。
          采集走 PipeWire native,默认跟随系统默认输入源。
        </p>
      </section>
    </div>
  );
}
