use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ocr_rs::{DetOptions, OcrEngine, OcrEngineConfig};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Read};
use std::path::{Path, PathBuf};
use std::process;
use std::ptr;
use std::time::Instant;
use thiserror::Error;

#[cfg(unix)]
use std::os::fd::IntoRawFd;
#[cfg(windows)]
use std::os::windows::io::{FromRawHandle, IntoRawHandle};

const DEFAULT_MODEL_PROFILE: &str = "ppocr-v5-mobile-general";
const LEGACY_DEFAULT_MODEL_PROFILE: &str = "ppocr-v5-mobile";
const MODEL_ROOT: &str = "models";
const MODEL_FAMILY_DIR: &str = "ppocr-v5-mobile";
const DET_MODEL_FILE: &str = "ppocrv5_mobile_det.mnn";

// ============================================================================
// 输入协议 — 常驻 JSON-RPC 格式
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResidentInput {
    /// JSON-RPC 请求 ID（可选，用于向后兼容）
    #[serde(default)]
    id: Option<u64>,
    method: String,
    params: serde_json::Value,
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
    /// 零拷贝：优先使用本地文件路径
    #[serde(default)]
    path: Option<String>,
    /// 兼容现有调用：path 不存在时回退到 dataUrl
    #[serde(default)]
    data_url: Option<String>,
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

// ============================================================================
// 模型 Profile 定义
// ============================================================================

#[derive(Debug)]
struct ModelProfile {
    id: &'static str,
    language: &'static str,
    rec_file: &'static str,
    dict_file: &'static str,
    aliases: &'static [&'static str],
}

const MODEL_PROFILES: &[ModelProfile] = &[
    ModelProfile {
        id: "ppocr-v5-mobile-general",
        language: "general",
        rec_file: "ppocrv5_mobile_rec_general.mnn",
        dict_file: "ppocrv5_mobile_dict_general.txt",
        aliases: &[LEGACY_DEFAULT_MODEL_PROFILE, "general", "auto"],
    },
    ModelProfile {
        id: "ppocr-v5-mobile-en",
        language: "en",
        rec_file: "ppocrv5_mobile_rec_en.mnn",
        dict_file: "ppocrv5_mobile_dict_en.txt",
        aliases: &["en", "english"],
    },
    ModelProfile {
        id: "ppocr-v5-mobile-ko",
        language: "ko",
        rec_file: "ppocrv5_mobile_rec_ko.mnn",
        dict_file: "ppocrv5_mobile_dict_ko.txt",
        aliases: &["ko", "korean"],
    },
    ModelProfile {
        id: "ppocr-v5-mobile-latin",
        language: "latin",
        rec_file: "ppocrv5_mobile_rec_latin.mnn",
        dict_file: "ppocrv5_mobile_dict_latin.txt",
        aliases: &["latin"],
    },
    ModelProfile {
        id: "ppocr-v5-mobile-arabic",
        language: "arabic",
        rec_file: "ppocrv5_mobile_rec_arabic.mnn",
        dict_file: "ppocrv5_mobile_dict_arabic.txt",
        aliases: &["ar", "arabic"],
    },
    ModelProfile {
        id: "ppocr-v5-mobile-cyrillic",
        language: "cyrillic",
        rec_file: "ppocrv5_mobile_rec_cyrillic.mnn",
        dict_file: "ppocrv5_mobile_dict_cyrillic.txt",
        aliases: &["cyrillic"],
    },
    ModelProfile {
        id: "ppocr-v5-mobile-el",
        language: "el",
        rec_file: "ppocrv5_mobile_rec_el.mnn",
        dict_file: "ppocrv5_mobile_dict_el.txt",
        aliases: &["el", "greek"],
    },
    ModelProfile {
        id: "ppocr-v5-mobile-devanagari",
        language: "devanagari",
        rec_file: "ppocrv5_mobile_rec_devanagari.mnn",
        dict_file: "ppocrv5_mobile_dict_devanagari.txt",
        aliases: &["devanagari"],
    },
    ModelProfile {
        id: "ppocr-v5-mobile-ta",
        language: "ta",
        rec_file: "ppocrv5_mobile_rec_ta.mnn",
        dict_file: "ppocrv5_mobile_dict_ta.txt",
        aliases: &["ta", "tamil"],
    },
    ModelProfile {
        id: "ppocr-v5-mobile-te",
        language: "te",
        rec_file: "ppocrv5_mobile_rec_te.mnn",
        dict_file: "ppocrv5_mobile_dict_te.txt",
        aliases: &["te", "telugu"],
    },
    ModelProfile {
        id: "ppocr-v5-mobile-th",
        language: "th",
        rec_file: "ppocrv5_mobile_rec_th.mnn",
        dict_file: "ppocrv5_mobile_dict_th.txt",
        aliases: &["th", "thai"],
    },
];

