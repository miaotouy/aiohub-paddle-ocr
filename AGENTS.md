# AGENTS.md - Paddle OCR 插件协作规范

## 插件定位

- 本仓库是 AIO Hub 的独立插件仓库，插件 ID 为 `paddle-ocr`。
- 插件类型为 `sidecar`，`manifest.json` 中配置为常驻 OCR 引擎：`resident: true`。
- 主要能力是通过本地 PP-OCRv5 Mobile 模型批量识别图片块，贡献点为 `ocr-engine`。
- 当前只声明了 Windows x64 sidecar：`target/x86_64-pc-windows-msvc/debug/aiohub-paddle-ocr.exe`。

## 关键文件

- `manifest.json` 是方法、模型 profile、贡献点、UI 和 sidecar 路径的事实来源。
- `src/main.rs` 是常驻 JSON Lines sidecar 主入口，负责模型加载、profile 切换、批量识别和输出协议。
- `models/ppocr-v5-mobile/` 存放 OCR 模型与字典文件；不要提交临时下载缓存或错误格式的权重文件。
- `MODEL_THIRD_PARTY_NOTICES.md` 记录模型相关第三方声明，模型变更时要同步检查。
- `PaddleOcr.vue` 是 UI 入口。
- `package.json` 定义 Bun、Rust、Vue 和打包脚本。

## 实现约束

- stdout 是宿主协议通道，必须保持单行 JSON；底层 OCR 库可能输出 stdout，已有 `NativeStdoutSilencer` 用于抑制污染，改动时不要破坏。
- `recognizeBatch` 支持图片 `path` 零拷贝优先，回退到 `dataUrl`；新增输入形态时保持这个优先级。
- 模型 profile 同时受 `manifest.json` 的 `contributions.modelProfiles` 和 `src/main.rs` 的 `MODEL_PROFILES` 约束，新增语言时必须两边同步。
- Windows 平台可以接入系统 OCR 作为兜底方案，但必须保持为能力探测后的 fallback：不要静默替代 Paddle/MNN 默认路径，返回结果仍需符合现有 camelCase 协议，并在错误或调试信息中能区分实际使用的是 Paddle 引擎还是 Windows OCR。
- Windows OCR 兜底只应解决本地模型不可用、模型加载失败或特定 profile 不支持等场景；不要为了兜底引入联网 OCR、外部服务或破坏离线插件定位。
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

本仓库是独立 Git 仓库，提交、tag 和发布包应在本目录内处理。

## 验证重点

- Rust 或模型加载逻辑改动至少运行 `bun run build:rust`。
- 后端 OCR 引擎和 sidecar 协议可以脱离宿主独立运行验证：构建后直接启动生成的 exe，通过 stdin 输入单行 JSON 调用 `recognizeBatch`，检查 stdout 的 JSON Lines、错误路径、模型切换和多图片批处理结果。
- 新增或调整 Windows OCR fallback 时，应独立验证 Paddle 成功路径、fallback 触发路径、fallback 不可用路径，以及两种引擎输出结构是否保持兼容。
- UI 改动至少运行 `bun run build:vue` 或 `bun run build`。
- 模型 profile、方法签名或贡献点变化后，检查 `manifest.json` 与 `src/main.rs` 是否一致。
- AIO Hub Tauri 真实运行态或宿主插件调用环境主要用于验证 UI 交互、插件发现、贡献点注册、宿主调用链和窗口内反馈；不要把纯后端 OCR 识别正确性绑定到宿主 UI 才能验证。
- 普通浏览器只能做明确 mock 后的 Vue 外观检查，不能验证 sidecar 协议、宿主插件调用链或真实 Tauri 运行态。

