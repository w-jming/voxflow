const state = {
  mediaRecorder: null,
  audioContext: null,
  analyser: null,
  dataArray: null,
  chunks: [],
  recording: false,
  animationId: null,
  downloadPoll: null,
  models: [],
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
  appHomeInput: document.getElementById("appHomeInput"),
  semanticToggle: document.getElementById("semanticToggle"),
  hotkeyMessage: document.getElementById("hotkeyMessage"),
  saveHotkeyBtn: document.getElementById("saveHotkeyBtn"),
  downloadModelBtn: document.getElementById("downloadModelBtn"),
  pauseDownloadBtn: document.getElementById("pauseDownloadBtn"),
  downloadProgress: document.getElementById("downloadProgress"),
  downloadText: document.getElementById("downloadText"),
  modelSummary: document.getElementById("modelSummary"),
  localModelPath: document.getElementById("localModelPath"),
  validateLocalModelBtn: document.getElementById("validateLocalModelBtn"),
  importLocalModelBtn: document.getElementById("importLocalModelBtn"),
  linkLocalModelBtn: document.getElementById("linkLocalModelBtn"),
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
  const [configResponse, modelsResponse, downloadResponse] = await Promise.all([
    fetch("/api/config"),
    fetch("/api/models"),
    fetch("/api/models/download/status"),
  ]);
  const config = await configResponse.json();
  const modelsPayload = await modelsResponse.json();
  const downloadPayload = await downloadResponse.json();
  state.models = modelsPayload.models || [];
  renderModelOptions(config.asr || {});
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
  el.appHomeInput.value = config.paths?.home || "";
  el.appHomeInput.disabled = Boolean(config.paths?.env_locked);
  renderDownloadStatus(downloadPayload);
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
  renderModelSummary();
}

function modelLabel(asr) {
  const selected = state.models.find((model) => modelMatchesAsr(model, asr));
  return selected ? selected.label : (asr?.model || "");
}

function selectedModel() {
  return state.models.find((model) => model.id === el.modelSelect.value) || state.models[0];
}

