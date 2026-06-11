# AIO Hub Paddle OCR 插件

AIO Hub Paddle OCR 是面向 AIO Hub Smart OCR 的本地离线 OCR sidecar 插件。插件通过独立进程加载 PaddleOCR MNN 模型，并向主应用暴露统一的批量识别方法，用于在 Smart OCR 中处理图片块、截图块和其他图像输入。

首版插件聚焦 Windows x64、本地 ZIP 导入和离线模型推理，不依赖插件市场在线安装，也不会在首次使用时联网下载模型。

## 项目状态

- 版本：`0.1.0`
- 插件 ID：`paddle-ocr`
- 插件类型：`sidecar`
- 首个方法：`recognizeBatch`
- 默认模型：`ppocr-v5-mobile-general`
- Smart OCR 扩展：通过 `manifest.json` 的 `contributions[]` 声明 `type: "ocr-engine"`
- 首个平台：Windows x64
- 分发方式：手动 ZIP 导入

当前仓库已包含 Vue 管理页、Rust sidecar、构建脚本、模型目录约定和 Windows x64 打包流程。模型文件需要放置在 `models/ppocr-v5-mobile/` 目录中，发布 ZIP 会随包分发模型。

## 功能范围

插件侧负责：

- `manifest.json` 插件声明。
- Rust sidecar 可执行文件。
- PaddleOCR / `ocr-rs` / MNN 推理接入。
- 模型文件定位、存在性检查、空文件检查和基础格式检查。
- `recognizeBatch` 批量 OCR 方法。
- 插件管理 UI `PaddleOcr.vue`。
- Windows x64 ZIP 打包。
- OCR 质量和性能验收记录。

主应用侧负责：

- sidecar/native adapter 的设置读取和运行态同步。
- 读取插件 `contributions`，把 `type: "ocr-engine"` 的能力注册到 Smart OCR 引擎列表。
- Smart OCR 插件引擎入口和缺失提示。
- 将 Smart OCR blocks 批量传入 `paddle-ocr.recognizeBatch`。

插件仓库不直接修改主应用文件。需要主应用配合的事项应记录在 README、issue 或对应主应用计划文档中。

## 目录结构

开发态目录：

```txt
plugins/aiohub-paddle-ocr/
├── manifest.json
├── package.json
├── build.js
├── vite.config.js
├── PaddleOcr.vue
├── src/
│   └── main.rs
├── models/
│   └── ppocr-v5-mobile/
│       ├── ppocrv5_mobile_det.mnn
│       ├── ppocrv5_mobile_rec_general.mnn
│       ├── ppocrv5_mobile_dict_general.txt
│       ├── ppocrv5_mobile_rec_en.mnn
│       ├── ppocrv5_mobile_dict_en.txt
│       ├── ...
│       └── README.md
└── README.md
```

发布 ZIP 目录：

```txt
paddle-ocr-v0.1.0-windows-x64.zip
├── manifest.json
├── PaddleOcr.js
├── style.css
├── bin/
│   └── aiohub-paddle-ocr-windows-x64.exe
├── models/
│   └── ppocr-v5-mobile/
│       ├── ppocrv5_mobile_det.mnn
│       ├── ppocrv5_mobile_rec_general.mnn
│       ├── ppocrv5_mobile_dict_general.txt
│       ├── ppocrv5_mobile_rec_en.mnn
│       ├── ppocrv5_mobile_dict_en.txt
│       └── ...
└── README.md
```

## 模型文件

首版模型目录为：

```txt
models/ppocr-v5-mobile/
```

必需文件由 sidecar 的模型 profile 注册表决定。当前注册的规范文件名包括：

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

这些文件名是插件内部规范名，不要求和 PaddleOCR 上游原始文件名一致。所有 `.mnn` 必须是真正的 MNN 模型文件，不能把 Hugging Face 或 ModelScope 上的 safetensors 权重直接改名为 `.mnn`。模型下载、转换和命名说明见 [models/ppocr-v5-mobile/README.md](models/ppocr-v5-mobile/README.md)。

sidecar 启动时会检查：

- 模型目录是否存在。
- 必需模型文件是否存在。
- 模型文件是否为空。
- `.mnn` 文件是否疑似 safetensors 权重误改名。

## 调用协议

主应用每次执行插件方法时，会启动 sidecar 并向 stdin 写入一行 JSON：

```ts
interface SidecarInput {
  method: "recognizeBatch";
  params: PaddleOcrBatchRequest;
  settings?: Record<string, unknown>;
}
```

sidecar 从 stdin 读取完整一行，解析 JSON，按 `method` 分发，完成 OCR 后通过 stdout 输出 JSON Lines。

成功结果：

```json
{ "type": "result", "data": { "results": [] } }
```

进度事件：

```json
{ "type": "progress", "data": { "message": "正在加载模型", "percent": 20 } }
```

错误事件：

```json
{ "type": "error", "data": "模型文件缺失: models/ppocr-v5-mobile/ppocrv5_mobile_det.mnn" }
```

约定：

- 正常日志不要写入 stderr，主应用会把 stderr 每一行视为 error 事件。
- 成功调用最终只输出一条 `type: "result"`。
- 非零退出码会被主应用视为执行失败。

