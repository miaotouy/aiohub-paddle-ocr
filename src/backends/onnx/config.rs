use crate::errors::SidecarError;
use crate::model_registry::OnnxModelPaths;
use serde_yaml::Value as YamlValue;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct Ppocrv6OnnxConfig {
    pub(super) det: DetOnnxConfig,
    pub(super) rec: RecOnnxConfig,
}

impl Ppocrv6OnnxConfig {
    pub(super) fn describe(&self) -> String {
        format!(
            "det(model={}, input={:?}, normalize=scale {}, mean {:?}, std {:?}, order {}, db thresh/box/unclip={}/{}/{}, maxCandidates={}); rec(model={}, input={:?}, post={}, chars={})",
            self.det.model_name,
            self.det.smoke_input_shape,
            self.det.normalize.scale,
            self.det.normalize.mean,
            self.det.normalize.std,
            self.det.normalize.order,
            self.det.post_process.thresh,
            self.det.post_process.box_thresh,
            self.det.post_process.unclip_ratio,
            self.det.post_process.max_candidates,
            self.rec.model_name,
            self.rec.smoke_input_shape,
            self.rec.post_process_name,
            self.rec.character_count()
        )
    }
}

#[derive(Debug, Clone)]
pub(super) struct DetOnnxConfig {
    model_name: String,
    pub(super) smoke_input_shape: [usize; 4],
    pub(super) normalize: NormalizeImageConfig,
    pub(super) post_process: DbPostProcessConfig,
}

#[derive(Debug, Clone)]
pub(super) struct NormalizeImageConfig {
    pub(super) mean: [f32; 3],
    pub(super) std: [f32; 3],
    pub(super) scale: f32,
    pub(super) order: String,
}

#[derive(Debug, Clone)]
pub(super) struct DbPostProcessConfig {
    pub(super) thresh: f32,
    pub(super) box_thresh: f32,
    pub(super) unclip_ratio: f32,
    pub(super) max_candidates: u32,
}

#[derive(Debug, Clone)]
pub(super) struct RecOnnxConfig {
    model_name: String,
    pub(super) smoke_input_shape: [usize; 4],
    post_process_name: String,
    pub(super) characters: Vec<String>,
}

impl RecOnnxConfig {
    pub(super) fn input_height(&self) -> usize {
        self.smoke_input_shape[2]
    }

    pub(super) fn input_width(&self) -> usize {
        self.smoke_input_shape[3]
    }

    pub(super) fn character_count(&self) -> usize {
        self.characters.len()
    }
}

pub(super) fn load_ppocrv6_onnx_config(
    model_paths: &OnnxModelPaths,
) -> Result<Ppocrv6OnnxConfig, SidecarError> {
    let det_yaml = read_yaml_config(&model_paths.det_config_path)?;
    let rec_yaml = read_yaml_config(&model_paths.rec_config_path)?;

    Ok(Ppocrv6OnnxConfig {
        det: parse_det_onnx_config(&det_yaml, &model_paths.det_config_path)?,
        rec: parse_rec_onnx_config(&rec_yaml, &model_paths.rec_config_path)?,
    })
}

fn parse_det_onnx_config(
    config: &YamlValue,
    config_path: &Path,
) -> Result<DetOnnxConfig, SidecarError> {
    let model_name = yaml_string(
        yaml_path(config, &["Global", "model_name"], config_path)?,
        config_path,
        "Global.model_name",
    )?;
    let normalize = parse_normalize_image(
        yaml_transform_op(config, "NormalizeImage", config_path)?,
        config_path,
    )?;
    let post_process_name = yaml_string(
        yaml_path(config, &["PostProcess", "name"], config_path)?,
        config_path,
        "PostProcess.name",
    )?;
    if post_process_name != "DBPostProcess" {
        return Err(config_error(
            config_path,
            format!(
                "PostProcess.name 期望 DBPostProcess，实际为 {}",
                post_process_name
            ),
        ));
    }

    let post_process = DbPostProcessConfig {
        thresh: yaml_f32(
            yaml_path(config, &["PostProcess", "thresh"], config_path)?,
            config_path,
            "PostProcess.thresh",
        )?,
        box_thresh: yaml_f32(
            yaml_path(config, &["PostProcess", "box_thresh"], config_path)?,
            config_path,
            "PostProcess.box_thresh",
        )?,
        unclip_ratio: yaml_f32(
            yaml_path(config, &["PostProcess", "unclip_ratio"], config_path)?,
            config_path,
            "PostProcess.unclip_ratio",
        )?,
        max_candidates: yaml_u32(
            yaml_path(config, &["PostProcess", "max_candidates"], config_path)?,
            config_path,
            "PostProcess.max_candidates",
        )?,
    };

    Ok(DetOnnxConfig {
        model_name,
        smoke_input_shape: parse_hpi_min_input_shape(config, config_path, [1, 3, 32, 32])?,
        normalize,
        post_process,
    })
}

