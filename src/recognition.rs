use crate::backends::{EngineHolder, OcrBackend};
use crate::errors::{ImageError, SidecarError};
use crate::model_registry::{resolve_model_profile, ModelRegistry};
use crate::protocol::{
    send_event_with_id, OcrImageInput, OcrLine, OcrStatus, PaddleOcrBatchResult,
    PaddleOcrImageResult, RecognizeBatchRequest,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rayon::prelude::*;
use std::fs;
use std::time::Instant;

pub(crate) fn handle_recognize_batch(
    model_registry: &ModelRegistry,
    engine_holder: &mut EngineHolder,
    request: RecognizeBatchRequest,
) -> Result<PaddleOcrBatchResult, SidecarError> {
    let started_at = Instant::now();
    let model_profile = resolve_model_profile(model_registry, request.options.as_ref())?;
    // 获取/加载引擎（自动处理动态切换），返回 Arc 便于跨线程共享
    let engine = engine_holder.get_or_load(model_profile, request.options.as_ref())?;

    let total = request.images.len();

    send_event_with_id(
        None,
        "progress",
        None,
        serde_json::json!({
            "backend": model_profile.backend.id(),
            "profile": model_profile.id.as_str(),
            "message": format!("开始并行识别 {} 个图片块", total),
            "percent": 5
        }),
    );

    // 并行策略：
    // 1. par_iter 并行读取图片字节（I/O 与 Base64 解码是 CPU/IO 密集，并行收益大）
    // 2. 推理阶段通过 Mutex 保护引擎（ocr_rs 可能非线程安全）
    let results: Vec<PaddleOcrImageResult> = request
        .images
        .par_iter()
        .map(|image| {
            // 并行执行：I/O（读文件/Base64解码）+ 推理
            // PaddleOcrEngine 通过内部 Mutex 保证线程安全
            recognize_single_image(engine.as_ref(), image)
        })
        .collect();
    let elapsed_ms = started_at.elapsed().as_millis();
    send_event_with_id(
        None,
        "progress",
        None,
        serde_json::json!({
            "backend": model_profile.backend.id(),
            "profile": model_profile.id.as_str(),
            "message": format!("批量识别完成，耗时 {} ms", elapsed_ms),
            "percent": 100
        }),
    );

    Ok(PaddleOcrBatchResult { results })
}

fn recognize_single_image(engine: &dyn OcrBackend, image: &OcrImageInput) -> PaddleOcrImageResult {
    let image_bytes = match read_image_bytes(image) {
        Ok(bytes) => bytes,
        Err(e) => {
            return PaddleOcrImageResult {
                block_id: image.block_id.clone(),
                image_id: image.image_id.clone(),
                text: String::new(),
                confidence: None,
                status: OcrStatus::Error,
                error: Some(e.to_string()),
                lines: None,
            };
        }
    };

    match engine.recognize(&image_bytes) {
        Ok(lines) => {
            let text = lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let confidence = average_confidence(&lines);

            PaddleOcrImageResult {
                block_id: image.block_id.clone(),
                image_id: image.image_id.clone(),
                text,
                confidence,
                status: OcrStatus::Success,
                error: None,
                lines: Some(lines),
            }
        }
        Err(error) => PaddleOcrImageResult {
            block_id: image.block_id.clone(),
            image_id: image.image_id.clone(),
            text: String::new(),
            confidence: None,
            status: OcrStatus::Error,
            error: Some(format!("{}: {}", engine.id(), error)),
            lines: None,
        },
    }
}

/// 读取图片字节：优先使用 path 零拷贝，回退到 dataUrl 解码
fn read_image_bytes(image: &OcrImageInput) -> Result<Vec<u8>, ImageError> {
    if let Some(ref path) = image.path {
        fs::read(path)
            .map_err(|e| ImageError::InvalidImage(format!("读取文件失败 {}: {}", path, e)))
    } else if let Some(ref data_url) = image.data_url {
        decode_data_url(data_url)
    } else {
        Err(ImageError::NoData)
    }
}

fn decode_data_url(data_url: &str) -> Result<Vec<u8>, ImageError> {
    let (_, encoded) = data_url.split_once(',').ok_or(ImageError::InvalidDataUrl)?;
    STANDARD
        .decode(encoded)
        .map_err(|error| ImageError::InvalidBase64(error.to_string()))
}

fn average_confidence(lines: &[OcrLine]) -> Option<f32> {
    if lines.is_empty() {
        return None;
    }
    let total: f32 = lines.iter().map(|line| line.score).sum();
    Some(total / lines.len() as f32)
}
