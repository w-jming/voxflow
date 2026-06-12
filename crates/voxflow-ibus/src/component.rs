pub const IBUS_ENGINE_NAME: &str = "voxflow";
pub const IBUS_ENGINE_LONGNAME: &str = "VoxFlow / 声流输入法";
pub const DEFAULT_ENGINE_EXEC: &str = "/usr/lib/voxflow/voxflow-ibus --ibus-engine";

pub fn component_xml(engine_exec: &str) -> String {
    let engine_exec = xml_escape(engine_exec);
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<component>
  <name>org.freedesktop.IBus.VoxFlow</name>
  <description>VoxFlow voice input method</description>
  <exec>{engine_exec}</exec>
  <version>{version}</version>
  <author>VoxFlow contributors</author>
  <license>MIT</license>
  <homepage>https://github.com/voxflow/voxflow</homepage>
  <textdomain>voxflow</textdomain>
  <engines>
    <engine>
      <name>{engine_name}</name>
      <language>zh_CN</language>
      <license>MIT</license>
      <author>VoxFlow contributors</author>
      <layout>default</layout>
      <longname>{longname}</longname>
      <description>VoxFlow / 声流输入法</description>
      <rank>99</rank>
      <icon>voxflow</icon>
      <symbol>声</symbol>
      <setup>/usr/bin/voxflow-control</setup>
    </engine>
  </engines>
</component>
"#,
        engine_name = IBUS_ENGINE_NAME,
        longname = IBUS_ENGINE_LONGNAME,
        version = env!("CARGO_PKG_VERSION")
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_declares_voxflow_engine() {
        let xml = component_xml(DEFAULT_ENGINE_EXEC);
        assert!(xml.contains("<name>voxflow</name>"));
        assert!(xml.contains("<longname>VoxFlow / 声流输入法</longname>"));
        assert!(xml.contains("<homepage>https://github.com/voxflow/voxflow</homepage>"));
        assert!(xml.contains("/usr/lib/voxflow/voxflow-ibus --ibus-engine"));
    }

    #[test]
    fn component_escapes_exec_path() {
        let xml = component_xml("/tmp/voxflow & test/voxflow-ibus");
        assert!(xml.contains("/tmp/voxflow &amp; test/voxflow-ibus"));
    }
}
