import manifest from "../manifest.json";
import embeddedRegistryJson from "../models/registry.json";
import type {
  ModelRegistry,
  ModelRegistryProfile,
  PaddleOcrBatchResult,
  PaddleOcrHealthCheckResult,
} from "../components/types";

const embeddedRegistry = embeddedRegistryJson as ModelRegistry;

export function getOcrContribution() {
  return (manifest.contributions || []).find(
    (contribution: { type?: string }) => contribution.type === "ocr-engine"
  ) as
    | {
        defaultModelProfile?: string;
        modelProfiles?: ModelRegistryProfile[];
      }
    | undefined;
}

export function getManifestProfiles(): ModelRegistryProfile[] {
  const contribution = getOcrContribution();
  return ((contribution?.modelProfiles || embeddedRegistry.profiles || []) as ModelRegistryProfile[]).map((profile) => ({
    ...profile,
  }));
}

export function getManifestDefaultProfile(): string {
  const contribution = getOcrContribution();
  return contribution?.defaultModelProfile || "ppocr-v6-small-onnx";
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function normalizeRelativePath(filePath?: string): string {
  const normalized = (filePath || "").replace(/\\/g, "/").replace(/^\/+/, "").trim();
  if (!normalized) {
    throw new Error("模型文件路径为空");
  }
  if (normalized.split("/").some((segment) => segment === ".." || segment === "")) {
    throw new Error(`模型文件路径无效: ${filePath}`);
  }
  return normalized;
}

export function createCustomModelId(modelName: string): string {
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

export function getFileNameFromPath(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() || "本地图片";
}

export function inferImageMimeType(path: string): string {
  const lower = path.toLowerCase();
  if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "image/jpeg";
  if (lower.endsWith(".webp")) return "image/webp";
  if (lower.endsWith(".gif")) return "image/gif";
  if (lower.endsWith(".bmp")) return "image/bmp";
  return "image/png";
}

export function readHealthCheckResult(value: unknown): PaddleOcrHealthCheckResult | null {
  if (!isRecord(value) || value.ready !== true || typeof value.status !== "string") {
    return null;
  }

  return {
    ready: value.ready,
    status: value.status,
    backend: typeof value.backend === "string" ? value.backend : "",
    profile: typeof value.profile === "string" ? value.profile : "",
    profileName: typeof value.profileName === "string" ? value.profileName : "",
    modelFiles: typeof value.modelFiles === "string" ? value.modelFiles : "",
  };
}

export function readBatchResult(value: unknown): PaddleOcrBatchResult | null {
  if (!isRecord(value) || !Array.isArray(value.results)) {
    return null;
  }

  return {
    results: value.results as PaddleOcrBatchResult["results"],
  };
}

export function readEnvelopeError(response: Record<string, unknown>): string {
  const candidates = [response.error, response.message, response.data];

  for (const candidate of candidates) {
    if (typeof candidate === "string" && candidate.trim()) {
      return candidate;
    }

    if (isRecord(candidate)) {
      const nested = candidate.message || candidate.error;
      if (typeof nested === "string" && nested.trim()) {
        return nested;
      }
    }
  }

  return "";
}

export function normalizeRecognizeBatchResponse(response: unknown): PaddleOcrBatchResult {
  const directResult = readBatchResult(response);
  if (directResult) {
    return directResult;
  }

  if (isRecord(response)) {
    if (response.success === false) {
      throw new Error(readEnvelopeError(response) || "OCR 调用返回失败");
    }

    const dataResult = readBatchResult(response.data);
    if (dataResult) {
      return dataResult;
    }
  }

  throw new Error("OCR 返回结构异常：未找到 results 或 data.results");
}

export function normalizeHealthCheckResponse(response: unknown): PaddleOcrHealthCheckResult {
  const directResult = readHealthCheckResult(response);
  if (directResult) {
    return directResult;
  }

  if (isRecord(response)) {
    if (response.success === false) {
      throw new Error(readEnvelopeError(response) || "健康检查返回失败");
    }

    const dataResult = readHealthCheckResult(response.data);
    if (dataResult) {
      return dataResult;
    }
  }

  throw new Error("健康检查返回结构异常：未找到 ready/status");
}