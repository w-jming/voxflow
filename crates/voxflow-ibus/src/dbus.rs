use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use zbus::blocking::Connection;

use crate::core_client::IbusCoreBridge;
use crate::factory::{ZbusIbusFactory, IBUS_FACTORY_OBJECT_PATH};
use crate::zbus_engine::ZbusIbusEngine;

pub const IBUS_ENGINE_BUS_NAME: &str = "org.freedesktop.IBus.VoxFlow";
pub const IBUS_ENGINE_OBJECT_PATH: &str = "/org/freedesktop/IBus/Engine/VoxFlow";

pub fn register_engine_probe_once() -> Result<String> {
    let connection = register_engine(None)?;
    Ok(connection
        .unique_name()
        .map(ToString::to_string)
        .unwrap_or_else(|| "(unnamed)".to_string()))
}

pub fn register_factory_probe_once(core_socket: PathBuf) -> Result<String> {
    let connection = register_factory(core_socket)?;
    Ok(connection
        .unique_name()
        .map(ToString::to_string)
        .unwrap_or_else(|| "(unnamed)".to_string()))
}

pub fn run_engine_forever(core_socket: PathBuf) -> Result<()> {
    let connection = register_factory(core_socket.clone())?;
    spawn_core_event_pump(core_socket, connection.clone());
    // ibus 总线消失(ibus restart)后必须退出,否则僵尸进程占着引擎名,
    // 让 daemon 新拉起的实例 RequestName 失败。
    loop {
        thread::park_timeout(Duration::from_secs(10));
        let alive = zbus::blocking::fdo::DBusProxy::new(&connection)
            .ok()
            .and_then(|proxy| {
                proxy
                    .name_has_owner(IBUS_ENGINE_BUS_NAME.try_into().expect("valid name"))
                    .ok()
            })
            .unwrap_or(false);
        if !alive {
            tracing::info!("ibus bus gone; engine exiting for clean respawn");
            return Ok(());
        }
    }
}

/// Streams Core dictation events into IBus engine signals so real-time
/// partial/stable/final reach the focused application. Reconnects with a
/// short backoff while the Core daemon is down.
fn spawn_core_event_pump(core_socket: PathBuf, connection: Connection) {
    thread::spawn(move || loop {
        match crate::core_client::CoreEventPump::connect(core_socket.clone()) {
            Ok(mut pump) => {
                tracing::info!("ibus event pump connected to core");
                loop {
                    match pump.next_operations() {
                        Ok(operations) if operations.is_empty() => {}
                        Ok(operations) => {
                            let path = crate::factory::ACTIVE_ENGINE_PATH
                                .lock()
                                .expect("engine path lock")
                                .clone()
                                .unwrap_or_else(|| IBUS_ENGINE_OBJECT_PATH.to_string());
                            let ctxt = match zbus::object_server::SignalContext::new(
                                connection.inner(),
                                path.as_str(),
                            ) {
                                Ok(ctxt) => ctxt,
                                Err(error) => {
                                    tracing::warn!(%error, "build signal context");
                                    break;
                                }
                            };
                            if let Err(error) =
                                crate::zbus_engine::emit_operations_via(&ctxt, &operations)
                            {
                                tracing::warn!(%error, "emit ibus operations");
                            }
                        }
                        Err(error) => {
                            tracing::warn!(%error, "ibus event pump read failed; reconnecting");
                            break;
                        }
                    }
                }
            }
            Err(error) => {
                tracing::debug!(%error, "ibus event pump connect failed; retrying");
            }
        }
        thread::sleep(Duration::from_secs(2));
    });
}

/// ibus-daemon runs its own message bus; engines must register there, not on
/// the session bus (factories on the session bus make SetGlobalEngine time
/// out because CreateEngine never reaches them).
fn connect_ibus_bus() -> Result<Connection> {
    let output = std::process::Command::new("ibus")
        .arg("address")
        .output()
        .context("run `ibus address`")?;
    let address = String::from_utf8_lossy(&output.stdout).trim().to_string();
    anyhow::ensure!(!address.is_empty(), "`ibus address` returned empty");
    zbus::blocking::connection::Builder::address(address.as_str())
        .context("parse ibus bus address")?
        .build()
        .context("connect ibus private bus")
}

fn register_factory(core_socket: PathBuf) -> Result<Connection> {
    let connection = connect_ibus_bus()?;
    connection
        .request_name(IBUS_ENGINE_BUS_NAME)
        .with_context(|| format!("request D-Bus name {IBUS_ENGINE_BUS_NAME}"))?;
    connection
        .object_server()
        .at(IBUS_FACTORY_OBJECT_PATH, ZbusIbusFactory::new(core_socket))
        .with_context(|| format!("register IBus factory object at {IBUS_FACTORY_OBJECT_PATH}"))?;
    Ok(connection)
}

fn register_engine(core_bridge: Option<Box<dyn IbusCoreBridge>>) -> Result<Connection> {
    let connection = connect_ibus_bus()?;
    connection
        .request_name(IBUS_ENGINE_BUS_NAME)
        .with_context(|| format!("request D-Bus name {IBUS_ENGINE_BUS_NAME}"))?;
    let engine = match core_bridge {
        Some(core_bridge) => ZbusIbusEngine::with_core_bridge(core_bridge),
        None => ZbusIbusEngine::default(),
    };
    connection
        .object_server()
        .at(IBUS_ENGINE_OBJECT_PATH, engine)
        .with_context(|| format!("register IBus engine object at {IBUS_ENGINE_OBJECT_PATH}"))?;
    Ok(connection)
}
