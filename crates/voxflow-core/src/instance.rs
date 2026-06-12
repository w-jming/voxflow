use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::paths::VoxflowPaths;

#[derive(Debug)]
pub struct InstanceGuard {
    lock_path: PathBuf,
}

impl InstanceGuard {
    pub fn acquire(paths: &VoxflowPaths) -> Result<Self> {
        fs::create_dir_all(&paths.run)
            .with_context(|| format!("create run dir {}", paths.run.display()))?;
        Self::acquire_path(paths.run.join("core.lock"))
    }

    fn acquire_path(lock_path: PathBuf) -> Result<Self> {
        match create_lock(&lock_path) {
            Ok(()) => Ok(Self { lock_path }),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                if lock_is_stale(&lock_path) {
                    fs::remove_file(&lock_path)
                        .with_context(|| format!("remove stale lock {}", lock_path.display()))?;
                    create_lock(&lock_path)
                        .with_context(|| format!("create lock {}", lock_path.display()))?;
                    Ok(Self { lock_path })
                } else {
                    bail!(
                        "voxflow-core is already running or lock is active at {}",
                        lock_path.display()
                    )
                }
            }
            Err(error) => {
                Err(error).with_context(|| format!("create lock {}", lock_path.display()))
            }
        }
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.lock_path) {
            if error.kind() != ErrorKind::NotFound {
                tracing::warn!(error = %error, lock = %self.lock_path.display(), "failed to remove lock");
            }
        }
    }
}

fn create_lock(path: &Path) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    writeln!(file, "{}", std::process::id())?;
    file.sync_all()
}

fn lock_is_stale(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return true;
    };
    let Ok(pid) = text.trim().parse::<u32>() else {
        return true;
    };
    !process_exists(pid)
}

#[cfg(target_os = "linux")]
fn process_exists(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

#[cfg(not(target_os = "linux"))]
fn process_exists(_pid: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn second_guard_is_rejected() {
        let lock = unique_lock("active");
        fs::create_dir_all(lock.parent().unwrap()).unwrap();
        let _guard = InstanceGuard::acquire_path(lock.clone()).unwrap();
        let error = InstanceGuard::acquire_path(lock).unwrap_err().to_string();
        assert!(error.contains("already running"));
    }

    #[test]
    fn stale_lock_is_replaced() {
        let lock = unique_lock("stale");
        fs::create_dir_all(lock.parent().unwrap()).unwrap();
        fs::write(&lock, "999999999\n").unwrap();
        let _guard = InstanceGuard::acquire_path(lock.clone()).unwrap();
        let text = fs::read_to_string(lock).unwrap();
        assert_eq!(text.trim(), std::process::id().to_string());
    }

    fn unique_lock(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!(
                "voxflow-instance-{name}-{}-{nanos}",
                std::process::id()
            ))
            .join("core.lock")
    }
}
