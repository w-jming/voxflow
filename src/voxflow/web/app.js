const state = {
  mediaRecorder: null,
  audioContext: null,
  analyser: null,
  dataArray: null,
  chunks: [],
  recording: false,
  animationId: null,
  models: [],
  semanticBackends: [],
};

const el = {
  status: document.getElementById("status"),
  statusText: document.getElementById("statusText"),
  configLine: document.getElementById("configLine"),
  recordBtn: document.getElementById("recordBtn"),
  stopBtn: document.getElementById("stopBtn"),
  meter: document.getElementById("meter"),
  rawText: document.getElementById("rawText"),
  processedText: document.getElementById("processedText"),
  actionList: document.getElementById("actionList"),
  injectToggle: document.getElementById("injectToggle"),
  manualInjectToggle: document.getElementById("manualInjectToggle"),
  manualText: document.getElementById("manualText"),
  manualBtn: document.getElementById("manualBtn"),
  copyRawBtn: document.getElementById("copyRawBtn"),
  copyProcessedBtn: document.getElementById("copyProcessedBtn"),
  hotkeyLabel: document.getElementById("hotkeyLabel"),
  modeLabel: document.getElementById("modeLabel"),
  modeStep: document.getElementById("modeStep"),
  hotkeyInput: document.getElementById("hotkeyInput"),
  modeSelect: document.getElementById("modeSelect"),
  scriptSelect: document.getElementById("scriptSelect"),
  modelSelect: document.getElementById("modelSelect"),
  semanticToggle: document.getElementById("semanticToggle"),
  semanticBackendSelect: document.getElementById("semanticBackendSelect"),
  hotkeyMessage: document.getElementById("hotkeyMessage"),
  saveHotkeyBtn: document.getElementById("saveHotkeyBtn"),
  downloadModelBtn: document.getElementById("downloadModelBtn"),
};

function setStatus(text, mode = "ready") {
  el.status.dataset.state = mode;
  el.statusText.textContent = text;
}

function renderActions(actions = []) {
  el.actionList.innerHTML = "";
  actions.forEach((action) => {
    if (action.backspace) {
      el.actionList.appendChild(actionRow("退格", `x ${action.backspace}`, true));
    }
    if (action.insert) {
      el.actionList.appendChild(actionRow("插入", action.insert, false));
    }
  });
}

function actionRow(kind, value, danger) {
  const row = document.createElement("div");
  row.className = "action-item";
  const label = document.createElement("div");
  label.className = `action-kind${danger ? " delete" : ""}`;
  label.textContent = kind;
  const content = document.createElement("div");
  content.className = "action-value";
  content.textContent = value;
  row.append(label, content);
  return row;
}

function renderResult(payload) {
  el.rawText.textContent = payload.raw_text || " ";
  el.rawText.classList.toggle("muted", !payload.raw_text);
  el.processedText.textContent = payload.processed_text || " ";
  renderActions(payload.actions || []);
}

async function loadConfig() {
  const [configResponse, modelsResponse, semanticResponse] = await Promise.all([
    fetch("/api/config"),
    fetch("/api/models"),
    fetch("/api/semantic-intent"),
  ]);
  const config = await configResponse.json();
  const modelsPayload = await modelsResponse.json();
  const semanticPayload = await semanticResponse.json();
  state.models = modelsPayload.models || [];
  state.semanticBackends = semanticPayload.backends || [];
  renderModelOptions(config.asr || {});
  renderSemanticBackendOptions(config.text?.semantic_intent_backend || "rules");
  const hotkey = config.daemon?.hotkey || "ctrl+space";
  const mode = config.daemon?.hotkey_mode || "toggle";
  const script = config.text?.script || "simplified";
  const semanticCorrection = config.text?.semantic_correction_enabled !== false;
  el.configLine.textContent = `${config.asr.backend} · ${modelLabel(config.asr)} · ${formatScript(script)} · ${formatHotkey(hotkey)} · ${formatMode(mode)}`;
  el.hotkeyLabel.textContent = formatHotkey(hotkey);
  el.modeLabel.textContent = formatMode(mode);
  el.modeStep.textContent = mode === "hold" ? "3. 按住说话，松开输入" : "3. 按一次开始，再按一次停止";
  el.hotkeyInput.value = hotkey;
  el.modeSelect.value = mode;
  el.scriptSelect.value = script;
  el.semanticToggle.checked = semanticCorrection;
}

