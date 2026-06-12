use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use voxflow_input::InputEvent;

use crate::config::ThresholdMode;

const DEFAULT_LEDGER_CAPACITY: usize = 50;
const FREEZE_AFTER_NEWER_SEGMENTS: usize = 10;
const STANDARD_CONFIDENCE_THRESHOLD: f32 = 0.85;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LedgerSegment {
    pub id: String,
    pub session_id: String,
    pub committed_text: String,
    pub normalized_text: String,
    pub token_start: usize,
    pub token_end: usize,
    pub source: SegmentSource,
    pub committed_at_ms: u64,
    pub cursor_context_hash: u64,
    pub frozen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SegmentSource {
    AsrStable,
    Refine,
    Correction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InjectionLedger {
    capacity: usize,
    segments: VecDeque<LedgerSegment>,
}

impl InjectionLedger {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            segments: VecDeque::new(),
        }
    }

    pub fn append(&mut self, mut segment: LedgerSegment) {
        segment.normalized_text = normalize_text(&segment.committed_text);
        self.segments.push_back(segment);
        while self.segments.len() > self.capacity {
            self.segments.pop_front();
        }
        self.freeze_segments_with_many_newer();
    }

    pub fn freeze_all(&mut self) {
        for segment in &mut self.segments {
            segment.frozen = true;
        }
    }

    pub fn freeze_segment(&mut self, segment_id: &str) -> bool {
        if let Some(segment) = self
            .segments
            .iter_mut()
            .find(|segment| segment.id == segment_id)
        {
            segment.frozen = true;
            return true;
        }
        false
    }

    pub fn freeze_older_than(&mut self, now_ms: u64, max_age_ms: u64) {
        for segment in &mut self.segments {
            if now_ms.saturating_sub(segment.committed_at_ms) > max_age_ms {
                segment.frozen = true;
            }
        }
    }

    pub fn segments(&self) -> std::collections::vec_deque::Iter<'_, LedgerSegment> {
        self.segments.iter()
    }

    pub fn recent_unfrozen(&self) -> Vec<&LedgerSegment> {
        self.segments
            .iter()
            .rev()
            .filter(|segment| !segment.frozen)
            .collect()
    }

    pub fn last_unfrozen(&self) -> Option<&LedgerSegment> {
        self.segments.iter().rev().find(|segment| !segment.frozen)
    }

    fn freeze_segments_with_many_newer(&mut self) {
        let len = self.segments.len();
        for (index, segment) in self.segments.iter_mut().enumerate() {
            if len.saturating_sub(index + 1) >= FREEZE_AFTER_NEWER_SEGMENTS {
                segment.frozen = true;
            }
        }
    }
}

