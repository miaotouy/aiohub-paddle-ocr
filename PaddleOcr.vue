<template>
  <div class="paddle-ocr">
    <input
      ref="imageInput"
      class="file-input"
      type="file"
      accept="image/*"
      @change="handleLocalFileInput"
    />

    <ImportModelDialog
      v-model:visible="isImportDialogVisible"
      :importing="isImportingModel"
      @imported="importCustomModel"
    />

    <ControlPanel
      v-model:selected-profile-id="selectedProfileId"
      :runtime-status="runtimeStatus"
      :model-status="modelStatus"
      :is-processing="isProcessing"
      :is-checking="isChecking"
      :registry-loading="registryLoading"
      :registry-error="registryError"
      :last-duration-ms="lastDurationMs"
      :model-profiles="modelProfiles"
      :selected-image-name="selectedImage?.name || null"
      :version="version"
      @select-image="selectImage"
      @check-runtime="checkRuntime"
      @run-ocr="runOcr"
      @import-model="isImportDialogVisible = true"
    />

    <PreviewPanel
      :selected-image="selectedImage"
      :is-processing="isProcessing"
      :ocr-lines="previewLines"
      @image-selected="handleImageFile"
      @image-path-selected="handleImagePath"
      @image-cleared="clearImage"
    />

    <ResultPanel
      :result="lastResult"
      :is-processing="isProcessing"
      :selected-image-name="selectedImage?.name || null"
      :error-message="lastError"
      @send-to-chat="handleSendToChat"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { customMessage, execute, pluginManager, useSendToChat } from "aiohub-sdk";
import manifest from "./manifest.json";
import embeddedRegistryJson from "./models/registry.json";
import ControlPanel from "./components/ControlPanel.vue";
import ImportModelDialog from "./components/ImportModelDialog.vue";
import PreviewPanel from "./components/PreviewPanel.vue";
import ResultPanel from "./components/ResultPanel.vue";
import type {
  ImportModelForm,
  ModelRegistry,
  ModelRegistryProfile,
  ModelStatus,
  PaddleOcrBatchResult,
  RuntimeStatus,
  SelectedOcrImage,
} from "./components/types";

const PLUGIN_ID = "paddle-ocr";
const CUSTOM_REGISTRY_FILE = "custom-registry.json";

const pluginContext = pluginManager.createPluginContext(PLUGIN_ID);
const version = manifest.version;
const manifestDefaultProfile = getManifestDefaultProfile();
const embeddedRegistry = embeddedRegistryJson as ModelRegistry;

const imageInput = ref<HTMLInputElement | null>(null);
const modelProfiles = ref<ModelRegistryProfile[]>(getManifestProfiles());
const selectedProfileId = ref(manifestDefaultProfile);
const registryLoading = ref(false);
const registryError = ref<string | null>(null);

const selectedImage = ref<SelectedOcrImage | null>(null);
const runtimeStatus = ref<RuntimeStatus>("idle");
const modelStatus = ref<ModelStatus>("unknown");
const lastDurationMs = ref<number | null>(null);
const lastResult = ref<PaddleOcrBatchResult | null>(null);
const lastError = ref<string | null>(null);

const isChecking = ref(false);
const isProcessing = ref(false);
const isImportDialogVisible = ref(false);
const isImportingModel = ref(false);
const { sendToChat } = useSendToChat();

const previewLines = computed(() => {
  const imageId = selectedImage.value?.id;
  if (!imageId) return [];
  return (
    lastResult.value?.results?.find((result) => result.imageId === imageId)?.lines || []
  );
});

onMounted(async () => {
  await loadRegistry();
});

async function loadRegistry() {
  registryLoading.value = true;
  registryError.value = null;

  let builtInProfiles = embeddedRegistry.profiles || getManifestProfiles();
  let defaultProfile = embeddedRegistry.defaultProfile || manifestDefaultProfile;
  const installRegistry = await loadInstallRegistry();
  if (installRegistry) {
    builtInProfiles = installRegistry.profiles || builtInProfiles;
    defaultProfile = installRegistry.defaultProfile || defaultProfile;
  }

  let customProfiles: ModelRegistryProfile[] = [];
  try {
    if (await pluginContext.storage.exists(CUSTOM_REGISTRY_FILE)) {
      const content = await pluginContext.storage.readText(CUSTOM_REGISTRY_FILE);
      const customRegistry = JSON.parse(content) as ModelRegistry;
      customProfiles = customRegistry.profiles || [];
    }
  } catch (error) {
    const message = `自定义模型清单读取失败：${toErrorMessage(error)}`;
    registryError.value = registryError.value ? `${registryError.value}；${message}` : message;
  } finally {
    registryLoading.value = false;
  }

  const mergedProfiles = mergeProfiles(builtInProfiles, customProfiles);
  modelProfiles.value = mergedProfiles.length > 0 ? mergedProfiles : getManifestProfiles();

  const hasSelected = modelProfiles.value.some(
    (profile) => profile.id === selectedProfileId.value
  );
  if (!hasSelected) {
    selectedProfileId.value =
      modelProfiles.value.find((profile) => profile.id === defaultProfile)?.id ||
      modelProfiles.value[0]?.id ||
      defaultProfile;
  }
}

