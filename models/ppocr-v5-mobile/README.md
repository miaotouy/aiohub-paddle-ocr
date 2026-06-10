# ppocr-v5-mobile 模型目录

首版发布 ZIP 需要在本目录放置以下文件：

```txt
det.mnn
rec.mnn
keys.txt
```

源码仓库不提交真实模型文件。打包脚本会在 `--package` 时校验这些文件存在且大小不为 0。

## 上游下载源

这里的 `det.mnn` / `rec.mnn` 是插件运行时约定的最终文件名，不是 PaddleOCR 上游直接发布的原始文件名。上游模型优先从官方/镜像源下载，再按最终推理后端转换或导出。

首版 `ppocr-v5-mobile` 建议来源：

- 检测模型：`PP-OCRv5_mobile_det`
  - Hugging Face: https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det
  - ModelScope 可搜索 `PaddlePaddle/PP-OCRv5_mobile_det` 或 `PP-OCRv5_mobile_det_safetensors`
- 识别模型：`PP-OCRv5_mobile_rec`
  - Hugging Face: https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec
  - ModelScope: https://www.modelscope.cn/models/PaddlePaddle/PP-OCRv5_mobile_rec_safetensors
- 字典文件：
  - PaddleOCR 仓库中的 `ppocr/utils/dict/ppocrv5_dict.txt`
  - 放入插件目录时统一命名为 `keys.txt`

如果优先追求准确率而不是包体和 CPU 性能，可以调研 server 模型：

- https://www.modelscope.cn/models/PaddlePaddle/PP-OCRv5_server_det
- https://www.modelscope.cn/models/PaddlePaddle/PP-OCRv5_server_rec_safetensors

## 转换约定

当前插件 POC 先预留 MNN 文件名：

```txt
det.mnn <- PP-OCRv5_mobile_det 转换产物
rec.mnn <- PP-OCRv5_mobile_rec 转换产物
keys.txt <- ppocrv5_dict.txt 重命名或复制
```

如果最终选择 ONNX Runtime、Paddle Inference 或其他 Rust OCR 库，而不是 MNN，需要同步修改：

- `src/main.rs` 的模型文件校验逻辑。
- `build.js` 的 `MODEL_FILES` 列表。
- 本目录的文件命名说明。