// ============================================================================
// 输出协议
// ============================================================================

/// 带 id 的输出事件，支持 JSON-RPC 匹配
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResidentOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
    #[serde(rename = "type")]
    output_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    event: Option<String>,
    data: serde_json::Value,
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

// ============================================================================
// 模型路径与引擎
// ============================================================================

#[derive(Debug)]
struct ModelPaths {
    det_path: PathBuf,
    rec_path: PathBuf,
    dict_path: PathBuf,
}

struct PaddleOcrEngine {
    engine: OcrEngine,
}

impl PaddleOcrEngine {
    fn load(model_paths: &ModelPaths, options: Option<&OcrOptions>) -> Result<Self, SidecarError> {
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

// ============================================================================
// 运行时状态：已加载引擎 + 当前 profile
// ============================================================================

struct EngineHolder {
    current_profile_id: Option<String>,
    engine: Option<PaddleOcrEngine>,
}

impl EngineHolder {
    fn new() -> Self {
        Self {
            current_profile_id: None,
            engine: None,
        }
    }

    /// 获取或加载引擎。如果请求的 profile 与当前不同，则切换。
    fn get_or_load(
        &mut self,
        profile: &'static ModelProfile,
        options: Option<&OcrOptions>,
    ) -> Result<&PaddleOcrEngine, SidecarError> {
        let profile_id = profile.id.to_string();

        if self.current_profile_id.as_deref() == Some(&profile_id) {
            // 命中缓存
            if let Some(ref engine) = self.engine {
                return Ok(engine);
            }
        }

        // 需要加载新模型
        let model_paths = validate_model_files(profile)?;
        send_event_with_id(
            None,
            "progress",
            None,
            serde_json::json!({ "message": format!("正在切换模型: {}", profile.id), "percent": 5 }),
        );
        let engine = PaddleOcrEngine::load(&model_paths, options)?;

        self.current_profile_id = Some(profile_id);
        self.engine = Some(engine);

        Ok(self.engine.as_ref().unwrap())
    }
}

// ============================================================================
// 错误类型
// ============================================================================

#[derive(Debug, Error)]
enum SidecarError {
    #[error("解析输入失败: {0}")]
    InvalidInput(#[from] serde_json::Error),
    #[error("不支持的模型 profile: {0}")]
    UnsupportedModelProfile(String),
    #[error("不支持的 language: {0}")]
    UnsupportedLanguage(String),
    #[error("模型目录缺失: {0}")]
    MissingModelDir(String),
    #[error("模型文件缺失: {0}")]
    MissingModelFile(String),
    #[error("模型文件为空: {0}")]
    EmptyModelFile(String),
    #[error("模型文件格式不正确: {0}。当前文件看起来是 safetensors 权重，请先转换为真正的 MNN 模型后再放入模型目录")]
    InvalidModelFormat(String),
    #[error("加载 OCR 引擎失败: {0}")]
    EngineLoadFailed(String),
}

#[derive(Debug, Error)]
enum ImageError {
    #[error("图片数据不足")]
    NoData,
    #[error("dataUrl 不是 base64 data URL")]
    InvalidDataUrl,
    #[error("图片 base64 解码失败: {0}")]
    InvalidBase64(String),
    #[error("图片解码失败: {0}")]
    InvalidImage(String),
    #[error("OCR 推理失败: {0}")]
    InferenceFailed(String),
}

// ============================================================================
// 主入口 — 常驻循环
// ============================================================================

fn main() {
    let stdin = io::stdin();
    let mut engine_holder = EngineHolder::new();

    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(line) => line,
            Err(e) => {
                send_event_with_id(
                    None,
                    "error",
                    None,
                    serde_json::json!(format!("读取 stdin 失败: {}", e)),
                );
                continue;
            }
        };

        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }

        let input: ResidentInput = match serde_json::from_str(&trimmed) {
            Ok(input) => input,
            Err(e) => {
                send_event_with_id(
                    None,
                    "error",
                    None,
                    serde_json::json!(format!("解析输入失败: {}", e)),
                );
                continue;
            }
        };

        let id = input.id;

        match input.method.as_str() {
            "recognizeBatch" => {
                let request: RecognizeBatchRequest = match serde_json::from_value(input.params) {
                    Ok(r) => r,
                    Err(e) => {
                        send_event_with_id(
                            id,
                            "error",
                            None,
                            serde_json::json!(format!("解析参数失败: {}", e)),
                        );
                        continue;
                    }
                };

                match handle_recognize_batch(&mut engine_holder, request) {
                    Ok(result) => {
                        send_event_with_id(
                            id,
                            "result",
                            None,
                            serde_json::to_value(result).unwrap_or_default(),
                        );
                    }
                    Err(e) => {
                        send_event_with_id(id, "error", None, serde_json::json!(e.to_string()));
                    }
                }
            }
            "shutdown" => {
                send_event_with_id(id, "result", None, serde_json::json!("shutdown"));
                process::exit(0);
            }
            method => {
                send_event_with_id(
                    id,
                    "error",
                    None,
                    serde_json::json!(format!("未知方法: {}", method)),
                );
            }
        }
    }
}

