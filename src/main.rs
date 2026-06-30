mod backends;
mod errors;
mod model_registry;
mod native_stdout;
mod protocol;
mod recognition;

use backends::EngineHolder;
use model_registry::{load_model_registry, resolve_model_profile, validate_model_files};
use protocol::{
    send_event_with_id, HealthCheckRequest, HealthCheckResult, RecognizeBatchRequest, ResidentInput,
};
use recognition::handle_recognize_batch;
use std::io::{self, BufRead};
use std::process;

// ============================================================================
// 主入口 — 常驻循环
// ============================================================================

fn main() {
    let stdin = io::stdin();
    if let Err(error) = load_model_registry() {
        send_event_with_id(None, "error", None, serde_json::json!(error.to_string()));
        process::exit(1);
    }

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
            "healthCheck" => {
                let model_registry = match load_model_registry() {
                    Ok(registry) => registry,
                    Err(error) => {
                        send_event_with_id(id, "error", None, serde_json::json!(error.to_string()));
                        continue;
                    }
                };

                let request: HealthCheckRequest = match serde_json::from_value(input.params) {
                    Ok(request) => request,
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

                let model_profile =
                    match resolve_model_profile(&model_registry, request.options.as_ref()) {
                        Ok(profile) => profile,
                        Err(error) => {
                            send_event_with_id(
                                id,
                                "error",
                                None,
                                serde_json::json!(error.to_string()),
                            );
                            continue;
                        }
                    };

                if let Err(error) = validate_model_files(model_profile) {
                    send_event_with_id(id, "error", None, serde_json::json!(error.to_string()));
                    continue;
                }

                let result = HealthCheckResult {
                    ready: true,
                    status: "ok".to_string(),
                    backend: model_profile.backend.id().to_string(),
                    profile: model_profile.id.clone(),
                    profile_name: model_profile.name.clone(),
                    model_files: "ok".to_string(),
                };
                send_event_with_id(
                    id,
                    "result",
                    None,
                    serde_json::to_value(result).unwrap_or_default(),
                );
            }
            "recognizeBatch" => {
                let model_registry = match load_model_registry() {
                    Ok(registry) => registry,
                    Err(error) => {
                        send_event_with_id(id, "error", None, serde_json::json!(error.to_string()));
                        continue;
                    }
                };

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
