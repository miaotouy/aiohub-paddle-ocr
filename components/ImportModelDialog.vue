<template>
  <BaseDialog
    v-model="dialogVisible"
    title="导入自定义模型"
    width="760px"
    max-height="86vh"
    content-class="paddle-import-dialog-content"
  >
    <div class="import-dialog">
      <el-form label-position="top" class="import-form">
        <div class="form-grid">
          <el-form-item label="模型名称" required>
            <el-input v-model="form.modelName" placeholder="例如 PP-OCR 自定义中文模型" />
          </el-form-item>

          <el-form-item label="语言标识" required>
            <el-input v-model="form.language" placeholder="general / en / custom" />
          </el-form-item>
        </div>

        <el-form-item label="后端类型" required>
          <el-radio-group v-model="form.backend">
            <el-radio-button label="mnn-ocr-rs">MNN / ocr-rs</el-radio-button>
            <el-radio-button label="onnxruntime">ONNX Runtime</el-radio-button>
          </el-radio-group>
        </el-form-item>

        <el-form-item label="模型文件夹" required>
          <div class="folder-row">
            <el-input
              :model-value="form.modelDir"
              readonly
              placeholder="选择包含模型文件的本地文件夹"
            />
            <el-button :loading="isScanning" @click="selectFolder">选择文件夹</el-button>
          </div>
          <p class="field-hint">
            已扫描 {{ availableFiles.length }} 个文件。支持直接文件，也支持 det/rec 等浅层子目录。
          </p>
        </el-form-item>

        <div v-if="form.backend === 'mnn-ocr-rs'" class="form-grid">
          <el-form-item label="检测模型 detModel" required>
            <el-select v-model="form.detModel" filterable placeholder="选择 .mnn 检测模型">
              <el-option v-for="file in mnnFiles" :key="file" :label="file" :value="file" />
            </el-select>
          </el-form-item>
          <el-form-item label="识别模型 recModel" required>
            <el-select v-model="form.recModel" filterable placeholder="选择 .mnn 识别模型">
              <el-option v-for="file in mnnFiles" :key="file" :label="file" :value="file" />
            </el-select>
          </el-form-item>
          <el-form-item label="字典 dict" required>
            <el-select v-model="form.dict" filterable placeholder="选择字典 .txt">
              <el-option v-for="file in textFiles" :key="file" :label="file" :value="file" />
            </el-select>
          </el-form-item>
        </div>

        <div v-else class="form-grid">
          <el-form-item label="检测模型 detOnnx" required>
            <el-select v-model="form.detOnnx" filterable placeholder="选择检测 .onnx">
              <el-option v-for="file in onnxFiles" :key="file" :label="file" :value="file" />
            </el-select>
          </el-form-item>
          <el-form-item label="识别模型 recOnnx" required>
            <el-select v-model="form.recOnnx" filterable placeholder="选择识别 .onnx">
              <el-option v-for="file in onnxFiles" :key="file" :label="file" :value="file" />
            </el-select>
          </el-form-item>
          <el-form-item label="检测配置 detConfig" required>
            <el-select v-model="form.detConfig" filterable placeholder="选择检测配置">
              <el-option v-for="file in configFiles" :key="file" :label="file" :value="file" />
            </el-select>
          </el-form-item>
          <el-form-item label="识别配置 recConfig" required>
            <el-select v-model="form.recConfig" filterable placeholder="选择识别配置">
              <el-option v-for="file in configFiles" :key="file" :label="file" :value="file" />
            </el-select>
          </el-form-item>
          <el-form-item label="字典 dict">
            <el-select v-model="form.dict" clearable filterable placeholder="可选">
              <el-option v-for="file in textFiles" :key="file" :label="file" :value="file" />
            </el-select>
          </el-form-item>
        </div>

        <div v-if="scanError" class="scan-error">{{ scanError }}</div>
      </el-form>
    </div>

    <template #footer>
      <el-button @click="dialogVisible = false">取消</el-button>
      <el-button type="primary" :loading="importing" @click="submitImport">
        导入模型
      </el-button>
    </template>
  </BaseDialog>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { customMessage, openDialog } from "aiohub-sdk";
import { BaseDialog } from "aiohub-ui";
import type { ImportModelForm } from "./types";

const props = withDefaults(
  defineProps<{
    visible: boolean;
    importing?: boolean;
  }>(),
  {
    importing: false,
  }
);

const emit = defineEmits<{
  (event: "update:visible", visible: boolean): void;
  (event: "imported", form: ImportModelForm): void;
}>();

const dialogVisible = computed({
  get: () => props.visible,
  set: (value: boolean) => emit("update:visible", value),
});

const form = reactive<ImportModelForm>({
  modelName: "",
  backend: "mnn-ocr-rs",
  language: "general",
  modelDir: "",
  detModel: undefined,
  recModel: undefined,
  dict: undefined,
  detOnnx: undefined,
  recOnnx: undefined,
  detConfig: undefined,
  recConfig: undefined,
});

const availableFiles = ref<string[]>([]);
const isScanning = ref(false);
const scanError = ref("");

const mnnFiles = computed(() =>
  availableFiles.value.filter((file) => file.toLowerCase().endsWith(".mnn"))
);

const onnxFiles = computed(() =>
  availableFiles.value.filter((file) => file.toLowerCase().endsWith(".onnx"))
);

const configFiles = computed(() =>
  availableFiles.value.filter((file) => /\.(yml|yaml|json)$/i.test(file))
);

const textFiles = computed(() =>
  availableFiles.value.filter((file) => file.toLowerCase().endsWith(".txt"))
);

