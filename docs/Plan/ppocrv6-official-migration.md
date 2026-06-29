# PP-OCRv6 官方模型迁移计划

创建日期：2026-06-29

## 背景

当前 `paddle-ocr` 插件使用 `ocr-rs` + MNN 推理，模型包来自第三方整合包 `BitYoungjae/ChalKak OCR Models v1`，不是 PaddleOCR 官方直接发布的模型包。虽然模型族标注为 PP-OCRv5 mobile，但实际质量会受模型转换、MNN 后处理、`ocr-rs` 默认参数和插件输入链路共同影响，不能等同于官方 PP-OCRv5 表现。

PaddleOCR 已在 2026-06-11 发布 3.7.0，并推出 PP-OCRv6。官方资料显示 PP-OCRv6 提供 tiny / small / medium 三档模型，统一多语种识别能力更强，并支持 Paddle、Transformers、ONNX Runtime 等推理方式。因此后续应优先迁移到官方模型和官方支持的推理链路，再考虑是否需要转换为 MNN。

## 官方来源

- PaddleOCR Release：`https://github.com/PaddlePaddle/PaddleOCR/releases`
- PP-OCRv6 文档：`https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/algorithm/PP-OCRv6/PP-OCRv6.en.md`
- 文本检测模块文档：`https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/module_usage/text_detection.en.md`
- 文本识别模块文档：`https://github.com/PaddlePaddle/PaddleOCR/blob/main/docs/version3.x/module_usage/text_recognition.en.md`
- 官方模型组织：`https://huggingface.co/PaddlePaddle`
- 参考模型页：
  - `https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det`
  - `https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec`
  - `https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_onnx`
  - `https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_onnx`
- ONNX Runtime 官方安装文档：`https://onnxruntime.ai/docs/install/`
- ONNX Runtime Execution Providers：`https://onnxruntime.ai/docs/execution-providers/`
- Tauri v2 sidecar / external binary 文档：`https://v2.tauri.app/develop/sidecar/`

## 迁移原则

1. 官方模型优先：模型来源应来自 PaddleOCR 官方发布渠道或 PaddlePaddle 官方 Hugging Face / ModelScope 仓库。
2. 官方推理链路优先：第一阶段不再把 safetensors 或 Paddle 模型强行转成 MNN。先用 PaddleOCR 官方 CLI / Python API 或 ONNX Runtime 验证质量。
3. 保持离线插件定位：发布包可以随包携带模型和运行时，但正式识别不依赖联网下载。
4. 质量与体积平衡：不因包体积过度妥协识别质量。默认随包自带官方小型（small/tiny）模型以保证开箱即用，同时设计“模型追加/导入”机制，允许用户手动追加 medium 档或多语言模型。
5. 保持现有宿主契约：`recognizeBatch` 输入输出结构继续兼容 Smart OCR，主应用无需一次性重构。
6. 先 benchmark 后替换默认：只有 PP-OCRv6 在真实 UI 截图集上稳定优于当前 v5/MNN 后，才切默认 profile。

## 后端方案对比

### 方案 A：Python PaddleOCR sidecar

做法：Rust sidecar 仍负责 JSON Lines 协议和进程生命周期，内部调用随插件打包的 Python 运行时或独立 Python sidecar，使用官方 `paddleocr` 包加载 `PP-OCRv6_small_det` + `PP-OCRv6_small_rec`。

**Python 方案的“重量”估算：**

- **磁盘占用**：嵌入式 Python 解释器 (~100MB) + PaddlePaddle CPU版 (~450MB) + OpenCV/NumPy等依赖 (~250MB) + 官方模型 (~15MB)。压缩包随插件分发约 **300MB - 400MB**，解压后运行时占用约 **1GB - 1.5GB**。
- **内存占用**：运行期常驻内存约 **300MB - 500MB**。
- **启动延迟**：由于 Python 解释器冷启动以及 `import paddleocr` 需要加载大量 C++ 绑定，冷启动延迟通常在 **3 - 5 秒** 左右。

**宿主机全局 Python 的影响与应对策略：**

