const fallbackState = {
  app_version: "0.3.0",
  connection: "connected",
  global_status: { label: "错误", tone: "error" },
  current_model: "streaming-zh-en-small",
  nav: [
    { id: "overview", label: "总览" },
    { id: "input", label: "输入" },
    { id: "models", label: "模型" },
    { id: "audio", label: "音频" },
    { id: "semantic", label: "语义修正" },
    { id: "data", label: "数据" },
    { id: "diagnostics", label: "诊断" },
    { id: "appearance", label: "外观" }
  ],
  overview_cards: [
    {
      id: "service",
      title: "输入服务",
      tone: "ready",
      badge: "运行中",
      description: "Core 0.3.0 已启动",
      action_label: "暂停听写",
      action_command: "dictation.pause"
    },
    {
      id: "frontend",
      title: "输入法前端",
      tone: "warning",
      badge: "待激活",
      description: "组件已注册,当前应用尚未连接输入上下文",
      action_label: "打开输入源",
      action_command: "frontend.open_settings"
    },
    {
      id: "audio",
      title: "麦克风",
      tone: "error",
      badge: "不可用",
      description: "未检测到可用输入设备或权限不足",
      action_label: "打开音频页",
      action_command: "page.audio"
    },
    {
      id: "model",
      title: "模型",
      tone: "degraded",
      badge: "规则模式",
      description: "ASR:streaming-zh-en-small; 语义分类器未加载",
      action_label: "校验模型",
      action_command: "model.verify"
    }
  ],
  semantic: {
    enabled: true,
    classifier_state: "not_loaded",
    classifier_version: null,
    threshold_mode: "standard",
    recent_record_count: 0
  },
  data_paths: [
    { label: "VoxFlow Home", path: "~/.voxflow" },
    { label: "模型", path: "~/.voxflow/models" },
    { label: "日志", path: "~/.voxflow/logs" },
    { label: "缓存", path: "~/.voxflow/cache" }
  ],
  config_revision: 1
};

const app = document.querySelector("#app");

function chipClass(tone) {
  if (tone === "error") return "status-chip chip-error";
  if (tone === "warning") return "status-chip chip-warning";
  return "status-chip chip-ready";
}

function render(state) {
  app.innerHTML = `
    <div class="frame">
      <header class="topbar">
        <div class="brand" role="img" aria-label="VoxFlow 声流输入法"></div>
        <span class="${chipClass(state.global_status.tone)}">${state.global_status.label}</span>
        <div class="topbar-model">当前模型 <span class="mono">${state.current_model}</span></div>
        <div class="topbar-actions">
          <button class="button" title="暂停听写" data-command="dictation.pause">暂停</button>
          <button class="button" title="打开日志" data-command="logs.open">日志</button>
          <select id="themeSelect" title="主题">
            <option value="system">跟随系统</option>
            <option value="light">浅色</option>
            <option value="dark">深色</option>
          </select>
        </div>
      </header>
      <div class="frame-body">
        <nav class="sidebar" aria-label="控制台页面">
          ${state.nav.map((item, index) => `
            <button class="nav-button ${index === 0 ? "active" : ""}" data-page="${item.id}">${item.label}</button>
          `).join("")}
        </nav>
        <main class="content">
          ${overviewPage(state)}
          ${inputPage()}
          ${modelsPage(state)}
          ${audioPage()}
          ${semanticPage(state)}
          ${dataPage(state)}
          ${diagnosticsPage()}
          ${appearancePage()}
        </main>
      </div>
    </div>
  `;
  bindNavigation();
  bindThemeSelect();
}

function overviewPage(state) {
  return `
    <section id="page-overview" class="page active">
      <div class="page-header">
        <div>
          <h1 class="page-title">总览</h1>
          <p class="page-subtitle">配置版本 <span class="mono">${state.config_revision}</span></p>
        </div>
        <button class="button primary" data-command="diagnostics.run">运行诊断</button>
      </div>
      <div class="overview-grid">
        ${state.overview_cards.map(statusCard).join("")}
      </div>
      <div class="panel">
        <strong>最近一次错误</strong>
        <p class="muted">麦克风不可用;输入法前端尚未连接真实桌面输入上下文。</p>
      </div>
    </section>
  `;
}

function statusCard(card) {
  return `
    <article class="status-card" data-tone="${card.tone}">
      <div class="card-symbol" aria-hidden="true"></div>
      <div>
        <div class="card-title-row">
          <h2 class="card-title">${card.title}</h2>
          <span class="${chipClass(card.tone)}">${card.badge}</span>
        </div>
        <p class="card-description">${card.description}</p>
        <button class="button ${card.tone === "ready" ? "" : "primary"}" data-command="${card.action_command}">${card.action_label}</button>
      </div>
    </article>
  `;
}

