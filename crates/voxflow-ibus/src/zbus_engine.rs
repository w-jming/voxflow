use voxflow_input::{FrontendCapabilities, FrontendEvent};
use zbus::{
    block_on, interface,
    object_server::SignalContext,
    zvariant::{Array, Dict, Signature, StructureBuilder, Value},
};

use crate::core_client::IbusCoreBridge;
use crate::engine::IbusOperation;

const IBUS_RELEASE_MASK: u32 = 1 << 30;
const IBUS_SHIFT_MASK: u32 = 1 << 0;
const IBUS_CONTROL_MASK: u32 = 1 << 2;
const IBUS_MOD1_MASK: u32 = 1 << 3;
const IBUS_SUPER_MASK: u32 = 1 << 26;

/// 解析 "Alt+S" / "Ctrl+Alt+D" 形式的快捷键为(修饰键掩码, 小写键值)。
fn parse_hotkey(spec: &str) -> Option<(u32, u32)> {
    let mut mask = 0_u32;
    let mut key = None;
    for part in spec.split('+') {
        match part.trim().to_ascii_lowercase().as_str() {
            "alt" => mask |= IBUS_MOD1_MASK,
            "ctrl" | "control" => mask |= IBUS_CONTROL_MASK,
            "shift" => mask |= IBUS_SHIFT_MASK,
            "super" | "meta" | "win" => mask |= IBUS_SUPER_MASK,
            other if other.chars().count() == 1 => key = other.chars().next().map(|ch| ch as u32),
            _ => return None,
        }
    }
    key.map(|keyval| (mask, keyval))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DictationAction {
    Start,
    Stop,
}

/// Pure hotkey state machine. Toggle: press flips listening. Hold: press
/// starts, release stops. Returns the transition to apply (if any).
fn decide_action(hold_mode: bool, listening: bool, is_press: bool) -> Option<DictationAction> {
    if hold_mode {
        match (is_press, listening) {
            (true, false) => Some(DictationAction::Start),
            (false, true) => Some(DictationAction::Stop),
            _ => None,
        }
    } else if is_press {
        Some(if listening {
            DictationAction::Stop
        } else {
            DictationAction::Start
        })
    } else {
        None
    }
}

/// `Some(is_press)` when the event matches the configured hotkey (D-14).
fn hotkey_match(hotkey: (u32, u32), keyval: u32, state: u32) -> Option<bool> {
    let (mask, key) = hotkey;
    let mods_held = state & mask == mask;
    let is_key = keyval == key || keyval == key.saturating_sub(0x20);
    (mods_held && is_key).then_some(state & IBUS_RELEASE_MASK == 0)
}

#[derive(Default)]
pub struct ZbusIbusEngine {
    frontend_events: Vec<FrontendEvent>,
    capabilities_mask: u32,
    pending_operations: Vec<IbusOperation>,
    listening: bool,
    hotkey: Option<(u32, u32)>,
    hold_mode: bool,
    settings_fetched_at: Option<std::time::Instant>,
    #[allow(clippy::type_complexity)]
    core_bridge: Option<Box<dyn IbusCoreBridge>>,
}

impl ZbusIbusEngine {
    pub fn with_core_bridge(core_bridge: Box<dyn IbusCoreBridge>) -> Self {
        Self {
            core_bridge: Some(core_bridge),
            ..Self::default()
        }
    }

    pub fn frontend_events(&self) -> &[FrontendEvent] {
        &self.frontend_events
    }

    pub fn capabilities_mask(&self) -> u32 {
        self.capabilities_mask
    }

    pub fn pending_operations(&self) -> &[IbusOperation] {
        &self.pending_operations
    }

    pub fn drain_pending_operations(&mut self) -> Vec<IbusOperation> {
        std::mem::take(&mut self.pending_operations)
    }

    fn report(&mut self, event: FrontendEvent) -> zbus::fdo::Result<()> {
        self.frontend_events.push(event.clone());
        if let Some(core_bridge) = &mut self.core_bridge {
            core_bridge
                .report_frontend_event(event)
                .map_err(to_fdo_error)?;
        }
        Ok(())
    }

    fn start_dictation(&mut self, ctxt: &SignalContext<'_>) -> zbus::fdo::Result<()> {
        tracing::info!("hotkey -> start_dictation");
        if let Some(core_bridge) = &mut self.core_bridge {
            let operations = match core_bridge.start_dictation() {
                Ok(operations) => operations,
                Err(error) => {
                    tracing::warn!(%error, "start_dictation failed");
                    return Err(to_fdo_error(error));
                }
            };
            self.emit_operations(ctxt, &operations)?;
            self.pending_operations.extend(operations);
        }
        Ok(())
    }

    fn stop_dictation(&mut self, ctxt: &SignalContext<'_>) -> zbus::fdo::Result<()> {
        tracing::info!("hotkey -> stop_dictation");
        if let Some(core_bridge) = &mut self.core_bridge {
            let operations = core_bridge.stop_dictation().map_err(to_fdo_error)?;
            self.emit_operations(ctxt, &operations)?;
            self.pending_operations.extend(operations);
        }
        Ok(())
    }

    pub fn handle_focus_in(&mut self) -> zbus::fdo::Result<()> {
        self.report(FrontendEvent::Focused { app_hint: None })?;
        self.start_dictation_no_signal()
    }

    pub fn handle_focus_out(&mut self) -> zbus::fdo::Result<()> {
        self.report(FrontendEvent::Blurred)?;
        self.stop_dictation_no_signal()
    }

    fn start_dictation_no_signal(&mut self) -> zbus::fdo::Result<()> {
        if let Some(core_bridge) = &mut self.core_bridge {
            self.pending_operations
                .extend(core_bridge.start_dictation().map_err(to_fdo_error)?);
        }
        Ok(())
    }

    /// 从 core 拉取快捷键/模式。**仅在未听写时调用**:一次听写过程中绝不重读
    /// (否则 hold 松手事件触发的 config.get 偶发失败会把模式回退成 toggle,
    /// 导致松手不停)。读取失败时保留上次值,不回退默认。
    fn refresh_settings(&mut self, force: bool) {
        let fresh = self
            .settings_fetched_at
            .map(|at| at.elapsed() < std::time::Duration::from_secs(1))
            .unwrap_or(false);
        if !force && fresh {
            return;
        }
        if let Some(bridge) = &mut self.core_bridge {
            let Some((hotkey, mode)) = bridge.input_settings() else {
                return; // keep last-known settings on a transient read failure
            };
            self.hotkey = parse_hotkey(&hotkey);
            self.hold_mode = mode == "hold";
            self.settings_fetched_at = Some(std::time::Instant::now());
        }
    }

    fn stop_dictation_no_signal(&mut self) -> zbus::fdo::Result<()> {
        if let Some(core_bridge) = &mut self.core_bridge {
            self.pending_operations
                .extend(core_bridge.stop_dictation().map_err(to_fdo_error)?);
        }
        Ok(())
    }

    fn emit_operations(
        &self,
        ctxt: &SignalContext<'_>,
        operations: &[IbusOperation],
    ) -> zbus::fdo::Result<()> {
        emit_operations_via(ctxt, operations)
    }
}

/// Emits IBus engine signals for projected operations; shared between the
/// method-call path and the core event pump thread.
pub(crate) fn emit_operations_via(
    ctxt: &SignalContext<'_>,
    operations: &[IbusOperation],
) -> zbus::fdo::Result<()> {
    {
        for operation in operations {
            match operation {
                IbusOperation::UpdatePreeditText {
                    text,
                    cursor_pos,
                    underline,
                } => {
                    let text = ibus_text_value(text, *underline).map_err(to_fdo_error)?;
                    block_on(ZbusIbusEngine::update_preedit_text(
                        ctxt,
                        &text,
                        *cursor_pos as u32,
                        true,
                    ))
                    .map_err(to_fdo_error_display)?;
                }
                IbusOperation::CommitText { text } => {
                    let text = ibus_text_value(text, false).map_err(to_fdo_error)?;
                    block_on(ZbusIbusEngine::commit_text(ctxt, &text))
                        .map_err(to_fdo_error_display)?;
                }
                IbusOperation::DeleteSurroundingText { chars } => {
                    block_on(ZbusIbusEngine::delete_surrounding_text(
                        ctxt,
                        -(*chars as i32),
                        *chars as u32,
                    ))
                    .map_err(to_fdo_error_display)?;
                }
                IbusOperation::ClearPreedit => {
                    let text = ibus_text_value("", false).map_err(to_fdo_error)?;
                    block_on(ZbusIbusEngine::update_preedit_text(ctxt, &text, 0, false))
                        .map_err(to_fdo_error_display)?;
                }
                IbusOperation::SessionStarted | IbusOperation::SessionStopped => {}
            }
        }
        Ok(())
    }
}

#[interface(interface = "org.freedesktop.IBus.Engine")]
impl ZbusIbusEngine {
    fn focus_in(
        &mut self,
        #[zbus(signal_context)] _ctxt: SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        self.refresh_settings(true);
        tracing::info!(hotkey = ?self.hotkey, hold = self.hold_mode, "focus_in");
        // Dictation is hotkey-driven (D-14); focus only reports state.
        self.report(FrontendEvent::Focused { app_hint: None })
    }

    fn focus_out(
        &mut self,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        self.report(FrontendEvent::Blurred)?;
        if self.listening {
            self.listening = false;
            return self.stop_dictation(&ctxt);
        }
        Ok(())
    }

    fn enable(&mut self) -> zbus::fdo::Result<()> {
        self.report(FrontendEvent::Activated)
    }

    fn disable(&mut self) -> zbus::fdo::Result<()> {
        self.report(FrontendEvent::Deactivated)
    }

    fn reset(&mut self) -> zbus::fdo::Result<()> {
        Ok(())
    }

    fn process_key_event(
        &mut self,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
        keyval: u32,
        _keycode: u32,
        state: u32,
    ) -> zbus::fdo::Result<bool> {
        let is_release = state & IBUS_RELEASE_MASK != 0;
        // Pick up hotkey/mode changes at the moment of a likely hotkey press
        // while idle — config is then quiescent so the read is reliable. Never
        // refresh during an active session (re-reading on the hold-release
        // event previously flipped the mode and broke "release to stop").
        let has_modifier =
            state & (IBUS_MOD1_MASK | IBUS_CONTROL_MASK | IBUS_SUPER_MASK | IBUS_SHIFT_MASK) != 0;
        if !is_release && !self.listening && has_modifier {
            self.refresh_settings(true);
        }

        let hotkey = self.hotkey.unwrap_or((IBUS_MOD1_MASK, 's' as u32));
        let Some(is_press) = hotkey_match(hotkey, keyval, state) else {
            return Ok(false);
        };
        match decide_action(self.hold_mode, self.listening, is_press) {
            Some(DictationAction::Start) => {
                self.listening = true;
                self.start_dictation(&ctxt)?;
            }
            Some(DictationAction::Stop) => {
                self.listening = false;
                self.stop_dictation(&ctxt)?;
            }
            None => {}
        }
        Ok(true)
    }

    fn set_cursor_location(
        &mut self,
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
    ) -> zbus::fdo::Result<()> {
        Ok(())
    }

    fn set_capabilities(&mut self, capabilities: u32) -> zbus::fdo::Result<()> {
        self.capabilities_mask = capabilities;
        self.report(FrontendEvent::Capabilities {
            capabilities: FrontendCapabilities::full(),
        })
    }

    fn property_activate(&mut self, _prop_name: &str, _prop_state: u32) -> zbus::fdo::Result<()> {
        Ok(())
    }

    fn property_show(&mut self, _prop_name: &str) -> zbus::fdo::Result<()> {
        Ok(())
    }

    fn property_hide(&mut self, _prop_name: &str) -> zbus::fdo::Result<()> {
        Ok(())
    }

    fn candidate_clicked(
        &mut self,
        _index: u32,
        _button: u32,
        _state: u32,
    ) -> zbus::fdo::Result<()> {
        Ok(())
    }

    fn page_up(&mut self) -> zbus::fdo::Result<()> {
        Ok(())
    }

    fn page_down(&mut self) -> zbus::fdo::Result<()> {
        Ok(())
    }

    fn cursor_up(&mut self) -> zbus::fdo::Result<()> {
        Ok(())
    }

    fn cursor_down(&mut self) -> zbus::fdo::Result<()> {
        Ok(())
    }

    #[zbus(signal)]
    async fn update_preedit_text(
        ctxt: &SignalContext<'_>,
        text: &Value<'_>,
        cursor_pos: u32,
        visible: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn commit_text(ctxt: &SignalContext<'_>, text: &Value<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn delete_surrounding_text(
        ctxt: &SignalContext<'_>,
        offset_from_cursor: i32,
        chars: u32,
    ) -> zbus::Result<()>;
}

pub(crate) fn to_fdo_error(error: anyhow::Error) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(error.to_string())
}

fn to_fdo_error_display(error: impl std::fmt::Display) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(error.to_string())
}

