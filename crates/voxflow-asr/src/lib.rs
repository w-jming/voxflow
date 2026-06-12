use anyhow::Result;
use serde::{Deserialize, Serialize};

pub type SessionId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioFrame {
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub pcm_i16: Vec<i16>,
}

impl AudioFrame {
    pub fn mono_silence(sample_rate_hz: u32, duration_ms: u32) -> Self {
        let sample_count = (sample_rate_hz as u64 * duration_ms as u64 / 1000) as usize;
        Self {
            sample_rate_hz,
            channels: 1,
            pcm_i16: vec![0; sample_count],
        }
    }

    pub fn mono_constant(sample_rate_hz: u32, duration_ms: u32, sample: i16) -> Self {
        let sample_count = (sample_rate_hz as u64 * duration_ms as u64 / 1000) as usize;
        Self {
            sample_rate_hz,
            channels: 1,
            pcm_i16: vec![sample; sample_count],
        }
    }

    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate_hz == 0 || self.channels == 0 {
            return 0;
        }
        let samples_per_channel = self.pcm_i16.len() as u64 / self.channels as u64;
        samples_per_channel * 1000 / self.sample_rate_hz as u64
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct VadDecision {
    pub is_speech: bool,
    pub speech_started: bool,
    pub speech_ended: bool,
    pub rms: f32,
}

pub trait Vad {
    fn process_frame(&mut self, frame: &TimestampedAudioFrame) -> VadDecision;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnergyVadConfig {
    pub speech_rms_threshold: f32,
    pub silence_rms_threshold: f32,
    pub speech_start_frames: usize,
    pub speech_end_frames: usize,
}

impl Default for EnergyVadConfig {
    fn default() -> Self {
        Self {
            speech_rms_threshold: 0.02,
            silence_rms_threshold: 0.01,
            speech_start_frames: 3,
            speech_end_frames: 8,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnergyVad {
    config: EnergyVadConfig,
    in_speech: bool,
    consecutive_speech_frames: usize,
    consecutive_silence_frames: usize,
}

impl EnergyVad {
    pub fn new(config: EnergyVadConfig) -> Self {
        Self {
            config,
            in_speech: false,
            consecutive_speech_frames: 0,
            consecutive_silence_frames: 0,
        }
    }
}

impl Default for EnergyVad {
    fn default() -> Self {
        Self::new(EnergyVadConfig::default())
    }
}

impl Vad for EnergyVad {
    fn process_frame(&mut self, frame: &TimestampedAudioFrame) -> VadDecision {
        let rms = frame_rms(&frame.frame);
        if self.in_speech {
            if rms <= self.config.silence_rms_threshold {
                self.consecutive_silence_frames += 1;
            } else {
                self.consecutive_silence_frames = 0;
            }
            let speech_ended = self.consecutive_silence_frames >= self.config.speech_end_frames;
            if speech_ended {
                self.in_speech = false;
                self.consecutive_speech_frames = 0;
            }
            return VadDecision {
                is_speech: !speech_ended,
                speech_started: false,
                speech_ended,
                rms,
            };
        }

        if rms >= self.config.speech_rms_threshold {
            self.consecutive_speech_frames += 1;
        } else {
            self.consecutive_speech_frames = 0;
        }
        let speech_started = self.consecutive_speech_frames >= self.config.speech_start_frames;
        if speech_started {
            self.in_speech = true;
            self.consecutive_silence_frames = 0;
        }
        VadDecision {
            is_speech: speech_started,
            speech_started,
            speech_ended: false,
            rms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimestampedAudioFrame {
    pub elapsed_ms: u64,
    pub frame: AudioFrame,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AsrEvent {
    Partial {
        revision: u64,
        text: String,
        tokens: Vec<Token>,
    },
    Stable {
        revision: u64,
        text: String,
        token_start: usize,
        token_end: usize,
    },
    Final {
        revision: u64,
        text: String,
        segment_id: String,
    },
}

pub trait StreamingRecognizer: Send {
    fn start_session(&mut self) -> Result<SessionId>;
    fn push_audio(&mut self, session: &SessionId, frame: AudioFrame) -> Result<()>;
    fn poll_events(&mut self, session: &SessionId) -> Result<Vec<AsrEvent>>;
    fn finish_session(&mut self, session: &SessionId) -> Result<Vec<AsrEvent>>;
}

#[derive(Debug, Clone)]
pub struct MockRecognizer {
    session_counter: u64,
    script: Vec<AsrEvent>,
    emitted: bool,
}

impl Default for MockRecognizer {
    fn default() -> Self {
        Self {
            session_counter: 0,
            script: vec![
                AsrEvent::Partial {
                    revision: 1,
                    text: "jin".to_string(),
                    tokens: vec![Token {
                        text: "jin".to_string(),
                        start_ms: 120,
                        end_ms: 240,
                    }],
                },
                AsrEvent::Partial {
                    revision: 2,
                    text: "今天下午".to_string(),
                    tokens: vec![
                        Token {
                            text: "今天".to_string(),
                            start_ms: 120,
                            end_ms: 360,
                        },
                        Token {
                            text: "下午".to_string(),
                            start_ms: 360,
                            end_ms: 680,
                        },
                    ],
                },
                AsrEvent::Stable {
                    revision: 3,
                    text: "今天下午".to_string(),
                    token_start: 0,
                    token_end: 2,
                },
                AsrEvent::Final {
                    revision: 4,
                    text: "今天下午三点开会".to_string(),
                    segment_id: "seg-mock-1".to_string(),
                },
            ],
            emitted: false,
        }
    }
}

impl MockRecognizer {
    pub fn with_script(script: Vec<AsrEvent>) -> Self {
        Self {
            session_counter: 0,
            script,
            emitted: false,
        }
    }
}

impl StreamingRecognizer for MockRecognizer {
    fn start_session(&mut self) -> Result<SessionId> {
        self.session_counter += 1;
        self.emitted = false;
        Ok(format!("mock-{}", self.session_counter))
    }

    fn push_audio(&mut self, _session: &SessionId, _frame: AudioFrame) -> Result<()> {
        Ok(())
    }

    fn poll_events(&mut self, _session: &SessionId) -> Result<Vec<AsrEvent>> {
        if self.emitted {
            return Ok(Vec::new());
        }
        self.emitted = true;
        Ok(self.script.clone())
    }

    fn finish_session(&mut self, _session: &SessionId) -> Result<Vec<AsrEvent>> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StablePrefixStabilizer {
    previous_tokens: Vec<Token>,
    stable_token_count: usize,
}

impl StablePrefixStabilizer {
    pub fn new() -> Self {
        Self {
            previous_tokens: Vec::new(),
            stable_token_count: 0,
        }
    }

    pub fn observe_partial(
        &mut self,
        revision: u64,
        text: impl Into<String>,
        tokens: Vec<Token>,
    ) -> Option<AsrEvent> {
        let text = text.into();
        let common = common_token_prefix_len(&self.previous_tokens, &tokens);
        let stable_count = common.min(tokens.len());
        self.previous_tokens = tokens;

        if stable_count <= self.stable_token_count {
            return None;
        }

        let token_start = self.stable_token_count;
        self.stable_token_count = stable_count;
        Some(AsrEvent::Stable {
            revision,
            text: stable_text_prefix(&text, &self.previous_tokens[..stable_count]),
            token_start,
            token_end: stable_count,
        })
    }
}

impl Default for StablePrefixStabilizer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayReport {
    pub session_id: SessionId,
    pub frame_count: usize,
    pub audio_ms: u64,
    pub event_count: usize,
    pub vad_speech_start_ms: Option<u64>,
    pub first_partial_ms: Option<u64>,
    pub first_stable_ms: Option<u64>,
    pub final_ms: Option<u64>,
    pub first_partial_latency_ms: Option<u64>,
    pub first_stable_latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LatencyBudget {
    pub first_partial_p90_ms: u64,
    pub first_stable_p90_ms: u64,
}

impl Default for LatencyBudget {
    fn default() -> Self {
        Self {
            first_partial_p90_ms: 500,
            first_stable_p90_ms: 1_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LatencyGate {
    pub metric: String,
    pub budget_ms: u64,
    pub observed_p90_ms: Option<u64>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayCaseReport {
    pub name: String,
    pub report: ReplayReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplaySuiteReport {
    pub case_count: usize,
    pub cases: Vec<ReplayCaseReport>,
    pub gates: Vec<LatencyGate>,
    pub passed: bool,
}

impl ReplaySuiteReport {
    pub fn from_cases(cases: Vec<ReplayCaseReport>, budget: LatencyBudget) -> Self {
        let first_partial_samples = cases
            .iter()
            .filter_map(|case| case.report.first_partial_latency_ms)
            .collect::<Vec<_>>();
        let first_stable_samples = cases
            .iter()
            .filter_map(|case| case.report.first_stable_latency_ms)
            .collect::<Vec<_>>();
        let first_partial_p90 = percentile_nearest_rank(first_partial_samples, 90);
        let first_stable_p90 = percentile_nearest_rank(first_stable_samples, 90);
        let gates = vec![
            latency_gate(
                "first_partial_p90_ms",
                budget.first_partial_p90_ms,
                first_partial_p90,
            ),
            latency_gate(
                "first_stable_p90_ms",
                budget.first_stable_p90_ms,
                first_stable_p90,
            ),
        ];
        let passed = gates.iter().all(|gate| gate.passed);
        Self {
            case_count: cases.len(),
            cases,
            gates,
            passed,
        }
    }
}

#[derive(Debug, Default)]
pub struct ReplayBenchmark;

impl ReplayBenchmark {
    pub fn run<R>(
        &self,
        recognizer: &mut R,
        frames: impl IntoIterator<Item = TimestampedAudioFrame>,
    ) -> Result<ReplayReport>
    where
        R: StreamingRecognizer,
    {
        let session_id = recognizer.start_session()?;
        let mut report = ReplayReport {
            session_id: session_id.clone(),
            frame_count: 0,
            audio_ms: 0,
            event_count: 0,
            vad_speech_start_ms: None,
            first_partial_ms: None,
            first_stable_ms: None,
            final_ms: None,
            first_partial_latency_ms: None,
            first_stable_latency_ms: None,
        };

        for timestamped in frames {
            report.frame_count += 1;
            report.audio_ms = report
                .audio_ms
                .max(timestamped.elapsed_ms + timestamped.frame.duration_ms());
            recognizer.push_audio(&session_id, timestamped.frame)?;
            let events = recognizer.poll_events(&session_id)?;
            record_events(&mut report, timestamped.elapsed_ms, &events);
        }

        let finish_events = recognizer.finish_session(&session_id)?;
        let finish_elapsed_ms = report.audio_ms;
        record_events(&mut report, finish_elapsed_ms, &finish_events);
        Ok(report)
    }

    pub fn run_with_vad<R, V>(
        &self,
        recognizer: &mut R,
        vad: &mut V,
        frames: impl IntoIterator<Item = TimestampedAudioFrame>,
    ) -> Result<ReplayReport>
    where
        R: StreamingRecognizer,
        V: Vad,
    {
        let session_id = recognizer.start_session()?;
        let mut report = ReplayReport {
            session_id: session_id.clone(),
            frame_count: 0,
            audio_ms: 0,
            event_count: 0,
            vad_speech_start_ms: None,
            first_partial_ms: None,
            first_stable_ms: None,
            final_ms: None,
            first_partial_latency_ms: None,
            first_stable_latency_ms: None,
        };

        for timestamped in frames {
            report.frame_count += 1;
            report.audio_ms = report
                .audio_ms
                .max(timestamped.elapsed_ms + timestamped.frame.duration_ms());
            let decision = vad.process_frame(&timestamped);
            if decision.speech_started {
                report
                    .vad_speech_start_ms
                    .get_or_insert(timestamped.elapsed_ms);
            }
            recognizer.push_audio(&session_id, timestamped.frame)?;
            let events = recognizer.poll_events(&session_id)?;
            record_events(&mut report, timestamped.elapsed_ms, &events);
        }

        let finish_events = recognizer.finish_session(&session_id)?;
        let finish_elapsed_ms = report.audio_ms;
        record_events(&mut report, finish_elapsed_ms, &finish_events);
        fill_latency_fields(&mut report);
        Ok(report)
    }
}

fn record_events(report: &mut ReplayReport, elapsed_ms: u64, events: &[AsrEvent]) {
    report.event_count += events.len();
    for event in events {
        match event {
            AsrEvent::Partial { .. } => {
                report.first_partial_ms.get_or_insert(elapsed_ms);
            }
            AsrEvent::Stable { .. } => {
                report.first_stable_ms.get_or_insert(elapsed_ms);
            }
            AsrEvent::Final { .. } => {
                report.final_ms = Some(elapsed_ms);
            }
        }
    }
}

fn fill_latency_fields(report: &mut ReplayReport) {
    if let (Some(start), Some(first_partial)) =
        (report.vad_speech_start_ms, report.first_partial_ms)
    {
        report.first_partial_latency_ms = first_partial.checked_sub(start);
    }
    if let (Some(start), Some(first_stable)) = (report.vad_speech_start_ms, report.first_stable_ms)
    {
        report.first_stable_latency_ms = first_stable.checked_sub(start);
    }
}

fn percentile_nearest_rank(mut samples: Vec<u64>, percentile: u8) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    samples.sort_unstable();
    let percentile = percentile.clamp(1, 100) as usize;
    let rank = (percentile * samples.len()).div_ceil(100);
    samples.get(rank.saturating_sub(1)).copied()
}

fn latency_gate(metric: &str, budget_ms: u64, observed_p90_ms: Option<u64>) -> LatencyGate {
    LatencyGate {
        metric: metric.to_string(),
        budget_ms,
        observed_p90_ms,
        passed: observed_p90_ms
            .map(|observed| observed <= budget_ms)
            .unwrap_or(false),
    }
}

fn frame_rms(frame: &AudioFrame) -> f32 {
    if frame.pcm_i16.is_empty() {
        return 0.0;
    }
    let sum_squares = frame
        .pcm_i16
        .iter()
        .map(|sample| {
            let normalized = *sample as f64 / i16::MAX as f64;
            normalized * normalized
        })
        .sum::<f64>();
    (sum_squares / frame.pcm_i16.len() as f64).sqrt() as f32
}

fn common_token_prefix_len(left: &[Token], right: &[Token]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left.text == right.text)
        .count()
}

fn stable_text_prefix(full_text: &str, tokens: &[Token]) -> String {
    let token_text = tokens
        .iter()
        .map(|token| token.text.as_str())
        .collect::<String>();
    if full_text.starts_with(&token_text) {
        token_text
    } else {
        full_text.chars().take(token_text.chars().count()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(text: &str, start_ms: u32, end_ms: u32) -> Token {
        Token {
            text: text.to_string(),
            start_ms,
            end_ms,
        }
    }

    #[test]
    fn mock_recognizer_replays_partial_stable_final_once() {
        let mut recognizer = MockRecognizer::default();
        let session = recognizer.start_session().unwrap();
        let first = recognizer.poll_events(&session).unwrap();
        assert!(matches!(first[0], AsrEvent::Partial { .. }));
        assert!(first
            .iter()
            .any(|event| matches!(event, AsrEvent::Stable { .. })));
        assert!(matches!(first.last().unwrap(), AsrEvent::Final { .. }));
        assert!(recognizer.poll_events(&session).unwrap().is_empty());
    }

    #[test]
    fn stable_prefix_uses_consecutive_partial_lcp() {
        let mut stabilizer = StablePrefixStabilizer::new();
        assert!(stabilizer
            .observe_partial(
                1,
                "今天下午",
                vec![token("今天", 0, 240), token("下午", 240, 480)]
            )
            .is_none());
        assert_eq!(
            stabilizer.observe_partial(
                2,
                "今天下午三点",
                vec![
                    token("今天", 0, 240),
                    token("下午", 240, 480),
                    token("三点", 480, 720),
                ],
            ),
            Some(AsrEvent::Stable {
                revision: 2,
                text: "今天下午".to_string(),
                token_start: 0,
                token_end: 2,
            })
        );
        assert_eq!(
            stabilizer.observe_partial(
                3,
                "今天下午三点开会",
                vec![
                    token("今天", 0, 240),
                    token("下午", 240, 480),
                    token("三点", 480, 720),
                    token("开会", 720, 960),
                ],
            ),
            Some(AsrEvent::Stable {
                revision: 3,
                text: "今天下午三点".to_string(),
                token_start: 2,
                token_end: 3,
            })
        );
    }

    #[test]
    fn replay_benchmark_records_first_event_latencies() {
        let mut recognizer = MockRecognizer::default();
        let frames = vec![
            TimestampedAudioFrame {
                elapsed_ms: 0,
                frame: AudioFrame::mono_silence(16_000, 20),
            },
            TimestampedAudioFrame {
                elapsed_ms: 20,
                frame: AudioFrame::mono_silence(16_000, 20),
            },
        ];
        let report = ReplayBenchmark.run(&mut recognizer, frames).unwrap();
        assert_eq!(report.frame_count, 2);
        assert_eq!(report.audio_ms, 40);
        assert_eq!(report.event_count, 4);
        assert_eq!(report.first_partial_ms, Some(0));
        assert_eq!(report.first_stable_ms, Some(0));
        assert_eq!(report.final_ms, Some(0));
        assert_eq!(report.vad_speech_start_ms, None);
        assert_eq!(report.first_partial_latency_ms, None);
    }

    #[test]
    fn energy_vad_uses_hysteresis_for_start_and_end() {
        let mut vad = EnergyVad::new(EnergyVadConfig {
            speech_rms_threshold: 0.02,
            silence_rms_threshold: 0.01,
            speech_start_frames: 2,
            speech_end_frames: 2,
        });
        let silence = TimestampedAudioFrame {
            elapsed_ms: 0,
            frame: AudioFrame::mono_silence(16_000, 20),
        };
        let speech_1 = TimestampedAudioFrame {
            elapsed_ms: 20,
            frame: AudioFrame::mono_constant(16_000, 20, 2000),
        };
        let speech_2 = TimestampedAudioFrame {
            elapsed_ms: 40,
            frame: AudioFrame::mono_constant(16_000, 20, 2000),
        };
        assert!(!vad.process_frame(&silence).speech_started);
        assert!(!vad.process_frame(&speech_1).speech_started);
        let decision = vad.process_frame(&speech_2);
        assert!(decision.is_speech);
        assert!(decision.speech_started);
        assert!(!vad.process_frame(&silence).speech_ended);
        assert!(vad.process_frame(&silence).speech_ended);
    }

    #[test]
    fn replay_benchmark_reports_latencies_from_vad_start() {
        struct DelayedRecognizer {
            polls: usize,
        }

        impl StreamingRecognizer for DelayedRecognizer {
            fn start_session(&mut self) -> Result<SessionId> {
                Ok("delayed".to_string())
            }

            fn push_audio(&mut self, _session: &SessionId, _frame: AudioFrame) -> Result<()> {
                Ok(())
            }

            fn poll_events(&mut self, _session: &SessionId) -> Result<Vec<AsrEvent>> {
                self.polls += 1;
                if self.polls == 3 {
                    Ok(vec![
                        AsrEvent::Partial {
                            revision: 1,
                            text: "今天".to_string(),
                            tokens: vec![token("今天", 0, 240)],
                        },
                        AsrEvent::Stable {
                            revision: 2,
                            text: "今天".to_string(),
                            token_start: 0,
                            token_end: 1,
                        },
                    ])
                } else {
                    Ok(Vec::new())
                }
            }

            fn finish_session(&mut self, _session: &SessionId) -> Result<Vec<AsrEvent>> {
                Ok(Vec::new())
            }
        }

        let mut recognizer = DelayedRecognizer { polls: 0 };
        let mut vad = EnergyVad::new(EnergyVadConfig {
            speech_rms_threshold: 0.02,
            silence_rms_threshold: 0.01,
            speech_start_frames: 1,
            speech_end_frames: 8,
        });
        let frames = vec![
            TimestampedAudioFrame {
                elapsed_ms: 20,
                frame: AudioFrame::mono_constant(16_000, 20, 2000),
            },
            TimestampedAudioFrame {
                elapsed_ms: 40,
                frame: AudioFrame::mono_constant(16_000, 20, 2000),
            },
            TimestampedAudioFrame {
                elapsed_ms: 60,
                frame: AudioFrame::mono_constant(16_000, 20, 2000),
            },
        ];
        let report = ReplayBenchmark
            .run_with_vad(&mut recognizer, &mut vad, frames)
            .unwrap();
        assert_eq!(report.vad_speech_start_ms, Some(20));
        assert_eq!(report.first_partial_ms, Some(60));
        assert_eq!(report.first_stable_ms, Some(60));
        assert_eq!(report.first_partial_latency_ms, Some(40));
        assert_eq!(report.first_stable_latency_ms, Some(40));
    }

    #[test]
    fn replay_suite_report_applies_nearest_rank_p90_gates() {
        let cases = vec![
            ReplayCaseReport {
                name: "fast".to_string(),
                report: ReplayReport {
                    session_id: "fast".to_string(),
                    frame_count: 1,
                    audio_ms: 20,
                    event_count: 2,
                    vad_speech_start_ms: Some(0),
                    first_partial_ms: Some(100),
                    first_stable_ms: Some(200),
                    final_ms: Some(220),
                    first_partial_latency_ms: Some(100),
                    first_stable_latency_ms: Some(200),
                },
            },
            ReplayCaseReport {
                name: "slow".to_string(),
                report: ReplayReport {
                    session_id: "slow".to_string(),
                    frame_count: 1,
                    audio_ms: 20,
                    event_count: 2,
                    vad_speech_start_ms: Some(0),
                    first_partial_ms: Some(650),
                    first_stable_ms: Some(900),
                    final_ms: Some(920),
                    first_partial_latency_ms: Some(650),
                    first_stable_latency_ms: Some(900),
                },
            },
        ];
        let report = ReplaySuiteReport::from_cases(
            cases,
            LatencyBudget {
                first_partial_p90_ms: 500,
                first_stable_p90_ms: 1_000,
            },
        );

        assert!(!report.passed);
        assert_eq!(report.case_count, 2);
        assert_eq!(report.gates[0].observed_p90_ms, Some(650));
        assert!(!report.gates[0].passed);
        assert_eq!(report.gates[1].observed_p90_ms, Some(900));
        assert!(report.gates[1].passed);
    }

    #[test]
    fn replay_suite_report_fails_when_latency_samples_are_missing() {
        let report = ReplaySuiteReport::from_cases(Vec::new(), LatencyBudget::default());
        assert!(!report.passed);
        assert_eq!(report.gates[0].observed_p90_ms, None);
        assert!(!report.gates[0].passed);
    }
}
