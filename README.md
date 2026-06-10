# AIO Hub Paddle OCR 插件施工指南

本文档给并行进入 `plugins/aiohub-paddle-ocr` 的 Codex 使用。主应用侧计划见：

```txt
E:\rc20\allinweb\all-in-one-tools\src\tools\smart-ocr\docs\Plan\paddle-ocr-pluginization-research.md
```

## 0. 当前仓库状态

已完成首版插件仓库初始化：

- 独立 Git 仓库已初始化。
- 已创建 `manifest.json`、`package.json`、`vite.config.js`、`build.js`、`PaddleOcr.vue`。
- 已创建 Rust sidecar，支持 stdin 单行 JSON、stdout JSON Lines、`recognizeBatch` 分发、模型文件校验、data URL base64 解码和批量结果结构。
- 已创建 `models/ppocr-v5-mobile/` 占位说明。
- `bun run build` 可通过。
- `bun run package` 会在真实模型缺失时中止并输出缺失文件列表。

尚未完成：

- 真实 Paddle OCR / MNN / `ocr-rs` 推理接入。
- 真实模型文件 `det.mnn`、`rec.mnn`、`keys.txt` 放置与校验 hash。
- 有模型后的 OCR benchmark 与 ZIP 验收。

## 1. 目标

实现官方 Paddle OCR sidecar 插件，让 Smart OCR 可以通过统一插件执行器调用：

```ts
execute({
  service: "paddle-ocr",
  method: "recognizeBatch",
  params,
});
```

首版目标：

- 插件 ID：`paddle-ocr`
- 插件目录：`plugins/aiohub-paddle-ocr`
- 插件类型：`sidecar`
- 首个方法：`recognizeBatch`
- 首个模型：`ppocr-v5-mobile`
- 首个平台：Windows x64
- 分发方式：手动 ZIP 导入

首版不要做：

- 不要改主应用 Smart OCR 代码。
- 不要做 native 动态库插件。
- 不要依赖插件市场在线安装。
- 不要首次使用时联网下载模型。
- 不要把模型或推理库塞进主应用。

## 2. 与主应用的边界

插件仓库负责：

- `manifest.json`
- Rust sidecar 可执行文件
- Paddle OCR / `ocr-rs` / MNN 接入
- 模型文件定位与完整性检查
- `recognizeBatch` 方法
- 插件管理 UI `PaddleOcr.vue`
- Windows x64 ZIP 打包
- OCR 质量和性能 benchmark

主应用仓库负责：

- 修复 sidecar/native adapter 的 settings await。
- 修复 sidecar/native adapter 的运行态同步。
- Smart OCR 新增 `plugin` 引擎。
- ControlPanel 新增插件引擎入口和缺失提示。
- 将 Smart OCR blocks 批量传给 `paddle-ocr.recognizeBatch`。

插件侧不要跨边界修改主应用文件；需要主应用支持的事项记录到 README 或 issue。

## 3. 建议目录结构

开发态：

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
│       ├── det.mnn
│       ├── rec.mnn
│       └── keys.txt
└── README.md
```

生产 ZIP：

```txt
paddle-ocr-v0.1.0-windows-x64.zip
├── manifest.json
├── PaddleOcr.js
├── style.css
├── bin/
│   └── aiohub-paddle-ocr-windows-x64.exe
├── models/
│   └── ppocr-v5-mobile/
│       ├── det.mnn
│       ├── rec.mnn
│       └── keys.txt
└── README.md
```

可参考同目录下的 `plugins/example-file-hasher`，但这个插件是 OCR 重型推理场景，必须坚持批量调用。

## 4. manifest.json 要求

首版 manifest 可从下面开始：

```json
{
  "id": "paddle-ocr",
  "name": "Paddle OCR",
  "version": "0.1.0",
  "description": "基于 PaddleOCR 的本地离线 OCR 插件",
  "author": "AIO Hub Team",
  "icon": "🔎",
  "tags": ["OCR", "PaddleOCR", "本地模型"],
  "host": {
    "appVersion": ">=0.6.3-alpha.5",
    "apiVersion": 2
  },
  "type": "sidecar",
  "sidecar": {
    "executable": {
      "win32-x64": "bin/aiohub-paddle-ocr-windows-x64.exe"
    },
    "args": []
  },
  "methods": [
    {
      "name": "recognizeBatch",
      "displayName": "批量 OCR 识别",
      "description": "使用本地 PaddleOCR 模型批量识别图片块中的文字",
      "parameters": [
        {
          "name": "images",
          "type": "array",
          "required": true,
          "description": "待识别图片块列表"
        },
        {
          "name": "options",
          "type": "object",
          "required": false,
          "description": "模型、语言和识别参数"
        }
      ],
      "returnType": "Promise<PaddleOcrBatchResult>"
    }
  ],
  "ui": {
    "displayName": "Paddle OCR",
    "component": "PaddleOcr.js",
    "icon": "🔎"
  }
}
```

后续补齐其他平台键：

- `win32-arm64`
- `darwin-x64`
- `darwin-arm64`
- `linux-x64`
- `linux-arm64`

## 5. Sidecar 输入输出协议

主应用每次调用会启动 sidecar，并向 stdin 写入一行 JSON：

```ts
interface SidecarInput {
  method: "recognizeBatch";
  params: PaddleOcrBatchRequest;
  settings?: Record<string, unknown>;
}
```

插件需要：

1. 从 stdin 读取完整一行。
2. 解析 JSON。
3. 根据 `method` 分发。
4. 完成 OCR。
5. 向 stdout 输出 JSON Lines。

成功结果必须输出一行：

```json
{ "type": "result", "data": { "results": [] } }
```

进度可输出：

```json
{ "type": "progress", "data": { "message": "正在加载模型", "percent": 10 } }
```

错误可输出：

```json
{ "type": "error", "data": "模型文件缺失: models/ppocr-v5-mobile/det.mnn" }
```

注意：

- 正常日志不要写 stderr；当前主应用会把 stderr 每一行当作 error 事件。
- 正常调试日志可以写 stdout 非 JSON 行，主应用会当作 log 事件。
- 最终只应输出一条 `type: "result"` 作为成功结果。
- 非零退出码会被主应用视为执行失败。

## 6. recognizeBatch 方法契约

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
    modelProfile?: "ppocr-v5-mobile";
    language?: "ch" | "en";
    detLimitSideLen?: number;
    detThresh?: number;
    boxThresh?: number;
    unclipRatio?: number;
  };
}
```

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
- 单张图片失败时返回该图片的 `{ status: "error", error }`，不要让整个批次直接失败。
- 只有模型缺失、输入无法解析、方法不存在等全局错误才让整个调用失败。
- 返回顺序建议与输入顺序一致。