function renderSemanticBackendOptions(selectedBackend) {
  el.semanticBackendSelect.innerHTML = "";
  state.semanticBackends.forEach((backend) => {
    const option = document.createElement("option");
    option.value = backend.id;
    option.textContent = backend.status === "available" ? backend.label : `${backend.label}（需训练/安装）`;
    option.title = `${backend.model} · ${backend.license}`;
    option.disabled = backend.status !== "available";
    el.semanticBackendSelect.appendChild(option);
  });
  el.semanticBackendSelect.value = selectedBackend;
}

function renderModelOptions(asr) {
  el.modelSelect.innerHTML = "";
  state.models.forEach((model) => {
    const option = document.createElement("option");
    option.value = model.id;
    option.textContent = model.label;
    option.title = `${model.license} · ${model.source}`;
    el.modelSelect.appendChild(option);
  });
  const selected = state.models.find((model) => modelMatchesAsr(model, asr));
  if (selected) {
    el.modelSelect.value = selected.id;
  }
}

function modelLabel(asr) {
  const selected = state.models.find((model) => modelMatchesAsr(model, asr));
  return selected ? selected.label : (asr?.model || "");
}

function modelMatchesAsr(model, asr) {
  if (!asr || model.backend !== asr.backend) return false;
  if (model.model === asr.model) return true;
  const name = model.model.split("/").pop();
  return Boolean(name && asr.model && asr.model.endsWith(`/${name}`));
}

function formatHotkey(value) {
  return value
    .split("+")
    .filter(Boolean)
    .map((part) => {
      if (part === "ctrl" || part === "control") return "Ctrl";
      if (part === "alt" || part === "mod1") return "Alt";
      if (part === "shift") return "Shift";
      if (part === "super" || part === "meta" || part === "win") return "Super";
      if (part === "space") return "Space";
      if (part.length === 1) return part.toUpperCase();
      return part.charAt(0).toUpperCase() + part.slice(1);
    })
    .join("+");
}

function formatMode(value) {
  return value === "hold" ? "按住录音" : "按键切换";
}

function formatScript(value) {
  if (value === "traditional") return "繁体中文";
  if (value === "original") return "模型原文";
  return "简体中文";
}

async function startRecording() {
  const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  state.audioContext = new AudioContext();
  const source = state.audioContext.createMediaStreamSource(stream);
  state.analyser = state.audioContext.createAnalyser();
  state.analyser.fftSize = 2048;
  state.dataArray = new Uint8Array(state.analyser.frequencyBinCount);
  source.connect(state.analyser);

  state.chunks = [];
  state.mediaRecorder = new MediaRecorder(stream);
  state.mediaRecorder.ondataavailable = (event) => {
    if (event.data.size > 0) state.chunks.push(event.data);
  };
  state.mediaRecorder.onstop = uploadRecording;
  state.mediaRecorder.start();
  state.recording = true;
  el.recordBtn.classList.add("recording");
  el.stopBtn.disabled = false;
  setStatus("录音中", "recording");
  drawMeter();
}

function stopRecording() {
  if (!state.mediaRecorder || state.mediaRecorder.state === "inactive") return;
  state.mediaRecorder.stop();
  state.mediaRecorder.stream.getTracks().forEach((track) => track.stop());
  state.recording = false;
  el.recordBtn.classList.remove("recording");
  el.stopBtn.disabled = true;
  if (state.animationId) cancelAnimationFrame(state.animationId);
  setStatus("识别中", "busy");
}

async function uploadRecording() {
  const blob = new Blob(state.chunks, { type: state.mediaRecorder.mimeType || "audio/webm" });
  const form = new FormData();
  form.append("audio", blob, "speech.webm");
  form.append("inject", el.injectToggle.checked ? "1" : "0");
  try {
    const response = await fetch("/api/transcribe", { method: "POST", body: form });
    const payload = await response.json();
    if (!response.ok) throw new Error(payload.error || "识别失败");
    renderResult(payload);
    setStatus("完成", "ready");
  } catch (error) {
    setStatus("错误", "error");
    el.rawText.textContent = error.message;
    el.rawText.classList.remove("muted");
  } finally {
    if (state.audioContext) state.audioContext.close();
  }
}

