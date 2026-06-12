use std::time::Instant;

use anyhow::Result;
use serde_json::{json, Value};

use crate::config::Config;
use crate::correction::{
    cursor_context_hash, CorrectionAction, CorrectionDecision, CorrectionHistory, CorrectionIntent,
    InjectionLedger, LedgerSegment, RuleIntentClassifier, SafetyGate, SafetyGateContext,
    SegmentSource,
};
use crate::diagnostics::run_diagnostics;
use crate::download::DownloadManager;
use crate::ipc::{
    AudioInfo, CoreInfo, DictationInfo, DictationState, Envelope, FrontendInfo, FrontendState,
    IntentClassifierInfo, MessageKind, ModelInfo, PathInfo, StatusSnapshot, PROTOCOL_VERSION,
};
use crate::model::{
    default_profile_dir, delete_model_by_id, ensure_model_ready_by_id, import_model_by_id,
    list_model_inventory, load_profiles, verify_model_by_id, ModelImportMode,
};
use crate::paths::VoxflowPaths;
use crate::recognizer::{AsrEvent, MockRecognizer, SessionId, StreamingRecognizer};
use voxflow_audio::list_input_devices;

#[derive(Debug)]
pub struct CommandOutcome {
    pub response: Envelope,
    pub events: Vec<Envelope>,
    pub shutdown: bool,
}

pub struct VoxflowCore {
    paths: VoxflowPaths,
    config: Config,
    config_revision: u64,
    started_at: Instant,
    dictation_state: DictationState,
    session_id: Option<SessionId>,
    frontend_kind: Option<String>,
    frontend_state: FrontendState,
    frontend_capabilities: Vec<String>,
    frontend_surrounding_tail: Option<String>,
    recognizer: Box<dyn StreamingRecognizer>,
    recognizer_backend: Option<crate::config::AsrBackend>,
    correction_ledger: InjectionLedger,
    correction_classifier: RuleIntentClassifier,
    correction_gate: SafetyGate,
    correction_history: CorrectionHistory,
    correction_operation_counter: u64,
    downloads: DownloadManager,
    event_sender: Option<tokio::sync::broadcast::Sender<Envelope>>,
}

impl VoxflowCore {
    pub fn load(paths: VoxflowPaths) -> Result<Self> {
        paths.ensure()?;
        let config = Config::load_or_default(&paths.config)?;
        Ok(Self {
            paths,
            config,
            config_revision: 1,
            started_at: Instant::now(),
            dictation_state: DictationState::Idle,
            session_id: None,
            frontend_kind: None,
            frontend_state: FrontendState::NotInstalled,
            frontend_capabilities: Vec::new(),
            frontend_surrounding_tail: None,
            recognizer: Box::new(MockRecognizer::default()),
            recognizer_backend: None,
            correction_ledger: InjectionLedger::default(),
            correction_classifier: RuleIntentClassifier,
            correction_gate: SafetyGate,
            correction_history: CorrectionHistory::default(),
            correction_operation_counter: 0,
            downloads: DownloadManager::default(),
            event_sender: None,
        })
    }

    /// Wires the broadcast channel used by background tasks (downloads) to
    /// publish events outside a command/response cycle.
    pub fn set_event_sender(&mut self, sender: tokio::sync::broadcast::Sender<Envelope>) {
        self.event_sender = Some(sender);
    }

    pub fn with_config(paths: VoxflowPaths, config: Config) -> Self {
        Self {
            paths,
            config,
            config_revision: 1,
            started_at: Instant::now(),
            dictation_state: DictationState::Idle,
            session_id: None,
            frontend_kind: None,
            frontend_state: FrontendState::NotInstalled,
            frontend_capabilities: Vec::new(),
            frontend_surrounding_tail: None,
            recognizer: Box::new(MockRecognizer::default()),
            recognizer_backend: None,
            correction_ledger: InjectionLedger::default(),
            correction_classifier: RuleIntentClassifier,
            correction_gate: SafetyGate,
            correction_history: CorrectionHistory::default(),
            correction_operation_counter: 0,
            downloads: DownloadManager::default(),
            event_sender: None,
        }
    }

