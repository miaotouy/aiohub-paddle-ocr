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
  },
  'windows-arm64': {
    rustTarget: 'aarch64-pc-windows-msvc',
    executable: 'aiohub-paddle-ocr.exe',
    manifestKey: 'win32-arm64',
    packageExecutable: 'aiohub-paddle-ocr-windows-arm64.exe'
  },
  'macos-x64': {
    rustTarget: 'x86_64-apple-darwin',
    executable: 'aiohub-paddle-ocr',
    manifestKey: 'darwin-x64',
    packageExecutable: 'aiohub-paddle-ocr-macos-x64'
  },
  'macos-arm64': {
    rustTarget: 'aarch64-apple-darwin',
    executable: 'aiohub-paddle-ocr',
    manifestKey: 'darwin-arm64',
    packageExecutable: 'aiohub-paddle-ocr-macos-arm64'
  },
  'linux-x64': {
    rustTarget: 'x86_64-unknown-linux-gnu',
    executable: 'aiohub-paddle-ocr',
    manifestKey: 'linux-x64',
    packageExecutable: 'aiohub-paddle-ocr-linux-x64'
  },
  'linux-arm64': {
    rustTarget: 'aarch64-unknown-linux-gnu',
    executable: 'aiohub-paddle-ocr',
    manifestKey: 'linux-arm64',
    packageExecutable: 'aiohub-paddle-ocr-linux-arm64'
  }
};

const MODEL_REGISTRY_FILE = 'models/registry.json';
const MODEL_REGISTRY = readJson(path.join(__dirname, MODEL_REGISTRY_FILE));
const MODEL_PROFILES = MODEL_REGISTRY.profiles || [];
const PACKAGE_MODEL_PROFILES = MODEL_PROFILES.filter((profile) => (
  profile.package !== false && profile.builtIn !== false
));

const MODEL_FILES = Array.from(new Set(
  PACKAGE_MODEL_PROFILES.flatMap((profile) => getRequiredModelFiles(profile))
));

const CURRENT_PLATFORM = process.platform === 'win32' ? 'windows'
  : process.platform === 'darwin' ? 'macos'
    : 'linux';
const CURRENT_ARCH = process.arch === 'x64' ? 'x64' : 'arm64';
const CURRENT_TARGET_KEY = `${CURRENT_PLATFORM}-${CURRENT_ARCH}`;

const VARIANTS = {
  'windows-x64': ['cpu', 'gpu-cuda12', 'gpu-cuda13'],
  'windows-arm64': ['cpu'],
  'macos-x64': ['cpu'],
  'macos-arm64': ['cpu'],
  'linux-x64': ['cpu', 'gpu-cuda12', 'gpu-cuda13'],
  'linux-arm64': ['cpu']
};

const args = process.argv.slice(2);
const isRelease = args.includes('--release');
const shouldPackage = args.includes('--package');
const mode = isRelease ? 'release' : 'debug';
const targetArg = args.find((arg) => arg.startsWith('--target='));
const targetPlatform = targetArg ? targetArg.split('=')[1] : CURRENT_TARGET_KEY;

const variantArg = args.find((arg) => arg.startsWith('--variant='));
const specifiedVariant = variantArg ? variantArg.split('=')[1] : null;

console.log(`构建 Sidecar 插件: ${PLUGIN_NAME}`);
console.log(`模式: ${mode}`);
console.log(`目标平台: ${targetPlatform}`);
if (specifiedVariant) {
  console.log(`指定变体: ${specifiedVariant}`);
}
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

function registryPath(...segments) {
  return path.posix.join(
    ...segments
      .filter(Boolean)
      .map((segment) => String(segment).replace(/\\/g, '/'))
  );
}

function requireProfileFile(profile, fieldName) {
  const value = profile[fieldName];
  if (typeof value !== 'string' || value.trim() === '') {
    throw new Error(`模型 registry 中的 ${profile.id} 缺少 ${fieldName}`);
  }
  return value;
}

function getOnnxRuntimeLibraryFiles(platform, variant) {
  const isGpuVariant = variant && variant.startsWith('gpu');
  const platformMap = {
    'windows-x64': {
      dir: `windows-x64-${variant}`,
      files: isGpuVariant
        ? ['onnxruntime.dll', 'onnxruntime_providers_cuda.dll', 'onnxruntime_providers_shared.dll']
        : ['onnxruntime.dll']
    },
    'windows-arm64': { dir: 'windows-arm64-cpu', files: ['onnxruntime.dll'] },
    'macos-x64': { dir: 'macos-x64-cpu', files: ['libonnxruntime.dylib'] },
    'macos-arm64': { dir: 'macos-arm64-cpu', files: ['libonnxruntime.dylib'] },
    'linux-x64': {
      dir: `linux-x64-${variant}`,
      files: isGpuVariant
        ? ['libonnxruntime.so', 'libonnxruntime_providers_cuda.so', 'libonnxruntime_providers_shared.so']
        : ['libonnxruntime.so']
    },
    'linux-arm64': { dir: 'linux-arm64-cpu', files: ['libonnxruntime.so'] }
  };
  const info = platformMap[platform];
  if (!info) return [];
  return info.files.map(file => `runtime/onnxruntime/${info.dir}/${file}`);
}