// ============================================================================
// recognizeBatch 处理
// ============================================================================

fn handle_recognize_batch(
    engine_holder: &mut EngineHolder,
    request: RecognizeBatchRequest,
) -> Result<PaddleOcrBatchResult, SidecarError> {
    let started_at = Instant::now();
    let model_profile = resolve_model_profile(request.options.as_ref())?;

    // 获取/加载引擎（自动处理动态切换）
    let engine = engine_holder.get_or_load(model_profile, request.options.as_ref())?;

    let total = request.images.len();
    let mut results = Vec::with_capacity(total);

    for (index, image) in request.images.iter().enumerate() {
        let percent = batch_percent(index, total);
        send_event_with_id(
            None,
            "progress",
            None,
            serde_json::json!({ "message": format!("正在识别 {}/{}", index + 1, total), "percent": percent }),
        );
        results.push(recognize_single_image(engine, image));
    }

    let elapsed_ms = started_at.elapsed().as_millis();
    send_event_with_id(
        None,
        "progress",
        None,
        serde_json::json!({ "message": format!("批量识别完成，耗时 {} ms", elapsed_ms), "percent": 100 }),
    );

    Ok(PaddleOcrBatchResult { results })
}

fn batch_percent(index: usize, total: usize) -> u32 {
    if total == 0 {
        return 90;
    }
    let ratio = index as f32 / total as f32;
    20 + (ratio * 75.0).round() as u32
}

// ============================================================================
// 单图识别（支持 path 零拷贝）
// ============================================================================

fn recognize_single_image(engine: &PaddleOcrEngine, image: &OcrImageInput) -> PaddleOcrImageResult {
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
            error: Some(error.to_string()),
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

// ============================================================================
// Model Profile 解析
// ============================================================================

fn resolve_model_profile(
    options: Option<&OcrOptions>,
) -> Result<&'static ModelProfile, SidecarError> {
    if let Some(model_profile) = options
        .and_then(|options| options.model_profile.as_deref())
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
    {
        return find_model_profile(model_profile)
            .ok_or_else(|| SidecarError::UnsupportedModelProfile(model_profile.to_string()));
    }

    if let Some(language) = options
        .and_then(|options| options.language.as_deref())
        .map(str::trim)
        .filter(|language| !language.is_empty())
    {
        return find_language_profile(language)
            .ok_or_else(|| SidecarError::UnsupportedLanguage(language.to_string()));
    }

    find_model_profile(DEFAULT_MODEL_PROFILE)
        .ok_or_else(|| SidecarError::UnsupportedModelProfile(DEFAULT_MODEL_PROFILE.to_string()))
}

fn find_model_profile(profile_id: &str) -> Option<&'static ModelProfile> {
    MODEL_PROFILES.iter().find(|profile| {
        profile.id.eq_ignore_ascii_case(profile_id)
            || profile
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(profile_id))
    })
}

fn find_language_profile(language: &str) -> Option<&'static ModelProfile> {
    MODEL_PROFILES.iter().find(|profile| {
        profile.language.eq_ignore_ascii_case(language)
            || profile
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(language))
    })
}

// ============================================================================
// 模型文件校验
// ============================================================================

