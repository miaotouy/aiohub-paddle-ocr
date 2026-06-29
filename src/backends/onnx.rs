mod config;
mod runtime;
mod smoke;

use super::OcrBackend;
use crate::errors::{ImageError, SidecarError};
use crate::model_registry::OnnxModelPaths;
use crate::native_stdout::with_native_stdout_suppressed;
use crate::protocol::OcrLine;
use config::{load_ppocrv6_onnx_config, Ppocrv6OnnxConfig};
use image::{imageops, RgbImage};
use ort::session::Session;
use ort::value::{Shape, Tensor};
use runtime::{
    initialize_onnx_runtime, load_onnx_session, onnx_runtime_library_path, validate_runtime_library,
};
use smoke::{run_onnx_tensor_smoke, OnnxSmokeSummary};
use std::collections::VecDeque;
use std::sync::Mutex;

const DET_LIMIT_SIDE_LEN: u32 = 1536;
const DET_STRIDE: u32 = 32;
const DET_DILATE_X: usize = 2;
const DET_DILATE_Y: usize = 1;
const MAX_REC_WIDTH: usize = 3200;
const REC_WIDTH_ALIGN: usize = 8;
const MIN_TEXT_SCORE: f32 = 0.01;

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

#[derive(Debug, Clone)]
struct PreparedDetInput {
    tensor_data: Vec<f32>,
    input_width: usize,
    input_height: usize,
    original_width: u32,
    original_height: u32,
}

#[derive(Debug, Clone)]
struct TextBox {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    score: f32,
}

impl TextBox {
    fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    fn height(&self) -> f32 {
        self.y1 - self.y0
    }

    fn bbox_points(&self) -> Vec<[f32; 2]> {
        vec![
            [self.x0, self.y0],
            [self.x1, self.y0],
            [self.x1, self.y1],
            [self.x0, self.y1],
        ]
    }
}

#[derive(Debug, Clone)]
struct Recognition {
    text: String,
    score: f32,
}

pub(crate) struct OnnxRuntimeBackend {
    det_session: Mutex<Session>,
    rec_session: Mutex<Session>,
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
            det_session: Mutex::new(det_session),
            rec_session: Mutex::new(rec_session),
            det_io,
            rec_io,
            config,
            smoke,
        })
    }

    fn recognize_image(&self, image_bytes: &[u8]) -> Result<Vec<OcrLine>, ImageError> {
        let image = image::load_from_memory(image_bytes)
            .map_err(|error| ImageError::InvalidImage(error.to_string()))?
            .to_rgb8();

        let boxes = self.detect_text_boxes(&image)?;
        let mut lines = Vec::new();

        for text_box in boxes {
            let Some(crop) = crop_text_region(&image, &text_box) else {
                continue;
            };
            let recognition = self.recognize_text_crop(&crop)?;
            let text = recognition.text.trim().to_string();
            if text.is_empty() || recognition.score < MIN_TEXT_SCORE {
                continue;
            }
            let score = (recognition.score + text_box.score) * 0.5;

            lines.push(OcrLine {
                text,
                score,
                bbox: text_box.bbox_points(),
            });
        }

        Ok(lines)
    }

    fn detect_text_boxes(&self, image: &RgbImage) -> Result<Vec<TextBox>, ImageError> {
        let prepared = prepare_detection_input(image, &self.config)?;
        let input_shape = [
            1_usize,
            3_usize,
            prepared.input_height,
            prepared.input_width,
        ];
        let input = Tensor::<f32>::from_array((
            input_shape,
            prepared.tensor_data.clone().into_boxed_slice(),
        ))
        .map_err(|error| ImageError::InferenceFailed(format!("det 输入张量构造失败: {}", error)))?;

        let (shape, data) = {
            let mut session = self.det_session.lock().map_err(|_| {
                ImageError::InferenceFailed("det session mutex poisoned".to_string())
            })?;
            let outputs = with_native_stdout_suppressed(|| session.run(ort::inputs![input]))
                .map_err(|error| ImageError::InferenceFailed(format!("det 推理失败: {}", error)))?;
            let (shape, data) = outputs[0].try_extract_tensor::<f32>().map_err(|error| {
                ImageError::InferenceFailed(format!("det 输出张量读取失败: {}", error))
            })?;

            (shape_to_usize(shape, 4, "det")?, data.to_vec())
        };

        postprocess_detection(&prepared, &shape, &data, &self.config)
    }

    fn recognize_text_crop(&self, crop: &RgbImage) -> Result<Recognition, ImageError> {
        let (input_shape, tensor_data) = prepare_recognition_input(crop, &self.config)?;
        let input = Tensor::<f32>::from_array((input_shape, tensor_data.into_boxed_slice()))
            .map_err(|error| {
                ImageError::InferenceFailed(format!("rec 输入张量构造失败: {}", error))
            })?;

        let (shape, data) = {
            let mut session = self.rec_session.lock().map_err(|_| {
                ImageError::InferenceFailed("rec session mutex poisoned".to_string())
            })?;
            let outputs = with_native_stdout_suppressed(|| session.run(ort::inputs![input]))
                .map_err(|error| ImageError::InferenceFailed(format!("rec 推理失败: {}", error)))?;
            let (shape, data) = outputs[0].try_extract_tensor::<f32>().map_err(|error| {
                ImageError::InferenceFailed(format!("rec 输出张量读取失败: {}", error))
            })?;

            (shape_to_usize(shape, 3, "rec")?, data.to_vec())
        };

        decode_ctc_output(&shape, &data, &self.config)
    }
}

