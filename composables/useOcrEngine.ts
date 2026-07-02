import { computed, ref, watch, type Ref } from "vue";
import { customMessage, execute } from "aiohub-sdk";
import type {
  PaddleOcrBatchResult,
  PaddleOcrHealthCheckResult,
  RuntimeStatus,
  ModelStatus,
  SelectedOcrImage,
} from "../components/types";
import {
  normalizeHealthCheckResponse,
  normalizeRecognizeBatchResponse,
  toErrorMessage,
} from "../utils/ocr-helpers";

const PLUGIN_ID = "paddle-ocr";

export function useOcrEngine(
  selectedProfileId: Ref<string>,
  selectedImage: Ref<SelectedOcrImage | null>
) {
  const runtimeStatus = ref<RuntimeStatus>("idle");
  const modelStatus = ref<ModelStatus>("unknown");
  const lastDurationMs = ref<number | null>(null);
  const lastResult = ref<PaddleOcrBatchResult | null>(null);
  const lastRawResult = ref<unknown | null>(null);
  const lastError = ref<string | null>(null);

  const isChecking = ref(false);
  const isProcessing = ref(false);

  const previewLines = computed(() => {
    const imageId = selectedImage.value?.id;
    if (!imageId) return [];
    return (
      lastResult.value?.results?.find((result) => result.imageId === imageId)?.lines || []
    );
  });

  // 核心优化：当图片发生变化（载入新图片或清除图片）时，自动重置 OCR 结果和错误状态
  watch(selectedImage, () => {
    lastResult.value = null;
    lastRawResult.value = null;
    lastError.value = null;
  });

  async function callRecognizeBatch(images: unknown[]) {
    const startedAt = performance.now();
    try {
      lastRawResult.value = null;
      const response = (await execute({
        service: PLUGIN_ID,
        method: "recognizeBatch",
        params: {
          images,
          options: {
            modelProfile: selectedProfileId.value,
          },
        },
      })) as unknown;

      lastRawResult.value = response;
      const result = normalizeRecognizeBatchResponse(response);
      lastResult.value = result;
      lastError.value = null;
      return result;
    } finally {
      lastDurationMs.value = performance.now() - startedAt;
    }
  }

  async function callHealthCheck(): Promise<PaddleOcrHealthCheckResult> {
    const startedAt = performance.now();
    try {
      lastRawResult.value = null;
      lastResult.value = null;
      const response = (await execute({
        service: PLUGIN_ID,
        method: "healthCheck",
        params: {
          options: {
            modelProfile: selectedProfileId.value,
          },
        },
      })) as unknown;

      lastRawResult.value = response;
      lastError.value = null;
      return normalizeHealthCheckResponse(response);
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
      await callHealthCheck();
      runtimeStatus.value = "ready";
      modelStatus.value = "ready";
      customMessage.success("Paddle OCR 健康检查通过");
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
      const result = await callRecognizeBatch([requestImage]);
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

  return {
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
  };
}