function drawMeter() {
  const canvas = el.meter;
  const ctx = canvas.getContext("2d");
  const width = canvas.width;
  const height = canvas.height;
  if (!state.analyser || !state.dataArray) return;

  state.analyser.getByteTimeDomainData(state.dataArray);
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = "#fffdf8";
  ctx.fillRect(0, 0, width, height);

  ctx.strokeStyle = "rgba(36,33,29,0.08)";
  ctx.lineWidth = 1;
  for (let x = 0; x < width; x += 56) {
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, height);
    ctx.stroke();
  }

  let sum = 0;
  for (let i = 0; i < state.dataArray.length; i += 1) {
    const centered = state.dataArray[i] - 128;
    sum += centered * centered;
  }
  const rms = Math.sqrt(sum / state.dataArray.length) / 128;

  const bandHeight = Math.max(10, rms * height * 0.92);
  ctx.fillStyle = "rgba(20,125,115,0.12)";
  ctx.fillRect(0, (height - bandHeight) / 2, width, bandHeight);

  ctx.beginPath();
  ctx.strokeStyle = rms > 0.22 ? "#b4323b" : "#147d73";
  ctx.lineWidth = 4;
  const slice = width / state.dataArray.length;
  for (let i = 0; i < state.dataArray.length; i += 1) {
    const x = i * slice;
    const y = (state.dataArray[i] / 255) * height;
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }
  ctx.stroke();

  const scale = 1 + Math.min(0.22, rms * 0.75);
  el.recordBtn.style.transform = state.recording ? `scale(${scale})` : "";
  state.animationId = requestAnimationFrame(drawMeter);
}

async function processManualText() {
  setStatus("处理中", "busy");
  const response = await fetch("/api/process-text", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ text: el.manualText.value, inject: el.manualInjectToggle.checked }),
  });
  const payload = await response.json();
  if (!response.ok) {
    setStatus("错误", "error");
    el.rawText.textContent = payload.error || "处理失败";
    return;
  }
  renderResult(payload);
  setStatus("完成", "ready");
}

async function saveSettings() {
  const hotkey = el.hotkeyInput.value.trim().toLowerCase();
  const hotkeyMode = el.modeSelect.value;
  const script = el.scriptSelect.value;
  const semanticCorrection = el.semanticToggle.checked;
  const semanticIntentBackend = el.semanticBackendSelect.value || "rules";
  const modelProfile = el.modelSelect.value;
  el.hotkeyMessage.textContent = "保存中";
  setStatus("保存中", "busy");
  const response = await fetch("/api/settings", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      hotkey,
      hotkey_mode: hotkeyMode,
      script,
      semantic_correction_enabled: semanticCorrection,
      semantic_intent_backend: semanticIntentBackend,
      model_profile: modelProfile,
      restart: true,
    }),
  });
  const payload = await response.json();
  if (!response.ok) {
    el.hotkeyMessage.textContent = payload.error || "保存失败";
    setStatus("错误", "error");
    return;
  }
  el.hotkeyLabel.textContent = formatHotkey(payload.hotkey);
  el.modeLabel.textContent = formatMode(payload.hotkey_mode);
  el.hotkeyInput.value = payload.hotkey;
  el.modeSelect.value = payload.hotkey_mode;
  el.scriptSelect.value = payload.script;
  el.semanticToggle.checked = payload.semantic_correction_enabled !== false;
  el.semanticBackendSelect.value = payload.semantic_intent_backend || "rules";
  el.hotkeyMessage.textContent = payload.daemon_restarted ? "已保存并重启" : "已保存";
  setStatus("完成", "ready");
  await loadConfig();
}

async function downloadSelectedModel() {
  const modelProfile = el.modelSelect.value;
  el.hotkeyMessage.textContent = "下载中";
  setStatus("下载中", "busy");
  const response = await fetch("/api/models/download", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ model_profile: modelProfile }),
  });
  const payload = await response.json();
  if (!response.ok) {
    el.hotkeyMessage.textContent = payload.error || "下载失败";
    setStatus("错误", "error");
    return;
  }
  el.hotkeyMessage.textContent = `已下载：${payload.path}`;
  setStatus("完成", "ready");
}

function copyText(target) {
  navigator.clipboard.writeText(target.textContent.trim());
}

el.recordBtn.addEventListener("click", () => {
  if (state.recording) stopRecording();
  else startRecording().catch((error) => {
    setStatus("错误", "error");
    el.rawText.textContent = error.message;
    el.rawText.classList.remove("muted");
  });
});
el.stopBtn.addEventListener("click", stopRecording);
el.manualBtn.addEventListener("click", processManualText);
el.saveHotkeyBtn.addEventListener("click", saveSettings);
el.downloadModelBtn.addEventListener("click", downloadSelectedModel);
el.copyRawBtn.addEventListener("click", () => copyText(el.rawText));
el.copyProcessedBtn.addEventListener("click", () => copyText(el.processedText));

loadConfig().catch(() => {
  el.configLine.textContent = "配置读取失败";
});
