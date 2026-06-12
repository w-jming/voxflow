use anyhow::{bail, Result};
use serde_json::json;
use voxflow_fcitx5::component::{addon_conf, inputmethod_conf, DEFAULT_ADDON_LIBRARY};
use voxflow_fcitx5::engine::Fcitx5EngineAdapter;
use voxflow_fcitx5::probe::probe_fcitx5;
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
        Some("addon-conf") => {
            let library = args
                .next()
                .unwrap_or_else(|| DEFAULT_ADDON_LIBRARY.to_string());
            print!("{}", addon_conf(&library));
            Ok(())
        }
        Some("inputmethod-conf") => {
            print!("{}", inputmethod_conf());
            Ok(())
        }
        Some("register-json") => {
            let command = frontend_register_command(
                "fcitx5-register-1",
                FrontendKind::Fcitx5,
                env!("CARGO_PKG_VERSION"),
                &FrontendCapabilities::full(),
            );
            println!("{}", serde_json::to_string(&command)?);
            Ok(())
        }
        Some("self-test") => self_test(),
        Some("probe") => {
            println!("{}", serde_json::to_string_pretty(&probe_fcitx5())?);
            Ok(())
        }
        Some(other) => bail!("unknown command: {other}"),
    }
}

fn self_test() -> Result<()> {
    let mut projector = DictationProjector::default();
    let mut adapter = Fcitx5EngineAdapter::new(FrontendCapabilities::full());
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
            "correction.applied",
            json!({
                "operation_id": "op-1",
                "input_events": [
                    { "type": "delete_before_cursor", "chars": 4 },
                    { "type": "commit", "text": "今天上午" }
                ]
            }),
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
        "voxflow-fcitx5 0.3.0\n\nUSAGE:\n  voxflow-fcitx5 addon-conf [library-name]\n  voxflow-fcitx5 inputmethod-conf\n  voxflow-fcitx5 register-json\n  voxflow-fcitx5 self-test\n  voxflow-fcitx5 probe\n"
    );
}