function renderModelSummary() {
  const model = selectedModel();
  if (!model) {
    el.modelSummary.textContent = "未读取到模型列表";
    return;
  }
  el.modelSummary.textContent = `${model.label} · ${model.size} · ${model.languages} · ${model.license}`;
  const canDownload = !model.model.startsWith("bundled:");
  el.downloadModelBtn.disabled = !canDownload;
  el.pauseDownloadBtn.disabled = true;
  if (!canDownload) {
    el.downloadText.textContent = "内置模型已随软件安装";
    el.downloadProgress.style.width = "100%";
  } else {
    el.downloadText.textContent = "未下载";
    el.downloadProgress.style.width = "0%";
  }
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

function formatBytes(value) {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let amount = Number(value || 0);
  for (const unit of units) {
    if (amount < 1024 || unit === units[units.length - 1]) {
      return unit === "B" ? `${amount.toFixed(0)} B` : `${amount.toFixed(1)} ${unit}`;
    }
    amount /= 1024;
  }
  return `${amount.toFixed(1)} TB`;
}

function statusLabel(value) {
  return {
    downloading: "下载中",
    pausing: "暂停中",
    paused: "已暂停",
    completed: "已完成",
    failed: "失败",
    idle: "未下载",
  }[value] || value;
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
  ctx.fillStyle = "rgba(29,167,232,0.15)";
  ctx.fillRect(0, (height - bandHeight) / 2, width, bandHeight);

  ctx.beginPath();
  ctx.strokeStyle = rms > 0.22 ? "#b4323b" : "#1da7e8";
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
  const modelProfile = el.modelSelect.value;
  const appHome = el.appHomeInput.value.trim();
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
      model_profile: modelProfile,
      app_home: appHome,
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
  el.appHomeInput.value = payload.paths?.home || el.appHomeInput.value;
  el.appHomeInput.disabled = Boolean(payload.paths?.env_locked);
  el.hotkeyMessage.textContent = payload.daemon_restarted ? "已保存并重启" : "已保存";
  setStatus("完成", "ready");
  await loadConfig();
}

async function downloadSelectedModel() {
  const modelProfile = el.modelSelect.value;
  const model = selectedModel();
  el.hotkeyMessage.textContent = `下载中：${model?.label || modelProfile}`;
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
  renderDownloadStatus(payload);
  startDownloadPolling();
}

async function pauseModelDownload() {
  const response = await fetch("/api/models/download/pause", { method: "POST" });
  const payload = await response.json();
  renderDownloadStatus(payload);
  el.hotkeyMessage.textContent = "已暂停，可继续下载";
  setStatus("就绪", "ready");
}

function startDownloadPolling() {
  if (state.downloadPoll) clearInterval(state.downloadPoll);
  state.downloadPoll = setInterval(refreshDownloadStatus, 1000);
}

async function refreshDownloadStatus() {
  const response = await fetch("/api/models/download/status");
  const payload = await response.json();
  renderDownloadStatus(payload);
  if (!["downloading", "pausing"].includes(payload.status)) {
    clearInterval(state.downloadPoll);
    state.downloadPoll = null;
    if (payload.status === "completed") {
      el.hotkeyMessage.textContent = "模型下载完成";
      setStatus("完成", "ready");
      await loadConfig();
    }
    if (payload.status === "failed") {
      el.hotkeyMessage.textContent = payload.error || "下载失败";
      setStatus("错误", "error");
    }
  }
}

function renderDownloadStatus(payload = {}) {
  const status = payload.status || "idle";
  const done = Number(payload.bytes || 0);
  const total = Number(payload.total_bytes || 0);
  const fraction = total > 0 ? Math.min(1, done / total) : 0;
  el.downloadProgress.style.width = `${Math.round(fraction * 100)}%`;
  el.pauseDownloadBtn.disabled = status !== "downloading";
  if (status === "idle") {
    if (!selectedModel()?.model?.startsWith("bundled:")) el.downloadText.textContent = "未下载";
    return;
  }
  const speed = Number(payload.speed_bps || 0);
  let text = `${statusLabel(status)} · ${formatBytes(done)} / ${formatBytes(total)}`;
  if (status === "downloading") text += ` · ${formatBytes(speed)}/s`;
  if (payload.label) text = `${payload.label} · ${text}`;
  if (payload.error) text += ` · ${payload.error}`;
  el.downloadText.textContent = text;
}

async function validateLocalModel() {
  await localModelAction("/api/models/validate-local", {});
}

async function importLocalModel(symlink) {
  await localModelAction("/api/models/import-local", { symlink });
  await loadConfig();
}

async function localModelAction(endpoint, extra) {
  const modelProfile = el.modelSelect.value;
  const path = el.localModelPath.value.trim();
  if (!path) {
    el.hotkeyMessage.textContent = "请输入本地模型目录";
    return;
  }
  el.hotkeyMessage.textContent = "校验中";
  setStatus("校验中", "busy");
  const response = await fetch(endpoint, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ model_profile: modelProfile, path, ...extra }),
  });
  const payload = await response.json();
  if (!response.ok) {
    el.hotkeyMessage.textContent = payload.error || "操作失败";
    setStatus("错误", "error");
    return;
  }
  el.hotkeyMessage.textContent = endpoint.includes("validate")
    ? `校验通过：${payload.path}`
    : `已导入：${payload.path}`;
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
el.pauseDownloadBtn.addEventListener("click", pauseModelDownload);
el.modelSelect.addEventListener("change", renderModelSummary);
el.validateLocalModelBtn.addEventListener("click", validateLocalModel);
el.importLocalModelBtn.addEventListener("click", () => importLocalModel(false));
el.linkLocalModelBtn.addEventListener("click", () => importLocalModel(true));
el.copyRawBtn.addEventListener("click", () => copyText(el.rawText));
el.copyProcessedBtn.addEventListener("click", () => copyText(el.processedText));

loadConfig().catch(() => {
  el.configLine.textContent = "配置读取失败";
});
