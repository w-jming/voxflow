use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use voxflow_ipc::{
    AudioInfo, CoreInfo, DictationInfo, DictationState, FrontendInfo, FrontendState,
    IntentClassifierInfo, ModelInfo, PathInfo, StatusSnapshot,
};

pub mod bridge;
pub mod shell;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum StatusTone {
    Ready,
    Warning,
    Error,
    Degraded,
    Loading,
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalStatus {
    pub label: String,
    pub tone: StatusTone,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NavItem {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusCard {
    pub id: String,
    pub title: String,
    pub tone: StatusTone,
    pub badge: String,
    pub description: String,
    pub action_label: String,
    pub action_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticStatus {
    pub enabled: bool,
    pub classifier_state: String,
    pub classifier_version: Option<String>,
    pub threshold_mode: String,
    pub recent_record_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataPathStatus {
    pub label: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlCenterSnapshot {
    pub app_version: String,
    pub connection: ConnectionState,
    pub global_status: GlobalStatus,
    pub current_model: String,
    pub nav: Vec<NavItem>,
    pub overview_cards: Vec<StatusCard>,
    pub semantic: SemanticStatus,
    pub data_paths: Vec<DataPathStatus>,
    pub config_revision: u64,
}

impl ControlCenterSnapshot {
    pub fn from_status(status: StatusSnapshot, connection: ConnectionState) -> Self {
        let overview_cards = vec![
            input_service_card(&status, &connection),
            frontend_card(&status.frontend),
            audio_card(&status.audio),
            model_card(&status.models),
        ];
        let global_status = aggregate_global_status(&connection, &overview_cards);
        let current_model = status.models.active_asr.clone();
        let semantic = SemanticStatus {
            enabled: true,
            classifier_state: status.models.intent_classifier.state.clone(),
            classifier_version: status.models.intent_classifier.version.clone(),
            threshold_mode: "standard".to_string(),
            recent_record_count: 0,
        };
        let data_paths = vec![
            DataPathStatus {
                label: "VoxFlow Home".to_string(),
                path: status.paths.home,
            },
            DataPathStatus {
                label: "模型".to_string(),
                path: status.paths.models,
            },
            DataPathStatus {
                label: "日志".to_string(),
                path: status.paths.logs,
            },
            DataPathStatus {
                label: "缓存".to_string(),
                path: status.paths.cache,
            },
        ];
        Self {
            app_version: status.core.version,
            connection,
            global_status,
            current_model,
            nav: default_nav(),
            overview_cards,
            semantic,
            data_paths,
            config_revision: status.config_revision,
        }
    }
}

pub fn sample_status_snapshot() -> StatusSnapshot {
    StatusSnapshot {
        core: CoreInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            state: "running".to_string(),
            uptime_ms: 42_000,
        },
        dictation: DictationInfo {
            state: DictationState::Idle,
            session_id: None,
        },
        frontend: FrontendInfo {
            kind: Some("ibus".to_string()),
            state: FrontendState::Registered,
            capabilities: vec!["preedit".to_string(), "commit".to_string()],
        },
        audio: AudioInfo {
            device_id: None,
            label: None,
            available: false,
            bluetooth_profile: None,
        },
        models: ModelInfo {
            active_asr: "streaming-zh-en-small".to_string(),
            active_refiner: None,
            intent_classifier: IntentClassifierInfo {
                state: "not_loaded".to_string(),
                version: None,
            },
        },
        paths: PathInfo {
            home: "~/.voxflow".to_string(),
            logs: "~/.voxflow/logs".to_string(),
            models: "~/.voxflow/models".to_string(),
            cache: "~/.voxflow/cache".to_string(),
        },
        config_revision: 1,
    }
}

pub fn sample_control_center_snapshot() -> ControlCenterSnapshot {
    ControlCenterSnapshot::from_status(sample_status_snapshot(), ConnectionState::Connected)
}

pub fn write_static_bundle(dir: impl AsRef<Path>) -> Result<()> {
    let dir = dir.as_ref();
    let assets = dir.join("assets");
    fs::create_dir_all(&assets).with_context(|| format!("create {}", assets.display()))?;
    fs::write(dir.join("index.html"), include_str!("../web/index.html"))
        .with_context(|| format!("write {}", dir.join("index.html").display()))?;
    fs::write(dir.join("app.css"), include_str!("../web/app.css"))
        .with_context(|| format!("write {}", dir.join("app.css").display()))?;
    fs::write(dir.join("app.js"), include_str!("../web/app.js"))
        .with_context(|| format!("write {}", dir.join("app.js").display()))?;
    fs::write(
        dir.join("mock-state.json"),
        serde_json::to_string_pretty(&sample_control_center_snapshot())?,
    )
    .with_context(|| format!("write {}", dir.join("mock-state.json").display()))?;
    for asset in STATIC_ASSETS {
        fs::write(assets.join(asset.name), asset.contents)
            .with_context(|| format!("write {}", assets.join(asset.name).display()))?;
    }
    Ok(())
}

pub struct StaticAsset {
    pub name: &'static str,
    pub contents: &'static str,
}

pub const STATIC_ASSETS: &[StaticAsset] = &[
    StaticAsset {
        name: "voxflow-logo.svg",
        contents: include_str!("../web/assets/voxflow-logo.svg"),
    },
    StaticAsset {
        name: "voxflow-logo-dark.svg",
        contents: include_str!("../web/assets/voxflow-logo-dark.svg"),
    },
    StaticAsset {
        name: "voxflow-symbol.svg",
        contents: include_str!("../web/assets/voxflow-symbol.svg"),
    },
    StaticAsset {
        name: "voxflow-symbol-dark.svg",
        contents: include_str!("../web/assets/voxflow-symbol-dark.svg"),
    },
];

fn default_nav() -> Vec<NavItem> {
    [
        ("overview", "总览"),
        ("input", "输入"),
        ("models", "模型"),
        ("audio", "音频"),
        ("semantic", "语义修正"),
        ("data", "数据"),
        ("diagnostics", "诊断"),
        ("appearance", "外观"),
    ]
    .into_iter()
    .map(|(id, label)| NavItem {
        id: id.to_string(),
        label: label.to_string(),
    })
    .collect()
}

fn input_service_card(status: &StatusSnapshot, connection: &ConnectionState) -> StatusCard {
    if *connection != ConnectionState::Connected {
        return StatusCard {
            id: "service".to_string(),
            title: "输入服务".to_string(),
            tone: StatusTone::Error,
            badge: "未连接".to_string(),
            description: "Core 未连接,控制台显示的是过期状态".to_string(),
            action_label: "重启服务".to_string(),
            action_command: "core.restart".to_string(),
        };
    }
    if status.core.state == "running" {
        StatusCard {
            id: "service".to_string(),
            title: "输入服务".to_string(),
            tone: StatusTone::Ready,
            badge: "运行中".to_string(),
            description: format!("Core {} 已启动", status.core.version),
            action_label: "暂停听写".to_string(),
            action_command: "dictation.pause".to_string(),
        }
    } else {
        StatusCard {
            id: "service".to_string(),
            title: "输入服务".to_string(),
            tone: StatusTone::Error,
            badge: "已停止".to_string(),
            description: "核心服务没有运行".to_string(),
            action_label: "启动服务".to_string(),
            action_command: "core.start".to_string(),
        }
    }
}

fn frontend_card(frontend: &FrontendInfo) -> StatusCard {
    match frontend.state {
        FrontendState::Active | FrontendState::Connected => StatusCard {
            id: "frontend".to_string(),
            title: "输入法前端".to_string(),
            tone: StatusTone::Ready,
            badge: "已连接".to_string(),
            description: format!(
                "{} 能力:{}",
                frontend.kind.as_deref().unwrap_or("前端"),
                frontend.capabilities.join(",")
            ),
            action_label: "刷新".to_string(),
            action_command: "frontend.refresh".to_string(),
        },
        FrontendState::Registered | FrontendState::Installed => StatusCard {
            id: "frontend".to_string(),
            title: "输入法前端".to_string(),
            tone: StatusTone::Warning,
            badge: "待激活".to_string(),
            description: "组件已注册,当前应用尚未连接输入上下文".to_string(),
            action_label: "打开输入源".to_string(),
            action_command: "frontend.open_settings".to_string(),
        },
        FrontendState::NotInstalled | FrontendState::Disconnected => StatusCard {
            id: "frontend".to_string(),
            title: "输入法前端".to_string(),
            tone: StatusTone::Error,
            badge: "不可用".to_string(),
            description: "输入法组件未安装或已断开".to_string(),
            action_label: "安装组件".to_string(),
            action_command: "frontend.install".to_string(),
        },
    }
}

fn audio_card(audio: &AudioInfo) -> StatusCard {
    if audio.available {
        StatusCard {
            id: "audio".to_string(),
            title: "麦克风".to_string(),
            tone: StatusTone::Ready,
            badge: "可用".to_string(),
            description: audio
                .label
                .clone()
                .unwrap_or_else(|| "默认输入设备".to_string()),
            action_label: "录音测试".to_string(),
            action_command: "audio.test_start".to_string(),
        }
    } else {
        StatusCard {
            id: "audio".to_string(),
            title: "麦克风".to_string(),
            tone: StatusTone::Error,
            badge: "不可用".to_string(),
            description: "未检测到可用输入设备或权限不足".to_string(),
            action_label: "打开音频页".to_string(),
            action_command: "page.audio".to_string(),
        }
    }
}

fn model_card(models: &ModelInfo) -> StatusCard {
    if models.active_asr.trim().is_empty() {
        return StatusCard {
            id: "model".to_string(),
            title: "模型".to_string(),
            tone: StatusTone::Error,
            badge: "缺失".to_string(),
            description: "还没有可用 ASR 模型".to_string(),
            action_label: "下载模型".to_string(),
            action_command: "model.download".to_string(),
        };
    }
    if models.intent_classifier.state != "ready" {
        return StatusCard {
            id: "model".to_string(),
            title: "模型".to_string(),
            tone: StatusTone::Degraded,
            badge: "规则模式".to_string(),
            description: format!("ASR:{}; 语义分类器未加载", models.active_asr),
            action_label: "校验模型".to_string(),
            action_command: "model.verify".to_string(),
        };
    }
    StatusCard {
        id: "model".to_string(),
        title: "模型".to_string(),
        tone: StatusTone::Ready,
        badge: "可用".to_string(),
        description: format!("ASR:{}; 语义分类器可用", models.active_asr),
        action_label: "切换".to_string(),
        action_command: "model.switch".to_string(),
    }
}

fn aggregate_global_status(connection: &ConnectionState, cards: &[StatusCard]) -> GlobalStatus {
    if *connection != ConnectionState::Connected {
        return GlobalStatus {
            label: "Core 未连接".to_string(),
            tone: StatusTone::Error,
        };
    }
    if cards.iter().any(|card| card.tone == StatusTone::Error) {
        return GlobalStatus {
            label: "错误".to_string(),
            tone: StatusTone::Error,
        };
    }
    if cards
        .iter()
        .any(|card| matches!(card.tone, StatusTone::Warning | StatusTone::Degraded))
    {
        return GlobalStatus {
            label: "需要处理".to_string(),
            tone: StatusTone::Warning,
        };
    }
    GlobalStatus {
        label: "可输入".to_string(),
        tone: StatusTone::Ready,
    }
}
