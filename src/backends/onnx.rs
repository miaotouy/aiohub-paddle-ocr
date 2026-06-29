mod config;
mod runtime;
mod smoke;

use super::OcrBackend;
use crate::errors::{ImageError, SidecarError};
use crate::model_registry::OnnxModelPaths;
use crate::protocol::OcrLine;
use config::{load_ppocrv6_onnx_config, Ppocrv6OnnxConfig};
use ort::session::Session;
use runtime::{
    initialize_onnx_runtime, load_onnx_session, onnx_runtime_library_path, validate_runtime_library,
};
use smoke::{run_onnx_tensor_smoke, OnnxSmokeSummary};
use std::sync::Mutex;

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

pub(crate) struct OnnxRuntimeBackend {
    _det_session: Mutex<Session>,
    _rec_session: Mutex<Session>,
    det_io: SessionIoSummary,
    rec_io: SessionIoSummary,
    config: Ppocrv6OnnxConfig,
    smoke: OnnxSmokeSummary,
}

impl OnnxRuntimeBackend {
    pub(crate) fn load(model_paths: &OnnxModelPaths) -> Result<Self, SidecarError> {
        let runtime_path = onnx_runtime_library_path();
        validate_runtime_library(&runtime_path)?;
        initialize_onnx_runtime(&runtime_path)?;

        let config = load_ppocrv6_onnx_config(model_paths)?;
        let mut det_session = load_onnx_session(&model_paths.det_onnx_path, "det")?;
        let mut rec_session = load_onnx_session(&model_paths.rec_onnx_path, "rec")?;
        let det_io = SessionIoSummary::from_session(&det_session);
        let rec_io = SessionIoSummary::from_session(&rec_session);
        let smoke = run_onnx_tensor_smoke(&mut det_session, &mut rec_session, &config)?;

        Ok(Self {
            _det_session: Mutex::new(det_session),
            _rec_session: Mutex::new(rec_session),
            det_io,
            rec_io,
            config,
            smoke,
        })
    }
}

impl OcrBackend for OnnxRuntimeBackend {
    fn id(&self) -> &'static str {
        "onnxruntime"
    }

    fn recognize(&self, _image_bytes: &[u8]) -> Result<Vec<OcrLine>, ImageError> {
        Err(ImageError::InferenceFailed(format!(
            "PP-OCRv6 ONNX Runtime tensor smoke 已通过，但完整 pipeline 尚未实现。config {}; smoke {}; det {}; rec {}; 下一步需要接入检测前后处理、透视裁剪、识别预处理和 CTC 解码",
            self.config.describe(),
            self.smoke.describe(),
            self.det_io.describe(),
            self.rec_io.describe()
        )))
    }
}
