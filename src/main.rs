use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ocr_rs::{DetOptions, OcrEngine, OcrEngineConfig};
use ort::session::Session;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use thiserror::Error;

#[cfg(unix)]
use std::os::fd::IntoRawFd;
#[cfg(windows)]
use std::os::windows::io::{FromRawHandle, IntoRawHandle};

const DEFAULT_MODEL_PROFILE: &str = "ppocr-v5-mobile-general";
const MODEL_ROOT: &str = "models";
const MODEL_REGISTRY_FILE: &str = "registry.json";

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
    #[serde(default)]
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

#[derive(Debug, Deserialize, Serialize, Default)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelRegistry {
    schema_version: u32,
    default_profile: String,
    profiles: Vec<ModelProfile>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ModelProfile {
    id: String,
    name: String,
    backend: ModelBackend,
    #[allow(dead_code)]
    family: Option<String>,
    #[allow(dead_code)]
    tier: Option<String>,
    language: String,
    model_dir: String,
    #[serde(default)]
    det_model: Option<String>,
    #[serde(default)]
    rec_model: Option<String>,
    #[serde(default)]
    dict: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    det_model_dir: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    rec_model_dir: Option<String>,
    #[serde(default)]
    det_onnx: Option<String>,
    #[serde(default)]
    rec_onnx: Option<String>,
    #[serde(default)]
    det_config: Option<String>,
    #[serde(default)]
    rec_config: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[allow(dead_code)]
    built_in: bool,
    #[allow(dead_code)]
    package: bool,
    #[allow(dead_code)]
    experimental: bool,
    #[allow(dead_code)]
    source_url: Option<String>,
    #[allow(dead_code)]
    revision: Option<String>,
    #[allow(dead_code)]
    sha256: Option<serde_json::Value>,
    #[allow(dead_code)]
    license: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
enum ModelBackend {
    #[serde(rename = "mnn-ocr-rs")]
    MnnOcrRs,
    #[serde(rename = "onnxruntime")]
    OnnxRuntime,
}

impl ModelBackend {
    fn id(self) -> &'static str {
        match self {
            Self::MnnOcrRs => "mnn-ocr-rs",
            Self::OnnxRuntime => "onnxruntime",
        }
    }
}

impl fmt::Display for ModelBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

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
enum ModelPaths {
    Mnn(MnnModelPaths),
    Onnx(OnnxModelPaths),
}

#[derive(Debug)]
struct MnnModelPaths {
    det_path: PathBuf,
    rec_path: PathBuf,
    dict_path: PathBuf,
}

#[derive(Debug)]
struct OnnxModelPaths {
    det_onnx_path: PathBuf,
    rec_onnx_path: PathBuf,
    det_config_path: PathBuf,
    rec_config_path: PathBuf,
}

trait OcrBackend: Send + Sync {
    fn id(&self) -> &'static str;
    fn recognize(&self, image_bytes: &[u8]) -> Result<Vec<OcrLine>, ImageError>;
}

#[derive(Debug)]
struct SessionIoSummary {
    inputs: Vec<String>,
    outputs: Vec<String>,
}

impl SessionIoSummary {
    fn from_session(session: &Session) -> Self {
        Self {
            inputs: session
                .inputs()
                .iter()
                .map(|input| format!("{}: {:?}", input.name(), input.dtype()))
                .collect(),
            outputs: session
                .outputs()
                .iter()
                .map(|output| format!("{}: {:?}", output.name(), output.dtype()))
                .collect(),
        }
    }

    fn describe(&self) -> String {
        format!(
            "inputs=[{}], outputs=[{}]",
            self.inputs.join(", "),
            self.outputs.join(", ")
        )
    }
}

/// MNN 后端包装 ocr_rs 引擎，通过内部 Mutex 保证 Send + Sync。
struct MnnOcrBackend {
    engine: Mutex<OcrEngine>,
}