- **直接调用的风险**：
  1. **版本不匹配**：宿主机全局 Python 版本（如 3.12+）可能与 PaddlePaddle 官方支持的版本（通常为 3.8 - 3.11）不兼容。
  2. **依赖缺失**：全局环境通常没有安装 `paddleocr`、`paddlepaddle` 等庞大的 C++ 绑定库，直接调用会抛出 `ModuleNotFoundError`。
  3. **环境污染与权限**：直接在全局环境中 `pip install` 会污染用户系统，且在 Windows 下可能因权限不足（需要管理员权限）而失败。
- **应对策略**：
  - **策略一：虚拟环境隔离（推荐）**：如果检测到全局 Python，插件不直接使用它，而是利用它在插件私有目录下创建一个独立的虚拟环境（`python -m venv .venv`），然后在这个隔离环境中安装依赖。
    - _缺点_：首次运行需要在线下载并安装数百MB的依赖，耗时极长（受网络和 pip 镜像影响），且容易因 C++ 编译环境缺失导致安装失败。
  - **策略二：自带嵌入式 Python 运行时**：不依赖宿主机任何 Python 环境，随包携带一个精简的嵌入式 Python 运行时（Embedded Python），所有依赖预先打包好。
    - _优点_：完全离线，开箱即用，100% 隔离，无任何环境冲突风险。
    - _缺点_：插件包体积会增加约 300MB。

优点：

- 最接近官方推理链路，质量验证成本最低。
- 能快速跟随 PaddleOCR 新模型和后处理更新。
- 可直接验证 `ocr_version="PP-OCRv6"`、`text_detection_model_name`、`text_recognition_model_name` 等官方配置。

缺点：

- 发布包体积和运行时复杂度显著增加（如上所述，解压后超 1GB）。
- 多平台打包、Python 依赖冻结、PaddlePaddle 原生库兼容成本高。
- stdout/stderr 污染风险更高，需要严格隔离协议输出。

适合：第一阶段质量基线和 Windows 原型。

### 方案 B：ONNX Runtime sidecar

做法：使用 PaddleOCR 官方支持的 ONNX Runtime engine，模型采用官方或官方导出的 ONNX 包。Rust 端改用 `ort` / ONNX Runtime，或引入一个最小 Python/Node ONNX runner。

**2026-06-29 调查结论：可行，且更适合作为正式主线；但不是“只替换推理库”。**

- 官方侧已具备直接落地条件：
  - PaddleOCR 通用 OCR pipeline 已支持 `engine="onnxruntime"`，并允许通过 Python API / CLI 使用 PP-OCRv6 默认模型或显式指定 det / rec 模型。
  - PaddleOCR 文本检测、文本识别模块均支持 `engine="onnxruntime"`；官方文档同时提示默认 `paddle_static` 在多数场景仍是推荐性能基线，因此 ONNX 方案必须用本插件真实截图集 benchmark，而不能只看官方单图数据。
  - PaddleX 提供 Paddle2ONNX 插件，可把 Paddle 静态图模型导出为 ONNX；如果官方 ONNX 包不可用或需要自定义模型，这是后备获取路径。
- 官方 Hugging Face 已发布 PP-OCRv6 ONNX 仓库：
  - `PaddlePaddle/PP-OCRv6_small_det_onnx`：仓库约 10.1MB，`inference.onnx` 约 9.88MB，包含 `inference.yml` / `inference.json`。
  - `PaddlePaddle/PP-OCRv6_small_rec_onnx`：仓库约 21.5MB，`inference.onnx` 约 21.2MB，包含 `inference.yml` / `inference.json`。
  - medium / tiny 也有对应 det / rec ONNX 仓库；small 是本插件默认候选，medium 更适合用户手动追加，tiny 可作为低配设备 profile。
  - ONNX 模型页标注 Apache-2.0，仍需要在插件发布包里记录来源 URL、commit / revision、文件 hash 和许可。
