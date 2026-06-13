//! VoxFlow Control Center — Tauri 2 shell over the core IPC bridge.
//!
//! One pump task owns the `ShellIpcSession`: it forwards core events to the
//! WebView on the three fixed channels (frontend/tauri-ui.md) and serves
//! `core_command` invocations sent through an mpsc channel. Reconnects with
//! backoff when the core socket drops.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use std::borrow::Borrow;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, oneshot};
use voxflow_control::bridge::ReconnectPolicy;
use voxflow_control::shell::{
    disconnected_retry_event, CoreCommandInvocation, ShellEvent, ShellEventSink, ShellIpcSession,
    DEFAULT_UI_SUBSCRIPTIONS,
};

/// Last payload per status channel, replayed via `resync` for WebViews that
/// mount after the pump already connected (startup race).
type EventCache = Arc<std::sync::Mutex<std::collections::HashMap<String, Value>>>;

/// Forwards shell events to the WebView, caching connection/snapshot state.
struct AppSink {
    app: AppHandle,
    cache: EventCache,
}

impl AppSink {
    fn new(app: &AppHandle) -> Self {
        Self {
            app: app.clone(),
            cache: app.state::<CachedEvents>().0.clone(),
        }
    }
}

fn set_tray_tooltip(app: &AppHandle, text: &str) {
    if let Some(tray) = app.tray_by_id("voxflow") {
        let _ = tray.set_tooltip(Some(text));
    }
}

impl ShellEventSink for AppSink {
    fn emit(&mut self, event: ShellEvent) {
        // 托盘标注引擎可用性(所有者要求:加载中需明确标注)。
        if event.name == voxflow_control::shell::TAURI_CORE_EVENT {
            let inner = &event.payload;
            if inner["name"] == "core.notice" && inner["payload"]["code"] == "asr.engine_ready" {
                set_tray_tooltip(&self.app, "VoxFlow — 引擎就绪,按 Alt+S 听写");
            }
        } else if event.name == voxflow_control::shell::TAURI_CONNECTION_EVENT {
            if event.payload["state"] == "connected" {
                set_tray_tooltip(&self.app, "VoxFlow — 引擎加载中…(此期间听写不可用)");
            } else {
                set_tray_tooltip(&self.app, "VoxFlow — Core 未连接");
            }
        }
        if event.name != voxflow_control::shell::TAURI_CORE_EVENT {
            self.cache
                .lock()
                .expect("event cache lock")
                .insert(event.name.clone(), event.payload.clone());
        }
        if let Err(error) = Emitter::emit(&self.app, event.name.as_str(), event.payload) {
            tracing::warn!(%error, channel = event.name, "failed to emit shell event");
        }
    }
}

struct CachedEvents(EventCache);

/// Replays the last connection/snapshot events; the frontend calls this once
/// its listeners are registered.
#[tauri::command]
fn resync(app: AppHandle, state: tauri::State<'_, CachedEvents>) {
    for (name, payload) in state.0.lock().expect("event cache lock").iter() {
        let _ = Emitter::emit(&app, name.as_str(), payload.clone());
    }
}

type CommandRequest = (
    CoreCommandInvocation,
    oneshot::Sender<Result<Value, String>>,
);

struct CommandQueue(mpsc::Sender<CommandRequest>);

fn core_socket_path() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("voxflow").join("core.sock")
}

#[tauri::command]
async fn core_command(
    state: tauri::State<'_, CommandQueue>,
    invocation: CoreCommandInvocation,
) -> Result<Value, String> {
    let (reply_tx, reply_rx) = oneshot::channel();
    state
        .0
        .send((invocation, reply_tx))
        .await
        .map_err(|_| "core.disconnected: bridge task stopped".to_string())?;
    reply_rx
        .await
        .map_err(|_| "core.disconnected: no reply from bridge task".to_string())?
}