impl MnnOcrBackend {
    fn load(
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

struct OnnxRuntimeBackend {
    _det_session: Mutex<Session>,
    _rec_session: Mutex<Session>,
    det_io: SessionIoSummary,
    rec_io: SessionIoSummary,
}

impl OnnxRuntimeBackend {
    fn load(model_paths: &OnnxModelPaths) -> Result<Self, SidecarError> {
        let runtime_path = onnx_runtime_library_path();
        validate_runtime_library(&runtime_path)?;
        initialize_onnx_runtime(&runtime_path)?;

        let det_session = load_onnx_session(&model_paths.det_onnx_path, "det")?;
        let rec_session = load_onnx_session(&model_paths.rec_onnx_path, "rec")?;
        let det_io = SessionIoSummary::from_session(&det_session);
        let rec_io = SessionIoSummary::from_session(&rec_session);

        Ok(Self {
            _det_session: Mutex::new(det_session),
            _rec_session: Mutex::new(rec_session),
            det_io,
            rec_io,
        })
    }
}

impl OcrBackend for OnnxRuntimeBackend {
    fn id(&self) -> &'static str {
        "onnxruntime"
    }

    fn recognize(&self, _image_bytes: &[u8]) -> Result<Vec<OcrLine>, ImageError> {
        Err(ImageError::InferenceFailed(format!(
            "PP-OCRv6 ONNX Runtime session smoke 已通过，但完整 pipeline 尚未实现。det {}; rec {}; 下一步需要接入检测前后处理、透视裁剪、识别预处理和 CTC 解码",
            self.det_io.describe(),
            self.rec_io.describe()
        )))
    }
}

// ============================================================================
// 运行时状态：已加载引擎 + 当前 profile
// ============================================================================

struct EngineHolder {
    current_engine_key: Option<String>,
    engine: Option<Arc<dyn OcrBackend>>,
}

impl EngineHolder {
    fn new() -> Self {
        Self {
            current_engine_key: None,
            engine: None,
        }
    }

