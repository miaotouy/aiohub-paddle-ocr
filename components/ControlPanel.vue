<template>
  <div class="control-panel glass-panel">
    <!-- 1. 标题和版本 -->
    <div class="panel-heading">
      <h2>Paddle OCR</h2>
      <span class="version-tag">v{{ version }}</span>
    </div>

    <div class="divider"></div>

    <!-- 2. 状态指示器 (带 Tooltip) -->
    <div class="status-indicator-wrapper">
      <el-tooltip placement="bottom" :show-after="200">
        <template #content>
          <div class="status-tooltip-content">
            <div class="tooltip-item">
              <span class="label">运行时:</span>
              <span :class="['value', runtimeStatus === 'ready' ? 'success' : 'danger']">
                {{ runtimeStatusText }}
              </span>
            </div>
            <div class="tooltip-item">
              <span class="label">模型状态:</span>
              <span :class="['value', modelStatus === 'ready' ? 'success' : 'danger']">
                {{ modelStatusText }}
              </span>
            </div>
            <div class="tooltip-item">
              <span class="label">当前后端:</span>
              <span class="value info">{{ selectedBackendLabel }}</span>
            </div>
            <div v-if="lastDurationMs !== null" class="tooltip-item">
              <span class="label">上次耗时:</span>
              <span class="value info">{{ lastDurationText }}</span>
            </div>
          </div>
        </template>

        <div class="status-badge" :class="isAllReady ? 'ready' : 'error'">
          <span class="status-dot"></span>
          <span class="status-text">{{ isAllReady ? '已就绪' : '未就绪' }}</span>
        </div>
      </el-tooltip>

      <!-- 刷新/检查按钮 -->
      <el-tooltip content="检查运行时与模型" placement="bottom" :show-after="500">
        <el-button
          class="refresh-btn"
          link
          :loading="isChecking"
          @click="$emit('check-runtime')"
        >
          <template #icon>
            <RefreshCw v-if="!isChecking" class="icon-spin-hover" />
          </template>
        </el-button>
      </el-tooltip>
    </div>

    <div class="divider"></div>

    <!-- 3. 模型 Profile 选择 + 导入按钮 -->
    <div class="model-select-group">
      <el-select
        :model-value="selectedProfileId"
        filterable
        class="profile-select"
        :loading="registryLoading"
        placeholder="选择模型 Profile"
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

      <!-- 导入模型按钮 -->
      <el-tooltip content="导入自定义模型" placement="bottom" :show-after="500">
        <el-button
          class="import-btn"
          plain
          @click="$emit('import-model')"
        >
          <template #icon>
            <Plus />
          </template>
        </el-button>
      </el-tooltip>
    </div>

    <!-- 4. 耗时显示 (如果有) -->
    <div v-if="lastDurationMs !== null" class="duration-badge">
      <Clock class="duration-icon" />
      <span>{{ lastDurationText }}</span>
    </div>

    <!-- 5. 核心操作按钮组 -->
    <div class="action-stack">
      <el-button plain @click="$emit('select-image')">
        <template #icon>
          <Image />
        </template>
        选择图片
      </el-button>
      <el-button
        type="primary"
        :loading="isProcessing"
        :disabled="!selectedImageName"
        @click="$emit('run-ocr')"
      >
        <template #icon>
          <Play v-if="!isProcessing" />
        </template>
        开始识别
      </el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { RefreshCw, Plus, Play, Image, Clock } from "lucide-vue-next";
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

const isAllReady = computed(() => props.runtimeStatus === "ready" && props.modelStatus === "ready");

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
/* ── 顶栏容器 ── */
.control-panel.glass-panel {
  width: 100%;
  min-width: 0;
  height: 48px !important;
  display: flex !important;
  flex-direction: row !important;
  align-items: center !important;
  gap: 12px !important;
  padding: 0 12px !important;
  flex-shrink: 0;
}

/* ── 标题与版本 ── */
.panel-heading {
  display: flex;
  align-items: baseline;
  gap: 6px;
  flex-shrink: 0;
}

.panel-heading h2 {
  margin: 0;
  color: var(--text-color);
  font-size: 14px;
  font-weight: 650;
  line-height: 1;
  white-space: nowrap;
}

.version-tag {
  color: var(--text-color-secondary);
  font-size: 10px;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  white-space: nowrap;
}

/* ── 分割线 ── */
.divider {
  width: 1px;
  height: 18px;
  background: var(--border-color);
  flex-shrink: 0;
}

