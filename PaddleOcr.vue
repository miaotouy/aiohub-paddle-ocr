<template>
  <div class="paddle-ocr">
    <header class="toolbar">
      <div>
        <h2>Paddle OCR</h2>
        <p>v{{ version }} · {{ selectedProfileName }} · {{ selectedBackendLabel }}</p>
      </div>
      <div class="toolbar-actions">
        <el-select v-model="selectedProfileId" class="profile-select" size="default">
          <el-option
            v-for="profile in modelProfiles"
            :key="profile.id"
            :label="profileLabel(profile)"
            :value="profile.id"
          />
        </el-select>
        <input
          ref="testImageInput"
          class="file-input"
          type="file"
          accept="image/*"
          @change="handleTestImageChange"
        />
        <el-button @click="selectTestImage">选择图片</el-button>
        <el-button :loading="checking" @click="checkRuntime">检查</el-button>
        <el-button type="primary" :loading="testing" @click="runSmokeTest">测试</el-button>
      </div>
    </header>

    <section class="status-grid">
      <div class="status-item">
        <span class="label">运行时</span>
        <strong :class="runtimeStatusClass">{{ runtimeStatusText }}</strong>
      </div>
      <div class="status-item">
        <span class="label">模型</span>
        <strong :class="modelStatusClass">{{ modelStatusText }}</strong>
      </div>
      <div class="status-item">
        <span class="label">后端</span>
        <strong>{{ selectedBackendLabel }}</strong>
      </div>
      <div class="status-item">
        <span class="label">最近耗时</span>
        <strong>{{ lastDurationText }}</strong>
      </div>
    </section>

    <section class="panel">
      <div class="panel-header">
        <h3>模型文件</h3>
        <span>{{ selectedModelDir }}</span>
      </div>
      <ul class="file-list">
        <li v-for="file in modelFiles" :key="file">{{ file }}</li>
      </ul>
    </section>

    <section class="panel">
      <div class="panel-header">
        <h3>模型来源</h3>
        <span>{{ benchmarkStatusText }}</span>
      </div>
      <dl class="meta-list">
        <div>
          <dt>来源</dt>
          <dd>{{ selectedSourceText }}</dd>
        </div>
        <div>
          <dt>Revision</dt>
          <dd>{{ selectedRevisionText }}</dd>
        </div>
        <div>
          <dt>Hash</dt>
          <dd>{{ selectedHashText }}</dd>
        </div>
        <div>
          <dt>测试图片</dt>
          <dd>{{ selectedTestImageLabel }}</dd>
        </div>
      </dl>
    </section>

    <section class="panel">
      <div class="panel-header">
        <h3>最近结果</h3>
        <span>{{ lastResultLabel }}</span>
      </div>
      <pre>{{ lastResultText }}</pre>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { customMessage, execute } from 'aiohub-sdk';
import manifest from './manifest.json';
import modelRegistryJson from './models/registry.json';

interface PaddleOcrResult {
  results?: Array<{
    blockId: string;
    imageId: string;
    text: string;
    confidence?: number;
    status: 'success' | 'error';
    error?: string;
  }>;
}

interface ModelRegistryProfile {
  id: string;
  name: string;
  language: string;
  backend: string;
  modelDir: string;
  family?: string;
  tier?: string;
  detModel?: string;
  recModel?: string;
  dict?: string;
  detOnnx?: string;
  recOnnx?: string;
  detConfig?: string;
  recConfig?: string;
  experimental?: boolean;
  sourceUrl?: string;
  revision?: string;
  sha256?: Record<string, string>;
  license?: string;
}

interface ModelRegistry {
  defaultProfile?: string;
  profiles: ModelRegistryProfile[];
}

const version = manifest.version;
const modelRegistry = modelRegistryJson as ModelRegistry;
const defaultModelProfile = modelRegistry.defaultProfile || 'ppocr-v5-mobile-general';
const modelProfiles = modelRegistry.profiles;

const checking = ref(false);
const testing = ref(false);
const selectedProfileId = ref(defaultModelProfile);
const testImageInput = ref<HTMLInputElement | null>(null);
const selectedTestImage = ref<{ name: string; dataUrl: string } | null>(null);
const runtimeStatus = ref<'idle' | 'ready' | 'error'>('idle');
const modelStatus = ref<'unknown' | 'ready' | 'missing'>('unknown');
const lastDurationMs = ref<number | null>(null);
const lastImageCount = ref(0);
const lastResult = ref<unknown>(null);

