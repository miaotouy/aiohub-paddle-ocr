/**
 * ONNX Runtime GPU 下载与部署脚本
 *
 * 自动检测系统 CUDA 版本（CUDA 12 或 CUDA 13），
 * 从 GitHub Releases 下载对应版本的官方 ONNX Runtime GPU 压缩包，
 * 并提取所需的 DLL 文件到 runtime/onnxruntime/windows-x64 目录。
 *
 * 使用方式：bun run download-ort-gpu.js
 */

import { execSync } from "node:child_process";
import { mkdir, rm } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";

const __dirname = dirname(fileURLToPath(import.meta.url));

const ORT_VERSION = "1.27.0";
const TARGET_DIR = join(__dirname, "runtime", "onnxruntime", "windows-x64");
const TEMP_ZIP = join(__dirname, "runtime", "onnxruntime-gpu.zip");
const EXTRACT_DIR = join(__dirname, "runtime", "temp-extract");

// 自动检测系统 CUDA 版本
function detectCudaVersion() {
  console.log("正在检测系统 CUDA 环境...");
  
  // 1. 尝试通过环境变量检测
  const cudaPath = process.env.CUDA_PATH;
  if (cudaPath) {
    const match = cudaPath.match(/v(\d+)\./);
    if (match) {
      const version = parseInt(match[1], 10);
      console.log(`通过 CUDA_PATH 环境变量检测到 CUDA 版本: v${version}`);
      return version;
    }
  }

  // 2. 尝试通过 nvcc 检测
  try {
    const nvccOutput = execSync("nvcc --version", { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
    const match = nvccOutput.match(/release (\d+)\./);
    if (match) {
      const version = parseInt(match[1], 10);
      console.log(`通过 nvcc 检测到 CUDA 版本: v${version}`);
      return version;
    }
  } catch {
    // 忽略错误，继续尝试其他方式
  }

  // 3. 尝试扫描默认安装路径
  const defaultProgramFiles = "C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA";
  if (process.platform === "win32" && fs.existsSync(defaultProgramFiles)) {
    try {
      const dirs = fs.readdirSync(defaultProgramFiles);
      const versions = dirs
        .map(dir => {
          const match = dir.match(/v(\d+)\./);
          return match ? parseInt(match[1], 10) : null;
        })
        .filter(v => v !== null)
        .sort((a, b) => b - a); // 降序排列，优先使用高版本

      if (versions.length > 0) {
        console.log(`通过默认安装路径检测到 CUDA 版本: v${versions[0]}`);
        return versions[0];
      }
    } catch {
      // 忽略
    }
  }

  console.log("未检测到明确的 CUDA 版本，默认使用 CUDA 12 兼容包");
  return 12;
}

async function downloadFile(url, dest) {
  console.log(`源地址: ${url}`);
  console.log(`保存到: ${dest}`);

  // 代理设置：优先读取系统环境变量，若无则尝试使用本地 7897 端口
  const systemProxy = process.env.HTTP_PROXY || process.env.HTTPS_PROXY || process.env.http_proxy || process.env.https_proxy;
  if (!systemProxy) {
    const defaultProxy = "http://127.0.0.1:7897";
    console.log(`未检测到系统代理环境变量，尝试使用本地默认代理: ${defaultProxy}`);
    process.env.HTTP_PROXY = defaultProxy;
    process.env.HTTPS_PROXY = defaultProxy;
  } else {
    console.log(`使用系统代理: ${systemProxy}`);
  }

  const response = await fetch(url, {
    redirect: "follow",
  });

  if (!response.ok) {
    throw new Error(`下载失败: ${response.status} ${response.statusText}`);
  }

  const contentLength = response.headers.get("content-length");
  const totalBytes = contentLength ? parseInt(contentLength, 10) : 0;

  if (!response.body) {
    throw new Error("响应体为空");
  }

  const reader = response.body.getReader();
  const chunks = [];
  let receivedBytes = 0;

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    chunks.push(value);
    receivedBytes += value.length;

    if (totalBytes > 0) {
      const progress = ((receivedBytes / totalBytes) * 100).toFixed(2);
      const receivedMB = (receivedBytes / 1024 / 1024).toFixed(2);
      const totalMB = (totalBytes / 1024 / 1024).toFixed(2);
      process.stdout.write(
        `\r进度: ${progress}% (${receivedMB}MB / ${totalMB}MB)`
      );
    } else {
      const receivedMB = (receivedBytes / 1024 / 1024).toFixed(2);
      process.stdout.write(`\r已下载: ${receivedMB}MB`);
    }
  }

  console.log("\n下载完成，正在写入临时文件...");
  const buffer = Buffer.concat(chunks);
  await fs.promises.writeFile(dest, buffer);
}

async function main() {
  try {
    const cudaVersion = detectCudaVersion();
    // ONNX Runtime 1.27.0 官方提供了 cuda12 和 cuda13 的包
    const cudaTag = cudaVersion >= 13 ? "cuda13" : "cuda12";
    const archiveName = `onnxruntime-win-x64-gpu_${cudaTag}-${ORT_VERSION}`;
    const url = `https://github.com/microsoft/onnxruntime/releases/download/v${ORT_VERSION}/${archiveName}.zip`;

    console.log(`\n=== 开始部署 ONNX Runtime GPU (${cudaTag}) ===`);
    await mkdir(dirname(TEMP_ZIP), { recursive: true });
    await mkdir(TARGET_DIR, { recursive: true });

    // 1. 下载 ZIP
    await downloadFile(url, TEMP_ZIP);

    // 2. 解压 ZIP (使用 Windows 自带的 PowerShell Expand-Archive)
    console.log("正在解压文件...");
    if (fs.existsSync(EXTRACT_DIR)) {
      await rm(EXTRACT_DIR, { recursive: true, force: true });
    }
    await mkdir(EXTRACT_DIR, { recursive: true });

    // 调用 PowerShell 解压
    execSync(`powershell -Command "Expand-Archive -Path '${TEMP_ZIP}' -DestinationPath '${EXTRACT_DIR}' -Force"`, {
      stdio: "inherit"
    });

    // 3. 移动所需的 DLL 到目标目录
    console.log("正在整理 DLL 文件...");
    const extractedFolder = join(EXTRACT_DIR, archiveName);
    const libDir = join(extractedFolder, "lib");

    const filesToCopy = [
      "onnxruntime.dll",
      "onnxruntime_providers_cuda.dll",
      "onnxruntime_providers_shared.dll"
    ];

    for (const file of filesToCopy) {
      const src = join(libDir, file);
      const dest = join(TARGET_DIR, file);
      if (fs.existsSync(src)) {
        fs.copyFileSync(src, dest);
        console.log(`已复制: ${file} -> ${dest}`);
      } else {
        console.warn(`警告: 未找到文件 ${file}`);
      }
    }

    // 4. 清理临时文件
    console.log("正在清理临时文件...");
    await rm(TEMP_ZIP, { force: true });
    await rm(EXTRACT_DIR, { recursive: true, force: true });

    console.log("\n=== ONNX Runtime GPU 部署成功！ ===");
  } catch (error) {
    console.error("部署失败:", error);
    process.exit(1);
  }
}

main();