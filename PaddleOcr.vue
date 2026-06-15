<template>
  <div class="paddle-ocr">
    <header class="toolbar">
      <div>
        <h2>Paddle OCR</h2>
        <p>v{{ version }} · {{ selectedProfileName }}</p>
      </div>
      <div class="toolbar-actions">
        <el-select v-model="selectedProfileId" class="profile-select" size="default">
          <el-option
            v-for="profile in modelProfiles"
            :key="profile.id"
            :label="profile.name"
            :value="profile.id"
          />
        </el-select>
        <el-button :loading="checking" @click="checkRuntime">检查</el-button>
        <el-button type="primary" :loading="testing" @click="runSmokeTest">测试</el-button>
      </div>
    </header>

    <section class="status-grid">
      <div class="status-item">
        <span class="label">后端</span>
        <strong :class="runtimeStatusClass">{{ runtimeStatusText }}</strong>
      </div>
      <div class="status-item">
        <span class="label">模型</span>
        <strong :class="modelStatusClass">{{ modelStatusText }}</strong>
      </div>
      <div class="status-item">
        <span class="label">最近耗时</span>
        <strong>{{ lastDurationText }}</strong>
      </div>
      <div class="status-item">
        <span class="label">模型数量</span>
        <strong>{{ modelProfiles.length }}</strong>
      </div>
    </section>

    <section class="panel">
      <div class="panel-header">
        <h3>模型文件</h3>
        <span>models/ppocr-v5-mobile</span>
      </div>
      <ul class="file-list">
        <li v-for="file in modelFiles" :key="file">{{ file }}</li>
      </ul>
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

const version = manifest.version;
const defaultModelProfile = 'ppocr-v5-mobile-general';
const modelProfiles = [
  { id: 'ppocr-v5-mobile-general', name: '通用', language: 'general' },
  { id: 'ppocr-v5-mobile-en', name: '英文', language: 'en' },
  { id: 'ppocr-v5-mobile-ko', name: '韩文', language: 'ko' },
  { id: 'ppocr-v5-mobile-latin', name: '拉丁文字', language: 'latin' },
  { id: 'ppocr-v5-mobile-arabic', name: '阿拉伯文字', language: 'arabic' },
  { id: 'ppocr-v5-mobile-cyrillic', name: '西里尔文字', language: 'cyrillic' },
  { id: 'ppocr-v5-mobile-el', name: '希腊文', language: 'el' },
  { id: 'ppocr-v5-mobile-devanagari', name: '天城文', language: 'devanagari' },
  { id: 'ppocr-v5-mobile-ta', name: '泰米尔文', language: 'ta' },
  { id: 'ppocr-v5-mobile-te', name: '泰卢固文', language: 'te' },
  { id: 'ppocr-v5-mobile-th', name: '泰文', language: 'th' }
];
const modelFiles = [
  'ppocrv5_mobile_det.mnn',
  'ppocrv5_mobile_rec_general.mnn',
  'ppocrv5_mobile_dict_general.txt',
  'ppocrv5_mobile_rec_en.mnn',
  'ppocrv5_mobile_dict_en.txt',
  'ppocrv5_mobile_rec_ko.mnn',
  'ppocrv5_mobile_dict_ko.txt',
  'ppocrv5_mobile_rec_latin.mnn',
  'ppocrv5_mobile_dict_latin.txt',
  'ppocrv5_mobile_rec_arabic.mnn',
  'ppocrv5_mobile_dict_arabic.txt',
  'ppocrv5_mobile_rec_cyrillic.mnn',
  'ppocrv5_mobile_dict_cyrillic.txt',
  'ppocrv5_mobile_rec_el.mnn',
  'ppocrv5_mobile_dict_el.txt',
  'ppocrv5_mobile_rec_devanagari.mnn',
  'ppocrv5_mobile_dict_devanagari.txt',
  'ppocrv5_mobile_rec_ta.mnn',
  'ppocrv5_mobile_dict_ta.txt',
  'ppocrv5_mobile_rec_te.mnn',
  'ppocrv5_mobile_dict_te.txt',
  'ppocrv5_mobile_rec_th.mnn',
  'ppocrv5_mobile_dict_th.txt'
];

const checking = ref(false);
const testing = ref(false);
const selectedProfileId = ref(defaultModelProfile);
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
  if (message.includes('模型文件缺失') || message.includes('模型文件为空')) {
    modelStatus.value = 'missing';
  }
  lastResult.value = { error: message };
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
  testing.value = true;
  try {
    const result = await callRecognizeBatch([
      {
        blockId: 'smoke-test-block',
        imageId: 'smoke-test-image',
        dataUrl: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
        width: 1,
        height: 1
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
  width: 180px;
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
}
</style>
