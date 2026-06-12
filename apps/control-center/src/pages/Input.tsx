import { useEffect, useState } from "react";
import { coreCommand } from "../bridge";
import { useControlStore } from "../store";

type Backend = "qwen3_vllm" | "volcano_api" | "zipformer_local" | "mock";

interface AsrConfigShape {
  backend: Backend;
  qwen3: { model: string; python: string; sidecar_script: string };
  volcano: {
    app_key: string;
    access_key: string;
    resource_id: string;
    model_name: string;
    endpoint: string;
  };
}

const BACKENDS: { key: Backend; label: string; description: string }[] = [
  {
    key: "qwen3_vllm",
    label: "Qwen3-ASR-1.7B + vLLM(默认,本地 GPU)",
    description: "开源 SOTA 准确率;本地推理,无数据外发;需要已部署的 vLLM sidecar。",
  },
  {
    key: "volcano_api",
    label: "火山引擎大模型语音识别(云端 API)",
    description: "需在下方配置 APP ID 与 Access Token;音频将发送至火山引擎服务。",
  },
  {
    key: "zipformer_local",
    label: "Zipformer 流式(本地 CPU 兜底)",
    description: "轻量本地模型(模型页下载安装);延迟最低,准确率次之。",
  },
];

export default function Input() {
  const lastError = useControlStore((state) => state.lastError);
  const [asr, setAsr] = useState<AsrConfigShape | null>(null);
  const [hotkey, setHotkey] = useState("Alt+S");
  const [saving, setSaving] = useState(false);
  const [savedTick, setSavedTick] = useState(0);

  useEffect(() => {
    void (async () => {
      const reply = await coreCommand("config.get");
      if (reply.kind === "response" && reply.payload) {
        const config = reply.payload.config as Record<string, unknown>;
        setAsr(config.asr as unknown as AsrConfigShape);
        setHotkey(
          ((config.input as Record<string, unknown>)?.hotkey as string) ??
            "Alt+S",
        );
      }
    })();
  }, []);

  const save = async (patch: Record<string, unknown>) => {
    setSaving(true);
    const reply = await coreCommand("config.update", { patch });
    setSaving(false);
    if (reply.kind === "response") {
      setSavedTick((tick) => tick + 1);
    }
  };

  if (!asr) {
    return (
      <div>
        <h2>输入</h2>
        <div className="card muted">等待 Core 连接后读取配置。</div>
      </div>
    );
  }

  return (
    <div>
      <h2>输入</h2>
      {lastError ? <div className="error-banner">{lastError}</div> : null}

      <div className="card">
        <strong>听写快捷键</strong>
        <div className="muted">当前:{hotkey}(toggle/hold 共用,D-14)</div>
      </div>

      <div className="card">
        <strong>语音识别后端</strong>
        <div className="muted" style={{ marginBottom: 10 }}>
          可自由切换;下次开始听写时生效。
        </div>
        {BACKENDS.map(({ key, label, description }) => (
          <label
            key={key}
            className="row"
            style={{ padding: "8px 0", cursor: "pointer", alignItems: "flex-start" }}
          >
            <input
              type="radio"
              name="asr-backend"
              checked={asr.backend === key}
              onChange={() => {
                setAsr({ ...asr, backend: key });
                void save({ asr: { backend: key } });
              }}
            />
            <div className="grow">
              <div>{label}</div>
              <div className="muted">{description}</div>
            </div>
          </label>
        ))}
      </div>

      {asr.backend === "volcano_api" ? (
        <div className="card">
          <strong>火山引擎密钥</strong>
          <div className="muted" style={{ margin: "4px 0 10px" }}>
            在火山引擎控制台(语音技术 → 大模型语音识别)获取;仅保存在本机
            ~/.voxflow/config.toml,不会上传。
          </div>
          <div className="row" style={{ marginBottom: 8 }}>
            <input
              className="vf grow"
              placeholder="APP ID(X-Api-App-Key)"
              value={asr.volcano.app_key}
              onChange={(event) =>
                setAsr({
                  ...asr,
                  volcano: { ...asr.volcano, app_key: event.target.value },
                })
              }
            />
            <input
              className="vf grow"
              type="password"
              placeholder="Access Token(X-Api-Access-Key)"
              value={asr.volcano.access_key}
              onChange={(event) =>
                setAsr({
                  ...asr,
                  volcano: { ...asr.volcano, access_key: event.target.value },
                })
              }
            />
          </div>
          <div className="row" style={{ marginBottom: 8 }}>
            <input
              className="vf grow"
              placeholder="Resource ID"
              value={asr.volcano.resource_id}
              onChange={(event) =>
                setAsr({
                  ...asr,
                  volcano: { ...asr.volcano, resource_id: event.target.value },
                })
              }
            />
            <input
              className="vf grow"
              placeholder="模型名(bigmodel)"
              value={asr.volcano.model_name}
              onChange={(event) =>
                setAsr({
                  ...asr,
                  volcano: { ...asr.volcano, model_name: event.target.value },
                })
              }
            />
          </div>
          <button
            className="vf"
            disabled={saving}
            onClick={() => void save({ asr: { volcano: asr.volcano } })}
          >
            保存密钥
          </button>
        </div>
      ) : null}

      {asr.backend === "qwen3_vllm" ? (
        <div className="card">
          <strong>Qwen3-ASR sidecar</strong>
          <div className="muted">
            模型:{asr.qwen3.model} · Python:{asr.qwen3.python}
            <br />
            首次开始听写时加载模型(约 1 分钟);权重已由部署脚本预下载,无需联网。
          </div>
        </div>
      ) : null}

      {savedTick > 0 ? (
        <p className="muted">✓ 已保存(第 {savedTick} 次)</p>
      ) : null}
    </div>
  );
}