fn parse_rec_onnx_config(
    config: &YamlValue,
    config_path: &Path,
) -> Result<RecOnnxConfig, SidecarError> {
    let model_name = yaml_string(
        yaml_path(config, &["Global", "model_name"], config_path)?,
        config_path,
        "Global.model_name",
    )?;
    let resize = yaml_transform_op(config, "RecResizeImg", config_path)?;
    let image_shape = yaml_usize_array3(
        yaml_path(resize, &["image_shape"], config_path)?,
        config_path,
        "RecResizeImg.image_shape",
    )?;
    let post_process_name = yaml_string(
        yaml_path(config, &["PostProcess", "name"], config_path)?,
        config_path,
        "PostProcess.name",
    )?;
    if post_process_name != "CTCLabelDecode" {
        return Err(config_error(
            config_path,
            format!(
                "PostProcess.name 期望 CTCLabelDecode，实际为 {}",
                post_process_name
            ),
        ));
    }

    let characters = yaml_string_vec(
        yaml_path(config, &["PostProcess", "character_dict"], config_path)?,
        config_path,
        "PostProcess.character_dict",
    )?;
    if characters.is_empty() {
        return Err(config_error(
            config_path,
            "PostProcess.character_dict 不能为空",
        ));
    }

    Ok(RecOnnxConfig {
        model_name,
        smoke_input_shape: [1, image_shape[0], image_shape[1], image_shape[2]],
        post_process_name,
        characters,
    })
}

fn parse_normalize_image(
    normalize: &YamlValue,
    config_path: &Path,
) -> Result<NormalizeImageConfig, SidecarError> {
    Ok(NormalizeImageConfig {
        mean: yaml_f32_array3(
            yaml_path(normalize, &["mean"], config_path)?,
            config_path,
            "NormalizeImage.mean",
        )?,
        std: yaml_f32_array3(
            yaml_path(normalize, &["std"], config_path)?,
            config_path,
            "NormalizeImage.std",
        )?,
        scale: yaml_f32(
            yaml_path(normalize, &["scale"], config_path)?,
            config_path,
            "NormalizeImage.scale",
        )?,
        order: yaml_string(
            yaml_path(normalize, &["order"], config_path)?,
            config_path,
            "NormalizeImage.order",
        )?,
    })
}

fn parse_hpi_min_input_shape(
    config: &YamlValue,
    config_path: &Path,
    default_shape: [usize; 4],
) -> Result<[usize; 4], SidecarError> {
    let Some(dynamic_shapes) = yaml_path_optional(
        config,
        &[
            "Hpi",
            "backend_configs",
            "paddle_infer",
            "trt_dynamic_shapes",
            "x",
        ],
    ) else {
        return Ok(default_shape);
    };

    let shapes = yaml_sequence(
        dynamic_shapes,
        config_path,
        "Hpi.backend_configs.paddle_infer.trt_dynamic_shapes.x",
    )?;
    let first_shape = shapes.first().ok_or_else(|| {
        config_error(
            config_path,
            "Hpi.backend_configs.paddle_infer.trt_dynamic_shapes.x 不能为空",
        )
    })?;
    yaml_usize_array4(
        first_shape,
        config_path,
        "Hpi.backend_configs.paddle_infer.trt_dynamic_shapes.x[0]",
    )
}

