# AGENTS.md - Paddle OCR 插件协作规范

## 插件定位

- 本仓库是 AIO Hub 的独立插件仓库，插件 ID 为 `paddle-ocr`。
- 插件类型为 `sidecar`，`manifest.json` 中配置为常驻 OCR 引擎：`resident: true`。
- 主要能力是通过本地 PP-OCRv6 ONNX 或 PP-OCRv5 Mobile 模型批量识别图片块，贡献点为 `ocr-engine`。
- 当前只声明了 Windows x64 sidecar：`target/x86_64-pc-windows-msvc/debug/aiohub-paddle-ocr.exe`。

## 关键文件

- `manifest.json` 是方法、模型 profile、贡献点、UI 和 sidecar 路径的事实来源。
- `src/main.rs` 是常驻 JSON Lines sidecar 主入口，负责模型加载、profile 切换、批量识别和输出协议。
- `models/ppocr-v5-mobile/` 和 `models/ppocr-v6-small-onnx/` 存放内置 OCR 模型与字典/配置文件；不要提交临时下载缓存或错误格式的权重文件。
- `MODEL_THIRD_PARTY_NOTICES.md` 记录模型相关第三方声明，模型变更时要同步检查。
- `PaddleOcr.vue` 是 UI 入口。
- `package.json` 定义 Bun、Rust、Vue 和打包脚本。

## 实现约束

- stdout 是宿主协议通道，必须保持单行 JSON；底层 OCR 库可能输出 stdout，已有 `NativeStdoutSilencer` 用于抑制污染，改动时不要破坏。
- `recognizeBatch` 支持图片 `path` 零拷贝优先，回退到 `dataUrl`；新增输入形态时保持这个优先级。
- 模型 profile 同时受 `manifest.json` 的 `contributions.modelProfiles` 和 `src/main.rs` 的 `MODEL_PROFILES` 约束，新增语言时必须两边同步。
- `manifest.json` / `package.json` 的插件版本是发布版本来源；`Cargo.toml` crate 版本可能独立，不要无依据地强行同步。
- OCR 结果结构使用 camelCase，与宿主和前端契约保持一致。

## 命令

- 安装依赖使用 Bun。
- 构建插件：`bun run build`
- Rust 调试构建：`bun run build:rust`
- Rust 发布构建：`bun run build:rust:release`
- Vue UI 构建：`bun run build:vue`
- 打包：`bun run package`
- 清理：`bun run clean`
- Vue UI 构建时，`vite.config.js` 的 `rollupOptions.output` 需配置 `codeSplitting: false`（Vite 8 / Rolldown 推荐写法），禁用代码分割，消灭分块 JS，彻底解决相对路径加载问题。
- Vue UI 中使用的 Tauri API / 插件（如 `@tauri-apps/api/core`、`@tauri-apps/api/path`、`@tauri-apps/plugin-fs`）以及 `aiohub-sdk`、`aiohub-ui` 均由宿主运行时提供，必须在 `vite.config.js` 的 `rollupOptions.external` 中显式声明为外部依赖，否则 CI 构建会因 Rolldown 无法在本地 `node_modules` 中解析这些包而失败。

本仓库是独立 Git 仓库，提交、tag 和发布包应在本目录内处理。

## 验证重点

- Rust 或模型加载逻辑改动至少运行 `bun run build:rust`。
- 后端 OCR 引擎 and sidecar 协议可以脱离宿主独立运行验证：构建后直接启动生成的 exe，通过 stdin 输入单行 JSON 调用 `recognizeBatch`，检查 stdout 的 JSON Lines、错误路径、模型切换和多图片批处理结果。
- UI 改动至少运行 `bun run build:vue` 或 `bun run build`。
- 模型 profile、方法签名或贡献点变化后，检查 `manifest.json` 与 `src/main.rs` 是否一致。
- AIO Hub Tauri 真实运行态或宿主插件调用环境主要用于验证 UI 交互、插件发现、贡献点注册、宿主调用链和窗口内反馈；不要把纯后端 OCR 识别正确性绑定到宿主 UI 才能验证。
- 普通浏览器只能做明确 mock 后的 Vue 外观检查，不能验证 sidecar 协议、宿主插件调用链或真实 Tauri 运行态。

