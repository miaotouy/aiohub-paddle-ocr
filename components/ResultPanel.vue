<template>
  <aside class="result-panel glass-panel">
    <div class="panel-heading">
      <div>
        <h3>识别结果</h3>
        <p>{{ resultSummary }}</p>
      </div>
      <el-button size="small" plain :disabled="!hasRawJson" @click="toggleRawJson">
        {{ rawPanelNames.length > 0 ? "隐藏 JSON" : "原始 JSON" }}
      </el-button>
    </div>

    <div class="toolbar">
      <el-button size="small" :disabled="!allText" @click="copyAllText">复制全部</el-button>
      <el-button size="small" plain :disabled="!allText" @click="$emit('send-to-chat', allText)">
        发送到聊天
      </el-button>
    </div>

    <div v-if="errorMessage" class="error-state">
      <strong>调用失败</strong>
      <p>{{ errorMessage }}</p>
    </div>

    <div v-else-if="isProcessing" class="empty-state">
      <strong>正在识别</strong>
      <p>结果会在完成后显示在这里。</p>
    </div>

    <div v-else-if="resultGroups.length === 0" class="empty-state">
      <strong>暂无结果</strong>
      <p>选择图片并开始识别后，这里会显示格式化文本。</p>
    </div>

    <div v-else class="result-list">
      <section
        v-for="group in resultGroups"
        :key="group.result.imageId"
        class="result-group"
      >
        <div class="group-header">
          <div>
            <strong>{{ selectedImageName || group.result.imageId }}</strong>
            <span>{{ group.result.status === "success" ? "识别成功" : "识别异常" }}</span>
          </div>
          <el-tag
            size="small"
            :type="group.result.status === 'success' ? 'success' : 'danger'"
          >
            {{ group.lines.length }} 行
          </el-tag>
        </div>

        <p v-if="group.result.status === 'error'" class="group-error">
          {{ group.result.error || "该图片识别失败" }}
        </p>

        <div
          v-for="line in group.lines"
          :key="line.key"
          class="line-item"
          @dblclick="startEdit(line.key, line.text)"
        >
          <div class="line-meta">
            <span>{{ line.label }}</span>
            <span v-if="line.score !== null">{{ formatConfidence(line.score) }}</span>
          </div>

          <div v-if="editingKey === line.key" class="line-editor">
            <el-input
              v-model="editingValue"
              type="textarea"
              :autosize="{ minRows: 2, maxRows: 4 }"
              @keydown.ctrl.enter.prevent="saveEdit"
            />
            <div class="line-actions">
              <el-button size="small" type="primary" @click="saveEdit">保存</el-button>
              <el-button size="small" plain @click="cancelEdit">取消</el-button>
            </div>
          </div>

          <template v-else>
            <p class="line-text">{{ getLineText(line.key, line.text) }}</p>
            <div class="line-actions">
              <el-button size="small" text @click="copyText(getLineText(line.key, line.text))">
                复制
              </el-button>
              <el-button size="small" text @click="startEdit(line.key, line.text)">
                编辑
              </el-button>
            </div>
          </template>
        </div>
      </section>
    </div>

    <el-collapse v-if="hasRawJson" v-model="rawPanelNames" class="raw-collapse">
      <el-collapse-item title="原始 JSON" name="raw">
        <pre class="raw-json">{{ rawJson }}</pre>
      </el-collapse-item>
    </el-collapse>
  </aside>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { customMessage } from "aiohub-sdk";
import type {
  OcrLine,
  PaddleOcrBatchResult,
  PaddleOcrImageResult,
} from "./types";

interface DisplayLine {
  key: string;
  label: string;
  text: string;
  score: number | null;
}

const props = defineProps<{
  result: PaddleOcrBatchResult | null;
  isProcessing: boolean;
  selectedImageName: string | null;
  errorMessage?: string | null;
}>();

defineEmits<{
  (event: "send-to-chat", text: string): void;
}>();

const editedTexts = reactive<Record<string, string>>({});
const editingKey = ref<string | null>(null);
const editingValue = ref("");
const rawPanelNames = ref<string[]>([]);

const resultGroups = computed(() =>
  (props.result?.results || []).map((result) => ({
    result,
    lines: buildDisplayLines(result),
  }))
);

const resultSummary = computed(() => {
  if (props.errorMessage) return "错误信息已捕获";
  const resultCount = props.result?.results?.length || 0;
  const lineCount = resultGroups.value.reduce((sum, group) => sum + group.lines.length, 0);
  if (resultCount === 0) return "等待识别";
  return `${resultCount} 张图片 · ${lineCount} 行文本`;
});

const rawJson = computed(() =>
  props.result ? JSON.stringify(props.result, null, 2) : props.errorMessage || ""
);

const hasRawJson = computed(() => Boolean(rawJson.value));

const allText = computed(() =>
  resultGroups.value
    .map((group) =>
      group.lines
        .map((line) => getLineText(line.key, line.text).trim())
        .filter(Boolean)
        .join("\n")
    )
    .filter(Boolean)
    .join("\n\n")
);

watch(
  () => props.result,
  () => {
    for (const key of Object.keys(editedTexts)) {
      delete editedTexts[key];
    }
    editingKey.value = null;
    editingValue.value = "";
    rawPanelNames.value = [];
  }
);

