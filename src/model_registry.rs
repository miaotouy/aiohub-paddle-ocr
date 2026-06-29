use crate::errors::SidecarError;
use crate::protocol::OcrOptions;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const DEFAULT_MODEL_PROFILE: &str = "ppocr-v5-mobile-general";
const MODEL_ROOT: &str = "models";
const MODEL_REGISTRY_FILE: &str = "registry.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelRegistry {
    schema_version: u32,
    default_profile: String,
    profiles: Vec<ModelProfile>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelProfile {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) backend: ModelBackend,
    #[allow(dead_code)]
    pub(crate) family: Option<String>,
    #[allow(dead_code)]
    pub(crate) tier: Option<String>,
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
pub(crate) enum ModelBackend {
    #[serde(rename = "mnn-ocr-rs")]
    MnnOcrRs,
    #[serde(rename = "onnxruntime")]
    OnnxRuntime,
}

impl ModelBackend {
    pub(crate) fn id(self) -> &'static str {
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

#[derive(Debug)]
pub(crate) enum ModelPaths {
    Mnn(MnnModelPaths),
    Onnx(OnnxModelPaths),
}

#[derive(Debug)]
pub(crate) struct MnnModelPaths {
    pub(crate) det_path: PathBuf,
    pub(crate) rec_path: PathBuf,
    pub(crate) dict_path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct OnnxModelPaths {
    pub(crate) det_onnx_path: PathBuf,
    pub(crate) rec_onnx_path: PathBuf,
    pub(crate) det_config_path: PathBuf,
    pub(crate) rec_config_path: PathBuf,
}

pub(crate) fn load_model_registry() -> Result<ModelRegistry, SidecarError> {
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

pub(crate) fn resolve_model_profile<'a>(
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

pub(crate) fn validate_model_files(
    model_profile: &ModelProfile,
) -> Result<ModelPaths, SidecarError> {
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

pub(crate) fn validate_model_file(file_path: &Path) -> Result<(), SidecarError> {
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

pub(crate) fn build_engine_key(profile: &ModelProfile, options: Option<&OcrOptions>) -> String {
    format!(
        "{}:{}:{}",
        profile.backend.id(),
        profile.id.as_str(),
        runtime_options_fingerprint(options)
    )
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

fn runtime_options_fingerprint(options: Option<&OcrOptions>) -> String {
    match options {
        Some(options) => {
            serde_json::to_string(options).unwrap_or_else(|_| "invalid-options".to_string())
        }
        None => "default-options".to_string(),
    }
}