- 当前插件现状与影响：
  - 现有 sidecar 已是 Rust 常驻进程 + JSON Lines 协议，`recognizeBatch`、`path` 优先、stdout 协议通道和 `NativeStdoutSilencer` 都应保留。
  - ONNX Runtime 方案不需要新增宿主侧协议；主要是在 sidecar 内新增 `OnnxRuntimeBackend`，并把当前 `ocr-rs` / MNN 实现降级为 `MnnOcrBackend`。
  - 当前 `manifest.json`、`src/main.rs`、`build.js` 都重复维护模型 profile；ONNX 接入前必须先把模型清单抽成 `models/registry.json`，否则 det / rec / yml / dict / runtime 文件会继续散落在多处。

**ONNX Runtime sidecar 建议形态：纯 Rust 优先，Python ONNX 只做对齐工具。**

1. **最终发布形态：运行时内置（CI/CD 自动拉取与打包），模型外置（按需加载与导入）**
   - **运行时内置与 CI/CD 交叉编译**：ONNX Runtime 动态库（Windows `onnxruntime.dll` ~10MB，macOS `libonnxruntime.dylib`）直接内置在程序中，作为 Tauri 的 `resources` 随应用一同打包分发。
     - **CI/CD 自动化**：利用 GitHub Actions 等 CI/CD 流程，在多平台构建时，根据当前 Runner 平台（Windows / macOS / Linux）自动拉取对应平台的官方 ONNX Runtime 动态库，并将其打包进最终的发布包中。
     - **本地开发适配**：在本地开发阶段，通过 `build.js` 或 `setup` 脚本，根据当前系统自动下载对应的动态库到本地临时目录，保证本地 `tauri dev` 开箱即用。
     - **动态加载与隔离**：Rust sidecar 启动时，通过 `ort` 的 `load-dynamic` 方式，显式从应用资源目录（或 sidecar 同级目录）加载该私有动态库。这能彻底防止加载到系统 `PATH` 中其他软件的旧版本 DLL 导致的冲突。
     - **安全合规**：macOS 下内置的 `libonnxruntime.dylib` 必须在打包阶段（`build.js`）显式加入 `codesign` 签名列表，确保通过 Gatekeeper 公证。
   - **模型外置**：模型文件（`.onnx`、`.yml`、字典）不硬编码进二进制，而是放在外置的 `models/` 目录（或用户 `appData` 目录）下。
     - 默认随包携带一套最轻量的 `ppocr-v6-small` 模型以保证基础功能可用。
     - 允许用户在前端界面动态导入其他 medium 档或多语言模型，实现“轻量核心 + 动态扩展”。
   - Windows 首期只启用 CPU Execution Provider，降低 CUDA / DirectML provider DLL、驱动版本和包体积风险。
   - 运行时加载失败要返回明确错误：缺少 ORT 动态库、ORT 版本不匹配、ONNX 模型无法加载、模型 metadata 缺失分别区分。
2. **对齐与回归工具：官方 PaddleOCR Python + `engine="onnxruntime"`**
   - 不作为正式发布路径，只作为 golden runner。
   - 用同一批输入图生成官方结果 JSON，记录 `dt_polys`、`dt_scores`、`rec_text`、`rec_score`，作为 Rust ONNX pipeline 的对齐基准。
   - 禁止依赖运行期联网下载；模型必须预先下载到测试目录，并记录 revision / hash。
3. **暂不推荐：Node ONNX runner**
   - `onnxruntime-node` 能减少 Rust 图像处理代码，但会引入 Node 打包、native addon 和跨平台二进制分发问题。
   - 对当前插件已有 Rust sidecar 来说，Node 只会多一层运行时边界，除非后续主应用已有统一 Node sidecar 分发能力。

**纯 Rust ONNX pipeline 需要实现的具体工作：**

1. **模型与清单**
   - 新增 `models/registry.json`，字段至少包含：`backend`、`family`、`tier`、`detModelDir`、`recModelDir`、`detOnnx`、`recOnnx`、`detConfig`、`recConfig`、`dict` / `characters`、`sourceUrl`、`revision`、`sha256`、`license`、`builtIn`。
   - 新增 `models/ppocr-v6-small-onnx/README.md`，说明模型文件不入仓、下载位置、校验方式和许可。
   - 预置 small det + rec；medium / tiny 先进入可导入 profile，不作为默认。
   - `inference.yml` / `inference.json` 要随模型保留，用于读取官方预处理、后处理和字典配置；如果识别模型没有独立 dict 文件，则从官方 metadata 中导出字符表并固定到 registry。