watch(
  () => form.backend,
  () => {
    clearDetectedFiles();
    autoMatchFiles();
  }
);

watch(
  () => props.visible,
  (visible) => {
    if (!visible) {
      scanError.value = "";
      isScanning.value = false;
    }
  }
);

async function selectFolder() {
  const selected = await openDialog({
    directory: true,
    multiple: false,
    title: "选择包含 Paddle OCR 模型的文件夹",
  });

  const selectedPath = Array.isArray(selected) ? selected[0] : selected;
  if (!selectedPath || typeof selectedPath !== "string") return;

  form.modelDir = selectedPath;
  if (!form.modelName.trim()) {
    form.modelName = deriveNameFromPath(selectedPath);
  }

  await scanSelectedDirectory();
}

async function scanSelectedDirectory() {
  if (!form.modelDir) return;

  isScanning.value = true;
  scanError.value = "";
  try {
    availableFiles.value = (await scanFiles(form.modelDir)).sort((a, b) =>
      a.localeCompare(b)
    );
    clearDetectedFiles();
    autoMatchFiles();
    if (availableFiles.value.length === 0) {
      customMessage.warning("没有在该文件夹中发现可导入的模型文件");
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    scanError.value = message;
    customMessage.error("扫描模型文件夹失败");
  } finally {
    isScanning.value = false;
  }
}

async function scanFiles(rootPath: string, currentPath = rootPath, prefix = "", depth = 0) {
  const { join } = await import("@tauri-apps/api/path");
  const { readDir } = await import("@tauri-apps/plugin-fs");
  const entries = await readDir(currentPath);
  const files: string[] = [];

  for (const entry of entries) {
    const name = entry.name;
    const relativePath = prefix ? `${prefix}/${name}` : name;
    if (entry.isDirectory && depth < 3) {
      const childPath = await join(currentPath, name);
      files.push(...(await scanFiles(rootPath, childPath, relativePath, depth + 1)));
    } else if (!entry.isDirectory) {
      files.push(relativePath.replace(/\\/g, "/"));
    }
  }

  return files;
}

function clearDetectedFiles() {
  form.detModel = undefined;
  form.recModel = undefined;
  form.detOnnx = undefined;
  form.recOnnx = undefined;
  form.detConfig = undefined;
  form.recConfig = undefined;
  form.dict = undefined;
}

function autoMatchFiles() {
  const files = availableFiles.value;
  if (form.backend === "mnn-ocr-rs") {
    form.detModel = pickFile(files, [".mnn"], ["det"]);
    form.recModel = pickFile(files, [".mnn"], ["rec"]);
    form.dict = pickFile(files, [".txt"], ["dict", "keys", "char"]);
    return;
  }

  form.detOnnx = pickFile(files, [".onnx"], ["det"]);
  form.recOnnx = pickFile(files, [".onnx"], ["rec"]);
  form.detConfig = pickFile(files, [".yml", ".yaml", ".json"], ["det"]);
  form.recConfig = pickFile(files, [".yml", ".yaml", ".json"], ["rec"]);
  form.dict = pickFile(files, [".txt"], ["dict", "keys", "char"]);
}

function pickFile(files: string[], extensions: string[], markers: string[]) {
  const normalizedExtensions = extensions.map((item) => item.toLowerCase());
  const candidates = files.filter((file) =>
    normalizedExtensions.some((extension) => file.toLowerCase().endsWith(extension))
  );

  const withMarker = candidates.find((file) => {
    const lower = file.toLowerCase();
    return markers.some((marker) => lower.includes(marker));
  });

  return withMarker || candidates[0];
}

function deriveNameFromPath(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() || "自定义 OCR 模型";
}

function submitImport() {
  const validationError = validateForm();
  if (validationError) {
    customMessage.warning(validationError);
    return;
  }

  emit("imported", {
    modelName: form.modelName.trim(),
    backend: form.backend,
    language: form.language.trim() || "custom",
    modelDir: form.modelDir,
    detModel: form.detModel,
    recModel: form.recModel,
    dict: form.dict,
    detOnnx: form.detOnnx,
    recOnnx: form.recOnnx,
    detConfig: form.detConfig,
    recConfig: form.recConfig,
  });
}

function validateForm() {
  if (!form.modelName.trim()) return "请填写模型名称";
  if (!form.language.trim()) return "请填写语言标识";
  if (!form.modelDir) return "请选择模型文件夹";

  if (form.backend === "mnn-ocr-rs") {
    if (!form.detModel || !form.recModel || !form.dict) {
      return "MNN 模型需要检测模型、识别模型和字典文件";
    }
    return "";
  }

  if (!form.detOnnx || !form.recOnnx || !form.detConfig || !form.recConfig) {
    return "ONNX 模型需要检测模型、识别模型和两份配置文件";
  }

  return "";
}
</script>

<style scoped>
.import-dialog {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.import-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.form-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.folder-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 10px;
  width: 100%;
}

.field-hint {
  margin: 8px 0 0;
  color: var(--text-color-secondary);
  font-size: 12px;
  line-height: 1.4;
}

.scan-error {
  padding: 10px 12px;
  border: var(--border-width, 1px) solid color-mix(in srgb, var(--el-color-danger) 50%, var(--border-color));
  border-radius: 8px;
  color: var(--el-color-danger);
  background: color-mix(in srgb, var(--el-color-danger) 8%, transparent);
  font-size: 13px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}

:deep(.paddle-import-dialog-content) {
  overflow-x: hidden;
}

@media (max-width: 720px) {
  .form-grid,
  .folder-row {
    grid-template-columns: 1fr;
  }
}
</style>
