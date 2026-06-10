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

const MODEL_FILES = [
  'models/ppocr-v5-mobile/det.mnn',
  'models/ppocr-v5-mobile/rec.mnn',
  'models/ppocr-v5-mobile/keys.txt'
];

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

  for (const relativePath of MODEL_FILES) {
    const fullPath = path.join(__dirname, relativePath);
    if (!fs.existsSync(fullPath)) {
      missing.push(relativePath);
      continue;
    }

    const stats = fs.statSync(fullPath);
    if (!stats.isFile() || stats.size === 0) {
      empty.push(relativePath);
    }
  }

  if (missing.length > 0 || empty.length > 0) {
    console.error('模型文件校验失败，无法打包发布 ZIP。');
    if (missing.length > 0) {
      console.error(`缺失: ${missing.join(', ')}`);
    }
    if (empty.length > 0) {
      console.error(`空文件: ${empty.join(', ')}`);
    }
    process.exit(1);
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
  const modelDestDir = path.join(distDir, 'models', 'ppocr-v5-mobile');

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
      console.log(`复制模型 -> models/ppocr-v5-mobile/${fileName}`);
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
