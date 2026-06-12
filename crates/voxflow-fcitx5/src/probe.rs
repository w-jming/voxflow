use std::env;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fcitx5Probe {
    pub command_available: bool,
    pub pkg_config_available: bool,
    pub version: Option<String>,
}

pub fn probe_fcitx5() -> Fcitx5Probe {
    Fcitx5Probe {
        command_available: command_exists("fcitx5"),
        pkg_config_available: pkg_config_has_fcitx5(),
        version: fcitx5_version(),
    }
}

fn command_exists(command: &str) -> bool {
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .any(|dir| dir.join(command).is_file())
}

fn pkg_config_has_fcitx5() -> bool {
    Command::new("pkg-config")
        .args(["--exists", "fcitx5"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn fcitx5_version() -> Option<String> {
    let output = Command::new("fcitx5").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_fcitx5_version_output(&String::from_utf8_lossy(&output.stdout))
        .or_else(|| parse_fcitx5_version_output(&String::from_utf8_lossy(&output.stderr)))
}

pub fn parse_fcitx5_version_output(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_from_common_output() {
        assert_eq!(
            parse_fcitx5_version_output("Fcitx 5.1.8 -- Gettext"),
            Some("5.1.8".to_string())
        );
    }
}
