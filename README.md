# AIO Hub Paddle OCR 插件

AIO Hub Paddle OCR 是面向 AIO Hub Smart OCR 的本地离线 OCR sidecar 插件。插件通过独立进程加载 PP-OCRv6 ONNX 模型或 PP-OCRv5 MNN 模型，并向主应用暴露统一的批量识别方法，用于在 Smart OCR 中处理图片块、截图块和其他图像输入。

本插件支持内置的 PP-OCRv6 ONNX 与 PP-OCRv5 Mobile 模型，并提供**双注册表动态合并方案**，允许用户在管理界面中**一键扫描并导入自定义模型**（支持 MNN 与 ONNX 运行时）。

## 项目状态

- **插件 ID**：`paddle-ocr`
- **插件类型**：`sidecar` (常驻 OCR 引擎：`resident: true`)
- **支持方法**：`healthCheck`、`recognizeBatch`、`shutdown`
- **默认模型**：`ppocr-v6-small-onnx` (PP-OCRv6 Small ONNX)
- **Smart OCR 扩展**：通过 `manifest.json` 的 `contributions[]` 声明 `type: "ocr-engine"`
- **前端 UI**：基于 Vue 3 + Element Plus + Tailwind CSS 的现代毛玻璃三栏式管理页，支持图片拖拽/粘贴预览、文本框坐标叠加绘制、识别结果单行自治编辑、复制与发送到聊天。

---

## 功能特性

### 1. 核心 OCR 引擎
- **ONNX 运行时**：默认使用 `onnxruntime` 后端，加载 PP-OCRv6 Small ONNX 模型，具备优秀的识别精度与定位块支持。
- **MNN 运行时**：支持 `mnn-ocr-rs` 后端，可加载 PP-OCRv5 Mobile 轻量化模型，具备极高的推理速度与极低的内存占用。

### 2. 双注册表动态合并与自定义模型导入
- **只读安装目录保护**：生产环境下插件安装目录通常是只读的，且升级时会被覆盖。因此，插件采用**双注册表合并方案**。
- **内置注册表**：读取安装目录下的 `models/registry.json`。
- **自定义注册表**：读取插件专属数据目录（`AIOHUB_PLUGIN_DATA_DIR`，即 `appConfigDir/plugins-data/paddle-ocr/`）下的 `custom-registry.json`。
- **一键扫描导入**：用户可在 UI 中选择本地任意包含模型的文件夹，系统会自动扫描并智能匹配检测模型（`.mnn`/`.onnx`）、识别模型（`.mnn`/`.onnx`）、配置文件（`.yml`）及字典文件（`.txt`），一键拷贝至专属数据目录并动态合并注册表，无需重启插件即可直接切换使用。

### 3. 现代化的管理界面 (`PaddleOcr.vue`)
  - **顶部控制栏 (ControlPanel)**：横向通栏，展示运行时状态指标卡片（运行时状态、模型状态、后端类型、最近耗时）、模型 Profile 动态选择、一键健康检查与识别测试。
  - **下方主内容区 (双栏自适应)**：
    - **左侧预览面板 (PreviewPanel)**：支持拖拽、粘贴剪贴板图片，并在图片上方根据 OCR 识别出的 `bbox` 坐标动态叠加绘制半透明文本框。
    - **右侧结果面板 (ResultPanel)**：格式化展示识别结果，支持**单行自治编辑**（不污染原始数据）、单行复制、一键复制全部、发送到聊天以及原始 JSON 树折叠预览。

---

## 目录结构

### 开发态目录

```txt
plugins/aiohub-paddle-ocr/
├── manifest.json             # 插件清单（方法、贡献点、UI 和 sidecar 路径的事实来源）
├── package.json              # 依赖与构建脚本
├── vite.config.js            # Vite 配置文件（配置外部依赖与禁用代码分割）
├── PaddleOcr.vue             # 插件管理页主入口
├── components/               # UI 子组件
│   ├── ControlPanel.vue      # 控制面板与状态卡片
│   ├── PreviewPanel.vue      # 图片预览与文本框叠加绘制
│   ├── ResultPanel.vue       # 格式化结果展示与自治编辑
│   ├── ImportModelDialog.vue # 自定义模型一键扫描导入弹窗
│   └── types.ts              # 前端类型定义
├── composables/              # 组合式函数
│   ├── useModelRegistry.ts   # 双注册表合并与导入逻辑
│   ├── useOcrEngine.ts       # OCR 引擎调用与状态同步
│   └── useOcrImage.ts        # 图片选择与处理
├── utils/
│   └── ocr-helpers.ts        # 坐标转换与辅助工具
├── src/                      # Rust Sidecar 源码
│   ├── main.rs               # Sidecar 主入口（常驻 stdin/stdout 循环）
│   ├── model_registry.rs     # Rust 侧双注册表动态合并与模型校验
│   ├── protocol.rs           # 统一的 JSON-RPC 交互协议
│   ├── recognition.rs        # 批量识别分发与零拷贝路径
│   ├── native_stdout.rs      # 抑制底层 C++ 库污染 stdout 的静音器
│   └── backends/             # 推理后端
│       ├── mod.rs            # 引擎缓存与动态切换加载器
│       ├── mnn.rs            # MNN 推理后端
│       └── onnx.rs           # ONNX Runtime 推理后端
└── models/
    ├── registry.json         # 内置模型注册表
    ├── ppocr-v5-mobile/      # 内置 PP-OCRv5 Mobile 模型目录
    └── ppocr-v6-small-onnx/  # 内置 PP-OCRv6 Small ONNX 模型目录
```

