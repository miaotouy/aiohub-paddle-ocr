use crate::errors::SidecarError;
use crate::model_registry::validate_model_file;
use crate::native_stdout::with_native_stdout_suppressed;
use ort::session::Session;
use std::path::{Path, PathBuf};

pub(super) fn validate_runtime_library(runtime_path: &Path) -> Result<(), SidecarError> {
    if !runtime_path.is_file() {
        return Err(SidecarError::MissingRuntimeLibrary(
            runtime_path.display().to_string(),
        ));
    }

    validate_model_file(runtime_path)
}

pub(super) fn initialize_onnx_runtime(runtime_path: &Path) -> Result<(), SidecarError> {
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

pub(super) fn load_onnx_session(model_path: &Path, role: &str) -> Result<Session, SidecarError> {
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

pub(super) fn onnx_runtime_library_path() -> PathBuf {
    Path::new("runtime")
        .join("onnxruntime")
        .join(onnx_runtime_platform_dir())
        .join(onnx_runtime_library_file())
}

fn map_onnx_runtime_load_error(runtime_path: &Path, error: ort::Error) -> SidecarError {
    let message = format!("{}: {}", runtime_path.display(), error);
    if message.contains("not compatible") || message.contains("expected version") {
        SidecarError::OnnxRuntimeVersionMismatch(message)
    } else {
        SidecarError::OnnxRuntimeLoadFailed(message)
    }
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