async function loadInstallRegistry() {
  try {
    const plugin =
      pluginManager.getPlugin(`${PLUGIN_ID}-dev`) || pluginManager.getPlugin(PLUGIN_ID);
    const installPath = plugin?.installPath;
    if (!installPath) return null;

    const { join } = await import("@tauri-apps/api/path");
    const fs = await import("@tauri-apps/plugin-fs");
    const registryPath = await join(installPath, "models", "registry.json");

    if (!(await fs.exists(registryPath))) {
      return null;
    }

    const content = await fs.readTextFile(registryPath);
    try {
      return JSON.parse(content) as ModelRegistry;
    } catch (error) {
      registryError.value = `内置模型清单解析失败，已使用内嵌清单：${toErrorMessage(error)}`;
      return null;
    }
  } catch {
    return null;
  }
}

function mergeProfiles(
  builtInProfiles: ModelRegistryProfile[],
  customProfiles: ModelRegistryProfile[]
) {
  const merged: ModelRegistryProfile[] = [];
  const seenIds = new Set<string>();

  for (const profile of [...builtInProfiles, ...customProfiles]) {
    const id = profile.id?.trim();
    if (!id) continue;
    const key = id.toLowerCase();
    if (seenIds.has(key)) continue;
    seenIds.add(key);
    merged.push(profile);
  }

  return merged;
}

async function importCustomModel(formData: ImportModelForm) {
  isImportingModel.value = true;
  try {
    const { join } = await import("@tauri-apps/api/path");
    const fs = await import("@tauri-apps/plugin-fs");
    const storage = pluginContext.storage;
    const modelId = createCustomModelId(formData.modelName);
    const targetModelDir = `custom-models/${modelId}`;
    const dataDir = await storage.getDataDir();
    const absoluteModelDir = await join(dataDir, "custom-models", modelId);

    await fs.mkdir(absoluteModelDir, { recursive: true });

    const normalizedFiles = getModelFilesToCopy(formData);
    for (const relativeFile of normalizedFiles) {
      const sourcePath = await join(formData.modelDir, ...relativeFile.split("/"));
      const targetDirParts = relativeFile.split("/").slice(0, -1);
      if (targetDirParts.length > 0) {
        const targetSubDir = await join(absoluteModelDir, ...targetDirParts);
        await fs.mkdir(targetSubDir, { recursive: true });
      }
      const data = await fs.readFile(sourcePath);
      await storage.writeBinary(`${targetModelDir}/${relativeFile}`, data);
    }

    const customProfile = buildCustomProfile(
      modelId,
      formData,
      absoluteModelDir.replace(/\\/g, "/")
    );
    await appendCustomProfile(customProfile);
    await loadRegistry();

    selectedProfileId.value = customProfile.id;
    isImportDialogVisible.value = false;
    customMessage.success(`自定义模型 "${customProfile.name}" 导入成功`);
  } catch (error) {
    customMessage.error(`导入自定义模型失败：${toErrorMessage(error)}`);
  } finally {
    isImportingModel.value = false;
  }
}

function getModelFilesToCopy(formData: ImportModelForm) {
  const files =
    formData.backend === "mnn-ocr-rs"
      ? [formData.detModel, formData.recModel, formData.dict]
      : [
          formData.detOnnx,
          formData.recOnnx,
          formData.detConfig,
          formData.recConfig,
          formData.dict,
        ];

  return Array.from(
    new Set(
      files
        .filter((file): file is string => Boolean(file))
        .map((file) => normalizeRelativePath(file))
    )
  );
}

function buildCustomProfile(
  id: string,
  formData: ImportModelForm,
  absoluteModelDir: string
): ModelRegistryProfile {
  const profile: ModelRegistryProfile = {
    id,
    name: formData.modelName.trim(),
    backend: formData.backend,
    language: formData.language.trim() || "custom",
    modelDir: absoluteModelDir,
    aliases: [id],
    builtIn: false,
    package: false,
    experimental: false,
  };

  if (formData.backend === "mnn-ocr-rs") {
    profile.detModel = normalizeRelativePath(formData.detModel);
    profile.recModel = normalizeRelativePath(formData.recModel);
    profile.dict = normalizeRelativePath(formData.dict);
  } else {
    profile.detOnnx = normalizeRelativePath(formData.detOnnx);
    profile.recOnnx = normalizeRelativePath(formData.recOnnx);
    profile.detConfig = normalizeRelativePath(formData.detConfig);
    profile.recConfig = normalizeRelativePath(formData.recConfig);
    if (formData.dict) {
      profile.dict = normalizeRelativePath(formData.dict);
    }
  }

  return profile;
}

