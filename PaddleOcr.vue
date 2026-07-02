<template>
  <div class="paddle-ocr">
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

    <div class="main-content">
      <PreviewPanel
        ref="previewPanelRef"
        :selected-image="selectedImage"
        :is-processing="isProcessing"
        :ocr-lines="previewLines"
        @image-selected="handleImageFile"
        @image-path-selected="handleImagePath"
        @image-cleared="clearImage"
      />

      <ResultPanel
        :result="lastResult"
        :raw-result="lastRawResult"
        :is-processing="isProcessing"
        :selected-image-name="selectedImage?.name || null"
        :error-message="lastError"
        @send-to-chat="handleSendToChat"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { customMessage, useSendToChat } from "aiohub-sdk";
import manifest from "./manifest.json";
import ControlPanel from "./components/ControlPanel.vue";
import ImportModelDialog from "./components/ImportModelDialog.vue";
import PreviewPanel from "./components/PreviewPanel.vue";
import ResultPanel from "./components/ResultPanel.vue";
import { useModelRegistry } from "./composables/useModelRegistry";
import { useOcrImage } from "./composables/useOcrImage";
import { useOcrEngine } from "./composables/useOcrEngine";

const version = manifest.version;

// 1. 引入模型清单管理 Composable
const {
  modelProfiles,
  selectedProfileId,
  registryLoading,
  registryError,
  isImportDialogVisible,
  isImportingModel,
  loadRegistry,
  importCustomModel,
} = useModelRegistry();

// 2. 引入图片管理 Composable
const {
  selectedImage,
  handleImageFile,
  handleImagePath,
  clearImage,
} = useOcrImage();

// 3. 引入 OCR 引擎 Composable
const {
  runtimeStatus,
  modelStatus,
  lastDurationMs,
  lastResult,
  lastRawResult,
  lastError,
  isChecking,
  isProcessing,
  previewLines,
  checkRuntime,
  runOcr,
} = useOcrEngine(selectedProfileId, selectedImage);

const { sendToChat } = useSendToChat();
const previewPanelRef = ref<InstanceType<typeof PreviewPanel> | null>(null);

onMounted(async () => {
  await loadRegistry();
});

// 核心优化：通过 ref 触发 PreviewPanel 内部的 openFileDialog，消灭重复的隐藏 input
function selectImage() {
  previewPanelRef.value?.openFileDialog();
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
</script>

<style scoped>
.paddle-ocr {
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
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

.main-content {
  display: flex;
  flex-direction: row;
  gap: 16px;
  flex: 1;
  min-height: 0;
  overflow: hidden;
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

@media (max-width: 1024px) {
  .main-content {
    flex-direction: column;
    overflow-y: auto;
  }

  :deep(.result-panel) {
    width: 100% !important;
    min-width: 0 !important;
    height: auto !important;
    min-height: 360px;
  }
}

@media (max-width: 768px) {
  .paddle-ocr {
    padding: 12px;
    overflow-y: auto;
  }
  .main-content {
    overflow: visible;
  }
}
</style>
