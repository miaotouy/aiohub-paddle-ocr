use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ocr_rs::{DetOptions, OcrEngine, OcrEngineConfig};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, BufRead, Read};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;
use thiserror::Error;

const DEFAULT_MODEL_PROFILE: &str = "ppocr-v5-mobile";
const MODEL_ROOT: &str = "models";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SidecarInput {
    method: String,
    params: serde_json::Value,
    #[allow(dead_code)]
    settings: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecognizeBatchRequest {
    images: Vec<OcrImageInput>,
    options: Option<OcrOptions>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OcrImageInput {
    block_id: String,
    image_id: String,
    data_url: String,
    #[allow(dead_code)]
    width: Option<u32>,
    #[allow(dead_code)]
    height: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct OcrOptions {
    model_profile: Option<String>,
    #[allow(dead_code)]
    language: Option<String>,
    #[allow(dead_code)]
    det_limit_side_len: Option<u32>,
    #[allow(dead_code)]
    det_thresh: Option<f32>,
    #[allow(dead_code)]
    box_thresh: Option<f32>,
    #[allow(dead_code)]
    unclip_ratio: Option<f32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PaddleOcrBatchResult {
    results: Vec<PaddleOcrImageResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PaddleOcrImageResult {
    block_id: String,
    image_id: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f32>,
    status: OcrStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<Vec<OcrLine>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OcrLine {
    text: String,
    score: f32,
    bbox: Vec<[f32; 2]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum OcrStatus {
    Success,
    Error,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    message: String,
    percent: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SidecarEvent<T: Serialize> {
    #[serde(rename = "type")]
    event_type: &'static str,
    data: T,
}

#[derive(Debug, Error)]
enum SidecarError {
    #[error("未收到输入")]
    MissingInput,
    #[error("解析输入失败: {0}")]
    InvalidInput(#[from] serde_json::Error),
    #[error("未知方法: {0}")]
    UnknownMethod(String),
    #[error("不支持的模型 profile: {0}")]
    UnsupportedModelProfile(String),
    #[error("模型目录缺失: {0}")]
    MissingModelDir(String),
    #[error("模型文件缺失: {0}")]
    MissingModelFile(String),
    #[error("模型文件为空: {0}")]
    EmptyModelFile(String),
    #[error("模型文件格式不正确: {0}。当前文件看起来是 safetensors 权重，请先转换为 MNN 模型后再命名为 det.mnn/rec.mnn")]
    InvalidModelFormat(String),
    #[error("加载 OCR 引擎失败: {0}")]
    EngineLoadFailed(String),
}

#[derive(Debug, Error)]
enum ImageError {
    #[error("dataUrl 不是 base64 data URL")]
    InvalidDataUrl,
    #[error("图片 base64 解码失败: {0}")]
    InvalidBase64(String),
    #[error("图片解码失败: {0}")]
    InvalidImage(String),
    #[error("OCR 推理失败: {0}")]
    InferenceFailed(String),
}

fn main() {
    if let Err(error) = run() {
        send_error(&error.to_string());
        process::exit(1);
    }
}

fn run() -> Result<(), SidecarError> {
    let input = read_single_line()?;
    let input: SidecarInput = serde_json::from_str(&input)?;

    match input.method.as_str() {
        "recognizeBatch" => {
            let request: RecognizeBatchRequest = serde_json::from_value(input.params)?;
            recognize_batch(request)?;
            Ok(())
        }
        method => Err(SidecarError::UnknownMethod(method.to_string())),
    }
}

fn read_single_line() -> Result<String, SidecarError> {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let line = lines
        .next()
        .ok_or(SidecarError::MissingInput)?
        .map_err(|_| SidecarError::MissingInput)?;

    if line.trim().is_empty() {
        return Err(SidecarError::MissingInput);
    }

    Ok(line)
}

fn recognize_batch(request: RecognizeBatchRequest) -> Result<(), SidecarError> {
    let started_at = Instant::now();
    let model_profile = request
        .options
        .as_ref()
        .and_then(|options| options.model_profile.as_deref())
        .unwrap_or(DEFAULT_MODEL_PROFILE);

    send_progress(5, "正在检查模型文件");
    let model_dir = validate_model_files(model_profile)?;

    send_progress(20, "正在加载 OCR 后端");
    let engine = PaddleOcrEngine::load(&model_dir, request.options.as_ref())?;

    let total = request.images.len();
    let mut results = Vec::with_capacity(total);

    for (index, image) in request.images.iter().enumerate() {
        let percent = batch_percent(index, total);
        send_progress(percent, &format!("正在识别 {}/{}", index + 1, total));
        results.push(recognize_single_image(&engine, image));
    }

    let elapsed_ms = started_at.elapsed().as_millis();
    send_progress(100, &format!("批量识别完成，耗时 {} ms", elapsed_ms));
    send_result(PaddleOcrBatchResult { results });
    Ok(())
}

fn batch_percent(index: usize, total: usize) -> u32 {
    if total == 0 {
        return 90;
    }

    let ratio = index as f32 / total as f32;
    20 + (ratio * 75.0).round() as u32
}

fn validate_model_files(model_profile: &str) -> Result<PathBuf, SidecarError> {
    if model_profile != DEFAULT_MODEL_PROFILE {
        return Err(SidecarError::UnsupportedModelProfile(
            model_profile.to_string(),
        ));
    }

    let model_dir = Path::new(MODEL_ROOT).join(model_profile);
    if !model_dir.is_dir() {
        return Err(SidecarError::MissingModelDir(
            model_dir.display().to_string(),
        ));
    }

    for file_name in ["det.mnn", "rec.mnn", "keys.txt"] {
        let file_path = model_dir.join(file_name);
        if !file_path.is_file() {
            return Err(SidecarError::MissingModelFile(
                file_path.display().to_string(),
            ));
        }

        let metadata = fs::metadata(&file_path)
            .map_err(|_| SidecarError::MissingModelFile(file_path.display().to_string()))?;

        if metadata.len() == 0 {
            return Err(SidecarError::EmptyModelFile(
                file_path.display().to_string(),
            ));
        }

        if file_name.ends_with(".mnn") && looks_like_safetensors(&file_path) {
            return Err(SidecarError::InvalidModelFormat(
                file_path.display().to_string(),
            ));
        }
    }

    Ok(model_dir)
}

fn looks_like_safetensors(file_path: &Path) -> bool {
    let mut file = match fs::File::open(file_path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut header = [0_u8; 128];
    let read_len = match file.read(&mut header) {
        Ok(read_len) => read_len,
        Err(_) => return false,
    };

    read_len >= 16 && header[8] == b'{' && header[9..read_len].windows(7).any(|w| w == b"\"dtype\"")
}

fn recognize_single_image(engine: &PaddleOcrEngine, image: &OcrImageInput) -> PaddleOcrImageResult {
    match decode_data_url(&image.data_url).and_then(|bytes| engine.recognize(&bytes)) {
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
            error: Some(error.to_string()),
            lines: None,
        },
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

struct PaddleOcrEngine {
    engine: OcrEngine,
}

impl PaddleOcrEngine {
    fn load(model_dir: &Path, options: Option<&OcrOptions>) -> Result<Self, SidecarError> {
        let det_path = model_dir.join("det.mnn");
        let rec_path = model_dir.join("rec.mnn");
        let keys_path = model_dir.join("keys.txt");
        let config = build_engine_config(options);
        let engine = OcrEngine::new(det_path, rec_path, keys_path, Some(config))
            .map_err(|error| SidecarError::EngineLoadFailed(error.to_string()))?;

        Ok(Self { engine })
    }

    fn recognize(&self, image_bytes: &[u8]) -> Result<Vec<OcrLine>, ImageError> {
        let image = image::load_from_memory(image_bytes)
            .map_err(|error| ImageError::InvalidImage(error.to_string()))?;
        let results = self
            .engine
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

fn send_progress(percent: u32, message: &str) {
    send_event(SidecarEvent {
        event_type: "progress",
        data: ProgressPayload {
            message: message.to_string(),
            percent,
        },
    });
}

fn send_result(data: PaddleOcrBatchResult) {
    send_event(SidecarEvent {
        event_type: "result",
        data,
    });
}

fn send_error(message: &str) {
    send_event(SidecarEvent {
        event_type: "error",
        data: message.to_string(),
    });
}

fn send_event<T: Serialize>(event: SidecarEvent<T>) {
    match serde_json::to_string(&event) {
        Ok(line) => println!("{}", line),
        Err(_) => println!(r#"{{"type":"error","data":"序列化 sidecar 输出失败"}}"#),
    }
}