async fn connection_pump(app: AppHandle, mut commands: mpsc::Receiver<CommandRequest>) {
    let policy = ReconnectPolicy::default();
    let mut attempt: u32 = 0;
    loop {
        let mut sink = AppSink::new(&app);
        let socket = core_socket_path();
        match ShellIpcSession::connect(&socket, DEFAULT_UI_SUBSCRIPTIONS, &mut sink).await {
            Ok(mut session) => {
                attempt = 0;
                pump_session(&app, &mut session, &mut commands).await;
            }
            Err(error) => {
                attempt = attempt.saturating_add(1);
                sink.emit(disconnected_retry_event(error.to_string(), attempt, policy));
            }
        }
        // Refuse queued commands while down so the UI fails fast.
        while let Ok((_, reply)) = commands.try_recv() {
            let _ = reply.send(Err("core.disconnected: core is not running".to_string()));
        }
        tokio::time::sleep(policy.delay_for_attempt(attempt.max(1))).await;
    }
}

async fn pump_session(
    app: &AppHandle,
    session: &mut ShellIpcSession,
    commands: &mut mpsc::Receiver<CommandRequest>,
) {
    let mut sink = AppSink::new(app);
    loop {
        while let Ok((invocation, reply)) = commands.try_recv() {
            let result = session
                .invoke_core_command(invocation, &mut sink)
                .await
                .map_err(|error| format!("core.disconnected: {error}"))
                .and_then(|envelope| {
                    serde_json::to_value(envelope).map_err(|error| error.to_string())
                });
            let failed = result.is_err();
            let _ = reply.send(result);
            if failed {
                return;
            }
        }
        // next_line() is cancellation-safe, so bounding the poll lets queued
        // commands interleave with the event stream on a single session.
        match tokio::time::timeout(
            Duration::from_millis(100),
            session.poll_core_event_once(&mut sink),
        )
        .await
        {
            Ok(Ok(true)) => {}
            Ok(Ok(false)) | Ok(Err(_)) => return,
            Err(_timeout) => {}
        }
    }
}

/// Sends a core command from tray handlers without waiting for the reply.
fn tray_core_command(queue: &mpsc::Sender<CommandRequest>, name: &str, payload: Value) {
    let (reply_tx, _reply_rx) = oneshot::channel();
    let invocation = CoreCommandInvocation {
        name: name.to_string(),
        payload,
    };
    let _ = queue.try_send((invocation, reply_tx));
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Set while a real quit is in progress, so the close-to-tray handler lets the
/// process actually exit instead of hiding the window (the previous behaviour
/// left a zombie holding the single-instance lock → relaunch focused the dead
/// instance).
static QUITTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Stops dictation and terminates the app for good.
fn quit_app(app: &AppHandle) {
    QUITTING.store(true, std::sync::atomic::Ordering::SeqCst);
    tray_core_command(
        app.state::<CommandQueue>().0.clone().borrow(),
        "dictation.stop",
        serde_json::json!({}),
    );
    // Give the stop a beat to flush, then exit hard so no thread keeps the
    // process (and the single-instance lock) alive.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        handle.exit(0);
        std::process::exit(0);
    });
}

#[tauri::command]
fn quit_app_command(app: AppHandle) {
    quit_app(&app);
}

/// Restarts the control-center process (re-exec the same binary).
#[tauri::command]
fn restart_app_command(app: AppHandle) {
    QUITTING.store(true, std::sync::atomic::Ordering::SeqCst);
    let exe = std::env::current_exe();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        if let Ok(exe) = exe {
            let _ = std::process::Command::new(exe).spawn();
        }
        app.exit(0);
        std::process::exit(0);
    });
}

