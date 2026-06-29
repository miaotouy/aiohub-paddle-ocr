mod mnn;
mod onnx;

use crate::errors::{ImageError, SidecarError};
use crate::model_registry::{build_engine_key, validate_model_files, ModelPaths, ModelProfile};
use crate::protocol::{send_event_with_id, OcrLine, OcrOptions};
use mnn::MnnOcrBackend;
use onnx::OnnxRuntimeBackend;
use std::sync::Arc;

pub(crate) trait OcrBackend: Send + Sync {
    fn id(&self) -> &'static str;
    fn recognize(&self, image_bytes: &[u8]) -> Result<Vec<OcrLine>, ImageError>;
}

pub(crate) struct EngineHolder {
    current_engine_key: Option<String>,
    engine: Option<Arc<dyn OcrBackend>>,
}

impl EngineHolder {
    pub(crate) fn new() -> Self {
        Self {
            current_engine_key: None,
            engine: None,
        }
    }

    /// 获取或加载引擎。如果 backend、profile 或运行时参数不同，则切换。
    /// 返回 Arc 以便在并行场景中克隆共享。
    pub(crate) fn get_or_load(
        &mut self,
        profile: &ModelProfile,
        options: Option<&OcrOptions>,
    ) -> Result<Arc<dyn OcrBackend>, SidecarError> {
        let engine_key = build_engine_key(profile, options);

        if self.current_engine_key.as_deref() == Some(&engine_key) {
            // 命中缓存
            if let Some(ref engine) = self.engine {
                return Ok(Arc::clone(engine));
            }
        }

        // 需要加载新模型
        let model_paths = validate_model_files(profile)?;
        send_event_with_id(
            None,
            "progress",
            None,
            serde_json::json!({
                "backend": profile.backend.id(),
                "profile": profile.id.as_str(),
                "message": format!("正在切换模型: {} ({})", profile.name.as_str(), profile.backend),
                "percent": 5
            }),
        );
        let engine: Arc<dyn OcrBackend> = match model_paths {
            ModelPaths::Mnn(paths) => Arc::new(MnnOcrBackend::load(&paths, options)?),
            ModelPaths::Onnx(paths) => Arc::new(OnnxRuntimeBackend::load(&paths)?),
        };

        self.current_engine_key = Some(engine_key);
        self.engine = Some(engine);

        Ok(Arc::clone(self.engine.as_ref().unwrap()))
    }
}
