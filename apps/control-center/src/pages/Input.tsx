import { useEffect, useState } from "react";
import { PageHead } from "../App";
import { coreCommand } from "../bridge";
import { useControlStore } from "../store";

type Backend = "qwen3_vllm" | "volcano_api" | "zipformer_local" | "mock";

interface AsrConfigShape {
  backend: Backend;
  qwen3: {
    model: string;
    python: string;
    sidecar_script: string;
    language: string;
  };
  volcano: {
    app_key: string;
    access_key: string;
    resource_id: string;
    model_name: string;
    endpoint: string;
  };
}

type OutputScript = "simplified" | "traditional" | "original";

const SCRIPTS: { key: OutputScript; label: string; description: string }[] = [
  { key: "simplified", label: "简体中文", description: "默认;繁体输出经 OpenCC 词组级转换为简体。" },
  { key: "traditional", label: "繁體中文", description: "简体输出转换为繁体(OpenCC)。" },
  { key: "original", label: "原样输出", description: "不转换,保留模型原始简/繁结果。" },
];

// Qwen3-ASR 支持的全部识别语言(英文名为引擎参数值)。
const QWEN_LANGS: { key: string; label: string }[] = [
  { key: "Chinese", label: "中文 Chinese(默认)" },
  { key: "English", label: "英文 English" },
  { key: "Cantonese", label: "粤语 Cantonese" },
  { key: "Japanese", label: "日语 Japanese" },
  { key: "Korean", label: "韩语 Korean" },
  { key: "Arabic", label: "阿拉伯语 Arabic" },
  { key: "German", label: "德语 German" },
  { key: "French", label: "法语 French" },
  { key: "Spanish", label: "西班牙语 Spanish" },
  { key: "Portuguese", label: "葡萄牙语 Portuguese" },
  { key: "Indonesian", label: "印尼语 Indonesian" },
  { key: "Italian", label: "意大利语 Italian" },
  { key: "Russian", label: "俄语 Russian" },
  { key: "Thai", label: "泰语 Thai" },
  { key: "Vietnamese", label: "越南语 Vietnamese" },
  { key: "Turkish", label: "土耳其语 Turkish" },
  { key: "Hindi", label: "印地语 Hindi" },
  { key: "Malay", label: "马来语 Malay" },
  { key: "Dutch", label: "荷兰语 Dutch" },
  { key: "Swedish", label: "瑞典语 Swedish" },
  { key: "Danish", label: "丹麦语 Danish" },
  { key: "Finnish", label: "芬兰语 Finnish" },
  { key: "Polish", label: "波兰语 Polish" },
  { key: "Czech", label: "捷克语 Czech" },
  { key: "Filipino", label: "菲律宾语 Filipino" },
  { key: "Persian", label: "波斯语 Persian" },
  { key: "Greek", label: "希腊语 Greek" },
  { key: "Romanian", label: "罗马尼亚语 Romanian" },
  { key: "Hungarian", label: "匈牙利语 Hungarian" },
  { key: "Macedonian", label: "马其顿语 Macedonian" },
  { key: "", label: "自动检测(混说时可能误翻译,不推荐)" },
];

const BACKENDS: { key: Backend; label: string; description: string }[] = [
  {
    key: "qwen3_vllm",
    label: "Qwen3-ASR-1.7B + vLLM(默认 · 本地 GPU)",
    description:
      "开源 SOTA 准确率,数据不出本机;守护进程启动时预载,按键即听。",
  },
  {
    key: "volcano_api",
    label: "火山引擎大模型语音识别(云端 API)",
    description:
      "需配置 APP ID 与 Access Token;音频发送至火山引擎。⚠ 未经真实服务验证。",
  },
  {
    key: "zipformer_local",
    label: "Zipformer 流式(本地 CPU 兜底)",
    description: "轻量本地模型,延迟最低;在「模型」页下载并激活。",
  },
];