2. **运行时依赖**
   - 新增 `runtime/onnxruntime/{platform}/` 或 `bin/onnxruntime/{platform}/` 目录约定，保存 ORT 动态库和 provider DLL。
   - `build.js` 增加 ORT 文件校验和复制，不再只校验 `.mnn`。
   - 发布包记录 ORT 版本、下载 URL、license、hash；Windows CPU 包首期目标是把新增运行时体积控制在几十 MB 级，而不是 Python 方案的数百 MB。
3. **Rust 后端抽象**
   - 增加 `OcrBackend` trait：

```txt
OcrBackend
├── id()
├── load(profile, runtimeOptions)
├── recognize(imageBytes | imagePath)
└── capabilities()
```

- `EngineHolder` 缓存键从单一 `profileId` 改为 `backend + profileId + runtimeOptionsFingerprint`。
- `MnnOcrBackend` 包装现有 `ocr-rs` 逻辑；`OnnxRuntimeBackend` 负责 det / rec 两个 ONNX session、配置解析和 pipeline。
- 错误类型补充 `MissingRuntimeLibrary`、`InvalidModelRegistry`、`OnnxSessionLoadFailed`、`OnnxInferenceFailed`、`PostProcessFailed`。

4. **检测前处理**
   - 按官方 `inference.yml` 读取 `DecodeImage(img_mode=BGR)`、`DetResizeForTest`、`NormalizeImage(mean/std/scale/order)`、`ToCHWImage` 和 `KeepKeys`。
   - 构造 `shape_list`，保留原图尺寸、resize 后尺寸和缩放比例，供 DB 后处理把坐标映射回原图。
   - 支持动态输入尺寸，保持边长约束和 32 倍数对齐；默认参数与官方 small det 配置对齐，例如 `thresh=0.2`、`box_thresh=0.45`、`max_candidates=3000`、`unclip_ratio=1.4`。
5. **检测后处理**
   - 复刻 PaddleOCR `DBPostProcess`：概率图阈值化、轮廓/连通域提取、box score、unclip、多边形裁剪、坐标反缩放、过小 box 过滤。
   - 优先用纯 Rust 依赖组合完成：`image` / `imageproc` 处理图像，`clipper2` 或等价库做 polygon offset；如果轮廓质量难以对齐，再评估 OpenCV，但 OpenCV 会显著增加打包复杂度。
   - 输出四点 `bbox` 必须与现有 `OcrLine.bbox` 兼容。
6. **裁剪、排序与识别前处理**
   - 对检测框执行 PaddleOCR 同款排序：先按 y 再按 x，同一行 y 差小于阈值时按 x 调整。
   - 实现四点透视裁剪 / 旋转裁剪；默认暂不启用文本行方向分类，保持 `use_textline_orientation=False`。
   - 识别输入按官方 rec 配置 resize、normalize、padding，支持批处理；为了避免超宽 UI 文本造成显存/内存尖峰，可以按宽度分桶批量识别。
7. **识别后处理**
   - PP-OCRv6 识别模型推理侧主要按 CTC 头解码：实现 blank 过滤、重复字符折叠、字符表映射、平均置信度。
   - 结果过滤对齐官方 `drop_score` / `score_thresh`；最终仍返回 `text`、`confidence`、`lines`，保持 `PaddleOcrBatchResult` 兼容。
8. **协议与并发**
   - `recognizeBatch` 入参不变；`options.modelProfile` 可选 `ppocr-v6-small-onnx`。
   - `path` 优先读取保持不变，`dataUrl` 仍用纯 JS / Rust base64 解码路径，不引入 `fetch(dataUrl)`。
   - ONNX session 是否可并发需要实测；首版可先像当前 MNN 一样用 Mutex 串行推理，待确认线程安全和收益后再放开 det / rec batch 并发。
   - progress / error 中附带 backend 和 profile，便于区分 MNN legacy、ONNX 官方链路和未来 fallback。