async function appendCustomProfile(profile: ModelRegistryProfile) {
  const storage = pluginContext.storage;
  let registry: ModelRegistry = {
    schemaVersion: 1,
    defaultProfile: "",
    profiles: [],
  };

  if (await storage.exists(CUSTOM_REGISTRY_FILE)) {
    try {
      registry = JSON.parse(await storage.readText(CUSTOM_REGISTRY_FILE)) as ModelRegistry;
    } catch {
      registry = {
        schemaVersion: 1,
        defaultProfile: "",
        profiles: [],
      };
    }
  }

  registry.schemaVersion = 1;
  registry.defaultProfile = registry.defaultProfile || "";
  registry.profiles = (registry.profiles || []).filter(
    (item) => item.id.toLowerCase() !== profile.id.toLowerCase()
  );
  registry.profiles.push(profile);

  await storage.writeText(CUSTOM_REGISTRY_FILE, `${JSON.stringify(registry, null, 2)}\n`);
}

function normalizeRelativePath(filePath?: string) {
  const normalized = (filePath || "").replace(/\\/g, "/").replace(/^\/+/, "").trim();
  if (!normalized) {
    throw new Error("模型文件路径为空");
  }
  if (normalized.split("/").some((segment) => segment === ".." || segment === "")) {
    throw new Error(`模型文件路径无效: ${filePath}`);
  }
  return normalized;
}

function createCustomModelId(modelName: string) {
  const slug =
    modelName
      .trim()
      .toLowerCase()
      .normalize("NFKD")
      .replace(/[^a-z0-9_-]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 48) || "model";
  return `custom-${Date.now()}-${slug}`;
}

function selectImage() {
  imageInput.value?.click();
}

function handleLocalFileInput(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (file) {
    handleImageFile(file);
  }
  input.value = "";
}

async function handleImageFile(file: File) {
  if (!file.type.startsWith("image/")) {
    customMessage.warning("请选择图片文件");
    return;
  }

  try {
    selectedImage.value = {
      id: `image-${Date.now()}`,
      name: file.name,
      dataUrl: await readBlobAsDataUrl(file),
    };
    lastResult.value = null;
    lastError.value = null;
    customMessage.success("图片已载入");
  } catch (error) {
    customMessage.error(toErrorMessage(error));
  }
}

async function handleImagePath(path: string) {
  try {
    selectedImage.value = {
      id: `image-${Date.now()}`,
      name: getFileNameFromPath(path),
      path,
      dataUrl: await readPathAsDataUrl(path),
    };
    lastResult.value = null;
    lastError.value = null;
    customMessage.success("图片已载入");
  } catch (error) {
    customMessage.error(`读取图片失败：${toErrorMessage(error)}`);
  }
}

function clearImage() {
  selectedImage.value = null;
  lastResult.value = null;
  lastError.value = null;
}

async function callRecognizeBatch(images: unknown[]) {
  const startedAt = performance.now();
  try {
    const result = (await execute({
      service: PLUGIN_ID,
      method: "recognizeBatch",
      params: {
        images,
        options: {
          modelProfile: selectedProfileId.value,
        },
      },
    })) as PaddleOcrBatchResult;

    lastResult.value = result;
    lastError.value = null;
    return result;
  } finally {
    lastDurationMs.value = performance.now() - startedAt;
  }
}

function updateStatusFromError(error: unknown) {
  runtimeStatus.value = "error";
  const message = toErrorMessage(error);
  lastResult.value = null;
  lastError.value = message;

  if (
    message.includes("模型目录缺失") ||
    message.includes("模型文件缺失") ||
    message.includes("模型文件为空") ||
    message.includes("模型 registry 无效") ||
    message.includes("不支持的模型 profile") ||
    message.includes("ONNX Runtime 动态库缺失")
  ) {
    modelStatus.value = "missing";
  }
}

async function checkRuntime() {
  isChecking.value = true;
  try {
    await callRecognizeBatch([]);
    runtimeStatus.value = "ready";
    modelStatus.value = "ready";
    customMessage.success("Paddle OCR 后端可调用");
  } catch (error) {
    updateStatusFromError(error);
    customMessage.error("Paddle OCR 检查失败");
  } finally {
    isChecking.value = false;
  }
}

