use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum SidecarError {
    #[error("解析输入失败: {0}")]
    InvalidInput(#[from] serde_json::Error),
    #[error("模型 registry 无效: {0}")]
    InvalidModelRegistry(String),
    #[error("ONNX 模型配置无效: {0}")]
    InvalidModelConfig(String),
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
pub(crate) enum ImageError {
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