impl OcrBackend for OnnxRuntimeBackend {
    fn id(&self) -> &'static str {
        "onnxruntime"
    }

    fn recognize(&self, image_bytes: &[u8]) -> Result<Vec<OcrLine>, ImageError> {
        self.recognize_image(image_bytes).map_err(|error| {
            ImageError::InferenceFailed(format!(
                "{}; config {}; smoke {}; det {}; rec {}",
                error,
                self.config.describe(),
                self.smoke.describe(),
                self.det_io.describe(),
                self.rec_io.describe()
            ))
        })
    }
}

fn prepare_detection_input(
    image: &RgbImage,
    config: &Ppocrv6OnnxConfig,
) -> Result<PreparedDetInput, ImageError> {
    let (original_width, original_height) = image.dimensions();
    if original_width == 0 || original_height == 0 {
        return Err(ImageError::InvalidImage("图片尺寸为空".to_string()));
    }

    let (input_width, input_height) = detection_resize_dims(original_width, original_height);
    let resized = imageops::resize(
        image,
        input_width,
        input_height,
        imageops::FilterType::Triangle,
    );
    let input_width = input_width as usize;
    let input_height = input_height as usize;
    let plane = input_width * input_height;
    let mut tensor_data = vec![0.0_f32; 3 * plane];

    for y in 0..input_height {
        for x in 0..input_width {
            let pixel = resized.get_pixel(x as u32, y as u32).0;
            let channels = [pixel[2], pixel[1], pixel[0]];
            let offset = y * input_width + x;
            for channel in 0..3 {
                let value = channels[channel] as f32 * config.det.normalize.scale;
                tensor_data[channel * plane + offset] = (value
                    - config.det.normalize.mean[channel])
                    / config.det.normalize.std[channel];
            }
        }
    }

    Ok(PreparedDetInput {
        tensor_data,
        input_width,
        input_height,
        original_width,
        original_height,
    })
}

fn detection_resize_dims(width: u32, height: u32) -> (u32, u32) {
    let max_side = width.max(height);
    let ratio = if max_side > DET_LIMIT_SIDE_LEN {
        DET_LIMIT_SIDE_LEN as f32 / max_side as f32
    } else {
        1.0
    };
    let resized_width = ((width as f32 * ratio).round() as u32).max(DET_STRIDE);
    let resized_height = ((height as f32 * ratio).round() as u32).max(DET_STRIDE);

    (
        align_to_stride(resized_width, DET_STRIDE),
        align_to_stride(resized_height, DET_STRIDE),
    )
}

