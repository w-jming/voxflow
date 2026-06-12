pub const FCITX5_ADDON_NAME: &str = "voxflow";
pub const FCITX5_INPUT_METHOD_NAME: &str = "voxflow";
pub const FCITX5_DISPLAY_NAME: &str = "VoxFlow / 声流输入法";
pub const DEFAULT_ADDON_LIBRARY: &str = "voxflow";

pub fn addon_conf(library_name: &str) -> String {
    let library_name = conf_escape(library_name);
    format!(
        r#"[Addon]
Name=VoxFlow
Category=InputMethod
Library={library_name}
Type=SharedLibrary
OnDemand=True
Configurable=True
Version={version}
Comment=VoxFlow voice input method frontend
"#,
        version = env!("CARGO_PKG_VERSION")
    )
}

pub fn inputmethod_conf() -> String {
    format!(
        r#"[InputMethod]
Name={display_name}
Icon=voxflow
Label=声
LangCode=zh_CN
Addon={addon_name}
Enabled=True
"#,
        display_name = FCITX5_DISPLAY_NAME,
        addon_name = FCITX5_ADDON_NAME
    )
}

fn conf_escape(value: &str) -> String {
    value.replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addon_conf_declares_voxflow_input_method_addon() {
        let conf = addon_conf(DEFAULT_ADDON_LIBRARY);
        assert!(conf.contains("[Addon]"));
        assert!(conf.contains("Category=InputMethod"));
        assert!(conf.contains("Library=voxflow"));
        assert!(conf.contains("Type=SharedLibrary"));
    }

    #[test]
    fn inputmethod_conf_declares_display_name_and_addon() {
        let conf = inputmethod_conf();
        assert!(conf.contains("[InputMethod]"));
        assert!(conf.contains("Name=VoxFlow / 声流输入法"));
        assert!(conf.contains("Addon=voxflow"));
        assert!(conf.contains("Label=声"));
    }

    #[test]
    fn addon_conf_sanitizes_newlines() {
        let conf = addon_conf("voxflow\nbad");
        assert!(conf.contains("Library=voxflow bad"));
        assert!(!conf.contains("Library=voxflow\nbad"));
    }
}