    /// 获取或加载引擎。如果 backend、profile 或运行时参数不同，则切换。
    /// 返回 Arc 以便在并行场景中克隆共享。
    fn get_or_load(
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

// ============================================================================
// 错误类型
// ============================================================================

#[derive(Debug, Error)]
enum SidecarError {
    #[error("解析输入失败: {0}")]
    InvalidInput(#[from] serde_json::Error),
    #[error("模型 registry 无效: {0}")]
    InvalidModelRegistry(String),
    #[error("不支持的模型 profile: {0}")]
    UnsupportedModelProfile(String),
    #[error("不支持的 language: {0}")]
    UnsupportedLanguage(String),
    #[allow(dead_code)]
    #[error("不支持的模型后端: {0}")]
    UnsupportedBackend(String),
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
    #[error("ONNX Runtime 动态库缺失: {0}")]
    MissingRuntimeLibrary(String),
    #[error("ONNX Runtime 加载失败: {0}")]
    OnnxRuntimeLoadFailed(String),
    #[error("ONNX Runtime 版本不匹配: {0}")]
    OnnxRuntimeVersionMismatch(String),
    #[error("ONNX session 加载失败: {0}")]
    OnnxSessionLoadFailed(String),
    #[allow(dead_code)]
    #[error("ONNX 推理失败: {0}")]
    OnnxInferenceFailed(String),
    #[allow(dead_code)]
    #[error("OCR 后处理失败: {0}")]
    PostProcessFailed(String),
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
    let model_registry = match load_model_registry() {
        Ok(registry) => registry,
        Err(error) => {
            send_event_with_id(None, "error", None, serde_json::json!(error.to_string()));
            process::exit(1);
        }
    };
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

                match handle_recognize_batch(&model_registry, &mut engine_holder, request) {
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

// ============================================================================
// 单图识别（支持 path 零拷贝）
// ============================================================================

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

// ============================================================================
// Model Profile 解析
// ============================================================================

fn load_model_registry() -> Result<ModelRegistry, SidecarError> {
    let registry_path = Path::new(MODEL_ROOT).join(MODEL_REGISTRY_FILE);
    let content = fs::read_to_string(&registry_path).map_err(|error| {
        SidecarError::InvalidModelRegistry(format!(
            "读取 {} 失败: {}",
            registry_path.display(),
            error
        ))
    })?;
    let registry: ModelRegistry = serde_json::from_str(&content).map_err(|error| {
        SidecarError::InvalidModelRegistry(format!(
            "解析 {} 失败: {}",
            registry_path.display(),
            error
        ))
    })?;

    validate_model_registry(&registry)?;
    Ok(registry)
}

fn validate_model_registry(registry: &ModelRegistry) -> Result<(), SidecarError> {
    if registry.schema_version != 1 {
        return Err(SidecarError::InvalidModelRegistry(format!(
            "不支持的 schemaVersion: {}",
            registry.schema_version
        )));
    }
    if registry.profiles.is_empty() {
        return Err(SidecarError::InvalidModelRegistry(
            "profiles 不能为空".to_string(),
        ));
    }
    if find_model_profile(registry, &registry.default_profile).is_none() {
        return Err(SidecarError::InvalidModelRegistry(format!(
            "默认 profile 不存在: {}",
            registry.default_profile
        )));
    }

    Ok(())
}

fn resolve_model_profile<'a>(
    registry: &'a ModelRegistry,
    options: Option<&OcrOptions>,
) -> Result<&'a ModelProfile, SidecarError> {
    if let Some(model_profile) = options
        .and_then(|options| options.model_profile.as_deref())
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
    {
        return find_model_profile(registry, model_profile)
            .ok_or_else(|| SidecarError::UnsupportedModelProfile(model_profile.to_string()));
    }

    if let Some(language) = options
        .and_then(|options| options.language.as_deref())
        .map(str::trim)
        .filter(|language| !language.is_empty())
    {
        return find_language_profile(registry, language)
            .ok_or_else(|| SidecarError::UnsupportedLanguage(language.to_string()));
    }

    find_model_profile(registry, &registry.default_profile)
        .or_else(|| find_model_profile(registry, DEFAULT_MODEL_PROFILE))
        .ok_or_else(|| SidecarError::UnsupportedModelProfile(registry.default_profile.clone()))
}

fn find_model_profile<'a>(
    registry: &'a ModelRegistry,
    profile_id: &str,
) -> Option<&'a ModelProfile> {
    registry.profiles.iter().find(|profile| {
        profile.id.eq_ignore_ascii_case(profile_id)
            || profile
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(profile_id))
    })
}

fn find_language_profile<'a>(
    registry: &'a ModelRegistry,
    language: &str,
) -> Option<&'a ModelProfile> {
    registry.profiles.iter().find(|profile| {
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
    let model_dir = Path::new(&model_profile.model_dir);
    if !model_dir.is_dir() {
        return Err(SidecarError::MissingModelDir(
            model_dir.display().to_string(),
        ));
    }

    match model_profile.backend {
        ModelBackend::MnnOcrRs => validate_mnn_model_files(model_profile, model_dir),
        ModelBackend::OnnxRuntime => validate_onnx_model_files(model_profile, model_dir),
    }
}

fn validate_mnn_model_files(
    model_profile: &ModelProfile,
    model_dir: &Path,
) -> Result<ModelPaths, SidecarError> {
    let paths = MnnModelPaths {
        det_path: model_dir.join(required_profile_file(
            model_profile,
            "detModel",
            &model_profile.det_model,
        )?),
        rec_path: model_dir.join(required_profile_file(
            model_profile,
            "recModel",
            &model_profile.rec_model,
        )?),
        dict_path: model_dir.join(required_profile_file(
            model_profile,
            "dict",
            &model_profile.dict,
        )?),
    };

    validate_model_file(&paths.det_path)?;
    validate_model_file(&paths.rec_path)?;
    validate_model_file(&paths.dict_path)?;

    Ok(ModelPaths::Mnn(paths))
}

fn validate_onnx_model_files(
    model_profile: &ModelProfile,
    model_dir: &Path,
) -> Result<ModelPaths, SidecarError> {
    let paths = OnnxModelPaths {
        det_onnx_path: model_dir.join(required_profile_file(
            model_profile,
            "detOnnx",
            &model_profile.det_onnx,
        )?),
        rec_onnx_path: model_dir.join(required_profile_file(
            model_profile,
            "recOnnx",
            &model_profile.rec_onnx,
        )?),
        det_config_path: model_dir.join(required_profile_file(
            model_profile,
            "detConfig",
            &model_profile.det_config,
        )?),
        rec_config_path: model_dir.join(required_profile_file(
            model_profile,
            "recConfig",
            &model_profile.rec_config,
        )?),
    };

    validate_model_file(&paths.det_onnx_path)?;
    validate_model_file(&paths.rec_onnx_path)?;
    validate_model_file(&paths.det_config_path)?;
    validate_model_file(&paths.rec_config_path)?;

    Ok(ModelPaths::Onnx(paths))
}

fn required_profile_file<'a>(
    model_profile: &ModelProfile,
    field_name: &str,
    value: &'a Option<String>,
) -> Result<&'a str, SidecarError> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|file| !file.is_empty())
        .ok_or_else(|| {
            SidecarError::InvalidModelRegistry(format!(
                "profile {} 缺少 {}",
                model_profile.id.as_str(),
                field_name
            ))
        })
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

