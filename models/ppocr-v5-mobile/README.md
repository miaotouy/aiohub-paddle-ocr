# ppocr-v5-mobile 模型目录

发布 ZIP 需要在本目录放置 sidecar 注册表声明的模型文件。文件名是插件内部规范名，不需要和 PaddleOCR 上游原始文件名保持一致。

## 当前文件

```txt
ppocrv5_mobile_det.mnn
ppocrv5_mobile_rec_general.mnn
ppocrv5_mobile_dict_general.txt
ppocrv5_mobile_rec_en.mnn
ppocrv5_mobile_dict_en.txt
ppocrv5_mobile_rec_ko.mnn
ppocrv5_mobile_dict_ko.txt
ppocrv5_mobile_rec_latin.mnn
ppocrv5_mobile_dict_latin.txt
ppocrv5_mobile_rec_arabic.mnn
ppocrv5_mobile_dict_arabic.txt
ppocrv5_mobile_rec_cyrillic.mnn
ppocrv5_mobile_dict_cyrillic.txt
ppocrv5_mobile_rec_el.mnn
ppocrv5_mobile_dict_el.txt
ppocrv5_mobile_rec_devanagari.mnn
ppocrv5_mobile_dict_devanagari.txt
ppocrv5_mobile_rec_ta.mnn
ppocrv5_mobile_dict_ta.txt
ppocrv5_mobile_rec_te.mnn
ppocrv5_mobile_dict_te.txt
ppocrv5_mobile_rec_th.mnn
ppocrv5_mobile_dict_th.txt
```

源码仓库不提交真实模型文件。打包脚本会在 `--package` 时校验这些文件存在、大小不为 0，并拦截误把 safetensors 权重重命名成 `.mnn` 的情况。

## Profile 映射

检测模型共用：

```txt
ppocrv5_mobile_det.mnn <- PP-OCRv5_mobile_det 转换产物
```

识别模型和字典按 profile 配对：

```txt
ppocr-v5-mobile-general -> ppocrv5_mobile_rec_general.mnn + ppocrv5_mobile_dict_general.txt
ppocr-v5-mobile-en      -> ppocrv5_mobile_rec_en.mnn      + ppocrv5_mobile_dict_en.txt
ppocr-v5-mobile-ko      -> ppocrv5_mobile_rec_ko.mnn      + ppocrv5_mobile_dict_ko.txt
ppocr-v5-mobile-latin   -> ppocrv5_mobile_rec_latin.mnn   + ppocrv5_mobile_dict_latin.txt
ppocr-v5-mobile-arabic  -> ppocrv5_mobile_rec_arabic.mnn  + ppocrv5_mobile_dict_arabic.txt
ppocr-v5-mobile-cyrillic -> ppocrv5_mobile_rec_cyrillic.mnn + ppocrv5_mobile_dict_cyrillic.txt
ppocr-v5-mobile-el      -> ppocrv5_mobile_rec_el.mnn      + ppocrv5_mobile_dict_el.txt
ppocr-v5-mobile-devanagari -> ppocrv5_mobile_rec_devanagari.mnn + ppocrv5_mobile_dict_devanagari.txt
ppocr-v5-mobile-ta      -> ppocrv5_mobile_rec_ta.mnn      + ppocrv5_mobile_dict_ta.txt
ppocr-v5-mobile-te      -> ppocrv5_mobile_rec_te.mnn      + ppocrv5_mobile_dict_te.txt
ppocr-v5-mobile-th      -> ppocrv5_mobile_rec_th.mnn      + ppocrv5_mobile_dict_th.txt
```

为兼容早期调用，`ppocr-v5-mobile` 在 sidecar 中会作为 `ppocr-v5-mobile-general` 的别名处理。

## 上游下载源

上游模型优先从官方/镜像源下载，再按最终推理后端转换或导出。

- 检测模型：`PP-OCRv5_mobile_det`
  - Hugging Face: https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det
  - ModelScope 可搜索 `PaddlePaddle/PP-OCRv5_mobile_det` 或 `PP-OCRv5_mobile_det_safetensors`
- 通用识别模型：`PP-OCRv5_mobile_rec`
  - Hugging Face: https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec
  - ModelScope: https://www.modelscope.cn/models/PaddlePaddle/PP-OCRv5_mobile_rec_safetensors
- 字典文件：
  - PaddleOCR 仓库中的 `ppocr/utils/dict/`

注意：Hugging Face / ModelScope 上的 `*_safetensors` 文件不能直接改名为 `.mnn` 使用。它们需要先通过 PaddleOCR 导出或 ONNX 中转，再用 MNNConvert 生成真正的 MNN flatbuffer；否则 sidecar 会返回“模型文件格式不正确”。

如果最终选择 ONNX Runtime、Paddle Inference 或其他 Rust OCR 库，而不是 MNN，需要同步修改：

- `src/main.rs` 的模型 profile 注册表和模型文件校验逻辑。
- `build.js` 的模型 profile 注册表。
- 本目录的文件命名说明。
