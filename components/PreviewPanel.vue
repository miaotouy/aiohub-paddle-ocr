<template>
  <section
    ref="dropZoneRef"
    class="preview-panel glass-panel"
    :class="{ dragging: isDraggingOver }"
    tabindex="0"
  >
    <input
      ref="fileInput"
      class="file-input"
      type="file"
      accept="image/*"
      @change="handleFileInput"
    />

    <div class="preview-toolbar">
      <div>
        <h3>图片预览</h3>
        <p>{{ selectedImage?.name || "拖拽、粘贴或选择一张图片" }}</p>
      </div>
      <div class="toolbar-actions">
        <el-button size="small" @click="openFileDialog">选择</el-button>
        <el-button
          size="small"
          plain
          :disabled="!selectedImage || isProcessing"
          @click="$emit('image-cleared')"
        >
          清除
        </el-button>
      </div>
    </div>

    <div v-if="selectedImage" class="image-shell">
      <div class="image-frame">
        <img
          ref="imageRef"
          :src="selectedImage.dataUrl"
          :alt="selectedImage.name"
          @load="updateNaturalSize"
        />
        <div v-if="overlayBoxes.length > 0" class="ocr-overlay">
          <div
            v-for="box in overlayBoxes"
            :key="box.key"
            class="ocr-box"
            :style="box.style"
            :title="box.title"
          >
            <span>{{ box.text }}</span>
          </div>
        </div>
        <div v-if="isProcessing" class="processing-mask">
          <span>识别中</span>
        </div>
        <div v-else-if="isPasting" class="processing-mask">
          <span>读取中</span>
        </div>
      </div>
    </div>

    <button
      v-else
      type="button"
      class="empty-drop-zone"
      @click="openFileDialog"
    >
      <span class="drop-title">选择图片开始识别</span>
      <span class="drop-copy">支持拖拽文件到这里，也可以直接粘贴剪贴板图片</span>
    </button>
  </section>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useImageFileInteraction } from "aiohub-sdk";
import type { OcrLine, SelectedOcrImage } from "./types";

const props = defineProps<{
  selectedImage: SelectedOcrImage | null;
  isProcessing: boolean;
  ocrLines?: OcrLine[];
}>();

const emit = defineEmits<{
  (event: "image-selected", file: File): void;
  (event: "image-path-selected", path: string): void;
  (event: "image-cleared"): void;
}>();

const fileInput = ref<HTMLInputElement | null>(null);
const imageRef = ref<HTMLImageElement | null>(null);
const dropZoneRef = ref<HTMLElement>();
const naturalSize = ref({ width: 0, height: 0 });
const interactionDisabled = computed(() => props.isProcessing);

const handleInteractionFiles = async (files: File[]) => {
  emitImageFile(files[0]);
};

const { isDraggingOver } = useImageFileInteraction({
  element: dropZoneRef,
  multiple: false,
  disabled: interactionDisabled,
  enablePaste: false,
  onFiles: handleInteractionFiles,
  onPaths: async (paths) => {
    const path = paths.find(isSupportedImagePath);
    if (path) {
      emit("image-path-selected", path);
    }
  },
});

const { isPasting } = useImageFileInteraction({
  multiple: false,
  disabled: interactionDisabled,
  showPasteMessage: false,
  preventDefaultOnPaste: true,
  onFiles: handleInteractionFiles,
});

const overlayBoxes = computed(() => {
  const width = naturalSize.value.width;
  const height = naturalSize.value.height;
  if (!width || !height) return [];

  return (props.ocrLines || [])
    .map((line, index) => {
      const points = line.bbox || [];
      const xs = points.map((point) => Number(point[0])).filter(Number.isFinite);
      const ys = points.map((point) => Number(point[1])).filter(Number.isFinite);
      if (xs.length === 0 || ys.length === 0) return null;

      const minX = Math.max(0, Math.min(...xs));
      const minY = Math.max(0, Math.min(...ys));
      const maxX = Math.min(width, Math.max(...xs));
      const maxY = Math.min(height, Math.max(...ys));
      const boxWidth = Math.max(1, maxX - minX);
      const boxHeight = Math.max(1, maxY - minY);

      return {
        key: `${index}-${line.text}`,
        text: line.text,
        title: `${line.text} (${Math.round(line.score * 100)}%)`,
        style: {
          left: `${(minX / width) * 100}%`,
          top: `${(minY / height) * 100}%`,
          width: `${(boxWidth / width) * 100}%`,
          height: `${(boxHeight / height) * 100}%`,
        },
      };
    })
    .filter((box): box is NonNullable<typeof box> => Boolean(box));
});