9. **验证与 benchmark**
   - 先做 parity harness：同一张图分别跑官方 Python `engine="onnxruntime"` 和 Rust ONNX，比较检测框数量、坐标 IoU、识别文本、score 差异。
   - 再跑真实 Smart OCR benchmark；默认切换前要求 small ONNX 在中文 UI、中英混排、小字号场景整体优于当前 v5/MNN general。
   - 单测覆盖：模型缺失、ORT DLL 缺失、错误 ONNX 文件、缺 metadata、空图、无文本图、多图批量、单图失败不影响整批。
   - 构建验证至少运行 `bun run build:rust`；涉及 UI / manifest 时再运行 `bun run build:vue` 或 `bun run build`。

**阶段建议：**

- **B0：官方 ONNX 可用性确认（1 天）**
  - 下载 small det / rec ONNX 仓库到临时目录，固定 revision 和 hash。
  - 用 Python PaddleOCR `engine="onnxruntime"` 在 5-10 张 Smart OCR 截图上跑通，输出 golden JSON。
- **B1：Rust ORT smoke（1-2 天）**
  - sidecar 能加载 ORT 动态库和两个 ONNX session。
  - 先跑 detection / recognition 单模块 tensor 输入输出，不要求完整 OCR 文本正确。
- **B2：完整 Rust pipeline MVP（3-5 天）**
  - 实现 det 前后处理、裁剪、rec 前后处理，接入 `recognizeBatch`。
  - 与 golden JSON 做误差分析，补齐 DB 后处理和裁剪细节。
- **B3：产品化接入（2-4 天）**
  - registry、manifest profile、build/package、notices、UI 展示 backend/source/hash。
  - 完成 benchmark 后再决定是否把默认 profile 从 v5/MNN 切到 v6/small/ONNX。

优点：

- 比 Python PaddleOCR 更适合插件分发和跨平台。
- 推理运行时边界清晰，长期可做成纯 Rust sidecar。
- 官方文档已明确 OCR pipeline、文本检测、文本识别模块支持 `engine="onnxruntime"`，且官方已经发布 PP-OCRv6 det / rec ONNX 仓库。

缺点：

- 需要复刻 PaddleOCR pipeline 的预处理、检测后处理、裁剪、识别解码、版面排序。
- 如果直接拆 det/rec 两个模块，质量取决于我们实现的后处理是否与官方一致。
- 需要维护 ORT 动态库、provider DLL、模型 metadata、hash 与多平台打包规则。
- 需要确认识别模型字符表来源：优先从官方 `inference.yml` / `inference.json` 固化，不能手写或沿用 v5 字典。

适合：第二阶段正式插件主线。

### 方案 C：继续 MNN / `ocr-rs`

做法：把 PP-OCRv6 转换成 MNN，并继续接入 `ocr-rs`。

优点：

- 当前代码改动最少。
- 包体积和运行时依赖较小。

缺点：

- 又会回到非官方转换链路，无法直接证明等价于官方效果。
- `ocr-rs` 当前默认检测参数、后处理和 profile 机制与官方 PP-OCRv6 可能不匹配。
- PP-OCRv6 输出结构、字典和后处理如有差异，可能出现静默质量劣化。

适合：仅作为性能探索，不作为迁移主线。

## 推荐路线

### 2026-06-29 开工记录：方案 B 基础层

已完成第一轮低风险接入，不切默认模型、不声称 PP-OCRv6 ONNX 已可用于正式识别：

- 新增 `models/registry.json`，把现有 PP-OCRv5/MNN profile 和 `ppocr-v6-small-onnx` experimental profile 统一登记。
- `src/main.rs` 已从 registry 读取模型 profile，并引入 `OcrBackend`、`MnnOcrBackend` 和 `OnnxRuntimeBackend` 边界；现有 MNN 识别路径保留。
- `EngineHolder` 缓存键已从单一 profile 改为 `backend + profile + runtime options`，避免参数变化后误复用旧引擎。
- ONNX profile 当前只完成模型文件和私有 ORT 动态库路径校验；完整 ORT session、DBPostProcess、透视裁剪、CTC 解码尚未实现。
- `manifest.json`、`build.js` 和 `PaddleOcr.vue` 已同步 registry；v6 profile 标记为 experimental，默认仍是 `ppocr-v5-mobile-general`。
- 管理页 smoke test 已改成用户选择真实图片后执行，不再使用 1x1 空图。
- 已补充 `models/ppocr-v6-small-onnx/README.md` 和 `MODEL_THIRD_PARTY_NOTICES.md` 中的 PP-OCRv6 / ONNX Runtime 待固定来源记录。
- 已验证：`bun run build:rust`、`bun run build:vue`、`bun run build` 通过。