fn read_yaml_config(config_path: &Path) -> Result<YamlValue, SidecarError> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|error| config_error(config_path, format!("读取配置失败: {}", error)))?;
    serde_yaml::from_str(&content)
        .map_err(|error| config_error(config_path, format!("解析 YAML 失败: {}", error)))
}

fn yaml_path<'a>(
    root: &'a YamlValue,
    path: &[&str],
    config_path: &Path,
) -> Result<&'a YamlValue, SidecarError> {
    yaml_path_optional(root, path)
        .ok_or_else(|| config_error(config_path, format!("缺少字段 {}", path.join("."))))
}

fn yaml_path_optional<'a>(root: &'a YamlValue, path: &[&str]) -> Option<&'a YamlValue> {
    let mut current = root;
    for segment in path {
        let YamlValue::Mapping(mapping) = current else {
            return None;
        };
        current = mapping.get(YamlValue::String((*segment).to_string()))?;
    }
    Some(current)
}

fn yaml_transform_op<'a>(
    root: &'a YamlValue,
    op_name: &str,
    config_path: &Path,
) -> Result<&'a YamlValue, SidecarError> {
    let ops = yaml_sequence(
        yaml_path(root, &["PreProcess", "transform_ops"], config_path)?,
        config_path,
        "PreProcess.transform_ops",
    )?;

    for op in ops {
        if let YamlValue::Mapping(mapping) = op {
            if let Some(value) = mapping.get(YamlValue::String(op_name.to_string())) {
                return Ok(value);
            }
        }
    }

    Err(config_error(
        config_path,
        format!("PreProcess.transform_ops 缺少 {}", op_name),
    ))
}

fn yaml_sequence<'a>(
    value: &'a YamlValue,
    config_path: &Path,
    field_name: &str,
) -> Result<&'a [YamlValue], SidecarError> {
    match value {
        YamlValue::Sequence(sequence) => Ok(sequence.as_slice()),
        _ => Err(config_error(
            config_path,
            format!("{} 必须是数组", field_name),
        )),
    }
}

fn yaml_string(
    value: &YamlValue,
    config_path: &Path,
    field_name: &str,
) -> Result<String, SidecarError> {
    match value {
        YamlValue::String(value) => Ok(value.clone()),
        YamlValue::Number(value) => Ok(value.to_string()),
        _ => Err(config_error(
            config_path,
            format!("{} 必须是字符串", field_name),
        )),
    }
}

fn yaml_string_vec(
    value: &YamlValue,
    config_path: &Path,
    field_name: &str,
) -> Result<Vec<String>, SidecarError> {
    yaml_sequence(value, config_path, field_name)?
        .iter()
        .enumerate()
        .map(|(index, item)| yaml_string(item, config_path, &format!("{}[{}]", field_name, index)))
        .collect()
}

fn yaml_f32(value: &YamlValue, config_path: &Path, field_name: &str) -> Result<f32, SidecarError> {
    match value {
        YamlValue::Number(value) => value
            .as_f64()
            .map(|value| value as f32)
            .ok_or_else(|| config_error(config_path, format!("{} 不是有效数字", field_name))),
        YamlValue::String(value) => parse_yaml_f32_expression(value).map_err(|error| {
            config_error(
                config_path,
                format!("{} 不是有效数字: {}", field_name, error),
            )
        }),
        _ => Err(config_error(
            config_path,
            format!("{} 必须是数字", field_name),
        )),
    }
}

fn yaml_u32(value: &YamlValue, config_path: &Path, field_name: &str) -> Result<u32, SidecarError> {
    let parsed = yaml_usize(value, config_path, field_name)?;
    u32::try_from(parsed).map_err(|_| {
        config_error(
            config_path,
            format!("{} 超出 u32 范围: {}", field_name, parsed),
        )
    })
}

