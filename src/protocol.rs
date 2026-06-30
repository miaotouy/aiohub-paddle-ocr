use serde::{Deserialize, Serialize};
use std::io::{self, Write};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResidentInput {
    /// JSON-RPC 请求 ID（可选，用于向后兼容）
    #[serde(default)]
    pub(crate) id: Option<u64>,
    pub(crate) method: String,
    pub(crate) params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecognizeBatchRequest {
    #[serde(default)]
    pub(crate) images: Vec<OcrImageInput>,
    pub(crate) options: Option<OcrOptions>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OcrImageInput {
    pub(crate) block_id: String,
    pub(crate) image_id: String,
    /// 零拷贝：优先使用本地文件路径
    #[serde(default)]
    pub(crate) path: Option<String>,
    /// 兼容现有调用：path 不存在时回退到 dataUrl
    #[serde(default)]
    pub(crate) data_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OcrOptions {
    pub(crate) model_profile: Option<String>,
    #[allow(dead_code)]
    pub(crate) language: Option<String>,
    #[allow(dead_code)]
    pub(crate) det_limit_side_len: Option<u32>,
    #[allow(dead_code)]
    pub(crate) det_thresh: Option<f32>,
    #[allow(dead_code)]
    pub(crate) box_thresh: Option<f32>,
    #[allow(dead_code)]
    pub(crate) unclip_ratio: Option<f32>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HealthCheckRequest {
    pub(crate) options: Option<OcrOptions>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HealthCheckResult {
    pub(crate) ready: bool,
    pub(crate) status: String,
    pub(crate) backend: String,
    pub(crate) profile: String,
    pub(crate) profile_name: String,
    pub(crate) model_files: String,
}

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
pub(crate) struct PaddleOcrBatchResult {
    pub(crate) results: Vec<PaddleOcrImageResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PaddleOcrImageResult {
    pub(crate) block_id: String,
    pub(crate) image_id: String,
    pub(crate) text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) confidence: Option<f32>,
    pub(crate) status: OcrStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) lines: Option<Vec<OcrLine>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OcrLine {
    pub(crate) text: String,
    pub(crate) score: f32,
    pub(crate) bbox: Vec<[f32; 2]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OcrStatus {
    Success,
    Error,
}

pub(crate) fn send_event_with_id(
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
