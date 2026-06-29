use super::config::Ppocrv6OnnxConfig;
use crate::errors::SidecarError;
use crate::native_stdout::with_native_stdout_suppressed;
use ort::session::Session;
use ort::value::Tensor;

#[derive(Debug, Clone)]
pub(super) struct OnnxSmokeSummary {
    det_outputs: Vec<String>,
    rec_outputs: Vec<String>,
}

impl OnnxSmokeSummary {
    pub(super) fn describe(&self) -> String {
        format!(
            "detOutputs=[{}], recOutputs=[{}]",
            self.det_outputs.join(", "),
            self.rec_outputs.join(", ")
        )
    }
}

pub(super) fn run_onnx_tensor_smoke(
    det_session: &mut Session,
    rec_session: &mut Session,
    config: &Ppocrv6OnnxConfig,
) -> Result<OnnxSmokeSummary, SidecarError> {
    with_native_stdout_suppressed(|| {
        Ok(OnnxSmokeSummary {
            det_outputs: run_single_onnx_tensor_smoke(
                det_session,
                config.det.smoke_input_shape,
                "det",
            )?,
            rec_outputs: run_single_onnx_tensor_smoke(
                rec_session,
                config.rec.smoke_input_shape,
                "rec",
            )?,
        })
    })
}

fn run_single_onnx_tensor_smoke(
    session: &mut Session,
    shape: [usize; 4],
    role: &str,
) -> Result<Vec<String>, SidecarError> {
    let element_count = shape
        .iter()
        .try_fold(1_usize, |acc, dimension| acc.checked_mul(*dimension))
        .ok_or_else(|| {
            SidecarError::OnnxInferenceFailed(format!(
                "{} smoke 输入 shape {:?} 元素数量溢出",
                role, shape
            ))
        })?;
    let input = Tensor::<f32>::from_array((
        shape.to_vec(),
        vec![0.0_f32; element_count].into_boxed_slice(),
    ))
    .map_err(|error| {
        SidecarError::OnnxInferenceFailed(format!(
            "{} smoke 输入张量构造失败 {:?}: {}",
            role, shape, error
        ))
    })?;
    let outputs = session.run(ort::inputs![input]).map_err(|error| {
        SidecarError::OnnxInferenceFailed(format!(
            "{} smoke 推理失败，输入 shape {:?}: {}",
            role, shape, error
        ))
    })?;
    let summary = outputs
        .iter()
        .map(|(name, value)| format!("{}: {}", name, value.dtype()))
        .collect::<Vec<_>>();

    if summary.is_empty() {
        return Err(SidecarError::OnnxInferenceFailed(format!(
            "{} smoke 推理没有返回输出",
            role
        )));
    }

    Ok(summary)
}