fn yaml_usize(
    value: &YamlValue,
    config_path: &Path,
    field_name: &str,
) -> Result<usize, SidecarError> {
    match value {
        YamlValue::Number(value) => {
            if let Some(value) = value.as_u64() {
                usize::try_from(value).map_err(|_| {
                    config_error(config_path, format!("{} 超出 usize 范围", field_name))
                })
            } else if let Some(value) = value.as_i64() {
                usize::try_from(value).map_err(|_| {
                    config_error(config_path, format!("{} 不能是负数: {}", field_name, value))
                })
            } else {
                Err(config_error(
                    config_path,
                    format!("{} 不是有效整数", field_name),
                ))
            }
        }
        YamlValue::String(value) => value.trim().parse::<usize>().map_err(|error| {
            config_error(
                config_path,
                format!("{} 不是有效整数: {}", field_name, error),
            )
        }),
        _ => Err(config_error(
            config_path,
            format!("{} 必须是整数", field_name),
        )),
    }
}

fn yaml_f32_array3(
    value: &YamlValue,
    config_path: &Path,
    field_name: &str,
) -> Result<[f32; 3], SidecarError> {
    let sequence = yaml_sequence(value, config_path, field_name)?;
    if sequence.len() != 3 {
        return Err(config_error(
            config_path,
            format!(
                "{} 必须包含 3 个数字，实际为 {}",
                field_name,
                sequence.len()
            ),
        ));
    }

    Ok([
        yaml_f32(&sequence[0], config_path, &format!("{}[0]", field_name))?,
        yaml_f32(&sequence[1], config_path, &format!("{}[1]", field_name))?,
        yaml_f32(&sequence[2], config_path, &format!("{}[2]", field_name))?,
    ])
}

fn yaml_usize_array3(
    value: &YamlValue,
    config_path: &Path,
    field_name: &str,
) -> Result<[usize; 3], SidecarError> {
    let sequence = yaml_sequence(value, config_path, field_name)?;
    if sequence.len() != 3 {
        return Err(config_error(
            config_path,
            format!(
                "{} 必须包含 3 个整数，实际为 {}",
                field_name,
                sequence.len()
            ),
        ));
    }

    Ok([
        yaml_usize(&sequence[0], config_path, &format!("{}[0]", field_name))?,
        yaml_usize(&sequence[1], config_path, &format!("{}[1]", field_name))?,
        yaml_usize(&sequence[2], config_path, &format!("{}[2]", field_name))?,
    ])
}

fn yaml_usize_array4(
    value: &YamlValue,
    config_path: &Path,
    field_name: &str,
) -> Result<[usize; 4], SidecarError> {
    let sequence = yaml_sequence(value, config_path, field_name)?;
    if sequence.len() != 4 {
        return Err(config_error(
            config_path,
            format!(
                "{} 必须包含 4 个整数，实际为 {}",
                field_name,
                sequence.len()
            ),
        ));
    }

    Ok([
        yaml_usize(&sequence[0], config_path, &format!("{}[0]", field_name))?,
        yaml_usize(&sequence[1], config_path, &format!("{}[1]", field_name))?,
        yaml_usize(&sequence[2], config_path, &format!("{}[2]", field_name))?,
        yaml_usize(&sequence[3], config_path, &format!("{}[3]", field_name))?,
    ])
}

fn parse_yaml_f32_expression(value: &str) -> Result<f32, String> {
    let value = value.trim();
    if let Some((numerator, denominator)) = value.split_once('/') {
        let denominator = parse_yaml_f32_token(denominator)?;
        if denominator == 0.0 {
            return Err("除数不能为 0".to_string());
        }
        return Ok(parse_yaml_f32_token(numerator)? / denominator);
    }

    parse_yaml_f32_token(value)
}

fn parse_yaml_f32_token(value: &str) -> Result<f32, String> {
    let value = value.trim();
    let normalized = if value.ends_with('.') {
        format!("{}0", value)
    } else {
        value.to_string()
    };
    normalized
        .parse::<f32>()
        .map_err(|error| format!("{} ({})", value, error))
}

fn config_error(config_path: &Path, message: impl Into<String>) -> SidecarError {
    SidecarError::InvalidModelConfig(format!("{}: {}", config_path.display(), message.into()))
}
