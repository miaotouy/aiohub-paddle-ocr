export type RuntimeStatus = "idle" | "ready" | "error";

export type ModelStatus = "unknown" | "ready" | "missing";

export type ModelBackend = "mnn-ocr-rs" | "onnxruntime" | string;

export interface ModelRegistryProfile {
  id: string;
  name: string;
  language: string;
  backend: ModelBackend;
  modelDir?: string;
  family?: string;
  tier?: string;
  detModel?: string;
  recModel?: string;
  dict?: string;
  detOnnx?: string;
  recOnnx?: string;
  detConfig?: string;
  recConfig?: string;
  aliases?: string[];
  builtIn?: boolean;
  package?: boolean;
  experimental?: boolean;
  sourceUrl?: string;
  revision?: string;
  sha256?: Record<string, string>;
  license?: string;
}

export interface ModelRegistry {
  schemaVersion?: number;
  defaultProfile?: string;
  profiles: ModelRegistryProfile[];
}

export interface SelectedOcrImage {
  id: string;
  name: string;
  dataUrl: string;
  path?: string;
}

export interface OcrLine {
  text: string;
  score: number;
  bbox: number[][];
}

export interface PaddleOcrImageResult {
  blockId: string;
  imageId: string;
  text: string;
  confidence?: number;
  status: "success" | "error";
  error?: string;
  lines?: OcrLine[];
}

export interface PaddleOcrBatchResult {
  results?: PaddleOcrImageResult[];
}

export interface PaddleOcrHealthCheckResult {
  ready: boolean;
  status: string;
  backend: ModelBackend;
  profile: string;
  profileName: string;
  modelFiles: string;
}

export interface ImportModelForm {
  modelName: string;
  backend: "mnn-ocr-rs" | "onnxruntime";
  language: string;
  modelDir: string;
  detModel?: string;
  recModel?: string;
  dict?: string;
  detOnnx?: string;
  recOnnx?: string;
  detConfig?: string;
  recConfig?: string;
}