function buildDisplayLines(result: PaddleOcrImageResult): DisplayLine[] {
  if (result.lines && result.lines.length > 0) {
    return result.lines.map((line, index) => normalizeOcrLine(result.imageId, line, index));
  }

  const fallbackLines = result.text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);

  if (fallbackLines.length === 0 && result.status === "error") {
    return [];
  }

  return (fallbackLines.length > 0 ? fallbackLines : [result.text]).map((text, index) => ({
    key: `${result.imageId}-${index}`,
    label: `行 ${index + 1}`,
    text,
    score: result.confidence ?? null,
  }));
}

function normalizeOcrLine(imageId: string, line: OcrLine, index: number): DisplayLine {
  return {
    key: `${imageId}-${index}`,
    label: `行 ${index + 1}`,
    text: line.text,
    score: line.score ?? null,
  };
}

function getLineText(key: string, originalText: string): string {
  return editedTexts[key] !== undefined ? editedTexts[key] : originalText;
}

function startEdit(key: string, originalText: string) {
  editingKey.value = key;
  editingValue.value = getLineText(key, originalText);
}

function saveEdit() {
  if (editingKey.value) {
    editedTexts[editingKey.value] = editingValue.value;
    editingKey.value = null;
    editingValue.value = "";
  }
}

function cancelEdit() {
  editingKey.value = null;
  editingValue.value = "";
}

function formatConfidence(score: number) {
  return `${Math.round(score * 100)}%`;
}

async function copyText(text: string) {
  if (!text.trim()) {
    customMessage.warning("没有可复制的文本");
    return;
  }

  try {
    await navigator.clipboard.writeText(text);
    customMessage.success("已复制");
  } catch {
    customMessage.error("复制失败");
  }
}

async function copyAllText() {
  await copyText(allText.value);
}

function toggleRawJson() {
  rawPanelNames.value = rawPanelNames.value.length > 0 ? [] : ["raw"];
}
</script>

<style scoped>
.result-panel {
  width: 400px;
  min-width: 360px;
  gap: 12px;
}

.panel-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  flex-shrink: 0;
}

.panel-heading h3 {
  margin: 0;
  color: var(--text-color);
  font-size: 16px;
  font-weight: 650;
  line-height: 1.3;
}

.panel-heading p {
  margin: 5px 0 0;
  color: var(--text-color-secondary);
  font-size: 12px;
}

.toolbar {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  flex-shrink: 0;
}

.result-list {
  min-height: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow: auto;
  padding-right: 2px;
}

.result-group {
  border: var(--border-width, 1px) solid var(--border-color);
  border-radius: 8px;
  overflow: hidden;
  background: color-mix(in srgb, var(--input-bg, transparent) 78%, transparent);
}

.group-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px;
  border-bottom: var(--border-width, 1px) solid var(--border-color);
}

.group-header strong {
  display: block;
  color: var(--text-color);
  font-size: 13px;
  line-height: 1.4;
  overflow-wrap: anywhere;
}

.group-header span,
.line-meta {
  color: var(--text-color-secondary);
  font-size: 12px;
}

.group-error {
  margin: 0;
  padding: 12px;
  color: var(--el-color-danger);
  font-size: 13px;
  line-height: 1.5;
}

.line-item {
  padding: 12px;
  border-top: var(--border-width, 1px) solid color-mix(in srgb, var(--border-color) 70%, transparent);
}

.line-item:first-of-type {
  border-top: 0;
}

.line-meta {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 8px;
}

.line-text {
  margin: 0;
  color: var(--text-color);
  font-size: 14px;
  line-height: 1.55;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.line-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 8px;
}

.line-editor {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.empty-state,
.error-state {
  display: flex;
  min-height: 180px;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 18px;
  border: var(--border-width, 1px) dashed var(--border-color);
  border-radius: 8px;
  text-align: center;
  background: color-mix(in srgb, var(--input-bg, transparent) 76%, transparent);
}

.empty-state strong,
.error-state strong {
  color: var(--text-color);
  font-size: 15px;
}

.empty-state p,
.error-state p {
  max-width: 280px;
  margin: 0;
  color: var(--text-color-secondary);
  font-size: 13px;
  line-height: 1.5;
}

.error-state p {
  color: var(--el-color-danger);
  overflow-wrap: anywhere;
}

.raw-collapse {
  flex-shrink: 0;
  border-top: var(--border-width, 1px) solid var(--border-color);
  border-bottom: 0;
}

.raw-collapse :deep(.el-collapse-item__header),
.raw-collapse :deep(.el-collapse-item__wrap) {
  color: var(--text-color);
  background: transparent;
  border-bottom-color: var(--border-color);
}

.raw-collapse :deep(.el-collapse-item__content) {
  padding-bottom: 0;
}

.raw-json {
  max-height: 220px;
  margin: 0;
  padding: 12px;
  border: var(--border-width, 1px) solid var(--border-color);
  border-radius: 8px;
  color: var(--text-color);
  background: color-mix(in srgb, var(--input-bg, transparent) 88%, transparent);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  overflow: auto;
}
</style>
