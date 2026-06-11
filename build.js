/**
 * Paddle OCR sidecar 插件构建脚本
 */

import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import archiver from 'archiver';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PLUGIN_NAME = 'aiohub-paddle-ocr';

const TARGETS = {
  'windows-x64': {
    rustTarget: 'x86_64-pc-windows-msvc',
    executable: 'aiohub-paddle-ocr.exe',
    manifestKey: 'win32-x64',
    packageExecutable: 'aiohub-paddle-ocr-windows-x64.exe'
  }
};

const MODEL_DIR = 'models/ppocr-v5-mobile';
const DET_MODEL_FILE = 'ppocrv5_mobile_det.mnn';
const MODEL_PROFILES = [
  {
    id: 'ppocr-v5-mobile-general',
    name: 'PP-OCRv5 Mobile General',
    language: 'general',
    recFile: 'ppocrv5_mobile_rec_general.mnn',
    dictFile: 'ppocrv5_mobile_dict_general.txt'
  },
  {
    id: 'ppocr-v5-mobile-en',
    name: 'PP-OCRv5 Mobile English',
    language: 'en',
    recFile: 'ppocrv5_mobile_rec_en.mnn',
    dictFile: 'ppocrv5_mobile_dict_en.txt'
  },
  {
    id: 'ppocr-v5-mobile-ko',
    name: 'PP-OCRv5 Mobile Korean',
    language: 'ko',
    recFile: 'ppocrv5_mobile_rec_ko.mnn',
    dictFile: 'ppocrv5_mobile_dict_ko.txt'
  },
  {
    id: 'ppocr-v5-mobile-latin',
    name: 'PP-OCRv5 Mobile Latin',
    language: 'latin',
    recFile: 'ppocrv5_mobile_rec_latin.mnn',
    dictFile: 'ppocrv5_mobile_dict_latin.txt'
  },
  {
    id: 'ppocr-v5-mobile-arabic',
    name: 'PP-OCRv5 Mobile Arabic',
    language: 'arabic',
    recFile: 'ppocrv5_mobile_rec_arabic.mnn',
    dictFile: 'ppocrv5_mobile_dict_arabic.txt'
  },
  {
    id: 'ppocr-v5-mobile-cyrillic',
    name: 'PP-OCRv5 Mobile Cyrillic',
    language: 'cyrillic',
    recFile: 'ppocrv5_mobile_rec_cyrillic.mnn',
    dictFile: 'ppocrv5_mobile_dict_cyrillic.txt'
  },
  {
    id: 'ppocr-v5-mobile-el',
    name: 'PP-OCRv5 Mobile Greek',
    language: 'el',
    recFile: 'ppocrv5_mobile_rec_el.mnn',
    dictFile: 'ppocrv5_mobile_dict_el.txt'
  },
  {
    id: 'ppocr-v5-mobile-devanagari',
    name: 'PP-OCRv5 Mobile Devanagari',
    language: 'devanagari',
    recFile: 'ppocrv5_mobile_rec_devanagari.mnn',
    dictFile: 'ppocrv5_mobile_dict_devanagari.txt'
  },
  {
    id: 'ppocr-v5-mobile-ta',
    name: 'PP-OCRv5 Mobile Tamil',
    language: 'ta',
    recFile: 'ppocrv5_mobile_rec_ta.mnn',
    dictFile: 'ppocrv5_mobile_dict_ta.txt'
  },
  {
    id: 'ppocr-v5-mobile-te',
    name: 'PP-OCRv5 Mobile Telugu',
    language: 'te',
    recFile: 'ppocrv5_mobile_rec_te.mnn',
    dictFile: 'ppocrv5_mobile_dict_te.txt'
  },
  {
    id: 'ppocr-v5-mobile-th',
    name: 'PP-OCRv5 Mobile Thai',
    language: 'th',
    recFile: 'ppocrv5_mobile_rec_th.mnn',
    dictFile: 'ppocrv5_mobile_dict_th.txt'
  }
];