fn ibus_text_value(text: &str, underline: bool) -> anyhow::Result<Value<'static>> {
    let attrs = if underline && !text.is_empty() {
        let mut attrs = Array::new(Signature::from_static_str_unchecked("v"));
        let attr = ibus_attribute_value(1, 1, 0, text.chars().count() as u32)?;
        attrs.append(attr)?;
        attrs
    } else {
        Array::new(Signature::from_static_str_unchecked("v"))
    };
    let attr_list = StructureBuilder::new()
        .add_field("IBusAttrList")
        .append_field(empty_attachments())
        .append_field(Value::Array(attrs))
        .build();
    let text = StructureBuilder::new()
        .add_field("IBusText")
        .append_field(empty_attachments())
        .add_field(text.to_string())
        .append_field(Value::Value(Box::new(Value::Structure(attr_list))))
        .build();
    Ok(Value::Value(Box::new(Value::Structure(text))))
}

fn ibus_attribute_value(
    attr_type: u32,
    value: u32,
    start_index: u32,
    end_index: u32,
) -> anyhow::Result<Value<'static>> {
    let attr = StructureBuilder::new()
        .add_field("IBusAttribute")
        .append_field(empty_attachments())
        .add_field(attr_type)
        .add_field(value)
        .add_field(start_index)
        .add_field(end_index)
        .build();
    Ok(Value::Value(Box::new(Value::Structure(attr))))
}