    pub fn set_mock_script(&mut self, script: Vec<AsrEvent>) {
        self.config.asr.backend = crate::config::AsrBackend::Mock;
        self.recognizer = Box::new(MockRecognizer::with_script(script));
        self.recognizer_backend = Some(crate::config::AsrBackend::Mock);
    }

    /// Rebuilds the recognizer when the configured backend changed (D-22).
    fn ensure_recognizer(&mut self) -> anyhow::Result<()> {
        let desired = self.config.asr.backend;
        if self.recognizer_backend == Some(desired) {
            return Ok(());
        }
        self.recognizer = crate::backend::build_recognizer(&self.config, &self.paths)?;
        self.recognizer_backend = Some(desired);
        Ok(())
    }

    pub fn status_snapshot(&self) -> StatusSnapshot {
        StatusSnapshot {
            core: CoreInfo {
                version: env!("CARGO_PKG_VERSION").to_string(),
                state: "running".to_string(),
                uptime_ms: self.started_at.elapsed().as_millis(),
            },
            dictation: DictationInfo {
                state: self.dictation_state.clone(),
                session_id: self.session_id.clone(),
            },
            frontend: FrontendInfo {
                kind: self.frontend_kind.clone(),
                state: self.frontend_state.clone(),
                capabilities: self.frontend_capabilities.clone(),
            },
            audio: self.audio_info(),
            models: ModelInfo {
                asr_backend: Some(
                    crate::backend::backend_label(self.config.asr.backend).to_string(),
                ),
                active_asr: self.config.models.active_asr.clone(),
                active_refiner: self.config.models.active_refiner.clone(),
                intent_classifier: IntentClassifierInfo {
                    state: "not_loaded".to_string(),
                    version: None,
                },
            },
            paths: PathInfo {
                home: self.paths.home.display().to_string(),
                logs: self.paths.logs.display().to_string(),
                models: self.paths.models.display().to_string(),
                cache: self.paths.cache.display().to_string(),
            },
            config_revision: self.config_revision,
        }
    }

    pub fn handle_command(&mut self, request: Envelope) -> CommandOutcome {
        if request.kind != MessageKind::Command {
            return self.error(
                request,
                "core.unknown_command",
                "message is not a command",
                true,
                json!({}),
            );
        }
        match request.name.as_str() {
            "core.hello" => self.core_hello(request),
            "core.status" => self.respond_json(request, self.status_snapshot()),
            "core.subscribe" => self.respond(request, json!({ "accepted": true })),
            "config.get" => self.respond(
                request,
                json!({ "config": self.config, "config_revision": self.config_revision }),
            ),
            "config.update" => self.config_update(request),
            "model.list" => self.model_list(request),
            "model.activate" => self.model_activate(request),
            "model.delete" => self.model_delete(request),
            "model.import" => self.model_import(request),
            "model.verify" => self.model_verify(request),
            "model.download" | "model.resume" => self.model_download(request),
            "model.pause" => self.model_pause(request),
            "model.cancel" => self.model_cancel(request),
            "audio.list_devices" => self.audio_list_devices(request),
            "correction.list_recent" => self.correction_list_recent(request),
            "dictation.start" => self.dictation_start(request),
            "dictation.stop" => self.dictation_stop(request),
            "frontend.register" => self.frontend_register(request),
            "frontend.report" => self.frontend_report(request),
            "diagnostics.run" => {
                self.respond(request, json!({ "checks": run_diagnostics(&self.paths) }))
            }
            "core.shutdown" => {
                let event = Envelope::event(
                    "core.state_changed",
                    json!({ "state": "stopping", "reason": "core.shutdown" }),
                );
                let mut outcome = self.respond(request, json!({ "accepted": true }));
                outcome.events.push(event);
                outcome.shutdown = true;
                outcome
            }
            _ => self.error(
                request,
                "core.unknown_command",
                "unknown command",
                true,
                json!({}),
            ),
        }
    }