fn validate_model_files(model_profile: &ModelProfile) -> Result<ModelPaths, SidecarError> {
    let model_dir = Path::new(MODEL_ROOT).join(MODEL_FAMILY_DIR);
    if !model_dir.is_dir() {
        return Err(SidecarError::MissingModelDir(
            model_dir.display().to_string(),
        ));
    }

    let paths = ModelPaths {
        det_path: model_dir.join(DET_MODEL_FILE),
        rec_path: model_dir.join(model_profile.rec_file),
        dict_path: model_dir.join(model_profile.dict_file),
    };

    validate_model_file(&paths.det_path)?;
    validate_model_file(&paths.rec_path)?;
    validate_model_file(&paths.dict_path)?;

    Ok(paths)
}

fn validate_model_file(file_path: &Path) -> Result<(), SidecarError> {
    if !file_path.is_file() {
        return Err(SidecarError::MissingModelFile(
            file_path.display().to_string(),
        ));
    }

    let metadata = fs::metadata(file_path)
        .map_err(|_| SidecarError::MissingModelFile(file_path.display().to_string()))?;

    if metadata.len() == 0 {
        return Err(SidecarError::EmptyModelFile(
            file_path.display().to_string(),
        ));
    }

    if file_path
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("mnn")
        && looks_like_safetensors(file_path)
    {
        return Err(SidecarError::InvalidModelFormat(
            file_path.display().to_string(),
        ));
    }

    Ok(())
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

// ============================================================================
// Native stdout 抑制（OCR 后端可能会污染 stdout）
// ============================================================================

fn with_native_stdout_suppressed<T>(operation: impl FnOnce() -> T) -> T {
    let _silencer = NativeStdoutSilencer::new();
    operation()
}

struct NativeStdoutSilencer {
    saved_fd: i32,
    null_fd: i32,
}

impl NativeStdoutSilencer {
    fn new() -> Option<Self> {
        unsafe {
            let saved_fd = dup_fd(1);
            if saved_fd < 0 {
                return None;
            }

            let Some(null_fd) = open_null_fd() else {
                close_fd(saved_fd);
                return None;
            };

            libc::fflush(ptr::null_mut());
            if dup2_fd(null_fd, 1) < 0 {
                close_fd(saved_fd);
                close_fd(null_fd);
                return None;
            }

            Some(Self { saved_fd, null_fd })
        }
    }
}

impl Drop for NativeStdoutSilencer {
    fn drop(&mut self) {
        unsafe {
            libc::fflush(ptr::null_mut());
            dup2_fd(self.saved_fd, 1);
            close_fd(self.saved_fd);
            close_fd(self.null_fd);
        }
    }
}

#[cfg(windows)]
fn open_null_fd() -> Option<i32> {
    let file = OpenOptions::new().write(true).open("NUL").ok()?;
    let handle = file.into_raw_handle();
    let fd = unsafe { libc::open_osfhandle(handle as isize, 0) };
    if fd < 0 {
        let _ = unsafe { std::fs::File::from_raw_handle(handle) };
        return None;
    }
    Some(fd)
}

#[cfg(unix)]
fn open_null_fd() -> Option<i32> {
    Some(
        OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .ok()?
            .into_raw_fd(),
    )
}

#[cfg(windows)]
unsafe fn dup_fd(fd: i32) -> i32 {
    libc::dup(fd)
}

#[cfg(unix)]
unsafe fn dup_fd(fd: i32) -> i32 {
    libc::dup(fd)
}

#[cfg(windows)]
unsafe fn dup2_fd(source_fd: i32, target_fd: i32) -> i32 {
    libc::dup2(source_fd, target_fd)
}

#[cfg(unix)]
unsafe fn dup2_fd(source_fd: i32, target_fd: i32) -> i32 {
    libc::dup2(source_fd, target_fd)
}

#[cfg(windows)]
unsafe fn close_fd(fd: i32) -> i32 {
    libc::close(fd)
}

#[cfg(unix)]
unsafe fn close_fd(fd: i32) -> i32 {
    libc::close(fd)
}

// ============================================================================
// Engine 配置构建
// ============================================================================

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

// ============================================================================
// 事件发送
// ============================================================================

fn send_event_with_id(
    id: Option<u64>,
    output_type: &str,
    event: Option<&str>,
    data: serde_json::Value,
) {
    let output = ResidentOutput {
        id,
        output_type: output_type.to_string(),
        event: event.map(|s| s.to_string()),
        data,
    };
    match serde_json::to_string(&output) {
        Ok(line) => println!("{}", line),
        Err(_) => eprintln!("序列化输出失败"),
    }
}
