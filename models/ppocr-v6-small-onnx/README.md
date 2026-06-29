# PP-OCRv6 Small ONNX 模型目录

本目录用于放置 PaddlePaddle 官方 PP-OCRv6 Small ONNX 模型。源码仓库不提交真实模型文件。

## 预期布局

```txt
models/ppocr-v6-small-onnx/
  det/
    inference.onnx
    inference.yml
    inference.json
  rec/
    inference.onnx
    inference.yml
    inference.json
```

## 官方来源

- 检测模型：`https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_onnx`
- 识别模型：`https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_onnx`
- License: Apache-2.0

下载到发布包或 CI 缓存时必须固定 revision，并在 `MODEL_THIRD_PARTY_NOTICES.md` 记录文件 hash。

当前 B0 固定 revision：

- det: `28fe5895c24fd108c19eb3e8479f4ab385fbfc62`
- rec: `b8f84f0b80c529de40b4fbb3544b84fa7233a513`

## 当前接入状态

`ppocr-v6-small-onnx` 已进入模型 registry 和 manifest，默认不打包、不作为默认 profile。完整 Rust ONNX pipeline、DBPostProcess、透视裁剪和 CTC 解码对齐完成并通过 benchmark 后，才能把它切成默认路径。