    fn core_hello(&self, request: Envelope) -> CommandOutcome {
        let versions = request
            .payload
            .get("proto_versions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let supported = versions
            .iter()
            .any(|value| value.as_u64() == Some(PROTOCOL_VERSION as u64));
        if !supported {
            return self.error(
                request,
                "core.proto_unsupported",
                "protocol version is not supported",
                false,
                json!({ "supported": [PROTOCOL_VERSION] }),
            );
        }
        self.respond(
            request,
            json!({
                "selected_version": PROTOCOL_VERSION,
                "core_version": env!("CARGO_PKG_VERSION"),
                "server": "voxflow-core"
            }),
        )
    }

    fn audio_info(&self) -> AudioInfo {
        let inventory = list_input_devices();
        let device = inventory
            .default_device_id
            .as_deref()
            .and_then(|id| inventory.devices.iter().find(|device| device.id == id))
            .or_else(|| inventory.devices.first());
        AudioInfo {
            device_id: device.map(|device| device.id.clone()),
            label: device.map(|device| device.label.clone()),
            available: device.map(|device| device.available).unwrap_or(false),
            bluetooth_profile: device.and_then(|device| device.bluetooth_profile.clone()),
        }
    }

    fn audio_list_devices(&self, request: Envelope) -> CommandOutcome {
        let inventory = list_input_devices();
        self.respond(
            request,
            json!({
                "devices": inventory.devices,
                "default_device_id": inventory.default_device_id,
                "warnings": inventory.warnings,
                "probe": inventory.probe,
            }),
        )
    }

    fn config_update(&mut self, request: Envelope) -> CommandOutcome {
        let patch = request.payload.get("patch").cloned().unwrap_or(Value::Null);
        let previous = self.config.clone();
        match self.config.apply_json_patch(patch) {
            Ok(()) => {
                if let Err(error) = self.config.save(&self.paths.config) {
                    self.config = previous;
                    return self.error(
                        request,
                        "config.invalid",
                        format!("failed to save config: {error}"),
                        true,
                        json!({}),
                    );
                }
                self.config_revision += 1;
                let mut outcome =
                    self.respond(request, json!({ "config_revision": self.config_revision }));
                outcome.events.push(Envelope::event(
                    "config.changed",
                    json!({ "config_revision": self.config_revision }),
                ));
                outcome
            }
            Err(error) => {
                self.config = previous;
                self.error(
                    request,
                    "config.invalid",
                    format!("invalid config patch: {error}"),
                    true,
                    json!({}),
                )
            }
        }
    }

    fn model_list(&self, request: Envelope) -> CommandOutcome {
        match list_model_inventory(&self.paths, &self.config.models.active_asr) {
            Ok(models) => self.respond(request, json!({ "models": models })),
            Err(error) => self.error(
                request,
                "model.profile_unavailable",
                format!("failed to load model profiles: {error}"),
                true,
                json!({}),
            ),
        }
    }

    fn model_verify(&self, request: Envelope) -> CommandOutcome {
        let Some(model_id) = request
            .payload
            .get("model_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        else {
            return self.error(
                request,
                "config.invalid",
                "model_id is required",
                true,
                json!({ "field": "model_id" }),
            );
        };
        match verify_model_by_id(
            &default_profile_dir(),
            &self.paths,
            &self.config.models.active_asr,
            &model_id,
        ) {
            Ok(model) => self.respond(request, json!({ "model": model })),
            Err(error) if error.to_string().starts_with("model.not_found:") => self.error(
                request,
                "model.not_found",
                format!("unknown model_id: {model_id}"),
                true,
                json!({ "model_id": model_id }),
            ),
            Err(error) => self.error(
                request,
                "model.profile_unavailable",
                format!("failed to verify model: {error}"),
                true,
                json!({ "model_id": model_id }),
            ),
        }
    }

    fn model_import(&self, request: Envelope) -> CommandOutcome {
        let Some(model_id) = request
            .payload
            .get("model_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        else {
            return self.error(
                request,
                "config.invalid",
                "model_id is required",
                true,
                json!({ "field": "model_id" }),
            );
        };
        let Some(path) = request
            .payload
            .get("path")
            .and_then(Value::as_str)
            .map(std::path::PathBuf::from)
        else {
            return self.error(
                request,
                "config.invalid",
                "path is required",
                true,
                json!({ "field": "path" }),
            );
        };
        let mode_value = request
            .payload
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("copy")
            .to_string();
        let mode = match mode_value.as_str() {
            "copy" => ModelImportMode::Copy,
            "symlink" => ModelImportMode::Symlink,
            other => {
                return self.error(
                    request,
                    "config.invalid",
                    format!("unsupported import mode: {other}"),
                    true,
                    json!({ "field": "mode" }),
                );
            }
        };

        match import_model_by_id(
            &default_profile_dir(),
            &self.paths,
            &self.config.models.active_asr,
            &model_id,
            &path,
            mode,
        ) {
            Ok(result) => self.respond(
                request,
                json!({
                    "task_id": format!("import-{}-{}", model_id, result.manifest.installed_at_unix),
                    "import": result
                }),
            ),
            Err(error) => self.model_error(request, &model_id, error),
        }
    }

    fn model_activate(&mut self, request: Envelope) -> CommandOutcome {
        let Some(model_id) = request
            .payload
            .get("model_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        else {
            return self.error(
                request,
                "config.invalid",
                "model_id is required",
                true,
                json!({ "field": "model_id" }),
            );
        };
        let previous_active_asr = self.config.models.active_asr.clone();
        let model = match ensure_model_ready_by_id(
            &default_profile_dir(),
            &self.paths,
            &previous_active_asr,
            &model_id,
        ) {
            Ok(model) => model,
            Err(error) => return self.model_error(request, &model_id, error),
        };

        if previous_active_asr == model_id {
            return self.respond(
                request,
                json!({
                    "previous_active_asr": previous_active_asr,
                    "active_asr": model_id,
                    "runtime_smoke": "pending_runtime_integration",
                    "model": model
                }),
            );
        }

        let previous_config = self.config.clone();
        self.config.models.active_asr = model_id.clone();
        if let Err(error) = self.config.save(&self.paths.config) {
            self.config = previous_config;
            return self.error(
                request,
                "model.activate_failed",
                format!("failed to persist active model: {error}"),
                true,
                json!({ "model_id": model_id }),
            );
        }
        self.config_revision += 1;
        let active_model = verify_model_by_id(
            &default_profile_dir(),
            &self.paths,
            &self.config.models.active_asr,
            &model_id,
        )
        .unwrap_or(model);
        let mut outcome = self.respond(
            request,
            json!({
                "previous_active_asr": previous_active_asr,
                "active_asr": self.config.models.active_asr,
                "runtime_smoke": "pending_runtime_integration",
                "model": active_model
            }),
        );
        outcome.events.push(Envelope::event(
            "config.changed",
            json!({ "config_revision": self.config_revision }),
        ));
        outcome.events.push(Envelope::event(
            "model.state_changed",
            json!({
                "model_id": self.config.models.active_asr,
                "state": "active",
                "previous_active_asr": previous_active_asr
            }),
        ));
        outcome
    }

    fn model_delete(&self, request: Envelope) -> CommandOutcome {
        let Some(model_id) = request
            .payload
            .get("model_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        else {
            return self.error(
                request,
                "config.invalid",
                "model_id is required",
                true,
                json!({ "field": "model_id" }),
            );
        };
        match delete_model_by_id(
            &default_profile_dir(),
            &self.paths,
            &self.config.models.active_asr,
            &model_id,
        ) {
            Ok(result) => {
                let mut outcome = self.respond(request, json!({ "delete": result }));
                outcome.events.push(Envelope::event(
                    "model.state_changed",
                    json!({
                        "model_id": model_id,
                        "state": "not_installed"
                    }),
                ));
                outcome
            }
            Err(error) => self.model_error(request, &model_id, error),
        }
    }

    fn correction_list_recent(&self, request: Envelope) -> CommandOutcome {
        self.respond(
            request,
            json!({
                "records": self.correction_history.recent()
            }),
        )
    }

    fn dictation_start(&mut self, request: Envelope) -> CommandOutcome {
        if let Err(error) = self.ensure_recognizer() {
            self.dictation_state = DictationState::Error;
            return self.error(
                request,
                "dictation.model_unavailable",
                format!("asr backend unavailable: {error}"),
                true,
                json!({ "backend": crate::backend::backend_label(self.config.asr.backend) }),
            );
        }
        self.dictation_state = DictationState::Listening;
        let session = match self.recognizer.start_session() {
            Ok(session) => session,
            Err(error) => {
                self.dictation_state = DictationState::Error;
                return self.error(
                    request,
                    "dictation.model_unavailable",
                    format!("recognizer failed to start session: {error}"),
                    true,
                    json!({}),
                );
            }
        };
        self.session_id = Some(session.clone());
        let asr_events = self.recognizer.poll_events(&session).unwrap_or_default();
        let mut outcome = self.respond(request, json!({ "session_id": session }));
        outcome.events.push(Envelope::event(
            "dictation.state_changed",
            json!({ "state": "listening", "session_id": self.session_id.clone() }),
        ));
        for event in asr_events {
            outcome
                .events
                .extend(self.project_asr_event(&session, event));
        }
        outcome
    }

    fn dictation_stop(&mut self, request: Envelope) -> CommandOutcome {
        let session = self.session_id.take();
        self.dictation_state = DictationState::Idle;
        let mut outcome = self.respond(request, json!({ "session_id": session }));
        outcome.events.push(Envelope::event(
            "dictation.state_changed",
            json!({ "state": "idle", "session_id": null }),
        ));
        outcome
    }

    fn frontend_register(&mut self, request: Envelope) -> CommandOutcome {
        let requested_kind = request
            .payload
            .get("kind")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let kind = match requested_kind.as_deref() {
            Some(kind) if is_supported_frontend_kind(kind) => kind.to_string(),
            Some(kind) => {
                return self.error(
                    request,
                    "frontend.capability_missing",
                    format!("unsupported frontend kind: {kind}"),
                    true,
                    json!({ "field": "kind" }),
                );
            }
            None => {
                return self.error(
                    request,
                    "config.invalid",
                    "frontend kind is required",
                    true,
                    json!({ "field": "kind" }),
                );
            }
        };
        let capabilities = request
            .payload
            .get("capabilities")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|capability| is_supported_frontend_capability(capability))
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        self.frontend_kind = Some(kind.clone());
        self.frontend_state = FrontendState::Connected;
        self.frontend_capabilities = capabilities.clone();

        let mut outcome = self.respond(
            request,
            json!({
                "accepted": true,
                "kind": kind,
                "capabilities": capabilities,
            }),
        );
        outcome.events.push(Envelope::event(
            "frontend.state_changed",
            json!({
                "kind": self.frontend_kind,
                "state": self.frontend_state,
                "capabilities": self.frontend_capabilities,
            }),
        ));
        outcome
    }