下一步应进入 B0/B1：固定官方 det / rec ONNX 仓库 revision/hash，下载模型与 ORT runtime 到私有目录，先实现 Rust ORT session smoke，再做完整 pipeline 对齐。

### 2026-06-29 继续记录：B0/B1 session smoke

在上一轮基础层之上，已继续完成方案 B 的 B0/B1 最小闭环：

- 固定 Hugging Face 官方 ONNX 仓库 revision：
  - `PaddlePaddle/PP-OCRv6_small_det_onnx`：`28fe5895c24fd108c19eb3e8479f4ab385fbfc62`
  - `PaddlePaddle/PP-OCRv6_small_rec_onnx`：`b8f84f0b80c529de40b4fbb3544b84fa7233a513`
- `download-v6-models.js` 已改为按固定 revision 下载，并对 6 个模型/配置文件做 SHA256 校验。
- `Cargo.toml` 已固定 `ort = 2.0.0-rc.12`，关闭默认自动下载/复制动态库，启用 `api-20` + `load-dynamic`，与当前私有 ONNX Runtime 1.20.1 DLL 对齐。
- `OnnxRuntimeBackend` 已从占位推进到真实加载私有 `runtime/onnxruntime/windows-x64/onnxruntime.dll`，并加载 det / rec 两个 ONNX session。
- sidecar 空 batch smoke 已通过：`ppocr-v6-small-onnx` 能完成 registry 解析、ORT 动态库加载、det session 加载、rec session 加载，并保持 stdout 为单行 JSON Lines。
- 单图调用仍返回 per-image error，明确提示完整 PP-OCRv6 pipeline 尚未实现，同时输出 det / rec input/output shape，便于后续前处理和 CTC 解码对齐。
- `.gitignore` 已补充 v6 ONNX 模型目录与 ORT 动态库，避免本地验证用二进制误提交。
- 已验证：`bun run build:rust`、`bun run build:vue`、`bun run build` 通过。

### 2026-06-29 继续记录：B2 配置解析与 tensor smoke

在 B0/B1 session smoke 基础上，已完成 B2 前半的配置解析与真实张量推理 smoke：

- 新增 `serde_yaml`，Rust sidecar 现在会读取官方 det / rec `inference.yml`，解析 `NormalizeImage`、`DBPostProcess`、`RecResizeImg` 和 `CTCLabelDecode.character_dict`。
- `OnnxRuntimeBackend::load` 在加载 det / rec session 后会立即运行一次零张量 smoke，确认 ORT runtime、ONNX 模型、配置输入 shape 和模型输出同时可用。
- det smoke 使用官方 HPI 最小动态输入 `[1, 3, 32, 32]`，输出已验证为 `fetch_name_0: Tensor<f32>(1, 1, 32, 32)`。
- rec smoke 使用官方识别输入 `[1, 3, 48, 320]`，输出已验证为 `fetch_name_0: Tensor<f32>(1, 40, 18710)`；当前解析到 `character_dict` 18708 项，后续 CTC 解码需要显式确认 blank / 特殊 token 对齐。
- v6 单图调用仍返回 per-image error，明确说明完整 pipeline 尚未实现，同时包含 config、smoke、det / rec IO 摘要，stdout 保持单行 JSON Lines。
- 已验证：`bun run build:rust` 通过；debug sidecar 空 batch 和单图占位输入 smoke 均通过。

下一步进入 B2 检测链路：实现图片解码后的 det 前处理、DBPostProcess、检测框排序与最小 bbox 输出，再接入裁剪和 rec 预处理 / CTC 解码。

### 阶段 0：建立真实 benchmark