function openFileDialog() {
  fileInput.value?.click();
}

defineExpose({
  openFileDialog,
});

function updateNaturalSize() {
  const image = imageRef.value;
  naturalSize.value = {
    width: image?.naturalWidth || 0,
    height: image?.naturalHeight || 0,
  };
}

function emitImageFile(file: File | undefined | null) {
  if (!file) return;
  if (!file.type.startsWith("image/")) return;
  emit("image-selected", file);
}

function handleFileInput(event: Event) {
  const input = event.target as HTMLInputElement;
  emitImageFile(input.files?.[0]);
  input.value = "";
}

function isSupportedImagePath(path: string) {
  return /\.(png|jpe?g|webp|gif|bmp)$/i.test(path);
}
</script>

<style scoped>
.preview-panel {
  min-width: 0;
  flex: 1;
  gap: 14px;
}

@media (max-width: 1024px) {
  .preview-panel {
    min-height: 400px;
    height: 50vh;
  }
}

.preview-panel.dragging {
  border-color: color-mix(in srgb, var(--primary-color) 72%, var(--border-color));
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--primary-color) 55%, transparent);
}

.file-input {
  display: none;
}

.preview-toolbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  flex-shrink: 0;
}

.preview-toolbar h3 {
  margin: 0;
  color: var(--text-color);
  font-size: 16px;
  font-weight: 650;
  line-height: 1.3;
}

.preview-toolbar p {
  margin: 5px 0 0;
  color: var(--text-color-secondary);
  font-size: 12px;
  line-height: 1.4;
  overflow-wrap: anywhere;
}

.toolbar-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.image-shell {
  min-height: 0;
  flex: 1;
  display: grid;
  place-items: center;
  overflow: auto;
  border: var(--border-width, 1px) solid var(--border-color);
  border-radius: 8px;
  background:
    linear-gradient(45deg, color-mix(in srgb, var(--input-bg, transparent) 75%, transparent) 25%, transparent 25%),
    linear-gradient(-45deg, color-mix(in srgb, var(--input-bg, transparent) 75%, transparent) 25%, transparent 25%),
    linear-gradient(45deg, transparent 75%, color-mix(in srgb, var(--input-bg, transparent) 75%, transparent) 75%),
    linear-gradient(-45deg, transparent 75%, color-mix(in srgb, var(--input-bg, transparent) 75%, transparent) 75%);
  background-position: 0 0, 0 8px, 8px -8px, -8px 0;
  background-size: 16px 16px;
}

.image-frame {
  position: relative;
  max-width: 100%;
  max-height: 100%;
}

.image-frame img {
  display: block;
  max-width: 100%;
  max-height: calc(100vh - 190px);
  object-fit: contain;
  border-radius: 6px;
}

.ocr-overlay,
.processing-mask {
  position: absolute;
  inset: 0;
  pointer-events: none;
}

.ocr-box {
  position: absolute;
  min-width: 6px;
  min-height: 6px;
  border: 1px solid color-mix(in srgb, var(--el-color-success) 78%, white);
  border-radius: 4px;
  background: color-mix(in srgb, var(--el-color-success) 18%, transparent);
  overflow: hidden;
}

.ocr-box span {
  display: block;
  max-width: 100%;
  padding: 1px 4px;
  color: var(--el-color-success);
  background: color-mix(in srgb, var(--card-bg, white) 88%, transparent);
  font-size: 11px;
  line-height: 1.3;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.processing-mask {
  display: grid;
  place-items: center;
  border-radius: 6px;
  background: color-mix(in srgb, var(--card-bg, white) 45%, transparent);
  backdrop-filter: blur(4px);
}

.processing-mask span {
  padding: 8px 12px;
  border: var(--border-width, 1px) solid var(--border-color);
  border-radius: 999px;
  color: var(--text-color);
  background: var(--card-bg);
  font-size: 13px;
}

.empty-drop-zone {
  flex: 1;
  min-height: 360px;
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 24px;
  border: var(--border-width, 1px) dashed var(--border-color);
  border-radius: 8px;
  color: var(--text-color);
  background: color-mix(in srgb, var(--input-bg, transparent) 76%, transparent);
  cursor: pointer;
}

.empty-drop-zone:hover {
  border-color: color-mix(in srgb, var(--primary-color) 60%, var(--border-color));
}

.drop-title {
  font-size: 18px;
  font-weight: 650;
  line-height: 1.3;
}

.drop-copy {
  max-width: 360px;
  color: var(--text-color-secondary);
  font-size: 13px;
  line-height: 1.5;
  text-align: center;
}
</style>