---

## 调用协议

主应用启动 sidecar 并保持进程。每次执行插件方法时，向 sidecar 的 `stdin` 写入一行 JSON，sidecar 处理后通过 `stdout` 输出单行 JSON Lines 响应。

### 输入协议 (stdin)

```typescript
interface ResidentInput {
  id?: number;                  // 请求 ID（用于 JSON-RPC 匹配）
  method: "healthCheck" | "recognizeBatch" | "shutdown";
  params: any;
}
```

### 1. healthCheck (运行时健康检查)

检查 sidecar 运行状态、模型清单和指定 profile 的模型文件是否可用，不执行实际的 OCR 识别。

**调用参数**：
```typescript
interface HealthCheckRequest {
  options?: {
    modelProfile?: string;      // 指定检查的 profile ID
    language?: string;          // 指定检查的语言
  };
}
```

**返回数据** (`type: "result"`)：
```typescript
interface HealthCheckResult {
  ready: true;
  status: "ok";
  backend: "mnn-ocr-rs" | "onnxruntime";
  profile: string;              // 当前解析出的 profile ID
  profileName: string;          // profile 名称
  modelFiles: "ok";
}
```

### 2. recognizeBatch (批量 OCR 识别)

支持多张图片批量识别。**零拷贝优先**：如果传入了本地图片 `path`，sidecar 将直接从磁盘读取，避免 base64 编码带来的内存与 CPU 开销；若无 `path` 则回退到 `dataUrl`。

**调用参数**：
```typescript
interface RecognizeBatchRequest {
  images: Array<{
    blockId: string;
    imageId: string;
    path?: string;              // 零拷贝本地路径（优先）
    dataUrl?: string;           // base64 data URL（回退）
  }>;
  options?: {
    modelProfile?: string;      // 指定使用的模型 profile
    language?: string;          // 语言映射
    detLimitSideLen?: number;
    detThresh?: number;
    boxThresh?: number;
    unclipRatio?: number;
  };
}
```

**返回数据** (`type: "result"`)：
```typescript
interface PaddleOcrBatchResult {
  results: Array<{
    blockId: string;
    imageId: string;
    text: string;               // 合并后的完整文本
    confidence?: number;        // 平均置信度
    status: "success" | "error";
    error?: string;             // 单张图片失败时的错误信息
    lines?: Array<{             // 识别出的单行文本及坐标
      text: string;
      score: number;
      bbox: Array<[number, number]>; // 四点坐标 [[x1,y1], [x2,y2], [x3,y3], [x4,y4]]
    }>;
  }>;
}
```

---

## 构建与打包

本插件使用 **Bun** 作为前端包管理器与构建工具，**Cargo** 作为 Rust 编译器。

### 1. 安装依赖
```bash
bun install
```

### 2. 编译 Rust Sidecar
```bash
# 调试构建
bun run build:rust

# 发布构建
bun run build:rust:release
```

### 3. 编译 Vue UI
```bash
bun run build:vue
```
*注：Vite 构建时已配置禁用代码分割（`codeSplitting: false`），消灭分块 JS，彻底解决相对路径加载问题。同时将 Tauri API 及 `aiohub-sdk` 声明为 external 依赖。*

### 4. 一键构建与打包
```bash
# 构建 UI 与 Sidecar
bun run build

# 校验模型、复制产物并打包当前平台 ZIP
bun run package
```

---

## 验收与验证重点
## 验收与验证重点

1. **零拷贝路径验证**：确保 `recognizeBatch` 优先使用 `path` 读取图片，且在 `path` 缺失时能完美回退到 `dataUrl`。
2. **双注册表合并验证**：导入自定义模型后，检查 `AIOHUB_PLUGIN_DATA_DIR/custom-registry.json` 是否正确生成，且前端与 Rust 侧均能无缝加载该自定义 profile。
3. **UI 交互与自治状态**：验证在 `ResultPanel` 中双击修改某行文本后，点击“复制全部”或“发送到聊天”时，导出的文本是否为修改后的内容，且不污染原始的 `result` prop。
4. **Stdout 纯净度**：确保 sidecar 进程的 stdout 只有单行 JSON Lines，无任何底层 C++ 库或 MNN 框架输出的杂质日志（已通过 `NativeStdoutSilencer` 抑制）。