async function runOcr() {
  if (!selectedImage.value) {
    customMessage.warning("请先选择一张图片");
    return;
  }

  isProcessing.value = true;
  try {
    const image = selectedImage.value;
    const requestImage = image.path
      ? {
          blockId: `block-${image.id}`,
          imageId: image.id,
          path: image.path,
          dataUrl: image.dataUrl,
        }
      : {
          blockId: `block-${image.id}`,
          imageId: image.id,
          dataUrl: image.dataUrl,
        };
    const result = await callRecognizeBatch([
      requestImage,
    ]);
    runtimeStatus.value = "ready";
    modelStatus.value = "ready";

    const first = result.results?.[0];
    if (first?.status === "error") {
      customMessage.warning(first.error || "图片未完成识别");
    } else {
      customMessage.success("识别完成");
    }
  } catch (error) {
    updateStatusFromError(error);
    customMessage.error("识别调用失败");
  } finally {
    isProcessing.value = false;
  }
}

async function handleSendToChat(text: string) {
  if (!text.trim()) {
    customMessage.warning("没有可发送的文本");
    return;
  }

  const sent = sendToChat(text, {
    format: "plain",
    position: "append",
    successMessage: "已发送到聊天输入框",
  });
  if (sent) return;

  try {
    await navigator.clipboard.writeText(text);
    customMessage.info("当前插件 SDK 暂未暴露发送到聊天接口，已复制文本");
  } catch {
    customMessage.error("发送到聊天失败，复制也未成功");
  }
}

async function readPathAsDataUrl(path: string) {
  const fs = await import("@tauri-apps/plugin-fs");
  const data = await fs.readFile(path);
  const blob = new Blob([data], { type: inferImageMimeType(path) });
  return readBlobAsDataUrl(blob);
}

function readBlobAsDataUrl(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === "string") {
        resolve(reader.result);
      } else {
        reject(new Error("图片读取结果无效"));
      }
    };
    reader.onerror = () => reject(reader.error || new Error("图片读取失败"));
    reader.readAsDataURL(blob);
  });
}

function getFileNameFromPath(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() || "本地图片";
}

function inferImageMimeType(path: string) {
  const lower = path.toLowerCase();
  if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "image/jpeg";
  if (lower.endsWith(".webp")) return "image/webp";
  if (lower.endsWith(".gif")) return "image/gif";
  if (lower.endsWith(".bmp")) return "image/bmp";
  return "image/png";
}

function getManifestProfiles() {
  const contribution = getOcrContribution();
  return ((contribution?.modelProfiles || embeddedRegistry.profiles || []) as ModelRegistryProfile[]).map((profile) => ({
    ...profile,
  }));
}

function getManifestDefaultProfile() {
  const contribution = getOcrContribution();
  return contribution?.defaultModelProfile || "ppocr-v5-mobile-general";
}

function getOcrContribution() {
  return (manifest.contributions || []).find(
    (contribution: { type?: string }) => contribution.type === "ocr-engine"
  ) as
    | {
        defaultModelProfile?: string;
        modelProfiles?: ModelRegistryProfile[];
      }
    | undefined;
}

function toErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
</script>

<style scoped>
.paddle-ocr {
  box-sizing: border-box;
  display: flex;
  gap: 16px;
  height: 100%;
  min-height: 0;
  padding: 16px;
  color: var(--text-color);
  background: transparent;
  overflow: hidden;
}

.paddle-ocr * {
  box-sizing: border-box;
}

.file-input {
  display: none;
}

:deep(.glass-panel) {
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 16px;
  border: var(--border-width, 1px) solid var(--border-color);
  border-radius: 8px;
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--card-bg, white) 94%, transparent), var(--card-bg, white)),
    var(--card-bg);
  backdrop-filter: blur(var(--ui-blur, 12px));
  -webkit-backdrop-filter: blur(var(--ui-blur, 12px));
  overflow: hidden;
}

@supports not (backdrop-filter: blur(1px)) {
  :deep(.glass-panel) {
    background: var(--card-bg);
  }
}

@media (max-width: 1180px) {
  .paddle-ocr {
    display: grid;
    grid-template-columns: 300px minmax(0, 1fr);
    grid-auto-rows: minmax(260px, auto);
    overflow: auto;
  }

  :deep(.result-panel) {
    grid-column: 1 / -1;
    width: 100%;
    min-height: 360px;
  }
}

@media (max-width: 860px) {
  .paddle-ocr {
    grid-template-columns: 1fr;
    padding: 12px;
  }

  :deep(.control-panel),
  :deep(.result-panel) {
    width: 100%;
    min-width: 0;
  }
}
</style>