const MODEL_FILES = Array.from(new Set([
  `${MODEL_DIR}/${DET_MODEL_FILE}`,
  ...MODEL_PROFILES.flatMap((profile) => [
    `${MODEL_DIR}/${profile.recFile}`,
    `${MODEL_DIR}/${profile.dictFile}`
  ])
]));

const CURRENT_PLATFORM = process.platform === 'win32' ? 'windows'
  : process.platform === 'darwin' ? 'macos'
    : 'linux';
const CURRENT_ARCH = process.arch === 'x64' ? 'x64' : 'arm64';
const CURRENT_TARGET_KEY = `${CURRENT_PLATFORM}-${CURRENT_ARCH}`;

const args = process.argv.slice(2);
const isRelease = args.includes('--release');
const shouldPackage = args.includes('--package');
const mode = isRelease ? 'release' : 'debug';
const targetArg = args.find((arg) => arg.startsWith('--target='));
const targetPlatform = targetArg ? targetArg.split('=')[1] : CURRENT_TARGET_KEY;

console.log(`构建 Sidecar 插件: ${PLUGIN_NAME}`);
console.log(`模式: ${mode}`);
console.log(`目标平台: ${targetPlatform}`);
console.log('');

function removeIfExists(targetPath) {
  if (fs.existsSync(targetPath)) {
    fs.rmSync(targetPath, { recursive: true, force: true });
  }
}

function ensureDir(targetPath) {
  fs.mkdirSync(targetPath, { recursive: true });
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf-8'));
}

function writeJson(filePath, data) {
  fs.writeFileSync(filePath, `${JSON.stringify(data, null, 2)}\n`);
}

function run(command) {
  execSync(command, {
    stdio: 'inherit',
    cwd: __dirname
  });
}

function buildVueComponent() {
  console.log('构建 Vue 管理页...');
  run('bun x vite build');
  console.log('Vue 管理页构建完成');
}

function buildTarget(targetKey) {
  const target = TARGETS[targetKey];
  if (!target) {
    console.error(`不支持的目标平台: ${targetKey}`);
    console.error(`当前首版支持: ${Object.keys(TARGETS).join(', ')}`);
    process.exit(1);
  }

  const buildCmd = isRelease
    ? `cargo build --release --target ${target.rustTarget}`
    : `cargo build --target ${target.rustTarget}`;

  console.log(`构建 Rust sidecar: ${targetKey}`);
  run(`rustup target add ${target.rustTarget}`);
  run(buildCmd);
  console.log(`${targetKey} 构建完成`);
}

function validateModelFiles() {
  const missing = [];
  const empty = [];
  const invalid = [];

  for (const relativePath of MODEL_FILES) {
    const fullPath = path.join(__dirname, relativePath);
    if (!fs.existsSync(fullPath)) {
      missing.push(relativePath);
      continue;
    }

    const stats = fs.statSync(fullPath);
    if (!stats.isFile() || stats.size === 0) {
      empty.push(relativePath);
      continue;
    }

    if (relativePath.endsWith('.mnn') && looksLikeSafetensors(fullPath)) {
      invalid.push(relativePath);
    }
  }

  if (missing.length > 0 || empty.length > 0 || invalid.length > 0) {
    console.error('模型文件校验失败，无法打包发布 ZIP。');
    if (missing.length > 0) {
      console.error(`缺失: ${missing.join(', ')}`);
    }
    if (empty.length > 0) {
      console.error(`空文件: ${empty.join(', ')}`);
    }
    if (invalid.length > 0) {
      console.error(`格式错误，疑似 safetensors 而不是 MNN: ${invalid.join(', ')}`);
      console.error('请先把 PaddleOCR 模型转换为真正的 MNN，再放入模型目录。');
    }
    process.exit(1);
  }
}

function looksLikeSafetensors(fullPath) {
  const fd = fs.openSync(fullPath, 'r');
  try {
    const buffer = Buffer.alloc(128);
    const readLen = fs.readSync(fd, buffer, 0, buffer.length, 0);
    return readLen >= 16
      && buffer[8] === 0x7b
      && buffer.subarray(9, readLen).includes(Buffer.from('"dtype"'));
  } finally {
    fs.closeSync(fd);
  }
}

