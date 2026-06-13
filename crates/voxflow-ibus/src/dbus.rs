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
    loop {
        thread::park_timeout(Duration::from_secs(3600));
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
                            let ctxt = match zbus::object_server::SignalContext::new(
                                connection.inner(),
                                IBUS_ENGINE_OBJECT_PATH,
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

fn register_factory(core_socket: PathBuf) -> Result<Connection> {
    let connection = Connection::session().context("connect D-Bus session bus")?;
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
    let connection = Connection::session().context("connect D-Bus session bus")?;
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