function checkOnnxRuntimeLibrary(platform, variant) {
  const files = getOnnxRuntimeLibraryFiles(platform, variant);
  const missingFiles = files.filter(file => !fs.existsSync(path.join(__dirname, file)));
  
  if (missingFiles.length > 0) {
    console.error(`\n错误: 缺失 ONNX Runtime [${platform}][${variant}] 依赖库:`);
    for (const file of missingFiles) {
      console.error(`  - ${file}`);
    }
    console.error(`请先运行下载脚本下载依赖库:`);
    console.error(`  bun run download-ort --variant=${variant}`);
    process.exit(1);
  }
  
  console.log(`ONNX Runtime [${platform}][${variant}] 依赖库已完整。`);
}


function getRequiredModelFiles(profile) {
  const modelDir = requireProfileFile(profile, 'modelDir');

  if (profile.backend === 'mnn-ocr-rs') {
    return [
      registryPath(modelDir, requireProfileFile(profile, 'detModel')),
      registryPath(modelDir, requireProfileFile(profile, 'recModel')),
      registryPath(modelDir, requireProfileFile(profile, 'dict'))
    ];
  }

  if (profile.backend === 'onnxruntime') {
    const files = [
      registryPath(modelDir, requireProfileFile(profile, 'detOnnx')),
      registryPath(modelDir, requireProfileFile(profile, 'recOnnx')),
      registryPath(modelDir, requireProfileFile(profile, 'detConfig')),
      registryPath(modelDir, requireProfileFile(profile, 'recConfig')),
      registryPath(modelDir, requireProfileFile(profile, 'detConfig').replace(/\.yml$/, '.json')),
      registryPath(modelDir, requireProfileFile(profile, 'recConfig').replace(/\.yml$/, '.json'))
    ];
    if (profile.dict) {
      files.push(registryPath(modelDir, profile.dict));
    }
    return files;
  }

  throw new Error(`不支持的模型 backend: ${profile.backend}`);
}

function copyFilePreservingPath(relativePath, distDir) {
  const sourcePath = path.join(__dirname, relativePath);
  const targetPath = path.join(distDir, relativePath);
  ensureDir(path.dirname(targetPath));
  fs.copyFileSync(sourcePath, targetPath);
}

function copyIfExistsPreservingPath(relativePath, distDir) {
  const sourcePath = path.join(__dirname, relativePath);
  if (fs.existsSync(sourcePath)) {
    copyFilePreservingPath(relativePath, distDir);
    return true;
  }
  return false;
}

function copyModelMetadata(distDir) {
  copyFilePreservingPath(MODEL_REGISTRY_FILE, distDir);
  console.log(`复制模型清单 -> ${MODEL_REGISTRY_FILE}`);

  const readmeFiles = Array.from(new Set(
    MODEL_PROFILES
      .filter((profile) => typeof profile.modelDir === 'string')
      .map((profile) => registryPath(profile.modelDir, 'README.md'))
  ));

  for (const readmeFile of readmeFiles) {
    if (copyIfExistsPreservingPath(readmeFile, distDir)) {
      console.log(`复制模型说明 -> ${readmeFile}`);
    }
  }
}

function toManifestModelProfile(profile) {
  const result = {
    id: profile.id,
    name: profile.name,
    language: profile.language,
    backend: profile.backend
  };

  for (const key of ['family', 'tier', 'experimental', 'sourceUrl', 'revision', 'license']) {
    if (profile[key] !== undefined) {
      result[key] = profile[key];
    }
  }

  return result;
}

