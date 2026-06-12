use anyhow::{bail, Result};
use serde_json::json;
use voxflow_ibus::component::{component_xml, DEFAULT_ENGINE_EXEC};
use voxflow_ibus::core_client::{default_core_socket, run_mock_roundtrip, CoreEngineSession};
use voxflow_ibus::dbus::{register_factory_probe_once, run_engine_forever};
use voxflow_ibus::engine::IbusEngineAdapter;
use voxflow_ibus::zbus_engine::ZbusIbusEngine;
use voxflow_input::{
    frontend_register_command, DictationProjector, FrontendCapabilities, FrontendKind,
};
use voxflow_ipc::Envelope;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some("component-xml") => {
            let exec = args
                .next()
                .unwrap_or_else(|| DEFAULT_ENGINE_EXEC.to_string());
            print!("{}", component_xml(&exec));
            Ok(())
        }
        Some("register-json") => {
            let command = frontend_register_command(
                "ibus-register-1",
                FrontendKind::Ibus,
                env!("CARGO_PKG_VERSION"),
                &FrontendCapabilities::full(),
            );
            println!("{}", serde_json::to_string(&command)?);
            Ok(())
        }
        Some("self-test") => self_test(),
        Some("core-roundtrip") => {
            let socket = args
                .next()
                .map(Into::into)
                .unwrap_or(default_core_socket()?);
            for operation in run_mock_roundtrip(socket)? {
                println!("{}", serde_json::to_string(&operation)?);
            }
            Ok(())
        }
        Some("engine-focus-smoke") => {
            let socket = args
                .next()
                .map(Into::into)
                .unwrap_or(default_core_socket()?);
            let core_session = CoreEngineSession::connect(socket)?;
            let mut engine = ZbusIbusEngine::with_core_bridge(Box::new(core_session));
            engine.handle_focus_in().map_err(anyhow::Error::from)?;
            for operation in engine.drain_pending_operations() {
                println!("{}", serde_json::to_string(&operation)?);
            }
            engine.handle_focus_out().map_err(anyhow::Error::from)?;
            for operation in engine.drain_pending_operations() {
                println!("{}", serde_json::to_string(&operation)?);
            }
            Ok(())
        }
        Some("--ibus-engine") => {
            let mut probe_once = false;
            let mut socket = None;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--probe-once" => probe_once = true,
                    "--core-socket" => socket = args.next().map(Into::into),
                    other => bail!("unknown --ibus-engine option: {other}"),
                }
            }
            if probe_once {
                let socket = socket.unwrap_or(default_core_socket()?);
                let unique_name = register_factory_probe_once(socket)?;
                println!("registered org.freedesktop.IBus.Factory as {unique_name}");
                Ok(())
            } else {
                let socket = socket.unwrap_or(default_core_socket()?);
                run_engine_forever(socket)
            }
        }
        Some(other) => bail!("unknown command: {other}"),
    }
}

fn self_test() -> Result<()> {
    let mut projector = DictationProjector::default();
    let mut adapter = IbusEngineAdapter::new(FrontendCapabilities::full());
    let events = [
        Envelope::event(
            "dictation.state_changed",
            json!({ "state": "listening", "session_id": "s" }),
        ),
        Envelope::event(
            "dictation.partial",
            json!({ "session_id": "s", "revision": 1, "text": "今天下午" }),
        ),
        Envelope::event(
            "dictation.stable",
            json!({ "session_id": "s", "revision": 2, "text": "今天下午" }),
        ),
        Envelope::event(
            "dictation.final",
            json!({ "session_id": "s", "revision": 3, "text": "今天下午三点开会" }),
        ),
    ];
    for event in events {
        for input_event in projector.project(&event)? {
            for operation in adapter.translate(input_event) {
                println!("{}", serde_json::to_string(&operation)?);
            }
        }
    }
    Ok(())
}

fn print_help() {
    println!(
        "voxflow-ibus 0.3.0\n\nUSAGE:\n  voxflow-ibus component-xml [engine-exec]\n  voxflow-ibus register-json\n  voxflow-ibus self-test\n  voxflow-ibus core-roundtrip [core-socket]\n  voxflow-ibus engine-focus-smoke [core-socket]\n  voxflow-ibus --ibus-engine [--probe-once] [--core-socket path]\n"
    );
}
