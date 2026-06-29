use super::OcrBackend;
use crate::errors::{ImageError, SidecarError};
use crate::model_registry::MnnModelPaths;
use crate::native_stdout::with_native_stdout_suppressed;
use crate::protocol::{OcrLine, OcrOptions};
use ocr_rs::{DetOptions, OcrEngine, OcrEngineConfig};
use std::sync::Mutex;

/// MNN 后端包装 ocr_rs 引擎，通过内部 Mutex 保证 Send + Sync。
pub(crate) struct MnnOcrBackend {
    engine: Mutex<OcrEngine>,
}

impl MnnOcrBackend {
    pub(crate) fn load(
        model_paths: &MnnModelPaths,
        options: Option<&OcrOptions>,
    ) -> Result<Self, SidecarError> {
        let config = build_engine_config(options);
        let engine = with_native_stdout_suppressed(|| {
            OcrEngine::new(
                model_paths.det_path.clone(),
                model_paths.rec_path.clone(),
                model_paths.dict_path.clone(),
                Some(config),
            )
        })
        .map_err(|error| SidecarError::EngineLoadFailed(error.to_string()))?;

        Ok(Self {
            engine: Mutex::new(engine),
        })
    }
}

impl OcrBackend for MnnOcrBackend {
    fn id(&self) -> &'static str {
        "mnn-ocr-rs"
    }

    fn recognize(&self, image_bytes: &[u8]) -> Result<Vec<OcrLine>, ImageError> {
        let image = image::load_from_memory(image_bytes)
            .map_err(|error| ImageError::InvalidImage(error.to_string()))?;
        let engine = self.engine.lock().expect("OCR 引擎锁被异常持有");
        let results = engine
            .recognize(&image)
            .map_err(|error| ImageError::InferenceFailed(error.to_string()))?;

        Ok(results
            .into_iter()
            .map(|result| OcrLine {
                text: result.text,
                score: result.confidence,
                bbox: bbox_points(&result.bbox),
            })
            .collect())
    }
}

fn build_engine_config(options: Option<&OcrOptions>) -> OcrEngineConfig {
    let mut det_options = DetOptions::default();

    if let Some(options) = options {
        if let Some(limit) = options.det_limit_side_len {
            det_options.max_side_len = limit;
        }
        if let Some(threshold) = options.det_thresh {
            det_options.score_threshold = threshold;
        }
        if let Some(threshold) = options.box_thresh {
            det_options.box_threshold = threshold;
        }
        if let Some(ratio) = options.unclip_ratio {
            det_options.unclip_ratio = ratio;
        }
    }

    OcrEngineConfig::new().with_det_options(det_options)
}

fn bbox_points(text_box: &ocr_rs::TextBox) -> Vec<[f32; 2]> {
    if let Some(points) = text_box.points {
        return points
            .iter()
            .map(|point| [point.x, point.y])
            .collect::<Vec<_>>();
    }

    let left = text_box.rect.left() as f32;
    let top = text_box.rect.top() as f32;
    let right = left + text_box.rect.width() as f32;
    let bottom = top + text_box.rect.height() as f32;

    vec![[left, top], [right, top], [right, bottom], [left, bottom]]
}