## 7. 模型路径

sidecar 进程的工作目录当前是插件根目录，因此模型路径可以按相对路径定位：

```txt
models/ppocr-v5-mobile/det.mnn
models/ppocr-v5-mobile/rec.mnn
models/ppocr-v5-mobile/keys.txt
```

启动时应检查：

- 模型目录存在。
- 必需文件存在。
- 文件大小不为 0。
- 读取失败时返回清晰错误。

首版模型随 ZIP 分发。不要在首版实现联网下载模型。

## 8. Rust sidecar 建议

建议先做最小闭环：

1. 读取 stdin。
2. 解析 `SidecarInput`。
3. 校验 `method === "recognizeBatch"`。
4. 校验模型文件存在。
5. 解码 data URL/base64。
6. 单图 OCR。
7. 批量循环，逐张收集结果。
8. 输出 `type: "result"`。

data URL 解码注意：

- 输入可能是 `data:image/png;base64,...`。
- 不要把 data URL 当 URL 去 fetch。
- sidecar 里直接截取逗号后的 base64 部分解码。

日志建议：

- 用 stdout 输出进度 JSON。
- 普通 debug 可先输出 stdout 非 JSON 行。
- stderr 只用于真正需要主应用判错的致命错误。

## 9. 插件 UI 首版

`PaddleOcr.vue` 首版只需要管理页能力，不要耦合 Smart OCR 主流程。

建议显示：

- 插件版本。
- 当前后端可执行文件状态。
- 当前模型 profile。
- 模型文件完整性。
- 简单单图测试入口。
- 最近一次识别耗时和图片数量。

UI 不负责 Smart OCR 的正式识别流程。正式识别走 `recognizeBatch`。

## 10. 打包与验证

包管理器使用 Bun。运行脚本前先读取当前 `package.json`。

建议脚本：

```txt
bun run build
bun run package
```

具体以插件仓库实际 `package.json` 为准。

Windows x64 首版验收：

- ZIP 能通过主应用插件导入预检。
- 插件能安装到 app data 插件目录。
- 插件能启用和禁用。
- `execute({ service: "paddle-ocr", method: "recognizeBatch" })` 能返回结果。
- 中文 UI 截图能识别。
- 英文 UI 截图能识别。
- 中英混排截图能识别。
- 多 block 批量任务只启动一次 sidecar 调用。
- 模型缺失时错误清晰。

Benchmark 记录：

- 测试样本名称。
- 图片数量。
- 总像素或尺寸。
- 首次调用耗时。
- 批量总耗时。
- 峰值内存。
- 平均置信度。
- 失败图片数。
- ZIP 大小。

## 11. 后续路线

POC 稳定后再做：

- 文件路径输入，减少 base64/stdin 压力。
- 更多模型 profile。
- 模型完整性 hash 校验。
- 多平台二进制。
- 按平台拆分 ZIP。
- 插件市场元数据。
- 常驻 sidecar daemon。

native 插件只作为后续性能优化备选，不作为首版方向。
