use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::recognizer::{
    AsrEvent, SessionId, StablePrefixStabilizer, StreamingRecognizer, TimestampedAudioFrame, Vad,
};
use voxflow_audio::{measure_level, AudioSource, CaptureConfig};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamingPipelineOptions {
    pub generate_stable_from_partials: bool,
}

impl Default for StreamingPipelineOptions {
    fn default() -> Self {
        Self {
            generate_stable_from_partials: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamingPipelineReport {
    pub session_id: SessionId,
    pub frame_count: usize,
    pub audio_ms: u64,
    pub event_count: usize,
    pub generated_stable_count: usize,
    pub max_audio_peak: f32,
    pub max_audio_rms: f32,
    pub vad_speech_start_ms: Option<u64>,
    pub first_partial_ms: Option<u64>,
    pub first_stable_ms: Option<u64>,
    pub final_ms: Option<u64>,
    pub first_partial_latency_ms: Option<u64>,
    pub first_stable_latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamingPipelineRun {
    pub report: StreamingPipelineReport,
    pub events: Vec<AsrEvent>,
}

pub fn run_streaming_pipeline<S, R, V>(
    source: &mut S,
    recognizer: &mut R,
    vad: &mut V,
    capture_config: CaptureConfig,
    options: StreamingPipelineOptions,
) -> Result<StreamingPipelineRun>
where
    S: AudioSource,
    R: StreamingRecognizer,
    V: Vad,
{
    source.start(capture_config)?;
    let result = run_started_streaming_pipeline(source, recognizer, vad, options);
    let stop_result = source.stop();
    match (result, stop_result) {
        (Ok(run), Ok(())) => Ok(run),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}

fn run_started_streaming_pipeline<S, R, V>(
    source: &mut S,
    recognizer: &mut R,
    vad: &mut V,
    options: StreamingPipelineOptions,
) -> Result<StreamingPipelineRun>
where
    S: AudioSource,
    R: StreamingRecognizer,
    V: Vad,
{
    let session_id = recognizer.start_session()?;
    let mut report = StreamingPipelineReport {
        session_id: session_id.clone(),
        frame_count: 0,
        audio_ms: 0,
        event_count: 0,
        generated_stable_count: 0,
        max_audio_peak: 0.0,
        max_audio_rms: 0.0,
        vad_speech_start_ms: None,
        first_partial_ms: None,
        first_stable_ms: None,
        final_ms: None,
        first_partial_latency_ms: None,
        first_stable_latency_ms: None,
    };
    let mut events = Vec::new();
    let mut stabilizer = StablePrefixStabilizer::new();

    while let Some(timestamped) = source.next_frame()? {
        report_frame(&mut report, &timestamped);
        let decision = vad.process_frame(&timestamped);
        if decision.speech_started {
            report
                .vad_speech_start_ms
                .get_or_insert(timestamped.elapsed_ms);
        }

        recognizer.push_audio(&session_id, timestamped.frame)?;
        let polled_events = recognizer.poll_events(&session_id)?;
        append_pipeline_events(
            &mut report,
            &mut events,
            &mut stabilizer,
            timestamped.elapsed_ms,
            polled_events,
            options.generate_stable_from_partials,
        );
    }

    let finish_elapsed_ms = report.audio_ms;
    let finish_events = recognizer.finish_session(&session_id)?;
    append_pipeline_events(
        &mut report,
        &mut events,
        &mut stabilizer,
        finish_elapsed_ms,
        finish_events,
        options.generate_stable_from_partials,
    );
    fill_pipeline_latency_fields(&mut report);

    Ok(StreamingPipelineRun { report, events })
}

fn report_frame(report: &mut StreamingPipelineReport, timestamped: &TimestampedAudioFrame) {
    report.frame_count += 1;
    report.audio_ms = report
        .audio_ms
        .max(timestamped.elapsed_ms + timestamped.frame.duration_ms());
    let level = measure_level(&timestamped.frame);
    report.max_audio_peak = report.max_audio_peak.max(level.peak);
    report.max_audio_rms = report.max_audio_rms.max(level.rms);
}

fn append_pipeline_events(
    report: &mut StreamingPipelineReport,
    output: &mut Vec<AsrEvent>,
    stabilizer: &mut StablePrefixStabilizer,
    elapsed_ms: u64,
    events: Vec<AsrEvent>,
    generate_stable_from_partials: bool,
) {
    for event in events {
        let generated_stable = if generate_stable_from_partials {
            generated_stable_event(stabilizer, &event)
        } else {
            None
        };
        record_pipeline_event(report, elapsed_ms, &event);
        output.push(event);

        if let Some(stable) = generated_stable {
            report.generated_stable_count += 1;
            record_pipeline_event(report, elapsed_ms, &stable);
            output.push(stable);
        }
    }
}

fn generated_stable_event(
    stabilizer: &mut StablePrefixStabilizer,
    event: &AsrEvent,
) -> Option<AsrEvent> {
    match event {
        AsrEvent::Partial {
            revision,
            text,
            tokens,
        } => stabilizer.observe_partial(*revision, text.clone(), tokens.clone()),
        AsrEvent::Stable { .. } | AsrEvent::Final { .. } => None,
    }
}

fn record_pipeline_event(report: &mut StreamingPipelineReport, elapsed_ms: u64, event: &AsrEvent) {
    report.event_count += 1;
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

fn fill_pipeline_latency_fields(report: &mut StreamingPipelineReport) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recognizer::{AudioFrame, EnergyVad, EnergyVadConfig, Token};
    use anyhow::Result;
    use voxflow_audio::SyntheticAudioSource;

    fn token(text: &str, start_ms: u32, end_ms: u32) -> Token {
        Token {
            text: text.to_string(),
            start_ms,
            end_ms,
        }
    }

    #[test]
    fn pipeline_runs_synthetic_audio_through_vad_and_recognizer() {
        let mut source = SyntheticAudioSource::constant(60, 2000);
        let mut recognizer = crate::recognizer::MockRecognizer::default();
        let mut vad = EnergyVad::new(EnergyVadConfig {
            speech_start_frames: 1,
            ..EnergyVadConfig::default()
        });
        let run = run_streaming_pipeline(
            &mut source,
            &mut recognizer,
            &mut vad,
            CaptureConfig {
                frame_duration_ms: 20,
                ..CaptureConfig::default()
            },
            StreamingPipelineOptions::default(),
        )
        .unwrap();

        assert_eq!(run.report.frame_count, 3);
        assert_eq!(run.report.audio_ms, 60);
        assert_eq!(run.report.vad_speech_start_ms, Some(0));
        assert_eq!(run.report.first_partial_latency_ms, Some(0));
        assert!(run.report.max_audio_peak > 0.0);
        assert!(run
            .events
            .iter()
            .any(|event| matches!(event, AsrEvent::Partial { .. })));
        assert!(run
            .events
            .iter()
            .any(|event| matches!(event, AsrEvent::Stable { .. })));
        assert!(run
            .events
            .iter()
            .any(|event| matches!(event, AsrEvent::Final { .. })));
    }

    #[test]
    fn pipeline_can_generate_stable_events_from_repeated_partial_prefix() {
        struct PartialOnlyRecognizer {
            polls: usize,
        }

        impl StreamingRecognizer for PartialOnlyRecognizer {
            fn start_session(&mut self) -> Result<SessionId> {
                Ok("partial-only".to_string())
            }

            fn push_audio(&mut self, _session: &SessionId, _frame: AudioFrame) -> Result<()> {
                Ok(())
            }

            fn poll_events(&mut self, _session: &SessionId) -> Result<Vec<AsrEvent>> {
                self.polls += 1;
                match self.polls {
                    1 => Ok(vec![AsrEvent::Partial {
                        revision: 1,
                        text: "今天".to_string(),
                        tokens: vec![token("今天", 0, 240)],
                    }]),
                    2 => Ok(vec![AsrEvent::Partial {
                        revision: 2,
                        text: "今天下午".to_string(),
                        tokens: vec![token("今天", 0, 240), token("下午", 240, 480)],
                    }]),
                    _ => Ok(Vec::new()),
                }
            }

            fn finish_session(&mut self, _session: &SessionId) -> Result<Vec<AsrEvent>> {
                Ok(Vec::new())
            }
        }

        let mut source = SyntheticAudioSource::constant(40, 2000);
        let mut recognizer = PartialOnlyRecognizer { polls: 0 };
        let mut vad = EnergyVad::new(EnergyVadConfig {
            speech_start_frames: 1,
            ..EnergyVadConfig::default()
        });
        let run = run_streaming_pipeline(
            &mut source,
            &mut recognizer,
            &mut vad,
            CaptureConfig {
                frame_duration_ms: 20,
                ..CaptureConfig::default()
            },
            StreamingPipelineOptions::default(),
        )
        .unwrap();

        assert_eq!(run.report.generated_stable_count, 1);
        assert_eq!(run.report.first_stable_ms, Some(20));
        assert_eq!(run.report.first_stable_latency_ms, Some(20));
        assert!(matches!(run.events.last(), Some(AsrEvent::Stable { .. })));
    }
}
