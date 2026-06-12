use std::env;
use std::path::Path;

use serde::{Deserialize, Serialize};
use voxflow_audio::probe_pipewire_runtime;

use crate::paths::VoxflowPaths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Warning,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticCheck {
    pub id: String,
    pub status: CheckStatus,
    pub summary: String,
    pub action_hint: Option<String>,
}

pub fn run_diagnostics(paths: &VoxflowPaths) -> Vec<DiagnosticCheck> {
    vec![
        path_check("paths.home", &paths.home, "VoxFlow home"),
        path_check(
            "paths.runtime",
            &paths.runtime_dir,
            "runtime socket directory",
        ),
        command_check("ibus", "IBus command"),
        ibus_component_check(),
        command_check("fcitx5", "Fcitx5 command"),
        command_check("pipewire", "PipeWire command"),
        command_check("wpctl", "WirePlumber wpctl command"),
        pipewire_runtime_check(),
        pipewire_development_check(),
    ]
}

fn path_check(id: &str, path: &Path, label: &str) -> DiagnosticCheck {
    if path.exists() {
        DiagnosticCheck {
            id: id.to_string(),
            status: CheckStatus::Pass,
            summary: format!("{label} exists: {}", redact_home(path)),
            action_hint: None,
        }
    } else {
        DiagnosticCheck {
            id: id.to_string(),
            status: CheckStatus::Warning,
            summary: format!("{label} does not exist yet: {}", redact_home(path)),
            action_hint: Some("start core once to create directories".to_string()),
        }
    }
}

fn command_check(command: &str, label: &str) -> DiagnosticCheck {
    if command_exists(command) {
        DiagnosticCheck {
            id: format!("command.{command}"),
            status: CheckStatus::Pass,
            summary: format!("{label} is available"),
            action_hint: None,
        }
    } else {
        DiagnosticCheck {
            id: format!("command.{command}"),
            status: CheckStatus::Warning,
            summary: format!("{label} is not found"),
            action_hint: Some(
                "install the matching desktop dependency before input frontend tests".to_string(),
            ),
        }
    }
}

fn command_exists(command: &str) -> bool {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .any(|dir| dir.join(command).is_file())
}

fn ibus_component_check() -> DiagnosticCheck {
    let mut locations = vec![Path::new("/usr/share/ibus/component/voxflow.xml").to_path_buf()];
    if let Some(home) = env::var_os("HOME") {
        locations.push(
            Path::new(&home)
                .join(".local")
                .join("share")
                .join("ibus")
                .join("component")
                .join("voxflow.xml"),
        );
    }
    if locations.iter().any(|path| path.exists()) {
        DiagnosticCheck {
            id: "ibus.component".to_string(),
            status: CheckStatus::Pass,
            summary: "VoxFlow IBus component file is installed".to_string(),
            action_hint: None,
        }
    } else {
        DiagnosticCheck {
            id: "ibus.component".to_string(),
            status: CheckStatus::Warning,
            summary: "VoxFlow IBus component file is not installed".to_string(),
            action_hint: Some(
                "install packaging/linux/ibus/voxflow.xml to the user or system IBus component directory, then run ibus write-cache".to_string(),
            ),
        }
    }
}

fn pipewire_runtime_check() -> DiagnosticCheck {
    let probe = probe_pipewire_runtime();
    if probe.pipewire_command && probe.pw_cli_command && probe.libpipewire_runtime {
        DiagnosticCheck {
            id: "audio.pipewire.runtime".to_string(),
            status: CheckStatus::Pass,
            summary: format!(
                "PipeWire runtime is available{}",
                probe
                    .version
                    .map(|version| format!(": {version}"))
                    .unwrap_or_default()
            ),
            action_hint: None,
        }
    } else {
        DiagnosticCheck {
            id: "audio.pipewire.runtime".to_string(),
            status: CheckStatus::Warning,
            summary: "PipeWire runtime is incomplete".to_string(),
            action_hint: Some(
                "install pipewire runtime packages before live audio tests".to_string(),
            ),
        }
    }
}

fn pipewire_development_check() -> DiagnosticCheck {
    let probe = probe_pipewire_runtime();
    if probe.pkg_config_development_files {
        DiagnosticCheck {
            id: "audio.pipewire.development".to_string(),
            status: CheckStatus::Pass,
            summary: "PipeWire development files are available through pkg-config".to_string(),
            action_hint: None,
        }
    } else {
        DiagnosticCheck {
            id: "audio.pipewire.development".to_string(),
            status: CheckStatus::Warning,
            summary: "PipeWire development pkg-config files are not available".to_string(),
            action_hint: Some(
                "install libpipewire-0.3-dev before enabling the native PipeWire capture backend"
                    .to_string(),
            ),
        }
    }
}

fn redact_home(path: &Path) -> String {
    match env::var_os("HOME") {
        Some(home) => {
            let home = Path::new(&home);
            match path.strip_prefix(home) {
                Ok(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
                Err(_) => path.display().to_string(),
            }
        }
        None => path.display().to_string(),
    }
}