## recognizeBatch

调用方式：

```ts
execute({
  service: "paddle-ocr",
  method: "recognizeBatch",
  params,
});
```

输入：

```ts
interface PaddleOcrBatchRequest {
  images: Array<{
    blockId: string;
    imageId: string;
    dataUrl: string;
    width?: number;
    height?: number;
  }>;
  options?: {
    modelProfile?:
      | "ppocr-v5-mobile-general"
      | "ppocr-v5-mobile-en"
      | "ppocr-v5-mobile-ko"
      | "ppocr-v5-mobile-latin"
      | "ppocr-v5-mobile-arabic"
      | "ppocr-v5-mobile-cyrillic"
      | "ppocr-v5-mobile-el"
      | "ppocr-v5-mobile-devanagari"
      | "ppocr-v5-mobile-ta"
      | "ppocr-v5-mobile-te"
      | "ppocr-v5-mobile-th";
    language?: "en" | "ko" | "latin" | "arabic" | "cyrillic" | "el" | "devanagari" | "ta" | "te" | "th";
    detLimitSideLen?: number;
    detThresh?: number;
    boxThresh?: number;
    unclipRatio?: number;
  };
}
```

语言能力由当前 `modelProfile` 对应的识别模型和字典决定。未传 `modelProfile` 时，sidecar 会优先尝试用 `language` 映射到对应 profile；两者都未传时使用 `ppocr-v5-mobile-general`。为兼容早期调用，`ppocr-v5-mobile` 会作为通用模型的别名处理。

输出：

```ts
interface PaddleOcrBatchResult {
  results: Array<{
    blockId: string;
    imageId: string;
    text: string;
    confidence?: number;
    status: "success" | "error";
    error?: string;
    lines?: Array<{
      text: string;
      score: number;
      bbox: Array<[number, number]>;
    }>;
  }>;
}
```

结果要求：

- `results.length` 尽量与输入 `images.length` 一致。
- 返回顺序与输入顺序保持一致。
- 单张图片失败时返回该图片的 `{ status: "error", error }`，不让整个批次直接失败。
- 模型缺失、输入无法解析、方法不存在等全局错误才让整个调用失败。

## 管理页

`PaddleOcr.vue` 是插件管理页，不耦合 Smart OCR 主识别流程。当前管理页提供：

- 插件版本展示。
- 当前模型 profile 展示和切换测试。
- 后端调用状态检查。
- 模型文件状态检查。
- 单图 smoke test。
- 最近一次调用耗时、模型数量和结果预览。

正式 OCR 识别流程统一走 `recognizeBatch`。

## 构建与打包

包管理器使用 Bun。运行脚本前可先查看 `package.json` 中的脚本定义。

开发构建：

```txt
bun run build
```

发布打包：

```txt
bun run package
```

`bun run build` 会构建 Vue 管理页和 Rust sidecar。`bun run package` 会额外校验必需模型文件，复制发布产物，并生成 Windows x64 ZIP。

## 验收清单

- ZIP 能通过主应用插件导入预检。
- 插件能安装到 app data 插件目录。
- 插件能启用和禁用。
- `execute({ service: "paddle-ocr", method: "recognizeBatch" })` 能返回结果。
- 中文 UI 截图能识别。
- 英文 UI 截图能识别。
- 中英混排截图能识别。
- 多 block 批量任务只启动一次 sidecar 调用。
- 模型缺失时错误清晰。

Benchmark 建议记录：

- 测试样本名称。
- 图片数量。
- 总像素或尺寸。
- 首次调用耗时。
- 批量总耗时。
- 峰值内存。
- 平均置信度。
- 失败图片数。
- ZIP 大小。

## 路线图

已完成或已进入首版实现的计划项：

- ~~初始化独立插件仓库。~~
- ~~创建 `manifest.json`、`package.json`、`vite.config.js` 和 `build.js`。~~
- ~~创建 `PaddleOcr.vue` 插件管理页。~~
- ~~创建 Rust sidecar。~~
- ~~实现 stdin 单行 JSON 输入和 stdout JSON Lines 输出。~~
- ~~实现 `recognizeBatch` 方法分发。~~
- ~~实现模型文件存在性、空文件和 safetensors 误用校验。~~
- ~~实现 data URL base64 解码。~~
- ~~实现批量结果结构，单图失败不影响整个批次。~~
- ~~接入 `ocr-rs` / MNN 推理后端。~~
- ~~放置 PP-OCRv5 Mobile 通用模型文件。~~
- ~~放置 `ppocr-v5-mobile` 模型目录说明。~~
- ~~支持 Windows x64 构建。~~
- ~~发布打包时复制 sidecar、管理页、manifest、README 和模型文件。~~

待完成或后续优化：

- 补齐正式 OCR benchmark 与验收记录。
- 增加模型完整性 hash 校验。
- 支持文件路径输入，减少 base64/stdin 压力。
- 增加更多模型 profile。
- 增加 `win32-arm64`、`darwin-x64`、`darwin-arm64`、`linux-x64`、`linux-arm64` 平台产物。
- 按平台拆分 ZIP。
- 补充插件市场元数据。
- 评估常驻 sidecar daemon。

native 插件仅作为后续性能优化备选，不作为首版方向。