const runtimeStatusText = computed(() => {
  if (runtimeStatus.value === 'ready') return '可调用';
  if (runtimeStatus.value === 'error') return '异常';
  return '未检查';
});

const modelStatusText = computed(() => {
  if (modelStatus.value === 'ready') return '完整';
  if (modelStatus.value === 'missing') return '缺失';
  return '未检查';
});

const runtimeStatusClass = computed(() => `state state-${runtimeStatus.value}`);
const modelStatusClass = computed(() => `state state-${modelStatus.value}`);
const lastDurationText = computed(() => (
  lastDurationMs.value === null ? '-' : `${lastDurationMs.value.toFixed(0)} ms`
));
const lastResultLabel = computed(() => (
  lastResult.value ? '已更新' : '暂无'
));
const lastResultText = computed(() => (
  lastResult.value ? JSON.stringify(lastResult.value, null, 2) : ''
));
const selectedProfileName = computed(() => (
  modelProfiles.find((profile) => profile.id === selectedProfileId.value)?.name || selectedProfileId.value
));
const selectedProfile = computed(() => (
  modelProfiles.find((profile) => profile.id === selectedProfileId.value) || modelProfiles[0]
));
const selectedBackendLabel = computed(() => {
  if (selectedProfile.value?.backend === 'onnxruntime') return 'ONNX Runtime';
  if (selectedProfile.value?.backend === 'mnn-ocr-rs') return 'MNN / ocr-rs';
  return selectedProfile.value?.backend || '-';
});
const selectedModelDir = computed(() => selectedProfile.value?.modelDir || 'models');
const modelFiles = computed(() => {
  const profile = selectedProfile.value;
  if (!profile) return [];

  const fields = profile.backend === 'onnxruntime'
    ? ['detOnnx', 'recOnnx', 'detConfig', 'recConfig', 'dict'] as const
    : ['detModel', 'recModel', 'dict'] as const;

  return fields
    .map((field) => profile[field])
    .filter((file): file is string => Boolean(file))
    .map((file) => `${profile.modelDir}/${file}`);
});
const benchmarkStatusText = computed(() => (
  selectedProfile.value?.experimental ? '待 benchmark' : '当前基线'
));
const selectedSourceText = computed(() => selectedProfile.value?.sourceUrl || '-');
const selectedRevisionText = computed(() => selectedProfile.value?.revision || '-');
const selectedHashText = computed(() => {
  const sha256 = selectedProfile.value?.sha256;
  if (sha256 && Object.keys(sha256).length > 0) {
    return `${Object.keys(sha256).length} 个文件已记录`;
  }
  return selectedProfile.value?.experimental ? '待固定' : '见第三方声明';
});
const selectedTestImageLabel = computed(() => selectedTestImage.value?.name || '未选择');

const profileLabel = (profile: ModelRegistryProfile) => (
  profile.experimental ? `${profile.name} · 实验` : profile.name
);

const callRecognizeBatch = async (images: unknown[]) => {
  const startedAt = performance.now();
  const result = await execute({
    service: 'paddle-ocr',
    method: 'recognizeBatch',
    params: {
      images,
      options: {
        modelProfile: selectedProfileId.value
      }
    }
  }) as PaddleOcrResult;

  lastDurationMs.value = performance.now() - startedAt;
  lastImageCount.value = images.length;
  lastResult.value = result;
  return result;
};

const updateStatusFromError = (error: unknown) => {
  runtimeStatus.value = 'error';
  const message = error instanceof Error ? error.message : String(error);
  if (
    message.includes('模型目录缺失')
    || message.includes('模型文件缺失')
    || message.includes('模型文件为空')
    || message.includes('ONNX Runtime 动态库缺失')
  ) {
    modelStatus.value = 'missing';
  }
  lastResult.value = { error: message };
};

const readFileAsDataUrl = (file: File) => new Promise<string>((resolve, reject) => {
  const reader = new FileReader();
  reader.onload = () => {
    if (typeof reader.result === 'string') {
      resolve(reader.result);
    } else {
      reject(new Error('图片读取结果无效'));
    }
  };
  reader.onerror = () => reject(reader.error || new Error('图片读取失败'));
  reader.readAsDataURL(file);
});