fn align_to_stride(value: u32, stride: u32) -> u32 {
    (((value + stride - 1) / stride) * stride).max(stride)
}

fn postprocess_detection(
    prepared: &PreparedDetInput,
    shape: &[usize],
    data: &[f32],
    config: &Ppocrv6OnnxConfig,
) -> Result<Vec<TextBox>, ImageError> {
    if shape.len() != 4 || shape[0] != 1 || shape[1] != 1 {
        return Err(ImageError::InferenceFailed(format!(
            "det 输出 shape 期望 [1,1,H,W]，实际 {:?}",
            shape
        )));
    }
    let map_height = shape[2];
    let map_width = shape[3];
    if data.len() != map_width * map_height {
        return Err(ImageError::InferenceFailed(format!(
            "det 输出元素数量不匹配: shape {:?}, data {}",
            shape,
            data.len()
        )));
    }

    let threshold = config.det.post_process.thresh;
    let mut mask = data
        .iter()
        .map(|value| *value > threshold)
        .collect::<Vec<_>>();
    mask = dilate_mask(&mask, map_width, map_height, DET_DILATE_X, DET_DILATE_Y);

    let mut visited = vec![false; mask.len()];
    let mut boxes = Vec::new();
    let max_candidates = config.det.post_process.max_candidates as usize;
    let score_threshold = (config.det.post_process.box_thresh * 0.75).min(0.45);
    let sx = prepared.original_width as f32 / map_width as f32;
    let sy = prepared.original_height as f32 / map_height as f32;

    for y in 0..map_height {
        for x in 0..map_width {
            let index = y * map_width + x;
            if visited[index] || !mask[index] {
                continue;
            }

            let component = collect_component(&mask, &mut visited, map_width, map_height, x, y);
            if component.pixel_count < 3 {
                continue;
            }

            let score = component_score(
                data,
                map_width,
                component.min_x,
                component.min_y,
                component.max_x,
                component.max_y,
                threshold,
            );
            if score < score_threshold {
                continue;
            }

            let width = component.max_x.saturating_sub(component.min_x) + 1;
            let height = component.max_y.saturating_sub(component.min_y) + 1;
            if width < 3 || height < 3 {
                continue;
            }

            let expand_x = ((width as f32 * (config.det.post_process.unclip_ratio - 1.0) * 0.5)
                .max(2.0)) as isize;
            let expand_y = ((height as f32 * (config.det.post_process.unclip_ratio - 1.0) * 0.5)
                .max(2.0)) as isize;
            let x0 = component.min_x as isize - expand_x;
            let y0 = component.min_y as isize - expand_y;
            let x1 = component.max_x as isize + expand_x;
            let y1 = component.max_y as isize + expand_y;

            boxes.push(TextBox {
                x0: clamp_f32(x0 as f32 * sx, 0.0, prepared.original_width as f32),
                y0: clamp_f32(y0 as f32 * sy, 0.0, prepared.original_height as f32),
                x1: clamp_f32((x1 + 1) as f32 * sx, 0.0, prepared.original_width as f32),
                y1: clamp_f32((y1 + 1) as f32 * sy, 0.0, prepared.original_height as f32),
                score,
            });

            if boxes.len() >= max_candidates {
                break;
            }
        }
        if boxes.len() >= max_candidates {
            break;
        }
    }

    boxes.retain(|text_box| text_box.width() >= 4.0 && text_box.height() >= 4.0);
    sort_text_boxes(&mut boxes);
    Ok(boxes)
}

#[derive(Debug, Clone)]
struct Component {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
    pixel_count: usize,
}