fn empty_attachments() -> Value<'static> {
    Value::Dict(Dict::new(
        Signature::from_static_str_unchecked("s"),
        Signature::from_static_str_unchecked("v"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use zbus::object_server::Interface;

    #[derive(Default)]
    struct FakeBridge {
        reports: Vec<FrontendEvent>,
        start_count: usize,
        stop_count: usize,
    }

    impl IbusCoreBridge for FakeBridge {
        fn report_frontend_event(&mut self, event: FrontendEvent) -> Result<()> {
            self.reports.push(event);
            Ok(())
        }

        fn start_dictation(&mut self) -> Result<Vec<IbusOperation>> {
            self.start_count += 1;
            Ok(vec![IbusOperation::UpdatePreeditText {
                text: "今天下午".to_string(),
                cursor_pos: 4,
                underline: true,
            }])
        }

        fn stop_dictation(&mut self) -> Result<Vec<IbusOperation>> {
            self.stop_count += 1;
            Ok(vec![IbusOperation::ClearPreedit])
        }
    }

    #[test]
    fn zbus_interface_name_matches_ibus_engine_contract() {
        assert_eq!(
            ZbusIbusEngine::name().as_str(),
            "org.freedesktop.IBus.Engine"
        );
    }

    #[test]
    fn focus_and_enable_generate_frontend_reports() {
        let mut engine = ZbusIbusEngine::default();
        engine.handle_focus_in().unwrap();
        engine.enable().unwrap();
        engine.handle_focus_out().unwrap();
        assert_eq!(
            engine.frontend_events(),
            &[
                FrontendEvent::Focused { app_hint: None },
                FrontendEvent::Activated,
                FrontendEvent::Blurred
            ]
        );
    }

    #[test]
    fn alt_s_press_matches_hotkey_and_other_keys_pass_through() {
        let alt_s = parse_hotkey("Alt+S").unwrap();
        assert_eq!(hotkey_match(alt_s, 's' as u32, IBUS_MOD1_MASK), Some(true));
        assert_eq!(
            hotkey_match(alt_s, 'S' as u32, IBUS_MOD1_MASK | IBUS_RELEASE_MASK),
            Some(false)
        );
        assert_eq!(hotkey_match(alt_s, 'a' as u32, IBUS_MOD1_MASK), None);
        assert_eq!(hotkey_match(alt_s, 's' as u32, 0), None);
        let ctrl_alt_d = parse_hotkey("Ctrl+Alt+D").unwrap();
        assert_eq!(
            hotkey_match(ctrl_alt_d, 'd' as u32, IBUS_CONTROL_MASK | IBUS_MOD1_MASK),
            Some(true)
        );
        assert_eq!(parse_hotkey("Alt+Enter怪"), None);
    }

    #[test]
    fn hold_mode_starts_on_press_and_stops_on_release() {
        // press while idle -> start; release while listening -> stop.
        assert_eq!(
            decide_action(true, false, true),
            Some(DictationAction::Start)
        );
        assert_eq!(
            decide_action(true, true, false),
            Some(DictationAction::Stop)
        );
        // auto-repeat press while already listening -> no-op (no restart).
        assert_eq!(decide_action(true, true, true), None);
        // stray release while idle -> no-op.
        assert_eq!(decide_action(true, false, false), None);
    }

    #[test]
    fn toggle_mode_flips_on_press_only() {
        assert_eq!(
            decide_action(false, false, true),
            Some(DictationAction::Start)
        );
        assert_eq!(
            decide_action(false, true, true),
            Some(DictationAction::Stop)
        );
        // releases are ignored in toggle mode.
        assert_eq!(decide_action(false, true, false), None);
        assert_eq!(decide_action(false, false, false), None);
    }

    #[test]
    #[ignore = "superseded: key handling now requires a SignalContext; logic covered above"]
    fn process_key_event_is_not_handled_by_poc_engine() {
        let engine = ZbusIbusEngine::default();
        let _ = engine;
    }

    #[test]
    fn focus_in_and_out_drive_core_bridge_operations() {
        let mut engine = ZbusIbusEngine::with_core_bridge(Box::<FakeBridge>::default());
        engine.handle_focus_in().unwrap();
        assert_eq!(
            engine.pending_operations(),
            &[IbusOperation::UpdatePreeditText {
                text: "今天下午".to_string(),
                cursor_pos: 4,
                underline: true,
            }]
        );
        assert_eq!(
            engine.drain_pending_operations(),
            vec![IbusOperation::UpdatePreeditText {
                text: "今天下午".to_string(),
                cursor_pos: 4,
                underline: true,
            }]
        );
        engine.handle_focus_out().unwrap();
        assert_eq!(engine.pending_operations(), &[IbusOperation::ClearPreedit]);
    }

    #[test]
    fn ibus_text_value_matches_ibus_runtime_signature() {
        let text = ibus_text_value("今天下午", true).unwrap();
        assert_eq!(text.value_signature().as_str(), "v");
        assert_eq!(
            text.to_string(),
            "<(\"IBusText\", @a{sv} {}, \"今天下午\", <(\"IBusAttrList\", @a{sv} {}, [<(\"IBusAttribute\", @a{sv} {}, uint32 1, uint32 1, uint32 0, uint32 4)>])>)>"
        );
    }

    #[test]
    fn empty_ibus_text_hides_preedit_without_attributes() {
        let text = ibus_text_value("", false).unwrap();
        assert_eq!(
            text.to_string(),
            "<(\"IBusText\", @a{sv} {}, \"\", <(\"IBusAttrList\", @a{sv} {}, @av [])>)>"
        );
    }
}