fn build_engine_key(profile: &ModelProfile, options: Option<&OcrOptions>) -> String {
    format!(
        "{}:{}:{}",
        profile.backend.id(),
        profile.id.as_str(),
        runtime_options_fingerprint(options)
    )
}

fn runtime_options_fingerprint(options: Option<&OcrOptions>) -> String {
    match options {
        Some(options) => {
            serde_json::to_string(options).unwrap_or_else(|_| "invalid-options".to_string())
        }
        None => "default-options".to_string(),
    }
}

fn validate_runtime_library(runtime_path: &Path) -> Result<(), SidecarError> {
    if !runtime_path.is_file() {
        return Err(SidecarError::MissingRuntimeLibrary(
            runtime_path.display().to_string(),
        ));
    }

    validate_model_file(runtime_path)
}

fn initialize_onnx_runtime(runtime_path: &Path) -> Result<(), SidecarError> {
    with_native_stdout_suppressed(|| {
        let builder = ort::init_from(runtime_path)
            .map_err(|error| map_onnx_runtime_load_error(runtime_path, error))?;
        builder
            .with_name("aiohub-paddle-ocr-onnxruntime")
            .with_telemetry(false)
            .commit();
        Ok(())
    })
}

fn map_onnx_runtime_load_error(runtime_path: &Path, error: ort::Error) -> SidecarError {
    let message = format!("{}: {}", runtime_path.display(), error);
    if message.contains("not compatible") || message.contains("expected version") {
        SidecarError::OnnxRuntimeVersionMismatch(message)
    } else {
        SidecarError::OnnxRuntimeLoadFailed(message)
    }
}

fn load_onnx_session(model_path: &Path, role: &str) -> Result<Session, SidecarError> {
    with_native_stdout_suppressed(|| {
        let mut builder = Session::builder().map_err(|error| {
            SidecarError::OnnxSessionLoadFailed(format!(
                "{} session builder 初始化失败: {}",
                role, error
            ))
        })?;
        builder.commit_from_file(model_path).map_err(|error| {
            SidecarError::OnnxSessionLoadFailed(format!(
                "{} 模型 {} 加载失败: {}",
                role,
                model_path.display(),
                error
            ))
        })
    })
}

fn onnx_runtime_library_path() -> PathBuf {
    Path::new("runtime")
        .join("onnxruntime")
        .join(onnx_runtime_platform_dir())
        .join(onnx_runtime_library_file())
}

#[cfg(all(windows, target_arch = "x86_64"))]
fn onnx_runtime_platform_dir() -> &'static str {
    "windows-x64"
}

#[cfg(all(windows, target_arch = "aarch64"))]
fn onnx_runtime_platform_dir() -> &'static str {
    "windows-arm64"
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn onnx_runtime_platform_dir() -> &'static str {
    "macos-x64"
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn onnx_runtime_platform_dir() -> &'static str {
    "macos-arm64"
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn onnx_runtime_platform_dir() -> &'static str {
    "linux-x64"
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn onnx_runtime_platform_dir() -> &'static str {
    "linux-arm64"
}

#[cfg(windows)]
fn onnx_runtime_library_file() -> &'static str {
    "onnxruntime.dll"
}

#[cfg(target_os = "macos")]
fn onnx_runtime_library_file() -> &'static str {
    "libonnxruntime.dylib"
}

#[cfg(all(unix, not(target_os = "macos")))]
fn onnx_runtime_library_file() -> &'static str {
    "libonnxruntime.so"
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
        Ok(line) => {
            println!("{}", line);
            let _ = io::stdout().flush();
        }
        Err(_) => {
            eprintln!("序列化输出失败");
            let _ = io::stderr().flush();
        }
    }
}