/* ── 状态指示器 ── */
.status-indicator-wrapper {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
}

.status-badge {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 2px 8px;
  border-radius: 4px;
  cursor: default;
  height: 24px;
}

.status-badge.ready {
  background: color-mix(in srgb, var(--el-color-success) 12%, transparent);
}

.status-badge.error {
  background: color-mix(in srgb, var(--el-color-danger) 12%, transparent);
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.status-badge.ready .status-dot {
  background: var(--el-color-success);
  box-shadow: 0 0 4px color-mix(in srgb, var(--el-color-success) 60%, transparent);
}

.status-badge.error .status-dot {
  background: var(--el-color-danger);
  box-shadow: 0 0 4px color-mix(in srgb, var(--el-color-danger) 60%, transparent);
}

.status-text {
  font-size: 11px;
  font-weight: 500;
  line-height: 1;
  white-space: nowrap;
}

.status-badge.ready .status-text {
  color: var(--el-color-success);
}

.status-badge.error .status-text {
  color: var(--el-color-danger);
}

/* ── Tooltip 内容 ── */
.status-tooltip-content {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 2px 0;
}

.tooltip-item {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  line-height: 1.4;
}

.tooltip-item .label {
  color: var(--text-color-secondary);
  flex-shrink: 0;
}

.tooltip-item .value {
  font-weight: 500;
}

.tooltip-item .value.success {
  color: var(--el-color-success);
}

.tooltip-item .value.danger {
  color: var(--el-color-danger);
}

.tooltip-item .value.info {
  color: var(--text-color);
}

/* ── 刷新按钮 ── */
.refresh-btn {
  height: 24px;
  width: 24px;
  padding: 0 !important;
  flex-shrink: 0;
}

.refresh-btn :deep(.el-button__icon) {
  margin: 0;
}

.refresh-btn :deep(svg) {
  width: 14px;
  height: 14px;
  color: var(--text-color-secondary);
  transition: transform 0.3s ease;
}

.refresh-btn:hover :deep(svg) {
  color: var(--text-color);
}

.icon-spin-hover {
  transition: transform 0.3s ease;
}

.refresh-btn:hover .icon-spin-hover {
  transform: rotate(60deg);
}

/* ── 模型选择组 ── */
.model-select-group {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.profile-select {
  width: 260px;
}

.profile-select :deep(.el-input__wrapper) {
  height: 28px;
  padding: 0 8px;
}

.profile-select :deep(.el-input__inner) {
  font-size: 12px;
  height: 28px;
}

.profile-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
}

.profile-option span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
}

.profile-option small {
  flex-shrink: 0;
  color: var(--text-color-secondary);
  font-size: 10px;
}

/* ── 导入模型按钮 ── */
.import-btn {
  height: 28px;
  width: 28px;
  padding: 0 !important;
  flex-shrink: 0;
}

.import-btn :deep(.el-button__icon) {
  margin: 0;
}

.import-btn :deep(svg) {
  width: 14px;
  height: 14px;
}

/* ── 耗时显示 ── */
.duration-badge {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--input-bg, transparent) 60%, transparent);
  flex-shrink: 0;
  height: 24px;
  font-size: 11px;
  color: var(--text-color-secondary);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  white-space: nowrap;
}

.duration-icon {
  width: 12px;
  height: 12px;
  flex-shrink: 0;
}

/* ── 操作按钮组 ── */
.action-stack {
  display: flex;
  flex-direction: row;
  gap: 6px;
  margin-left: auto;
  flex-shrink: 0;
}

.action-stack :deep(.el-button) {
  height: 28px;
  padding: 0 10px;
  font-size: 12px;
}

.action-stack :deep(.el-button .el-button__icon) {
  margin-right: 4px;
}

.action-stack :deep(.el-button svg) {
  width: 14px;
  height: 14px;
}

/* ── 响应式 ── */
@media (max-width: 1100px) {
  .control-panel.glass-panel {
    height: auto !important;
    flex-wrap: wrap !important;
    padding: 10px 12px !important;
    gap: 10px !important;
  }

  .action-stack {
    margin-left: 0;
    width: 100%;
    justify-content: flex-end;
  }
}

@media (max-width: 640px) {
  .control-panel.glass-panel {
    flex-direction: column !important;
    align-items: stretch !important;
  }

  .model-select-group {
    width: 100%;
  }

  .profile-select {
    flex: 1;
    width: 0;
  }

  .action-stack {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
}
</style>