const selectTestImage = () => {
  testImageInput.value?.click();
};

const handleTestImageChange = async (event: Event) => {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;

  if (!file.type.startsWith('image/')) {
    customMessage.warning('请选择图片文件');
    input.value = '';
    return;
  }

  try {
    selectedTestImage.value = {
      name: file.name,
      dataUrl: await readFileAsDataUrl(file)
    };
    customMessage.success('测试图片已选择');
  } catch (error) {
    selectedTestImage.value = null;
    const message = error instanceof Error ? error.message : String(error);
    customMessage.error(message);
  }
};

const checkRuntime = async () => {
  checking.value = true;
  try {
    await callRecognizeBatch([]);
    runtimeStatus.value = 'ready';
    modelStatus.value = 'ready';
    customMessage.success('Paddle OCR 后端可调用');
  } catch (error) {
    updateStatusFromError(error);
    customMessage.error('Paddle OCR 检查失败');
  } finally {
    checking.value = false;
  }
};

const runSmokeTest = async () => {
  if (!selectedTestImage.value) {
    customMessage.warning('请先选择一张测试图片');
    return;
  }

  testing.value = true;
  try {
    const result = await callRecognizeBatch([
      {
        blockId: 'smoke-test-block',
        imageId: 'smoke-test-image',
        dataUrl: selectedTestImage.value.dataUrl
      }
    ]);
    runtimeStatus.value = 'ready';
    modelStatus.value = 'ready';
    const first = result.results?.[0];
    if (first?.status === 'error') {
      customMessage.warning(first.error || '测试图片未完成识别');
    } else {
      customMessage.success('测试调用完成');
    }
  } catch (error) {
    updateStatusFromError(error);
    customMessage.error('测试调用失败');
  } finally {
    testing.value = false;
  }
};
</script>

<style scoped>
.paddle-ocr {
  box-sizing: border-box;
  height: 100%;
  padding: 20px;
  color: var(--text-color);
  background: var(--card-bg);
  overflow: auto;
}

.paddle-ocr * {
  box-sizing: border-box;
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
}

.toolbar h2,
.panel h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 650;
  line-height: 1.3;
}

.toolbar p,
.panel-header span {
  margin: 4px 0 0;
  color: var(--text-color-secondary);
  font-size: 12px;
}

.toolbar-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.profile-select {
  width: 240px;
}

.file-input {
  display: none;
}

.status-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
  margin-bottom: 16px;
}

.status-item {
  min-height: 76px;
  padding: 14px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: var(--input-bg);
}

.label {
  display: block;
  margin-bottom: 10px;
  color: var(--text-color-secondary);
  font-size: 12px;
}

.status-item strong {
  font-size: 16px;
  line-height: 1.25;
}

.state-ready,
.state-idle {
  color: var(--el-color-success);
}

.state-error,
.state-missing {
  color: var(--el-color-danger);
}

.state-unknown {
  color: var(--text-color-secondary);
}

.panel {
  margin-top: 12px;
  padding: 14px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  background: var(--input-bg);
}

.panel-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
}

.file-list {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.file-list li {
  padding: 9px 10px;
  border-radius: 6px;
  background: var(--card-bg);
  color: var(--text-color);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 12px;
  overflow-wrap: anywhere;
}

.meta-list {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px 14px;
  margin: 0;
}

.meta-list div {
  min-width: 0;
}

.meta-list dt {
  margin-bottom: 4px;
  color: var(--text-color-secondary);
  font-size: 12px;
}

.meta-list dd {
  margin: 0;
  color: var(--text-color);
  font-size: 13px;
  line-height: 1.4;
  overflow-wrap: anywhere;
}

pre {
  min-height: 140px;
  max-height: 280px;
  margin: 0;
  padding: 12px;
  border-radius: 6px;
  background: var(--card-bg);
  color: var(--text-color);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  overflow: auto;
}

@media (max-width: 900px) {
  .status-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .file-list {
    grid-template-columns: 1fr;
  }

  .meta-list {
    grid-template-columns: 1fr;
  }
}
</style>