- 收集至少 30 张真实 Smart OCR 输入图片：
  - 中文 UI 截图
  - 英文 UI 截图
  - 中英混排
  - 小字号、低对比度、深色主题
  - 长截图切图前后
- 固化期望文本或人工校验结果。
- 记录 native OCR、当前 PP-OCRv5/MNN general、当前 PP-OCRv5/MNN en、PP-OCRv6 small、PP-OCRv6 medium 的结果。
- 指标至少包含：
  - 字符错误率或人工通过率
  - 漏行数
  - 错行顺序
  - 平均耗时
  - 首次加载耗时
  - 峰值内存
  - 发布包体积

### 阶段 1：官方 Python 原型

- 新增实验 runner，不替换当前默认引擎。
- 使用官方 PaddleOCR API：
  - `ocr_version="PP-OCRv6"`，或
  - 显式指定 `text_detection_model_name="PP-OCRv6_small_det"`、`text_recognition_model_name="PP-OCRv6_small_rec"`。
- 输出转换成现有 `PaddleOcrBatchResult`。
- 验证 `path` 输入，不重新走 base64。
- 禁止运行期联网下载：模型必须预置在插件目录或安装步骤中完成离线缓存。

### 阶段 2：抽象引擎层与模型管理

把当前 `PaddleOcrEngine` 拆成接口：

```txt
OcrBackend
├── load(profile, runtimeOptions)
├── recognize(imageBytes | imagePath)
└── capabilities()
```

实现：

- `MnnOcrBackend`：保留当前 v5/MNN 作为兼容路径。
- `Ppocrv6OfficialBackend`：官方 Python 或 ONNX Runtime 实验后端。

同时把模型 profile 从 Rust 和 `build.js` 的重复常量中抽出，改成单一清单，并支持**用户自定义追加模型**：

```txt
models/registry.json
```

清单字段建议：

```json
{
  "id": "ppocr-v6-small",
  "family": "ppocr-v6",
  "backend": "paddleocr-python",
  "detModel": "PP-OCRv6_small_det",
  "recModel": "PP-OCRv6_small_rec",
  "languages": ["zh-CN", "zh-TW", "en", "ja", "latin"],
  "source": "PaddlePaddle",
  "license": "Apache-2.0",
  "isBuiltIn": true
}
```

**模型追加机制设计（⚠️ 依赖主应用升级）**：

1. **本地导入**：在 `PaddleOcr.vue` 界面提供“导入自定义模型”入口，允许用户选择本地的 `.onnx` 或 Paddle 格式模型文件。
2. **动态注册**：导入后，插件将模型文件拷贝至主应用注入的专属数据目录 `AIOHUB_PLUGIN_DATA_DIR/custom_models/{modelId}/` 下，并在用户级 `custom_registry.json` 中追加记录。
   - _注：此机制必须依赖主应用升级支持 `AIOHUB_PLUGIN_DATA_DIR` 环境变量，否则在插件升级时，写入插件安装目录下的自定义模型会被物理抹除。_
3. **按需加载**：`OcrBackend` 启动时，合并内置的 `registry.json` 与专属数据目录下的 `custom_registry.json`，允许用户在前端下拉菜单中切换到追加的 medium 档或多语言模型。

### 阶段 3：ONNX Runtime 主线化

- 使用官方 `PaddlePaddle/PP-OCRv6_small_det_onnx` + `PaddlePaddle/PP-OCRv6_small_rec_onnx` 作为首个内置 ONNX profile；PaddleX Paddle2ONNX 作为后备导出流程。
- 将 `Ppocrv6OfficialBackend` 落到可发布的 ONNX Runtime 实现。
- 对齐官方 pipeline：
  - 检测预处理
  - DB 后处理
  - box unclip
  - 裁剪旋转
  - 识别 resize / normalize
  - CTC 或模型对应解码
  - 行排序与置信度过滤
- 只有 benchmark 通过后，将 `defaultModelProfile` 切到 `ppocr-v6-small`。

## 插件文件改造点