fn collect_component(
    mask: &[bool],
    visited: &mut [bool],
    width: usize,
    height: usize,
    start_x: usize,
    start_y: usize,
) -> Component {
    let mut queue = VecDeque::new();
    let start_index = start_y * width + start_x;
    visited[start_index] = true;
    queue.push_back((start_x, start_y));

    let mut component = Component {
        min_x: start_x,
        min_y: start_y,
        max_x: start_x,
        max_y: start_y,
        pixel_count: 0,
    };

    while let Some((x, y)) = queue.pop_front() {
        component.pixel_count += 1;
        component.min_x = component.min_x.min(x);
        component.min_y = component.min_y.min(y);
        component.max_x = component.max_x.max(x);
        component.max_y = component.max_y.max(y);

        for (nx, ny) in neighbors4(x, y, width, height) {
            let index = ny * width + nx;
            if visited[index] || !mask[index] {
                continue;
            }
            visited[index] = true;
            queue.push_back((nx, ny));
        }
    }

    component
}

fn neighbors4(x: usize, y: usize, width: usize, height: usize) -> [(usize, usize); 4] {
    [
        (x.saturating_sub(1), y),
        ((x + 1).min(width - 1), y),
        (x, y.saturating_sub(1)),
        (x, (y + 1).min(height - 1)),
    ]
}

fn component_score(
    data: &[f32],
    width: usize,
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
    threshold: f32,
) -> f32 {
    let mut sum = 0.0_f32;
    let mut count = 0_usize;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let value = data[y * width + x];
            if value > threshold {
                sum += value;
                count += 1;
            }
        }
    }

    if count == 0 {
        0.0
    } else {
        sum / count as f32
    }
}

fn dilate_mask(
    mask: &[bool],
    width: usize,
    height: usize,
    radius_x: usize,
    radius_y: usize,
) -> Vec<bool> {
    let mut output = vec![false; mask.len()];
    for y in 0..height {
        let y0 = y.saturating_sub(radius_y);
        let y1 = (y + radius_y).min(height - 1);
        for x in 0..width {
            let x0 = x.saturating_sub(radius_x);
            let x1 = (x + radius_x).min(width - 1);
            let mut active = false;
            'search: for ny in y0..=y1 {
                for nx in x0..=x1 {
                    if mask[ny * width + nx] {
                        active = true;
                        break 'search;
                    }
                }
            }
            output[y * width + x] = active;
        }
    }
    output
}

fn sort_text_boxes(boxes: &mut [TextBox]) {
    boxes.sort_by(|a, b| {
        let same_line_threshold = a.height().min(b.height()).max(10.0) * 0.5;
        if (a.y0 - b.y0).abs() <= same_line_threshold {
            a.x0.total_cmp(&b.x0)
        } else {
            a.y0.total_cmp(&b.y0)
        }
    });
}

fn crop_text_region(image: &RgbImage, text_box: &TextBox) -> Option<RgbImage> {
    let image_width = image.width() as f32;
    let image_height = image.height() as f32;
    let pad_x = (text_box.width() * 0.03).max(2.0);
    let pad_y = (text_box.height() * 0.15).max(2.0);

    let x0 = clamp_f32(text_box.x0 - pad_x, 0.0, image_width).floor() as u32;
    let y0 = clamp_f32(text_box.y0 - pad_y, 0.0, image_height).floor() as u32;
    let x1 = clamp_f32(text_box.x1 + pad_x, 0.0, image_width).ceil() as u32;
    let y1 = clamp_f32(text_box.y1 + pad_y, 0.0, image_height).ceil() as u32;

    if x1 <= x0 || y1 <= y0 {
        return None;
    }

    Some(imageops::crop_imm(image, x0, y0, x1 - x0, y1 - y0).to_image())
}