function inputPage() {
  return page("input", "输入", `
    <div class="panel settings-grid">
      ${settingRow("输入模式", segmented(["输入法模式", "兼容注入模式"], 0))}
      ${settingRow("快捷键", '<button class="button mono">Alt+S</button>')}
      ${settingRow("按键逻辑", segmented(["按一次开始", "按住说话"], 0))}
      ${settingRow("输出字形", segmented(["简体", "繁体", "模型原文"], 0))}
      ${settingRow("光标处反馈", '<span class="status-chip chip-warning">等待输入上下文</span>')}
      ${settingRow("状态指示器", '<label class="toggle"><input type="checkbox" checked />启用</label>')}
      ${settingRow("自动标点", '<label class="toggle"><input type="checkbox" checked />启用</label>')}
      ${settingRow("口语助词清理", '<label class="toggle"><input type="checkbox" checked />启用</label>')}
    </div>
  `);
}

function modelsPage(state) {
  return page("models", "模型", `
    <div class="panel">
      <div class="model-row">
        <div>
          <strong>${state.current_model}</strong>
          <div class="muted">实时流式 ASR · Apache-2.0 · zh/en</div>
        </div>
        <span class="status-chip chip-warning">待校验</span>
      </div>
      <div class="model-row">
        <div>
          <strong>semantic-intent-small</strong>
          <div class="muted">语义撤销分类器 · 本地 ONNX 目标</div>
        </div>
        <button class="button">下载</button>
      </div>
    </div>
  `);
}

function audioPage() {
  return page("audio", "音频", `
    <div class="panel settings-grid">
      ${settingRow("当前设备", '<span class="status-chip chip-error">未检测到输入设备</span>')}
      ${settingRow("实时电平", '<div class="level-bar" aria-label="实时电平"><span style="width: 0%"></span></div>')}
      ${settingRow("VAD 灵敏度", segmented(["低", "标准", "高"], 1))}
      ${settingRow("录音测试", '<button class="button primary">开始</button>')}
      ${settingRow("蓝牙诊断", '<button class="button">重新检测</button>')}
    </div>
  `);
}

function semanticPage(state) {
  const classifierBadge = state.semantic.classifier_state === "ready"
    ? '<span class="status-chip chip-ready">可用</span>'
    : '<span class="status-chip chip-warning">已降级为规则模式</span>';
  return page("semantic", "语义修正", `
    <div class="panel settings-grid">
      ${settingRow("智能撤销", '<label class="toggle"><input type="checkbox" checked />启用</label>')}
      ${settingRow("分类器状态", classifierBadge)}
      ${settingRow("阈值模式", segmented(["保守", "标准", "积极"], 1))}
    </div>
    <div class="record-row">
      <div>
        <strong>最近修正记录</strong>
        <div class="muted">当前没有可恢复记录</div>
      </div>
      <button class="button" disabled>恢复</button>
    </div>
  `);
}

function dataPage(state) {
  return page("data", "数据", `
    <div class="panel path-list">
      ${state.data_paths.map((item) => `
        <div class="path-item">
          <strong>${item.label}</strong>
          <span class="mono">${item.path}</span>
          <button class="button">打开</button>
        </div>
      `).join("")}
    </div>
  `);
}

function diagnosticsPage() {
  return page("diagnostics", "诊断", `
    <div class="panel">
      <button class="button primary">一键 doctor</button>
      <button class="button">复制诊断摘要</button>
      <div class="model-row">
        <div>
          <strong>PipeWire runtime</strong>
          <div class="muted">等待 Core doctor 结果</div>
        </div>
        <span class="status-chip chip-warning">未运行</span>
      </div>
    </div>
  `);
}

function appearancePage() {
  return page("appearance", "外观", `
    <div class="panel settings-grid">
      ${settingRow("主题", segmented(["跟随系统", "浅色", "深色"], 0))}
      ${settingRow("动效", segmented(["标准", "减少动效"], 0))}
      ${settingRow("状态指示器", '<div class="hud-preview"><span class="hud-dot"></span><span>听写中</span><div class="level-bar" style="width:44px"><span></span></div></div>')}
    </div>
  `);
}

function page(id, title, body) {
  return `
    <section id="page-${id}" class="page">
      <div class="page-header">
        <div>
          <h1 class="page-title">${title}</h1>
        </div>
      </div>
      ${body}
    </section>
  `;
}

function settingRow(label, control) {
  return `
    <div class="setting-row">
      <div class="setting-label">${label}</div>
      <div>${control}</div>
    </div>
  `;
}

function segmented(items, activeIndex) {
  return `
    <div class="segmented">
      ${items.map((item, index) => `<button class="${index === activeIndex ? "active" : ""}">${item}</button>`).join("")}
    </div>
  `;
}

function bindNavigation() {
  document.querySelectorAll(".nav-button").forEach((button) => {
    button.addEventListener("click", () => {
      document.querySelectorAll(".nav-button").forEach((item) => item.classList.remove("active"));
      document.querySelectorAll(".page").forEach((page) => page.classList.remove("active"));
      button.classList.add("active");
      document.querySelector(`#page-${button.dataset.page}`).classList.add("active");
    });
  });
}

function bindThemeSelect() {
  const select = document.querySelector("#themeSelect");
  const storedTheme = localStorage.getItem("voxflow-theme") || "system";
  select.value = storedTheme;
  select.addEventListener("change", () => {
    localStorage.setItem("voxflow-theme", select.value);
    const prefersDark = window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches;
    document.documentElement.dataset.theme = select.value === "system" ? (prefersDark ? "dark" : "light") : select.value;
  });
}

render(fallbackState);