export default function Input() {
  const lastError = useControlStore((state) => state.lastError);
  const [asr, setAsr] = useState<AsrConfigShape | null>(null);
  const [hotkey, setHotkey] = useState("Alt+S");
  const [mode, setMode] = useState<"toggle" | "hold">("toggle");
  const [script, setScript] = useState<OutputScript>("simplified");
  const [saving, setSaving] = useState(false);
  const [savedTick, setSavedTick] = useState(0);
  const [capturing, setCapturing] = useState(false);

  useEffect(() => {
    void (async () => {
      const reply = await coreCommand("config.get");
      if (reply.kind === "response" && reply.payload) {
        const config = reply.payload.config as Record<string, unknown>;
        setAsr(config.asr as unknown as AsrConfigShape);
        const input = config.input as Record<string, unknown> | undefined;
        setHotkey((input?.hotkey as string) ?? "Alt+S");
        setMode(((input?.mode as string) ?? "toggle") as "toggle" | "hold");
        const text = config.text as Record<string, unknown> | undefined;
        setScript(((text?.output_script as string) ?? "simplified") as OutputScript);
      }
    })();
  }, []);

  const save = async (patch: Record<string, unknown>) => {
    setSaving(true);
    const reply = await coreCommand("config.update", { patch });
    setSaving(false);
    if (reply.kind === "response") {
      setSavedTick((tick) => tick + 1);
      void useControlStore.getState().refreshConfig();
    }
  };

  // Capture a real key combo: read modifiers + the main key from a keydown,
  // build a "Ctrl+Alt+D"-style string the engine parser accepts.
  const captureHotkey = (event: React.KeyboardEvent) => {
    event.preventDefault();
    const key = event.key;
    // Ignore lone modifier presses; wait for the main key.
    if (["Control", "Alt", "Shift", "Meta", "OS"].includes(key)) {
      return;
    }
    const mods: string[] = [];
    if (event.ctrlKey) mods.push("Ctrl");
    if (event.altKey) mods.push("Alt");
    if (event.shiftKey) mods.push("Shift");
    if (event.metaKey) mods.push("Super");
    if (mods.length === 0) {
      return; // require at least one modifier so it doesn't eat plain typing
    }
    let main = key;
    if (key === " ") main = "Space";
    else if (key.length === 1) main = key.toUpperCase();
    else return; // function/arrow keys etc. unsupported by the engine parser
    const combo = [...mods, main].join("+");
    setHotkey(combo);
    setCapturing(false);
    void save({ input: { hotkey: combo } });
  };

  return (
    <div>
      <PageHead
        index="02"
        title="输入"
        desc="听写快捷键、模式与语音识别后端;快捷键/模式即时生效,后端切换在下次听写生效。"
      />
      {lastError ? <div className="error-banner">{lastError}</div> : null}

      <section className="panel">
        <div className="panel-label">快捷键与模式</div>
        <div className="row" style={{ marginBottom: 12 }}>
          <span className="telemetry">
            <span className="k">听写快捷键</span>
            <span className="v glow">{hotkey}</span>
          </span>
          <button
            className={`vf ${capturing ? "" : "ghost"}`}
            onKeyDown={capturing ? captureHotkey : undefined}
            onClick={() => setCapturing(true)}
            onBlur={() => setCapturing(false)}
          >
            {capturing ? "请按下快捷键…(需含 Ctrl/Alt/Shift/Super)" : "点击录制快捷键"}
          </button>
        </div>
        <label className={`choice ${mode === "toggle" ? "selected" : ""}`}>
          <input
            type="radio"
            name="dictation-mode"
            checked={mode === "toggle"}
            onChange={() => {
              setMode("toggle");
              void save({ input: { mode: "toggle" } });
            }}
          />
          <div className="grow">
            <div>切换模式(Toggle)</div>
            <div className="muted">按一次开始听写,再按一次结束。</div>
          </div>
        </label>
        <label className={`choice ${mode === "hold" ? "selected" : ""}`}>
          <input
            type="radio"
            name="dictation-mode"
            checked={mode === "hold"}
            onChange={() => {
              setMode("hold");
              void save({ input: { mode: "hold" } });
            }}
          />
          <div className="grow">
            <div>按住模式(Hold / 按住说话)</div>
            <div className="muted">按住快捷键说话,松开即结束。</div>
          </div>
        </label>
        <p className="hint" style={{ margin: "6px 0 0" }}>
          快捷键与模式即时生效(下一次按键起);录制时请按含 Ctrl/Alt/Shift/Super 的组合。
        </p>
      </section>

      {!asr ? (
        <section className="panel muted">等待 Core 连接后读取配置…</section>
      ) : (
        <>
          <section className="panel">
            <div className="panel-label">识别后端</div>
            {BACKENDS.map(({ key, label, description }) => (
              <label
                key={key}
                className={`choice ${asr.backend === key ? "selected" : ""}`}
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
          </section>

          {asr.backend === "volcano_api" ? (
            <section className="panel">
              <div className="panel-label">火山引擎密钥</div>
              <p className="muted" style={{ marginTop: 0 }}>
                仅保存于本机 ~/.voxflow/config.toml,不会上传。
              </p>
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
                  placeholder="Access Token"
                  value={asr.volcano.access_key}
                  onChange={(event) =>
                    setAsr({
                      ...asr,
                      volcano: {
                        ...asr.volcano,
                        access_key: event.target.value,
                      },
                    })
                  }
                />
              </div>
              <div className="row" style={{ marginBottom: 10 }}>
                <input
                  className="vf grow"
                  placeholder="Resource ID"
                  value={asr.volcano.resource_id}
                  onChange={(event) =>
                    setAsr({
                      ...asr,
                      volcano: {
                        ...asr.volcano,
                        resource_id: event.target.value,
                      },
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
                      volcano: {
                        ...asr.volcano,
                        model_name: event.target.value,
                      },
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
            </section>
          ) : null}

          {asr.backend === "qwen3_vllm" ? (
            <section className="panel">
              <div className="panel-label">Qwen3 引擎与识别语言</div>
              <div className="muted" style={{ marginBottom: 10 }}>
                模型 <span className="mono">{asr.qwen3.model}</span> ·
                守护进程启动时预载并预热,权重缓存于本机,无需联网。
              </div>
              <div className="row">
                <span className="telemetry">
                  <span className="k">识别语言(Qwen 支持 30 种)</span>
                  <span className="muted">
                    固定语言可避免混说时被误翻译;改动后切换后端或重启 Core 生效。
                  </span>
                </span>
                <select
                  className="vf"
                  value={asr.qwen3.language}
                  onChange={(event) => {
                    const language = event.target.value;
                    setAsr({ ...asr, qwen3: { ...asr.qwen3, language } });
                    void save({ asr: { qwen3: { language } } });
                  }}
                >
                  {QWEN_LANGS.map(({ key, label }) => (
                    <option key={key || "auto"} value={key}>
                      {label}
                    </option>
                  ))}
                </select>
              </div>
            </section>
          ) : null}

          <section className="panel">
            <div className="panel-label">文本输出(简繁)</div>
            {SCRIPTS.map(({ key, label, description }) => (
              <label
                key={key}
                className={`choice ${script === key ? "selected" : ""}`}
              >
                <input
                  type="radio"
                  name="output-script"
                  checked={script === key}
                  onChange={() => {
                    setScript(key);
                    void save({ text: { output_script: key } });
                  }}
                />
                <div className="grow">
                  <div>{label}</div>
                  <div className="muted">{description}</div>
                </div>
              </label>
            ))}
          </section>
        </>
      )}
      {savedTick > 0 ? <p className="muted">✓ 已保存</p> : null}
    </div>
  );
}