function packagePlugin() {
  if (shouldPackage) {
    validateModelFiles();
  }

  console.log('');
  console.log('打包插件目录...');

  const target = TARGETS[targetPlatform];
  const distDir = path.join(__dirname, 'dist');
  const binDir = path.join(distDir, 'bin');
  const modelDestDir = path.join(distDir, MODEL_DIR);

  ensureDir(binDir);
  ensureDir(modelDestDir);

  const binaryPath = path.join(
    __dirname,
    'target',
    target.rustTarget,
    mode,
    target.executable
  );

  if (!fs.existsSync(binaryPath)) {
    console.error(`找不到构建产物: ${binaryPath}`);
    process.exit(1);
  }

  fs.copyFileSync(binaryPath, path.join(binDir, target.packageExecutable));
  console.log(`复制 sidecar -> bin/${target.packageExecutable}`);

  if (shouldPackage) {
    for (const relativePath of MODEL_FILES) {
      const fileName = path.basename(relativePath);
      fs.copyFileSync(path.join(__dirname, relativePath), path.join(modelDestDir, fileName));
      console.log(`复制模型 -> ${MODEL_DIR}/${fileName}`);
    }
  }

  const manifest = readJson(path.join(__dirname, 'manifest.json'));
  manifest.sidecar.executable = {
    [target.manifestKey]: `bin/${target.packageExecutable}`
  };

  if (manifest.ui?.component) {
    const componentBaseName = path.basename(manifest.ui.component, '.vue');
    const componentJsName = `${componentBaseName}.js`;
    const componentJsPath = path.join(distDir, componentJsName);
    if (!fs.existsSync(componentJsPath)) {
      console.error(`找不到编译后的组件: ${componentJsName}`);
      process.exit(1);
    }
    manifest.ui.component = componentJsName;
  }

  writeJson(path.join(distDir, 'manifest.json'), manifest);

  const readmePath = path.join(__dirname, 'README.md');
  if (fs.existsSync(readmePath)) {
    fs.copyFileSync(readmePath, path.join(distDir, 'README.md'));
  }

  console.log(`插件目录已输出: ${distDir}`);
  return distDir;
}

async function createZipArchive(distDir) {
  console.log('');
  console.log('创建 ZIP 发布包...');

  const manifest = readJson(path.join(distDir, 'manifest.json'));
  const zipFileName = `${manifest.id}-v${manifest.version}-${targetPlatform}.zip`;
  const zipPath = path.join(__dirname, zipFileName);

  removeIfExists(zipPath);

  return new Promise((resolve, reject) => {
    const output = fs.createWriteStream(zipPath);
    const archive = archiver('zip', { zlib: { level: 9 } });

    output.on('close', () => {
      const sizeInMB = (archive.pointer() / 1024 / 1024).toFixed(2);
      console.log(`ZIP 大小: ${sizeInMB} MB`);
      console.log(`发布包已创建: ${zipPath}`);
      resolve(zipPath);
    });

    archive.on('error', reject);
    archive.pipe(output);
    archive.directory(distDir, false);
    archive.finalize();
  });
}

async function main() {
  console.log('清理旧产物...');
  removeIfExists(path.join(__dirname, 'dist'));
  removeIfExists(path.join(__dirname, 'dist-ui'));
  for (const file of fs.readdirSync(__dirname)) {
    if (/^paddle-ocr-v.+\.zip$/.test(file)) {
      removeIfExists(path.join(__dirname, file));
    }
  }
  console.log('清理完成');
  console.log('');

  buildVueComponent();
  buildTarget(targetPlatform);

  if (shouldPackage) {
    const distDir = packagePlugin();
    await createZipArchive(distDir);
  } else {
    console.log('构建完成。如需发布 ZIP，请运行 bun run package。');
  }
}

main().catch((error) => {
  console.error('构建失败:', error);
  process.exit(1);
});
