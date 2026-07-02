import { ref } from "vue";
import { customMessage, pluginManager } from "aiohub-sdk";
import type {
  ImportModelForm,
  ModelRegistry,
  ModelRegistryProfile,
} from "../components/types";
import {
  createCustomModelId,
  getManifestDefaultProfile,
  getManifestProfiles,
  normalizeRelativePath,
  toErrorMessage,
} from "../utils/ocr-helpers";
import embeddedRegistryJson from "../models/registry.json";

const PLUGIN_ID = "paddle-ocr";
const CUSTOM_REGISTRY_FILE = "custom-registry.json";
const pluginContext = pluginManager.createPluginContext(PLUGIN_ID);
const embeddedRegistry = embeddedRegistryJson as ModelRegistry;

export function useModelRegistry() {
  const modelProfiles = ref<ModelRegistryProfile[]>(getManifestProfiles());
  const selectedProfileId = ref(getManifestDefaultProfile());
  const registryLoading = ref(false);
  const registryError = ref<string | null>(null);
  const isImportDialogVisible = ref(false);
  const isImportingModel = ref(false);

  async function loadRegistry() {
    registryLoading.value = true;
    registryError.value = null;

    let builtInProfiles = embeddedRegistry.profiles || getManifestProfiles();
    let defaultProfile = embeddedRegistry.defaultProfile || getManifestDefaultProfile();
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

  return {
    modelProfiles,
    selectedProfileId,
    registryLoading,
    registryError,
    isImportDialogVisible,
    isImportingModel,
    loadRegistry,
    importCustomModel,
  };
}