# Model Third Party Notices

This repository does not commit the real Paddle OCR model files. Release builds
download the model bundle from this repository's GitHub Releases.

## Legacy Packaged Model Bundle

- Bundle URL: https://github.com/miaotouy/aiohub-paddle-ocr/releases/download/modelsv1/aiohub-paddle-ocr-models-ppocr-v5-mobile-v1.tar.gz
- Bundle SHA256: `2615c35c0aa33bb4ea21ec8a1de6a33cc8823d68d524f36f0e3cbb17777d63de`
- Extracted layout: `models/ppocr-v5-mobile/`

## Source

- Bundle source: BitYoungjae/ChalKak OCR Models v1
- Source project: https://github.com/BitYoungjae/ChalKak
- Source release: https://github.com/BitYoungjae/ChalKak/releases/tag/ocr-models-v1
- Source asset: `chalkak-ocr-models-v1.tar.gz`

## Upstream Model Family

- Upstream project: PaddleOCR
- Upstream repository: https://github.com/PaddlePaddle/PaddleOCR
- Model family: PP-OCRv5 mobile OCR models
- License: Apache-2.0

The local file names are normalized for this plugin's sidecar model registry.

## Experimental PP-OCRv6 ONNX Profile

The `ppocr-v6-small-onnx` profile is registered for migration work but is not
packaged by default yet. Real model files must be downloaded from official
PaddlePaddle repositories, pinned to an exact revision, and recorded here before
release packaging.

- Detection model: https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_onnx
- Recognition model: https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_onnx
- Expected layout: `models/ppocr-v6-small-onnx/`
- License: Apache-2.0
- Detection revision: `28fe5895c24fd108c19eb3e8479f4ab385fbfc62` (resolved from `HEAD` on 2026-06-29)
- Recognition revision: `b8f84f0b80c529de40b4fbb3544b84fa7233a513` (resolved from `HEAD` on 2026-06-29)
- File SHA256:
  - `det/inference.onnx`: `d73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e`
  - `det/inference.yml`: `193f435274bf9f0b5f71a929bbfbcf148282df7e633b34e7c373e8f44741b516`
  - `det/inference.json`: `89240f689a4a77aad75ef55a8df0a15c8e1d4980a327d17e58f24bbadde5aeab`
  - `rec/inference.onnx`: `5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634`
  - `rec/inference.yml`: `ab078671bb49f06228eadccd34f1bb501e157f7a047095ffb943ba81512c77d1`
  - `rec/inference.json`: `f0bf53c853937a917affdd74467472167727f8ab0f0f7bded01c4a16c27e46e6`

## Experimental ONNX Runtime

ONNX Runtime is the planned runtime for the official PP-OCRv6 profile. It is not
packaged by default yet. Release packaging must use a plugin-private runtime
directory such as `runtime/onnxruntime/windows-x64/` and must not depend on a
system `PATH` copy of `onnxruntime.dll`.

- Runtime project: https://onnxruntime.ai/
- Runtime repository: https://github.com/microsoft/onnxruntime
- Runtime download: https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-win-x64-1.20.1.zip
- License: MIT
- Version: `1.20.1`
- File SHA256: `4cb41e89b8bf30578e1dd95e9c40292d61974a4bfcd666409302c4f0c5aa8ce0`