fn prepare_recognition_input(
    crop: &RgbImage,
    config: &Ppocrv6OnnxConfig,
) -> Result<([usize; 4], Vec<f32>), ImageError> {
    let crop_width = crop.width().max(1);
    let crop_height = crop.height().max(1);
    let rec_height = config.rec.input_height();
    let base_width = config.rec.input_width();
    let resized_width = ((crop_width as f32 * rec_height as f32 / crop_height as f32).ceil()
        as usize)
        .max(1)
        .min(MAX_REC_WIDTH);
    let input_width =
        align_usize(resized_width.max(base_width), REC_WIDTH_ALIGN).min(MAX_REC_WIDTH);
    let resized_width = resized_width.min(input_width);

    let resized = imageops::resize(
        crop,
        resized_width as u32,
        rec_height as u32,
        imageops::FilterType::Triangle,
    );
    let plane = rec_height * input_width;
    let mut tensor_data = vec![0.0_f32; 3 * plane];

    for y in 0..rec_height {
        for x in 0..resized_width {
            let pixel = resized.get_pixel(x as u32, y as u32).0;
            let channels = [pixel[2], pixel[1], pixel[0]];
            let offset = y * input_width + x;
            for channel in 0..3 {
                tensor_data[channel * plane + offset] =
                    (channels[channel] as f32 / 255.0 - 0.5) / 0.5;
            }
        }
    }

    Ok(([1, 3, rec_height, input_width], tensor_data))
}

fn decode_ctc_output(
    shape: &[usize],
    data: &[f32],
    config: &Ppocrv6OnnxConfig,
) -> Result<Recognition, ImageError> {
    if shape.len() != 3 || shape[0] != 1 {
        return Err(ImageError::InferenceFailed(format!(
            "rec 输出 shape 期望 [1,T,C]，实际 {:?}",
            shape
        )));
    }
    let steps = shape[1];
    let class_count = shape[2];
    if data.len() != steps * class_count {
        return Err(ImageError::InferenceFailed(format!(
            "rec 输出元素数量不匹配: shape {:?}, data {}",
            shape,
            data.len()
        )));
    }

    let mut previous_index = usize::MAX;
    let mut text = String::new();
    let mut confidence_sum = 0.0_f32;
    let mut confidence_count = 0_usize;

    for step in 0..steps {
        let offset = step * class_count;
        let mut best_index = 0_usize;
        let mut best_score = f32::NEG_INFINITY;
        for class_index in 0..class_count {
            let score = data[offset + class_index];
            if score > best_score {
                best_score = score;
                best_index = class_index;
            }
        }

        if best_index != 0 && best_index != previous_index {
            if let Some(character) = ctc_character(best_index, config) {
                text.push_str(character);
                confidence_sum += best_score;
                confidence_count += 1;
            }
        }
        previous_index = best_index;
    }

    let score = if confidence_count == 0 {
        0.0
    } else {
        confidence_sum / confidence_count as f32
    };

    Ok(Recognition { text, score })
}

fn ctc_character(index: usize, config: &Ppocrv6OnnxConfig) -> Option<&str> {
    if index == 0 {
        return None;
    }

    let dict_index = index - 1;
    if dict_index < config.rec.characters.len() {
        return Some(config.rec.characters[dict_index].as_str());
    }

    if index == config.rec.characters.len() + 1 {
        return Some(" ");
    }

    None
}

fn shape_to_usize(shape: &Shape, rank: usize, role: &str) -> Result<Vec<usize>, ImageError> {
    if shape.len() != rank {
        return Err(ImageError::InferenceFailed(format!(
            "{} 输出 rank 期望 {}，实际 {:?}",
            role, rank, shape
        )));
    }

    shape
        .iter()
        .map(|dimension| {
            usize::try_from(*dimension).map_err(|_| {
                ImageError::InferenceFailed(format!(
                    "{} 输出 shape 包含非法维度 {}: {:?}",
                    role, dimension, shape
                ))
            })
        })
        .collect()
}

fn align_usize(value: usize, stride: usize) -> usize {
    ((value + stride - 1) / stride) * stride
}

fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
    value.max(min).min(max)
}
