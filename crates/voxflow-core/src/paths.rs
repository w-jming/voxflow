use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use anyhow::{Context, Result};

pub const APP_HOME_ENV: &str = "VOXFLOW_HOME";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxflowPaths {
    pub home: PathBuf,
    pub config: PathBuf,
    pub models: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
    pub run: PathBuf,
    pub ledger: PathBuf,
    pub runtime_dir: PathBuf,
    pub socket: PathBuf,
}

impl VoxflowPaths {
    pub fn from_env() -> Result<Self> {
        let home = match env::var_os(APP_HOME_ENV) {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => home_dir()?.join(".voxflow"),
        };
        let runtime_base = match env::var_os("XDG_RUNTIME_DIR") {
            Some(value) if !value.is_empty() => PathBuf::from(value),
            _ => home.join("run"),
        };
        let runtime_dir = runtime_base.join("voxflow");
        Ok(Self {
            config: home.join("config.toml"),
            models: home.join("models"),
            cache: home.join("cache"),
            logs: home.join("logs"),
            run: home.join("run"),
            ledger: home.join("ledger"),
            socket: runtime_dir.join("core.sock"),
            runtime_dir,
            home,
        })
    }

    pub fn ensure(&self) -> Result<()> {
        for path in [
            &self.home,
            &self.models,
            &self.cache,
            &self.logs,
            &self.run,
            &self.ledger,
            &self.runtime_dir,
        ] {
            fs::create_dir_all(path).with_context(|| format!("create {}", path.display()))?;
        }
        #[cfg(unix)]
        {
            fs::set_permissions(&self.home, fs::Permissions::from_mode(0o700))?;
            fs::set_permissions(&self.runtime_dir, fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
}

fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .context("HOME is not set; set VOXFLOW_HOME explicitly")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn default_home_uses_home() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var(APP_HOME_ENV);
        env::set_var("HOME", "/tmp/voxflow-test-home");
        let paths = VoxflowPaths::from_env().unwrap();
        assert_eq!(paths.home, PathBuf::from("/tmp/voxflow-test-home/.voxflow"));
        assert!(paths.socket.ends_with("voxflow/core.sock"));
    }

    #[test]
    fn voxflow_home_overrides_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var(APP_HOME_ENV, "/tmp/custom-voxflow");
        let paths = VoxflowPaths::from_env().unwrap();
        assert_eq!(paths.home, PathBuf::from("/tmp/custom-voxflow"));
        env::remove_var(APP_HOME_ENV);
    }
}
