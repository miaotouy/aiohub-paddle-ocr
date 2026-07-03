/**
 * ONNX Runtime 依赖库下载脚本
 */

import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const ORT_VERSION = '1.27.0';

const TARGETS = {
  'windows-x64': {
    cpu: {
      archiveName: `onnxruntime-win-x64-${ORT_VERSION}`,
      files: ['onnxruntime.dll']
    },
    'gpu-cuda12': {
      archiveName: `onnxruntime-win-x64-gpu_cuda12-${ORT_VERSION}`,
      files: ['onnxruntime.dll', 'onnxruntime_providers_cuda.dll', 'onnxruntime_providers_shared.dll']
    },
    'gpu-cuda13': {
      archiveName: `onnxruntime-win-x64-gpu_cuda13-${ORT_VERSION}`,
      files: ['onnxruntime.dll', 'onnxruntime_providers_cuda.dll', 'onnxruntime_providers_shared.dll']
    }
  }
};

// 尝试读取本地的 .env 文件，避免硬编码代理
const envPath = path.join(__dirname, '.env');
if (fs.existsSync(envPath)) {
  const envContent = fs.readFileSync(envPath, 'utf-8');
  for (const line of envContent.split('\n')) {
    const trimmed = line.trim();
    if (trimmed && !trimmed.startsWith('#')) {
      const [key, ...valueParts] = trimmed.split('=');
      const value = valueParts.join('=').trim();
      if (key && value) {
        // 去除可能存在的引号
        const cleanValue = value.replace(/^['"]|['"]$/g, '');
        process.env[key.trim()] = cleanValue;
      }
    }
  }
}

const args = process.argv.slice(2);
const variantArg = args.find((arg) => arg.startsWith('--variant='));
const variant = variantArg ? variantArg.split('=')[1] : 'cpu';
const platform = 'windows-x64'; // 目前仅支持 windows-x64 自动下载

function ensureDir(targetPath) {
  fs.mkdirSync(targetPath, { recursive: true });
}

function removeIfExists(targetPath) {
  if (fs.existsSync(targetPath)) {
    fs.rmSync(targetPath, { recursive: true, force: true });
  }
}

async function main() {
  const platformTargets = TARGETS[platform];
  if (!platformTargets) {
    console.error(`不支持的平台: ${platform}`);
    process.exit(1);
  }

  const config = platformTargets[variant];
  if (!config) {
    console.error(`不支持的变体: ${variant}。可选值: ${Object.keys(platformTargets).join(', ')}`);
    process.exit(1);
  }

  console.log(`开始下载 ONNX Runtime [${platform}][${variant}] 依赖库...`);

  const runtimeDir = path.join(__dirname, 'runtime');
  const tempZip = path.join(runtimeDir, `ort-temp-${platform}-${variant}.zip`);
  const tempExtract = path.join(runtimeDir, `ort-temp-extract-${platform}-${variant}`);

  ensureDir(runtimeDir);

  const url = `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/${config.archiveName}.zip`;
  console.log(`下载地址: ${url}`);

  try {
    // 仅读取标准的系统环境变量，不硬编码任何代理端口
    const proxy = process.env.HTTP_PROXY || process.env.HTTPS_PROXY || process.env.http_proxy || process.env.https_proxy;
    const proxyArg = proxy ? `-x "${proxy}"` : '';
    
    if (proxy) {
      console.log(`使用代理下载: ${proxy}`);
    } else {
      console.log('直连下载（无代理）...');
    }

    if (process.platform === 'win32') {
      execSync(`curl.exe ${proxyArg} -L -o "${tempZip}" "${url}"`, { stdio: 'inherit' });
    } else {
      execSync(`curl ${proxyArg} -L -o "${tempZip}" "${url}"`, { stdio: 'inherit' });
    }
    console.log('下载完成，正在解压...');
  } catch (downloadError) {
    console.error(`下载 ONNX Runtime 失败: ${downloadError.message}`);
    process.exit(1);
  }

  if (fs.existsSync(tempExtract)) {
    fs.rmSync(tempExtract, { recursive: true, force: true });
  }
  ensureDir(tempExtract);

  try {
    if (process.platform === 'win32') {
      execSync(`powershell -Command "Expand-Archive -Path '${tempZip}' -DestinationPath '${tempExtract}' -Force"`);
    } else {
      execSync(`unzip -q "${tempZip}" -d "${tempExtract}"`);
    }
  } catch (extractError) {
    console.error(`解压失败: ${extractError.message}`);
    removeIfExists(tempZip);
    removeIfExists(tempExtract);
    process.exit(1);
  }

  console.log('正在整理并复制所需的 DLL 文件...');
  const libDir = path.join(tempExtract, config.archiveName, 'lib');
  const targetPlatformDir = path.join(runtimeDir, 'onnxruntime', `${platform}-${variant}`);
  ensureDir(targetPlatformDir);

  for (const file of config.files) {
    const src = path.join(libDir, file);
    const dest = path.join(targetPlatformDir, file);
    if (fs.existsSync(src)) {
      fs.copyFileSync(src, dest);
      console.log(`已部署到缓存: ${file} -> ${dest}`);
    } else {
      console.warn(`警告: 未在解压目录中找到 ${file}`);
    }
  }

  console.log('清理临时文件...');
  removeIfExists(tempZip);
  removeIfExists(tempExtract);
  console.log(`ONNX Runtime [${platform}][${variant}] 依赖库部署成功！`);
}

main().catch((error) => {
  console.error('下载失败:', error);
  process.exit(1);
});