/// 右上角托盘:状态切换、后端/模型快速切换、打开控制台、退出。
fn setup_tray(app: &AppHandle, queue: mpsc::Sender<CommandRequest>) -> tauri::Result<()> {
    use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
    use tauri::tray::TrayIconBuilder;

    let toggle = MenuItem::with_id(
        app,
        "toggle-dictation",
        "开始 / 停止听写 (Alt+S)",
        true,
        None::<&str>,
    )?;
    let backend_qwen = CheckMenuItem::with_id(
        app,
        "backend-qwen3_vllm",
        "Qwen3-ASR(本地 GPU)",
        true,
        true,
        None::<&str>,
    )?;
    let backend_volcano = CheckMenuItem::with_id(
        app,
        "backend-volcano_api",
        "火山引擎 API(云端)",
        true,
        false,
        None::<&str>,
    )?;
    let backend_zipformer = CheckMenuItem::with_id(
        app,
        "backend-zipformer_local",
        "Zipformer(本地 CPU)",
        true,
        false,
        None::<&str>,
    )?;
    let backend_menu = Submenu::with_items(
        app,
        "识别后端",
        true,
        &[&backend_qwen, &backend_volcano, &backend_zipformer],
    )?;
    let model_bilingual = MenuItem::with_id(
        app,
        "model-streaming-zh-en-small",
        "流式中英双语(标准)",
        true,
        None::<&str>,
    )?;
    let model_zh = MenuItem::with_id(
        app,
        "model-streaming-zh-2025",
        "流式中文 2025",
        true,
        None::<&str>,
    )?;
    let model_xl = MenuItem::with_id(
        app,
        "model-streaming-zh-xlarge-2025",
        "流式中文 XLarge",
        true,
        None::<&str>,
    )?;
    let model_menu = Submenu::with_items(
        app,
        "Zipformer 模型(需已安装)",
        true,
        &[&model_bilingual, &model_zh, &model_xl],
    )?;
    let open_console = MenuItem::with_id(app, "open-console", "打开控制台", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 VoxFlow", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &PredefinedMenuItem::separator(app)?,
            &backend_menu,
            &model_menu,
            &PredefinedMenuItem::separator(app)?,
            &open_console,
            &quit,
        ],
    )?;

    let checks = [
        backend_qwen.clone(),
        backend_volcano.clone(),
        backend_zipformer.clone(),
    ];
    let dictating = Arc::new(std::sync::atomic::AtomicBool::new(false));

    TrayIconBuilder::with_id("voxflow")
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .tooltip("VoxFlow — 启动中…")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "toggle-dictation" => {
                    let now = !dictating.load(std::sync::atomic::Ordering::Relaxed);
                    dictating.store(now, std::sync::atomic::Ordering::Relaxed);
                    let command = if now {
                        "dictation.start"
                    } else {
                        "dictation.stop"
                    };
                    tray_core_command(
                        app.state::<CommandQueue>().0.clone().borrow(),
                        command,
                        serde_json::json!({}),
                    );
                }
                "open-console" => show_main_window(app),
                "quit" => quit_app(app),
                id if id.starts_with("backend-") => {
                    let backend = id.trim_start_matches("backend-");
                    tray_core_command(
                        app.state::<CommandQueue>().0.clone().borrow(),
                        "config.update",
                        serde_json::json!({ "patch": { "asr": { "backend": backend } } }),
                    );
                    for check in &checks {
                        let _ = check.set_checked(check.id().as_ref() == id);
                    }
                }
                id if id.starts_with("model-") => {
                    let model_id = id.trim_start_matches("model-");
                    tray_core_command(
                        app.state::<CommandQueue>().0.clone().borrow(),
                        "model.activate",
                        serde_json::json!({ "model_id": model_id }),
                    );
                }
                _ => {}
            }
        })
        .build(app)?;
    let _ = queue;
    Ok(())
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let (command_tx, command_rx) = mpsc::channel::<CommandRequest>(32);

    tauri::Builder::default()
        // Must be the first plugin: a second launch (app icon / launcher)
        // focuses the running window instead of starting another instance.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main_window(app);
        }))
        .manage(CommandQueue(command_tx))
        .manage(CachedEvents(Arc::new(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        ))))
        .invoke_handler(tauri::generate_handler![
            core_command,
            resync,
            quit_app_command,
            restart_app_command
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            setup_tray(&handle, app.state::<CommandQueue>().0.clone())?;
            tauri::async_runtime::spawn(connection_pump(handle, command_rx));
            Ok(())
        })
        .on_window_event(|window, event| {
            // 关窗 = 隐藏到托盘(托盘/界面「退出」才真正退出)。退出进行中则放行。
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if QUITTING.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running VoxFlow Control Center");
}
