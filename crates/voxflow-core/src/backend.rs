//! ASR backend factory (D-22): builds the recognizer selected by
//! `config.asr.backend`. Construction is cheap for every backend — the
//! qwen3 sidecar only spawns (and loads the model) on the first session.

use anyhow::{Context, Result};

use crate::config::{AsrBackend, Config};
use crate::paths::VoxflowPaths;
use crate::recognizer::{MockRecognizer, StreamingRecognizer};

pub fn build_recognizer(
    config: &Config,
    paths: &VoxflowPaths,
) -> Result<Box<dyn StreamingRecognizer>> {
    match config.asr.backend {
        AsrBackend::Mock => Ok(Box::new(MockRecognizer::default())),
        AsrBackend::Qwen3Vllm => {
            let qwen3 = &config.asr.qwen3;
            let script = voxflow_asr_qwen3::resolve_sidecar_script(&qwen3.sidecar_script).context(
                "qwen3 sidecar script not found; set asr.qwen3.sidecar_script in config",
            )?;
            let mut options = voxflow_asr_qwen3::Qwen3SidecarOptions::new(script);
            options.python = qwen3.python.clone();
            options.model = qwen3.model.clone();
            options.gpu_memory_utilization = qwen3.gpu_memory_utilization;
            options.chunk_size_sec = qwen3.chunk_size_sec;
            options.unfixed_chunk_num = qwen3.unfixed_chunk_num;
            options.unfixed_token_num = qwen3.unfixed_token_num;
            options.max_new_tokens = qwen3.max_new_tokens;
            options.max_model_len = qwen3.max_model_len;
            Ok(Box::new(voxflow_asr_qwen3::Qwen3SidecarRecognizer::new(
                options,
            )))
        }
        AsrBackend::VolcanoApi => {
            let volcano = &config.asr.volcano;
            let recognizer =
                voxflow_asr_volcano::VolcanoRecognizer::new(voxflow_asr_volcano::VolcanoOptions {
                    endpoint: volcano.endpoint.clone(),
                    app_key: volcano.app_key.clone(),
                    access_key: volcano.access_key.clone(),
                    resource_id: volcano.resource_id.clone(),
                    model_name: volcano.model_name.clone(),
                    enable_itn: volcano.enable_itn,
                    enable_punc: volcano.enable_punc,
                    sample_rate_hz: 16_000,
                })?;
            Ok(Box::new(recognizer))
        }
        AsrBackend::ZipformerLocal => {
            let model_dir = paths.models.join(&config.models.active_asr);
            let sherpa_config =
                voxflow_asr_sherpa::SherpaStreamingConfig::from_model_dir(&model_dir)
                    .with_context(|| {
                        format!(
                            "zipformer model not installed at {} (model.download first)",
                            model_dir.display()
                        )
                    })?;
            Ok(Box::new(
                voxflow_asr_sherpa::SherpaStreamingRecognizer::new(sherpa_config)?,
            ))
        }
    }
}

pub fn backend_label(backend: AsrBackend) -> &'static str {
    match backend {
        AsrBackend::Qwen3Vllm => "qwen3_vllm",
        AsrBackend::VolcanoApi => "volcano_api",
        AsrBackend::ZipformerLocal => "zipformer_local",
        AsrBackend::Mock => "mock",
    }
}