function applyRegistryToManifest(manifest) {
  for (const contribution of manifest.contributions || []) {
    if (contribution.type === 'ocr-engine') {
      contribution.modelProfiles = MODEL_PROFILES.map(toManifestModelProfile);
      contribution.defaultModelProfile = MODEL_REGISTRY.defaultProfile;
    }
  }
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
    console.error(`当前支持: ${Object.keys(TARGETS).join(', ')}`);
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

function packagePlugin(variant) {
  if (shouldPackage) {
    validateModelFiles();
  }

  console.log('');
  console.log(`打包插件目录 [${variant}]...`);

  const target = TARGETS[targetPlatform];
  const distDir = path.join(__dirname, 'dist');
  const binDir = path.join(distDir, 'bin');

  // 仅清理并重建 bin 目录，不清理整个 dist 目录，以保留 Vue 编译产物
  removeIfExists(binDir);
  ensureDir(binDir);

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

  // 清理并复制 ONNX Runtime 动态库
  const ortDestPlatformDir = path.join(distDir, 'runtime', 'onnxruntime', targetPlatform);
  removeIfExists(ortDestPlatformDir);

  const ortLibPaths = getOnnxRuntimeLibraryFiles(targetPlatform, variant);
  for (const ortLibPath of ortLibPaths) {
    const cachePath = path.join(__dirname, ortLibPath);
    const standardRelativePath = ortLibPath.replace(`${targetPlatform}-${variant}`, targetPlatform);
    const destPath = path.join(distDir, standardRelativePath);

    if (fs.existsSync(cachePath)) {
      ensureDir(path.dirname(destPath));
      fs.copyFileSync(cachePath, destPath);
      console.log(`复制 ONNX Runtime 动态库 -> ${standardRelativePath}`);
    } else {
      if (ortLibPath.endsWith('onnxruntime.dll') || ortLibPath.endsWith('libonnxruntime.dylib') || ortLibPath.endsWith('libonnxruntime.so')) {
        console.warn(`警告: 未找到 ONNX Runtime 动态库 -> ${ortLibPath}`);
      }
    }
  }

  if (shouldPackage) {
    copyModelMetadata(distDir);
    for (const relativePath of MODEL_FILES) {
      copyFilePreservingPath(relativePath, distDir);
      console.log(`复制模型 -> ${relativePath}`);
    }
  }

  const manifest = readJson(path.join(__dirname, 'manifest.json'));
  applyRegistryToManifest(manifest);
  manifest.sidecar.executable = {
    [target.manifestKey]: `bin/${target.packageExecutable}`
  };

  if (manifest.ui?.component) {
    const componentBaseName = path.basename(manifest.ui.component, '.vue');
    const componentJsName = `${componentBaseName}.js`;
    const fallbackPath = path.join(distDir, componentJsName);
    if (!fs.existsSync(fallbackPath)) {
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

async function createZipArchive(distDir, variant) {
  console.log('');
  console.log(`创建 ZIP 发布包 [${variant}]...`);

  const manifest = readJson(path.join(distDir, 'manifest.json'));
  const zipFileName = `${manifest.id}-v${manifest.version}-${targetPlatform}-${variant}.zip`;
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
    if (/^paddle-ocr-v.+\.zip$/.test(file) || /^aiohub-paddle-ocr-v.+\.zip$/.test(file)) {
      removeIfExists(path.join(__dirname, file));
    }
  }
  console.log('清理完成');
  console.log('');

  // 1. 编译 Vue 管理页
  buildVueComponent();

  // 2. 编译 Rust sidecar (只需编译一次)
  buildTarget(targetPlatform);

  // 3. 确定要打包/部署的变体
  if (shouldPackage) {
    let variantsToPackage = [];
    if (specifiedVariant === 'all' || !specifiedVariant) {
      variantsToPackage = VARIANTS[targetPlatform] || ['cpu'];
    } else {
      variantsToPackage = [specifiedVariant];
    }

    console.log(`准备打包以下变体: ${variantsToPackage.join(', ')}`);

    for (const variant of variantsToPackage) {
      console.log(`\n--- 开始打包变体: ${variant} ---`);
      checkOnnxRuntimeLibrary(targetPlatform, variant);
      const distDir = packagePlugin(variant);
      await createZipArchive(distDir, variant);
    }
  } else {
    let localVariant = specifiedVariant || 'cpu';

    console.log(`本地开发环境将使用变体: ${localVariant}`);
    checkOnnxRuntimeLibrary(targetPlatform, localVariant);

    console.log(`正在部署变体 [${localVariant}] 到本地开发运行目录...`);
    const cacheFiles = getOnnxRuntimeLibraryFiles(targetPlatform, localVariant);
    const devDir = path.join(__dirname, 'runtime', 'onnxruntime', targetPlatform);
    ensureDir(devDir);

    for (const cacheFile of cacheFiles) {
      const src = path.join(__dirname, cacheFile);
      const dest = path.join(devDir, path.basename(cacheFile));
      if (fs.existsSync(src)) {
        fs.copyFileSync(src, dest);
        console.log(`已部署本地调试 DLL: ${path.basename(cacheFile)}`);
      }
    }

    console.log('\n构建完成。如需发布 ZIP，请运行 bun run package。');
  }
}

main().catch((error) => {
  console.error('构建失败:', error);
  process.exit(1);
});