    fn frontend_report(&mut self, request: Envelope) -> CommandOutcome {
        let event = request
            .payload
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        match event.as_str() {
            "activated" | "focused" => self.frontend_state = FrontendState::Active,
            "deactivated" | "blurred" => self.frontend_state = FrontendState::Connected,
            "capabilities" => {
                self.frontend_capabilities = request
                    .payload
                    .get("capabilities")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .filter(|capability| is_supported_frontend_capability(capability))
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
            }
            "surrounding_text_changed" => {
                self.frontend_surrounding_tail = request
                    .payload
                    .get("before_cursor_tail")
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
            }
            _ => {
                return self.error(
                    request,
                    "core.unknown_command",
                    "unknown frontend report event",
                    true,
                    json!({ "event": event }),
                );
            }
        }

        let mut outcome = self.respond(request, json!({ "accepted": true }));
        outcome.events.push(Envelope::event(
            "frontend.state_changed",
            json!({
                "kind": self.frontend_kind,
                "state": self.frontend_state,
                "capabilities": self.frontend_capabilities,
            }),
        ));
        outcome
    }

    fn request_model_id(&self, request: &Envelope) -> Option<String> {
        request
            .payload
            .get("model_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
    }

    fn model_download(&mut self, request: Envelope) -> CommandOutcome {
        let Some(model_id) = self.request_model_id(&request) else {
            return self.error(
                request,
                "config.invalid",
                "model_id is required",
                true,
                json!({ "field": "model_id" }),
            );
        };
        let profiles = match load_profiles(&default_profile_dir()) {
            Ok(profiles) => profiles,
            Err(error) => {
                return self.error(
                    request,
                    "model.profile_unavailable",
                    format!("failed to load model profiles: {error}"),
                    true,
                    json!({}),
                )
            }
        };
        let Some(profile) = profiles
            .into_iter()
            .find(|profile| profile.profile.id == model_id)
        else {
            return self.error(
                request,
                "model.not_found",
                format!("unknown model_id: {model_id}"),
                true,
                json!({ "model_id": model_id }),
            );
        };
        if !profile.source.url.starts_with("https://")
            || profile.source.url.contains("example.invalid")
        {
            return self.error(
                request,
                "model.source_unreachable",
                "profile download source is a placeholder or not https",
                true,
                json!({ "model_id": model_id, "url": profile.source.url }),
            );
        }
        if self.paths.models.join(&model_id).exists() {
            return self.error(
                request,
                "model.already_installed",
                format!("model {model_id} is already installed"),
                true,
                json!({ "model_id": model_id }),
            );
        }
        let sender = self
            .event_sender
            .get_or_insert_with(|| tokio::sync::broadcast::channel(64).0)
            .clone();
        match self
            .downloads
            .start(profile, self.paths.models.clone(), sender)
        {
            Ok(task_id) => self.respond(request, json!({ "task_id": task_id })),
            Err(error) if error.to_string().starts_with("core.busy") => self.error(
                request,
                "core.busy",
                format!("download already running for {model_id}"),
                true,
                json!({ "model_id": model_id }),
            ),
            Err(error) => self.error(
                request,
                "model.download_failed",
                format!("failed to start download: {error}"),
                true,
                json!({ "model_id": model_id }),
            ),
        }
    }

    fn model_pause(&mut self, request: Envelope) -> CommandOutcome {
        let Some(model_id) = self.request_model_id(&request) else {
            return self.error(
                request,
                "config.invalid",
                "model_id is required",
                true,
                json!({ "field": "model_id" }),
            );
        };
        match self.downloads.pause(&model_id) {
            Ok(task_id) => self.respond(request, json!({ "task_id": task_id })),
            Err(_) => self.error(
                request,
                "model.not_found",
                format!("no active download for {model_id}"),
                true,
                json!({ "model_id": model_id }),
            ),
        }
    }

    fn model_cancel(&mut self, request: Envelope) -> CommandOutcome {
        let Some(model_id) = self.request_model_id(&request) else {
            return self.error(
                request,
                "config.invalid",
                "model_id is required",
                true,
                json!({ "field": "model_id" }),
            );
        };
        match self.downloads.cancel(&model_id, &self.paths.models) {
            Ok(task_id) => self.respond(request, json!({ "task_id": task_id })),
            Err(error) => self.error(
                request,
                "model.download_failed",
                format!("failed to cancel download: {error}"),
                true,
                json!({ "model_id": model_id }),
            ),
        }
    }

    fn respond_json<T: serde::Serialize>(&self, request: Envelope, payload: T) -> CommandOutcome {
        self.respond(
            request,
            serde_json::to_value(payload).unwrap_or_else(|_| json!({})),
        )
    }

    fn respond(&self, request: Envelope, payload: Value) -> CommandOutcome {
        CommandOutcome {
            response: Envelope::response(request.id, request.name, payload),
            events: Vec::new(),
            shutdown: false,
        }
    }

    fn error(
        &self,
        request: Envelope,
        code: impl Into<String>,
        message: impl Into<String>,
        recoverable: bool,
        details: Value,
    ) -> CommandOutcome {
        CommandOutcome {
            response: Envelope::error(
                request.id,
                request.name,
                code,
                message,
                recoverable,
                details,
            ),
            events: Vec::new(),
            shutdown: false,
        }
    }

    fn model_error(
        &self,
        request: Envelope,
        model_id: &str,
        error: anyhow::Error,
    ) -> CommandOutcome {
        let message = error.to_string();
        let code = model_error_code(&message);
        self.error(
            request,
            code,
            message,
            true,
            json!({ "model_id": model_id }),
        )
    }

    fn project_asr_event(&mut self, session_id: &str, event: AsrEvent) -> Vec<Envelope> {
        match event {
            AsrEvent::Final {
                revision,
                text,
                segment_id,
            } => self.project_final_event(session_id, revision, text, segment_id),
            other => vec![asr_to_ipc_event(session_id, other)],
        }
    }

    fn project_final_event(
        &mut self,
        session_id: &str,
        revision: u64,
        text: String,
        segment_id: String,
    ) -> Vec<Envelope> {
        let candidate = self.correction_classifier.classify(&text);
        if matches!(
            candidate.intent,
            CorrectionIntent::Literal | CorrectionIntent::Uncertain
        ) {
            self.append_ledger_segment(
                session_id,
                &segment_id,
                &text,
                revision,
                SegmentSource::AsrStable,
            );
            return vec![dictation_final_event(
                session_id,
                revision,
                &text,
                &segment_id,
            )];
        }

        let context = SafetyGateContext {
            correction_enabled: self.config.correction.enabled,
            threshold_mode: self.config.correction.threshold_mode.clone(),
            surrounding_text: self.frontend_surrounding_tail.clone(),
            delete_supported: self
                .frontend_capabilities
                .iter()
                .any(|capability| capability == "delete_surrounding"),
            record_writable: true,
        };
        let decision = self
            .correction_gate
            .evaluate(&self.correction_ledger, candidate, &context);
        let operation_id = self.next_correction_operation_id();
        self.correction_history
            .push_decision(operation_id.clone(), &decision);

        if decision.applied {
            self.apply_correction_side_effects(&operation_id, session_id, &decision.action);
            return vec![correction_applied_event(operation_id, &decision)];
        }

        self.append_ledger_segment(
            session_id,
            &segment_id,
            &text,
            revision,
            SegmentSource::AsrStable,
        );
        vec![
            correction_rejected_event(operation_id, &decision),
            dictation_final_event(session_id, revision, &text, &segment_id),
        ]
    }

    fn append_ledger_segment(
        &mut self,
        session_id: &str,
        segment_id: &str,
        text: &str,
        token_end: u64,
        source: SegmentSource,
    ) {
        self.correction_ledger.append(LedgerSegment {
            id: segment_id.to_string(),
            session_id: session_id.to_string(),
            committed_text: text.to_string(),
            normalized_text: String::new(),
            token_start: 0,
            token_end: token_end as usize,
            source,
            committed_at_ms: self.started_at.elapsed().as_millis() as u64,
            cursor_context_hash: cursor_context_hash(text),
            frozen: false,
        });
    }

    fn apply_correction_side_effects(
        &mut self,
        operation_id: &str,
        session_id: &str,
        action: &CorrectionAction,
    ) {
        match action {
            CorrectionAction::Literal => {}
            CorrectionAction::Delete { segment_id, text } => {
                self.correction_ledger.freeze_segment(segment_id);
                self.replace_surrounding_suffix(text, "");
            }
            CorrectionAction::Replace {
                segment_id,
                segment_text,
                replacement_text,
                ..
            } => {
                self.correction_ledger.freeze_segment(segment_id);
                self.replace_surrounding_suffix(segment_text, replacement_text);
                if !replacement_text.is_empty() {
                    self.append_ledger_segment(
                        session_id,
                        &format!("{operation_id}-segment"),
                        replacement_text,
                        replacement_text.chars().count() as u64,
                        SegmentSource::Correction,
                    );
                }
            }
        }
    }

    fn replace_surrounding_suffix(&mut self, old_suffix: &str, new_suffix: &str) {
        let Some(current) = self.frontend_surrounding_tail.clone() else {
            return;
        };
        if let Some(prefix) = current.strip_suffix(old_suffix) {
            self.frontend_surrounding_tail = Some(format!("{prefix}{new_suffix}"));
        }
    }

    fn next_correction_operation_id(&mut self) -> String {
        self.correction_operation_counter += 1;
        format!("op-{}", self.correction_operation_counter)
    }
}

fn model_error_code(message: &str) -> &'static str {
    let Some((code, _detail)) = message.split_once(':') else {
        return "model.profile_unavailable";
    };
    match code {
        "model.not_found" => "model.not_found",
        "model.profile_invalid" => "model.profile_invalid",
        "model.import_source_invalid" => "model.import_source_invalid",
        "model.import_verify_failed" => "model.import_verify_failed",
        "model.already_installed" => "model.already_installed",
        "model.symlink_unsupported" => "model.symlink_unsupported",
        "model.not_ready" => "model.not_ready",
        "model.active_locked" => "model.active_locked",
        _ => "model.profile_unavailable",
    }
}

