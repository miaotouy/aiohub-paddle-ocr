<template>
  <aside class="control-panel glass-panel">
    <div class="panel-heading">
      <div>
        <h2>Paddle OCR</h2>
        <p>v{{ version }} · {{ selectedProfileName }}</p>
      </div>
      <el-tag size="small" :type="selectedProfile?.experimental ? 'warning' : 'success'">
        {{ selectedProfile?.experimental ? "实验" : "稳定" }}
      </el-tag>
    </div>

    <div class="status-grid">
      <div class="status-card">
        <span>运行时</span>
        <strong :class="runtimeStatusClass">{{ runtimeStatusText }}</strong>
      </div>
      <div class="status-card">
        <span>模型</span>
        <strong :class="modelStatusClass">{{ modelStatusText }}</strong>
      </div>
      <div class="status-card">
        <span>后端</span>
        <strong>{{ selectedBackendLabel }}</strong>
      </div>
      <div class="status-card">
        <span>耗时</span>
        <strong>{{ lastDurationText }}</strong>
      </div>
    </div>

    <div class="control-group">
      <label>模型 Profile</label>
      <el-select
        :model-value="selectedProfileId"
        filterable
        class="profile-select"
        :loading="registryLoading"
        @update:model-value="handleProfileChange"
      >
        <el-option
          v-for="profile in modelProfiles"
          :key="profile.id"
          :label="profileLabel(profile)"
          :value="profile.id"
        >
          <div class="profile-option">
            <span>{{ profile.name }}</span>
            <small>{{ backendLabel(profile.backend) }} · {{ profile.language }}</small>
          </div>
        </el-option>
      </el-select>
      <p v-if="registryError" class="hint danger">{{ registryError }}</p>
      <p v-else class="hint">{{ modelProfiles.length }} 个 profile 可用</p>
    </div>

    <div class="selected-image">
      <span>当前图片</span>
      <strong>{{ selectedImageName || "未选择" }}</strong>
    </div>

    <div class="action-stack">
      <el-button plain @click="$emit('import-model')">导入自定义模型</el-button>
      <el-button plain @click="$emit('select-image')">选择图片</el-button>
      <el-button :loading="isChecking" @click="$emit('check-runtime')">检查运行时</el-button>
      <el-button
        type="primary"
        :loading="isProcessing"
        :disabled="!selectedImageName"
        @click="$emit('run-ocr')"
      >
        开始识别
      </el-button>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed } from "vue";
import type {
  ModelRegistryProfile,
  ModelStatus,
  RuntimeStatus,
} from "./types";

const props = defineProps<{
  runtimeStatus: RuntimeStatus;
  modelStatus: ModelStatus;
  isProcessing: boolean;
  isChecking: boolean;
  registryLoading: boolean;
  registryError: string | null;
  lastDurationMs: number | null;
  selectedProfileId: string;
  modelProfiles: ModelRegistryProfile[];
  selectedImageName: string | null;
  version: string;
}>();

const emit = defineEmits<{
  (event: "update:selectedProfileId", profileId: string): void;
  (event: "select-image"): void;
  (event: "check-runtime"): void;
  (event: "run-ocr"): void;
  (event: "import-model"): void;
}>();

const selectedProfile = computed(() =>
  props.modelProfiles.find((profile) => profile.id === props.selectedProfileId)
);

const selectedProfileName = computed(
  () => selectedProfile.value?.name || props.selectedProfileId || "未选择"
);

const runtimeStatusText = computed(() => {
  if (props.runtimeStatus === "ready") return "可调用";
  if (props.runtimeStatus === "error") return "异常";
  return "未检查";
});

const modelStatusText = computed(() => {
  if (props.modelStatus === "ready") return "完整";
  if (props.modelStatus === "missing") return "缺失";
  return "未检查";
});

const runtimeStatusClass = computed(() => `state state-${props.runtimeStatus}`);
const modelStatusClass = computed(() => `state state-${props.modelStatus}`);

const lastDurationText = computed(() =>
  props.lastDurationMs === null ? "-" : `${props.lastDurationMs.toFixed(0)} ms`
);

const selectedBackendLabel = computed(() =>
  selectedProfile.value ? backendLabel(selectedProfile.value.backend) : "-"
);

function backendLabel(backend?: string) {
  if (backend === "onnxruntime") return "ONNX Runtime";
  if (backend === "mnn-ocr-rs") return "MNN / ocr-rs";
  return backend || "-";
}

function profileLabel(profile: ModelRegistryProfile) {
  return profile.experimental ? `${profile.name} · 实验` : profile.name;
}

function handleProfileChange(value: string | number | boolean) {
  emit("update:selectedProfileId", String(value));
}
</script>

<style scoped>
.control-panel {
  width: 320px;
  min-width: 300px;
  gap: 16px;
}

.panel-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.panel-heading h2 {
  margin: 0;
  color: var(--text-color);
  font-size: 20px;
  font-weight: 650;
  line-height: 1.25;
}

.panel-heading p {
  margin: 5px 0 0;
  color: var(--text-color-secondary);
  font-size: 12px;
  line-height: 1.4;
}

.status-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.status-card {
  min-width: 0;
  min-height: 72px;
  padding: 12px;
  border: var(--border-width, 1px) solid var(--border-color);
  border-radius: 8px;
  background: color-mix(in srgb, var(--input-bg, transparent) 88%, transparent);
}

.status-card span,
.selected-image span,
.control-group label {
  display: block;
  color: var(--text-color-secondary);
  font-size: 12px;
  line-height: 1.3;
}

.status-card strong {
  display: block;
  margin-top: 9px;
  color: var(--text-color);
  font-size: 16px;
  font-weight: 650;
  line-height: 1.25;
  overflow-wrap: anywhere;
}

.state-ready {
  color: var(--el-color-success) !important;
}

.state-error,
.state-missing {
  color: var(--el-color-danger) !important;
}

.state-idle,
.state-unknown {
  color: var(--text-color-secondary) !important;
}

.control-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.profile-select {
  width: 100%;
}

.profile-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
}

.profile-option span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.profile-option small {
  flex-shrink: 0;
  color: var(--text-color-secondary);
  font-size: 11px;
}

.hint {
  margin: 0;
  color: var(--text-color-secondary);
  font-size: 12px;
  line-height: 1.4;
}

.hint.danger {
  color: var(--el-color-danger);
}

.selected-image {
  min-height: 64px;
  padding: 12px;
  border: var(--border-width, 1px) dashed var(--border-color);
  border-radius: 8px;
  background: color-mix(in srgb, var(--card-bg, transparent) 70%, transparent);
}

.selected-image strong {
  display: block;
  margin-top: 8px;
  color: var(--text-color);
  font-size: 13px;
  font-weight: 600;
  line-height: 1.4;
  overflow-wrap: anywhere;
}

.action-stack {
  display: grid;
  grid-template-columns: 1fr;
  gap: 10px;
  margin-top: auto;
}
</style>