impl Default for InjectionLedger {
    fn default() -> Self {
        Self::new(DEFAULT_LEDGER_CAPACITY)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionIntent {
    Literal,
    UndoLast,
    UndoTarget,
    ReplaceEntity,
    RepairPrevious,
    Uncertain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntentCandidate {
    pub intent: CorrectionIntent,
    pub confidence: f32,
    pub target_hint: Option<String>,
    pub replacement_hint: Option<String>,
    pub reason_code: String,
    pub original_text: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RuleIntentClassifier;

impl RuleIntentClassifier {
    pub fn classify(&self, current_text: &str) -> IntentCandidate {
        let normalized = normalize_text(current_text);
        if is_explicit_literal_negative(&normalized) {
            return candidate(
                CorrectionIntent::Literal,
                current_text,
                0.99,
                None,
                None,
                "explicit_literal_negative",
            );
        }
        if contains_any(
            &normalized,
            &["刚才那句删掉", "删掉刚才那句", "撤销上一句", "删除上一句"],
        ) {
            return candidate(
                CorrectionIntent::UndoLast,
                current_text,
                0.95,
                None,
                None,
                "explicit_undo_last",
            );
        }
        if let Some((target, replacement)) = split_replace_entity(&normalized) {
            return candidate(
                CorrectionIntent::ReplaceEntity,
                current_text,
                0.9,
                Some(target),
                Some(replacement),
                "repair_marker_and_entity_pair",
            );
        }
        if let Some(replacement) = strip_repair_previous_marker(&normalized) {
            return candidate(
                CorrectionIntent::RepairPrevious,
                current_text,
                0.86,
                None,
                Some(replacement),
                "repair_previous_marker",
            );
        }
        candidate(
            CorrectionIntent::Literal,
            current_text,
            1.0,
            None,
            None,
            "no_rule_match",
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionAction {
    Literal,
    Delete {
        segment_id: String,
        text: String,
    },
    Replace {
        segment_id: String,
        segment_text: String,
        target: String,
        replacement: String,
        replacement_text: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GateCheck {
    pub code: String,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CorrectionDecision {
    pub applied: bool,
    pub action: CorrectionAction,
    pub gate_checks: Vec<GateCheck>,
    pub reason_code: String,
    pub candidate: IntentCandidate,
}

impl CorrectionDecision {
    pub fn to_input_events(&self) -> Vec<InputEvent> {
        if !self.applied {
            return Vec::new();
        }

        match &self.action {
            CorrectionAction::Literal => Vec::new(),
            CorrectionAction::Delete { text, .. } => delete_segment_events(text, None),
            CorrectionAction::Replace {
                segment_text,
                replacement_text,
                ..
            } => delete_segment_events(segment_text, Some(replacement_text.as_str())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CorrectionRecord {
    pub operation_id: String,
    pub applied: bool,
    pub intent: CorrectionIntent,
    pub target: Option<String>,
    pub replacement: Option<String>,
    pub confidence: f32,
    pub reason_code: String,
    pub gate_checks: Vec<GateCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafetyGateContext {
    pub correction_enabled: bool,
    pub threshold_mode: ThresholdMode,
    pub surrounding_text: Option<String>,
    pub delete_supported: bool,
    pub record_writable: bool,
}

impl Default for SafetyGateContext {
    fn default() -> Self {
        Self {
            correction_enabled: true,
            threshold_mode: ThresholdMode::Standard,
            surrounding_text: Some(String::new()),
            delete_supported: true,
            record_writable: true,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SafetyGate;

impl SafetyGate {
    pub fn evaluate(
        &self,
        ledger: &InjectionLedger,
        candidate: IntentCandidate,
        context: &SafetyGateContext,
    ) -> CorrectionDecision {
        if matches!(
            candidate.intent,
            CorrectionIntent::Literal | CorrectionIntent::Uncertain
        ) {
            return literal_decision(candidate, "literal_or_uncertain");
        }

        let mut checks = Vec::new();
        push_check(
            &mut checks,
            "correction_enabled",
            context.correction_enabled,
        );
        push_check(
            &mut checks,
            "confidence_threshold",
            candidate.confidence >= confidence_threshold(&context.threshold_mode),
        );
        push_check(&mut checks, "delete_supported", context.delete_supported);
        push_check(&mut checks, "record_writable", context.record_writable);

        let target_segment = target_segment(ledger, &candidate);
        push_check(
            &mut checks,
            "target_in_unfrozen_ledger",
            target_segment.is_some(),
        );

        let surrounding_ok =
            surrounding_matches(context.surrounding_text.as_deref(), target_segment);
        push_check(
            &mut checks,
            "surrounding_matches_ledger_tail",
            surrounding_ok,
        );
        push_check(
            &mut checks,
            "delete_range_within_ledger",
            target_segment.is_some(),
        );
        push_check(
            &mut checks,
            "replacement_text_resolvable",
            replacement_text_resolvable(target_segment, &candidate),
        );

        if checks.iter().any(|check| !check.passed) {
            return CorrectionDecision {
                applied: false,
                action: CorrectionAction::Literal,
                gate_checks: checks,
                reason_code: "gate_rejected".to_string(),
                candidate,
            };
        }

        let segment = target_segment.expect("checked above");
        let action = match candidate.intent {
            CorrectionIntent::UndoLast | CorrectionIntent::UndoTarget => CorrectionAction::Delete {
                segment_id: segment.id.clone(),
                text: segment.committed_text.clone(),
            },
            CorrectionIntent::ReplaceEntity => CorrectionAction::Replace {
                segment_id: segment.id.clone(),
                segment_text: segment.committed_text.clone(),
                target: candidate.target_hint.clone().unwrap_or_default(),
                replacement: candidate.replacement_hint.clone().unwrap_or_default(),
                replacement_text: replace_segment_text(segment, &candidate).unwrap_or_default(),
            },
            CorrectionIntent::RepairPrevious => CorrectionAction::Replace {
                segment_id: segment.id.clone(),
                segment_text: segment.committed_text.clone(),
                target: segment.committed_text.clone(),
                replacement: candidate.replacement_hint.clone().unwrap_or_default(),
                replacement_text: candidate.replacement_hint.clone().unwrap_or_default(),
            },
            CorrectionIntent::Literal | CorrectionIntent::Uncertain => CorrectionAction::Literal,
        };

        CorrectionDecision {
            applied: true,
            action,
            gate_checks: checks,
            reason_code: candidate.reason_code.clone(),
            candidate,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CorrectionHistory {
    capacity: usize,
    records: VecDeque<CorrectionRecord>,
}

impl CorrectionHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            records: VecDeque::new(),
        }
    }

    pub fn push_decision(
        &mut self,
        operation_id: impl Into<String>,
        decision: &CorrectionDecision,
    ) {
        let record = CorrectionRecord {
            operation_id: operation_id.into(),
            applied: decision.applied,
            intent: decision.candidate.intent,
            target: decision.candidate.target_hint.clone(),
            replacement: decision.candidate.replacement_hint.clone(),
            confidence: decision.candidate.confidence,
            reason_code: decision.reason_code.clone(),
            gate_checks: decision.gate_checks.clone(),
        };
        self.records.push_back(record);
        while self.records.len() > self.capacity {
            self.records.pop_front();
        }
    }

    pub fn recent(&self) -> Vec<CorrectionRecord> {
        self.records.iter().rev().cloned().collect()
    }
}

impl Default for CorrectionHistory {
    fn default() -> Self {
        Self::new(20)
    }
}

pub fn normalize_text(text: &str) -> String {
    text.chars()
        .filter(|ch| {
            !ch.is_whitespace() && !matches!(ch, '，' | ',' | '。' | '.' | '！' | '!' | '？' | '?')
        })
        .collect()
}

pub fn cursor_context_hash(text: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    text.chars()
        .rev()
        .take(16)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .fold(FNV_OFFSET, |hash, ch| {
            let mut next = hash;
            for byte in ch.to_string().as_bytes() {
                next ^= *byte as u64;
                next = next.wrapping_mul(FNV_PRIME);
            }
            next
        })
}

fn confidence_threshold(mode: &ThresholdMode) -> f32 {
    match mode {
        ThresholdMode::Conservative => STANDARD_CONFIDENCE_THRESHOLD + 0.05,
        ThresholdMode::Standard => STANDARD_CONFIDENCE_THRESHOLD,
        ThresholdMode::Aggressive => STANDARD_CONFIDENCE_THRESHOLD - 0.05,
    }
}

fn candidate(
    intent: CorrectionIntent,
    original_text: &str,
    confidence: f32,
    target_hint: Option<String>,
    replacement_hint: Option<String>,
    reason_code: &str,
) -> IntentCandidate {
    IntentCandidate {
        intent,
        confidence,
        target_hint,
        replacement_hint,
        reason_code: reason_code.to_string(),
        original_text: original_text.to_string(),
    }
}

fn is_explicit_literal_negative(normalized: &str) -> bool {
    contains_any(
        normalized,
        &["这不是问题", "不是不对", "不是说要删除", "不是要删除"],
    )
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn split_replace_entity(text: &str) -> Option<(String, String)> {
    for marker in ["不对应该是", "不对改成", "不对"] {
        if let Some((left, right)) = text.split_once(marker) {
            if !left.is_empty() && !right.is_empty() {
                return Some((left.to_string(), right.to_string()));
            }
        }
    }
    None
}

fn strip_repair_previous_marker(text: &str) -> Option<String> {
    for marker in [
        "不对应该是",
        "不对改成",
        "不对",
        "错了应该是",
        "错了改成",
        "错了",
    ] {
        if let Some(rest) = text.strip_prefix(marker) {
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}

fn target_segment<'a>(
    ledger: &'a InjectionLedger,
    candidate: &IntentCandidate,
) -> Option<&'a LedgerSegment> {
    match candidate.intent {
        CorrectionIntent::UndoLast | CorrectionIntent::RepairPrevious => ledger.last_unfrozen(),
        CorrectionIntent::UndoTarget | CorrectionIntent::ReplaceEntity => {
            let target = candidate
                .target_hint
                .as_ref()
                .map(|value| normalize_text(value))?;
            ledger
                .segments()
                .rev()
                .find(|segment| !segment.frozen && segment.normalized_text.contains(&target))
        }
        CorrectionIntent::Literal | CorrectionIntent::Uncertain => None,
    }
}

fn surrounding_matches(surrounding: Option<&str>, segment: Option<&LedgerSegment>) -> bool {
    let Some(segment) = segment else {
        return false;
    };
    match surrounding {
        Some(text) => text.ends_with(&segment.committed_text),
        None => false,
    }
}

fn replacement_text_resolvable(
    segment: Option<&LedgerSegment>,
    candidate: &IntentCandidate,
) -> bool {
    match candidate.intent {
        CorrectionIntent::ReplaceEntity => {
            replace_segment_text_option(segment, candidate).is_some()
        }
        CorrectionIntent::RepairPrevious => {
            segment.is_some() && candidate.replacement_hint.is_some()
        }
        CorrectionIntent::UndoLast | CorrectionIntent::UndoTarget => segment.is_some(),
        CorrectionIntent::Literal | CorrectionIntent::Uncertain => true,
    }
}

fn replace_segment_text(segment: &LedgerSegment, candidate: &IntentCandidate) -> Option<String> {
    replace_segment_text_option(Some(segment), candidate)
}

fn replace_segment_text_option(
    segment: Option<&LedgerSegment>,
    candidate: &IntentCandidate,
) -> Option<String> {
    let segment = segment?;
    let target = candidate.target_hint.as_deref()?;
    let replacement = candidate.replacement_hint.as_deref()?;
    if target.is_empty() || !segment.committed_text.contains(target) {
        return None;
    }
    Some(segment.committed_text.replacen(target, replacement, 1))
}

fn delete_segment_events(segment_text: &str, replacement_text: Option<&str>) -> Vec<InputEvent> {
    let mut events = Vec::new();
    let chars = segment_text.chars().count();
    if chars > 0 {
        events.push(InputEvent::DeleteBeforeCursor { chars });
    }
    if let Some(text) = replacement_text {
        if !text.is_empty() {
            events.push(InputEvent::Commit {
                text: text.to_string(),
            });
        }
    }
    events
}

fn push_check(checks: &mut Vec<GateCheck>, code: &str, passed: bool) {
    checks.push(GateCheck {
        code: code.to_string(),
        passed,
    });
}

fn literal_decision(candidate: IntentCandidate, reason_code: &str) -> CorrectionDecision {
    CorrectionDecision {
        applied: false,
        action: CorrectionAction::Literal,
        gate_checks: Vec::new(),
        reason_code: reason_code.to_string(),
        candidate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(id: &str, text: &str, committed_at_ms: u64) -> LedgerSegment {
        LedgerSegment {
            id: id.to_string(),
            session_id: "s-1".to_string(),
            committed_text: text.to_string(),
            normalized_text: normalize_text(text),
            token_start: 0,
            token_end: 1,
            source: SegmentSource::AsrStable,
            committed_at_ms,
            cursor_context_hash: cursor_context_hash(text),
            frozen: false,
        }
    }

    #[test]
    fn rule_classifier_rejects_not_negative_false_positives() {
        let classifier = RuleIntentClassifier;
        assert_eq!(
            classifier.classify("这不是问题").intent,
            CorrectionIntent::Literal
        );
        assert_eq!(
            classifier.classify("我说的不是不对").intent,
            CorrectionIntent::Literal
        );
    }

    #[test]
    fn rule_classifier_extracts_replace_and_repair() {
        let classifier = RuleIntentClassifier;
        let replace = classifier.classify("三点不对,四点");
        assert_eq!(replace.intent, CorrectionIntent::ReplaceEntity);
        assert_eq!(replace.target_hint.as_deref(), Some("三点"));
        assert_eq!(replace.replacement_hint.as_deref(), Some("四点"));

        let repair = classifier.classify("不对,四点");
        assert_eq!(repair.intent, CorrectionIntent::RepairPrevious);
        assert_eq!(repair.replacement_hint.as_deref(), Some("四点"));
    }

    #[test]
    fn safety_gate_allows_undo_last_when_ledger_and_surrounding_match() {
        let mut ledger = InjectionLedger::default();
        ledger.append(segment("seg-1", "今天下午三点开会", 0));
        let candidate = RuleIntentClassifier.classify("刚才那句删掉");
        let decision = SafetyGate.evaluate(
            &ledger,
            candidate,
            &SafetyGateContext {
                surrounding_text: Some("前缀今天下午三点开会".to_string()),
                ..SafetyGateContext::default()
            },
        );

        assert!(decision.applied);
        assert!(matches!(decision.action, CorrectionAction::Delete { .. }));
    }

    #[test]
    fn safety_gate_rejects_when_surrounding_was_manually_edited() {
        let mut ledger = InjectionLedger::default();
        ledger.append(segment("seg-1", "今天下午三点开会", 0));
        let candidate = RuleIntentClassifier.classify("刚才那句删掉");
        let decision = SafetyGate.evaluate(
            &ledger,
            candidate,
            &SafetyGateContext {
                surrounding_text: Some("用户改成了别的内容".to_string()),
                ..SafetyGateContext::default()
            },
        );

        assert!(!decision.applied);
        assert!(decision
            .gate_checks
            .iter()
            .any(|check| { check.code == "surrounding_matches_ledger_tail" && !check.passed }));
    }

    #[test]
    fn safety_gate_rejects_disabled_or_low_confidence() {
        let mut ledger = InjectionLedger::default();
        ledger.append(segment("seg-1", "今天下午三点开会", 0));
        let mut candidate = RuleIntentClassifier.classify("刚才那句删掉");
        candidate.confidence = 0.5;
        let decision = SafetyGate.evaluate(
            &ledger,
            candidate,
            &SafetyGateContext {
                correction_enabled: false,
                surrounding_text: Some("今天下午三点开会".to_string()),
                ..SafetyGateContext::default()
            },
        );

        assert!(!decision.applied);
        assert!(decision
            .gate_checks
            .iter()
            .any(|check| check.code == "correction_enabled" && !check.passed));
        assert!(decision
            .gate_checks
            .iter()
            .any(|check| check.code == "confidence_threshold" && !check.passed));
    }

    #[test]
    fn safety_gate_replaces_exact_target_in_unfrozen_segment() {
        let mut ledger = InjectionLedger::default();
        ledger.append(segment("seg-1", "今天下午三点开会", 0));
        let candidate = RuleIntentClassifier.classify("三点不对,四点");
        let decision = SafetyGate.evaluate(
            &ledger,
            candidate,
            &SafetyGateContext {
                surrounding_text: Some("今天下午三点开会".to_string()),
                ..SafetyGateContext::default()
            },
        );

        assert!(decision.applied);
        assert_eq!(
            decision.action,
            CorrectionAction::Replace {
                segment_id: "seg-1".to_string(),
                segment_text: "今天下午三点开会".to_string(),
                target: "三点".to_string(),
                replacement: "四点".to_string(),
                replacement_text: "今天下午四点开会".to_string(),
            }
        );
        assert_eq!(
            decision.to_input_events(),
            vec![
                InputEvent::DeleteBeforeCursor { chars: 8 },
                InputEvent::Commit {
                    text: "今天下午四点开会".to_string(),
                }
            ]
        );
    }

    #[test]
    fn correction_decision_projects_delete_to_input_event() {
        let mut ledger = InjectionLedger::default();
        ledger.append(segment("seg-1", "今天下午三点开会", 0));
        let candidate = RuleIntentClassifier.classify("刚才那句删掉");
        let decision = SafetyGate.evaluate(
            &ledger,
            candidate,
            &SafetyGateContext {
                surrounding_text: Some("今天下午三点开会".to_string()),
                ..SafetyGateContext::default()
            },
        );

        assert_eq!(
            decision.to_input_events(),
            vec![InputEvent::DeleteBeforeCursor { chars: 8 }]
        );
    }

    #[test]
    fn safety_gate_rejects_unresolvable_replacement_range() {
        let mut ledger = InjectionLedger::default();
        ledger.append(segment("seg-1", "今天下午三 点开会", 0));
        let candidate = RuleIntentClassifier.classify("三点不对,四点");
        let decision = SafetyGate.evaluate(
            &ledger,
            candidate,
            &SafetyGateContext {
                surrounding_text: Some("今天下午三 点开会".to_string()),
                ..SafetyGateContext::default()
            },
        );

        assert!(!decision.applied);
        assert!(decision
            .gate_checks
            .iter()
            .any(|check| { check.code == "replacement_text_resolvable" && !check.passed }));
        assert!(decision.to_input_events().is_empty());
    }

    #[test]
    fn frozen_segments_are_not_targets() {
        let mut ledger = InjectionLedger::default();
        ledger.append(segment("seg-1", "今天下午三点开会", 0));
        ledger.freeze_all();
        let candidate = RuleIntentClassifier.classify("刚才那句删掉");
        let decision = SafetyGate.evaluate(
            &ledger,
            candidate,
            &SafetyGateContext {
                surrounding_text: Some("今天下午三点开会".to_string()),
                ..SafetyGateContext::default()
            },
        );

        assert!(!decision.applied);
        assert!(decision
            .gate_checks
            .iter()
            .any(|check| { check.code == "target_in_unfrozen_ledger" && !check.passed }));
    }

    #[test]
    fn freeze_segment_marks_specific_segment_only() {
        let mut ledger = InjectionLedger::default();
        ledger.append(segment("seg-1", "今天下午三点开会", 0));
        ledger.append(segment("seg-2", "明天上午十点复盘", 1));

        assert!(ledger.freeze_segment("seg-1"));
        assert!(!ledger.freeze_segment("missing"));
        let states: Vec<_> = ledger
            .segments()
            .map(|segment| (segment.id.as_str(), segment.frozen))
            .collect();
        assert_eq!(states, vec![("seg-1", true), ("seg-2", false)]);
    }

    #[test]
    fn history_keeps_recent_records() {
        let mut history = CorrectionHistory::new(1);
        let candidate = RuleIntentClassifier.classify("这不是问题");
        let decision = SafetyGate.evaluate(
            &InjectionLedger::default(),
            candidate,
            &SafetyGateContext::default(),
        );
        history.push_decision("op-1", &decision);
        history.push_decision("op-2", &decision);

        let recent = history.recent();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].operation_id, "op-2");
    }
}