fn asr_to_ipc_event(session_id: &str, event: AsrEvent) -> Envelope {
    match event {
        AsrEvent::Partial {
            revision,
            text,
            tokens,
        } => Envelope::event(
            "dictation.partial",
            json!({ "session_id": session_id, "revision": revision, "text": text, "tokens": tokens }),
        ),
        AsrEvent::Stable {
            revision,
            text,
            token_start,
            token_end,
        } => Envelope::event(
            "dictation.stable",
            json!({
                "session_id": session_id,
                "segment_id": format!("seg-stable-{revision}"),
                "revision": revision,
                "text": text,
                "token_range": [token_start, token_end]
            }),
        ),
        AsrEvent::Final {
            revision,
            text,
            segment_id,
        } => Envelope::event(
            "dictation.final",
            json!({ "session_id": session_id, "segment_id": segment_id, "revision": revision, "text": text, "refined": false }),
        ),
    }
}

fn dictation_final_event(
    session_id: &str,
    revision: u64,
    text: &str,
    segment_id: &str,
) -> Envelope {
    Envelope::event(
        "dictation.final",
        json!({ "session_id": session_id, "segment_id": segment_id, "revision": revision, "text": text, "refined": false }),
    )
}

fn correction_applied_event(operation_id: String, decision: &CorrectionDecision) -> Envelope {
    Envelope::event(
        "correction.applied",
        json!({
            "operation_id": operation_id,
            "intent": decision.candidate.intent,
            "target": decision.candidate.target_hint,
            "replacement": decision.candidate.replacement_hint,
            "segments": correction_segments(&decision.action),
            "confidence": decision.candidate.confidence,
            "reason_code": decision.reason_code,
            "gate_checks": decision.gate_checks,
            "input_events": decision.to_input_events(),
        }),
    )
}

fn correction_rejected_event(operation_id: String, decision: &CorrectionDecision) -> Envelope {
    Envelope::event(
        "correction.rejected",
        json!({
            "operation_id": operation_id,
            "intent": decision.candidate.intent,
            "target": decision.candidate.target_hint,
            "replacement": decision.candidate.replacement_hint,
            "confidence": decision.candidate.confidence,
            "reason_code": decision.reason_code,
            "gate_checks": decision.gate_checks,
        }),
    )
}

fn correction_segments(action: &CorrectionAction) -> Vec<String> {
    match action {
        CorrectionAction::Literal => Vec::new(),
        CorrectionAction::Delete { segment_id, .. }
        | CorrectionAction::Replace { segment_id, .. } => vec![segment_id.clone()],
    }
}

fn is_supported_frontend_kind(kind: &str) -> bool {
    matches!(kind, "ibus" | "fcitx5" | "compatibility")
}

fn is_supported_frontend_capability(capability: &str) -> bool {
    matches!(
        capability,
        "preedit" | "surrounding_text" | "delete_surrounding"
    )
}