- `src/main.rs`
  - 拆分协议、模型注册、后端实现。
  - 新增 `OcrBackend` trait。
  - 新增 `OnnxRuntimeBackend`：负责 ORT 动态库加载、det / rec session、PP-OCRv6 pipeline。
  - `EngineHolder` 缓存键改为 backend + profile，而不是只按 profile。
  - 保留现有 `recognizeBatch` 协议。
- `Cargo.toml`
  - 新增 `ort` 以及图像几何 / YAML 解析所需依赖。
  - 首期使用 CPU ONNX Runtime；GPU / DirectML provider 后续单独评估。
- `manifest.json`
  - 新增 `ppocr-v6-small` / `ppocr-v6-medium` profile。
  - profile 元数据补充 backend、model source、是否 experimental。
  - 默认 profile 暂不切换，直到 benchmark 通过。
- `build.js`
  - 从 `models/registry.json` 读取模型清单。
  - 校验不同 backend 的不同文件格式，不再只校验 `.mnn`：ONNX profile 至少校验 `inference.onnx`、`inference.yml` / `inference.json`、字符表和 ORT 动态库。
  - 打包时复制 `onnxruntime.dll` / provider DLL 到 sidecar 可加载位置。
- `MODEL_THIRD_PARTY_NOTICES.md`
  - 明确当前 v5/MNN 为 legacy。
  - 新增 PaddlePaddle 官方 ONNX 模型来源、版本、revision、下载 URL 和 hash。
  - 新增 ONNX Runtime 运行时来源、版本、license 和 hash。
- `models/`
  - 新增 `models/ppocr-v6-small/README.md`。
  - 新增 `models/registry.json`。
  - 不提交真实模型文件。
- `PaddleOcr.vue`
  - 展示 backend、模型来源、模型 hash 和 benchmark 状态。
  - 管理页 smoke test 改为可选择真实测试图，而不是 1x1 空图。

## 风险与决策点

- Python 方案可能太重，只适合作为质量基线，不一定适合作为最终发布形态。
- ONNX 方案如果后处理实现不完整，质量可能再次低于官方结果。
- ONNX Runtime 本身好接，难点在 PaddleOCR pipeline 对齐；DBPostProcess、unclip、透视裁剪、CTC 字典任一处偏差都可能造成静默劣化。
- ORT 动态库加载必须使用插件私有路径，不要依赖系统 PATH；Windows 下尤其要避免加载到系统或其他软件携带的旧版本 `onnxruntime.dll`。
- **macOS 签名与公证风险**：macOS 下动态加载的 `libonnxruntime.dylib` 必须在 CI/CD 打包阶段进行正确的 `codesign` 签名，否则会被 Gatekeeper 拦截导致 sidecar 闪退。
- **CI/CD 跨平台依赖管理**：多平台构建时，CI/CD 脚本需要根据 Runner 平台拉取对应架构（如 Windows x64, macOS x64/arm64）的 ORT 动态库，脚本编写和版本对齐需要一次性调通。
- **⚠️ 主应用版本依赖风险**：本插件的 PP-OCRv6 官方模型迁移（特别是模型外置与追加机制）**强依赖主应用升级发布**。主应用必须先实现并发布支持 `AIOHUB_PLUGIN_DATA_DIR` 环境变量的版本（参见 [`docs/Plan/sidecar-plugin-data-directory-plan.md`](../../../../docs/Plan/sidecar-plugin-data-directory-plan.md)），本插件的迁移分支才能合并至主线，否则会导致用户自定义模型在插件升级时丢失。
- PP-OCRv6 medium 可能质量更好，但包体积和加载成本更高；默认应先评估 small。
- 当前 Smart OCR 的切图和缩放会影响所有 OCR 后端，benchmark 必须使用 Smart OCR 实际传入插件的图片块，而不是只测原图。

## 验收门槛

迁移默认模型前必须满足：

- 真实 benchmark 中，PP-OCRv6 small 的中文 UI、中英混排、小字号样本整体优于当前 PP-OCRv5/MNN general。
- 错误输出仍保持单图失败不影响整批。
- 无模型或模型损坏时错误能指出具体 profile、backend 和文件。
- Windows x64 发布包可离线运行。
- `bun run build:rust` 和 `bun run build:vue` 通过。
- 发布包模型来源、hash 和许可记录完整。
