use std::path::PathBuf;

use zbus::{
    interface,
    zvariant::{ObjectPath, OwnedObjectPath},
    ObjectServer,
};

use crate::{
    component::IBUS_ENGINE_NAME,
    core_client::CoreEngineSession,
    zbus_engine::{to_fdo_error, ZbusIbusEngine},
};

pub const IBUS_FACTORY_OBJECT_PATH: &str = "/org/freedesktop/IBus/Factory";
pub const IBUS_ENGINE_OBJECT_BASE: &str = "/org/freedesktop/IBus/Engine/VoxFlow";

/// 最近一次 CreateEngine 返回的对象路径;事件泵向它发信号。
pub static ACTIVE_ENGINE_PATH: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub struct ZbusIbusFactory {
    core_socket: PathBuf,
    next_engine_id: u64,
}

impl ZbusIbusFactory {
    pub fn new(core_socket: PathBuf) -> Self {
        Self {
            core_socket,
            next_engine_id: 0,
        }
    }

    fn next_engine_path(&mut self, engine_name: &str) -> zbus::fdo::Result<OwnedObjectPath> {
        self.next_engine_id += 1;
        let suffix = sanitize_engine_name(engine_name);
        let path = format!("{IBUS_ENGINE_OBJECT_BASE}/{suffix}/{}", self.next_engine_id);
        OwnedObjectPath::try_from(path).map_err(|error| zbus::fdo::Error::Failed(error.to_string()))
    }
}

#[interface(interface = "org.freedesktop.IBus.Factory")]
impl ZbusIbusFactory {
    async fn create_engine(
        &mut self,
        #[zbus(object_server)] object_server: &ObjectServer,
        engine_name: &str,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        if engine_name != IBUS_ENGINE_NAME {
            return Err(zbus::fdo::Error::Failed(format!(
                "unsupported IBus engine: {engine_name}"
            )));
        }

        tracing::info!(engine_name, "ibus CreateEngine called");
        let path = self.next_engine_path(engine_name)?;
        let core_session = match CoreEngineSession::connect(self.core_socket.clone()) {
            Ok(session) => session,
            Err(error) => {
                tracing::warn!(%error, "engine failed to connect to core");
                return Err(to_fdo_error(error));
            }
        };
        object_server
            .at(
                ObjectPath::from(&path),
                ZbusIbusEngine::with_core_bridge(Box::new(core_session)),
            )
            .await
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
        *ACTIVE_ENGINE_PATH.lock().expect("engine path lock") = Some(path.to_string());
        Ok(path)
    }

    async fn destroy(&mut self) -> zbus::fdo::Result<()> {
        Ok(())
    }
}

fn sanitize_engine_name(engine_name: &str) -> String {
    let sanitized: String = engine_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "engine".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::object_server::Interface;

    #[test]
    fn factory_interface_name_matches_ibus_contract() {
        assert_eq!(
            ZbusIbusFactory::name().as_str(),
            "org.freedesktop.IBus.Factory"
        );
    }

    #[test]
    fn engine_paths_are_valid_object_paths() {
        let mut factory = ZbusIbusFactory::new(PathBuf::from("/tmp/core.sock"));
        let path = factory.next_engine_path("voxflow").unwrap();
        assert_eq!(
            path.as_str(),
            "/org/freedesktop/IBus/Engine/VoxFlow/voxflow/1"
        );
    }

    #[test]
    fn engine_path_sanitizes_non_object_path_chars() {
        let mut factory = ZbusIbusFactory::new(PathBuf::from("/tmp/core.sock"));
        let path = factory.next_engine_path("table:foo-bar").unwrap();
        assert_eq!(
            path.as_str(),
            "/org/freedesktop/IBus/Engine/VoxFlow/table_foo_bar/1"
        );
    }
}
