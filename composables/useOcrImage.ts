import { ref } from "vue";
import { customMessage } from "aiohub-sdk";
import type { SelectedOcrImage } from "../components/types";
import {
  getFileNameFromPath,
  inferImageMimeType,
  toErrorMessage,
} from "../utils/ocr-helpers";

export function useOcrImage() {
  const selectedImage = ref<SelectedOcrImage | null>(null);

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
      customMessage.success("图片已载入");
    } catch (error) {
      customMessage.error(`读取图片失败：${toErrorMessage(error)}`);
    }
  }

  function clearImage() {
    selectedImage.value = null;
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

  return {
    selectedImage,
    handleImageFile,
    handleImagePath,
    clearImage,
  };
}