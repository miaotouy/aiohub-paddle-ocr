/**
 * PP-OCRv6 Small ONNX 模型下载脚本
 *
 * 从 Hugging Face 官方仓库下载 PP-OCRv6 Small ONNX 模型文件。
 * 使用方式：bun run download-v6-models.js
 *
 * 代理设置：默认读取 HTTP_PROXY / HTTPS_PROXY 环境变量
 * 示例：$env:HTTP_PROXY="http://127.0.0.1:7897"; $env:HTTPS_PROXY="http://127.0.0.1:7897"; bun run download-v6-models.js
 */

import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

const MODEL_BASE_DIR = join(__dirname, "models", "ppocr-v6-small-onnx");
const DET_REVISION = "28fe5895c24fd108c19eb3e8479f4ab385fbfc62";
const REC_REVISION = "b8f84f0b80c529de40b4fbb3544b84fa7233a513";

const FILES_TO_DOWNLOAD = [
  {
    url: `https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_onnx/resolve/${DET_REVISION}/inference.onnx`,
    path: join(MODEL_BASE_DIR, "det", "inference.onnx"),
    sha256: "d73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e",
  },
  {
    url: `https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_onnx/resolve/${DET_REVISION}/inference.yml`,
    path: join(MODEL_BASE_DIR, "det", "inference.yml"),
    sha256: "193f435274bf9f0b5f71a929bbfbcf148282df7e633b34e7c373e8f44741b516",
  },
  {
    url: `https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_onnx/resolve/${DET_REVISION}/inference.json`,
    path: join(MODEL_BASE_DIR, "det", "inference.json"),
    sha256: "89240f689a4a77aad75ef55a8df0a15c8e1d4980a327d17e58f24bbadde5aeab",
  },
  {
    url: `https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_onnx/resolve/${REC_REVISION}/inference.onnx`,
    path: join(MODEL_BASE_DIR, "rec", "inference.onnx"),
    sha256: "5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634",
  },
  {
    url: `https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_onnx/resolve/${REC_REVISION}/inference.yml`,
    path: join(MODEL_BASE_DIR, "rec", "inference.yml"),
    sha256: "ab078671bb49f06228eadccd34f1bb501e157f7a047095ffb943ba81512c77d1",
  },
  {
    url: `https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_onnx/resolve/${REC_REVISION}/inference.json`,
    path: join(MODEL_BASE_DIR, "rec", "inference.json"),
    sha256: "f0bf53c853937a917affdd74467472167727f8ab0f0f7bded01c4a16c27e46e6",
  },
];

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

async function downloadFile(url, filePath) {
  console.log(`正在下载: ${url}`);
  console.log(`保存到: ${filePath}`);

  await mkdir(dirname(filePath), { recursive: true });

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

  console.log("\n下载完成，正在写入文件...");
  const buffer = Buffer.concat(chunks);
  const actualSha256 = sha256(buffer);
  if (actualSha256 !== file.sha256) {
    throw new Error(
      `SHA256 校验失败: expected ${file.sha256}, actual ${actualSha256}`
    );
  }

  await writeFile(filePath, buffer);
  console.log(
    `文件已保存: ${filePath} (${(buffer.length / 1024 / 1024).toFixed(2)}MB)\n`
  );
}

async function main() {
  console.log("=== PP-OCRv6 Small ONNX 模型下载工具 ===");
  console.log(`目标目录: ${MODEL_BASE_DIR}\n`);

  if (process.env.HTTP_PROXY || process.env.HTTPS_PROXY) {
    console.log(
      `检测到代理配置: ${process.env.HTTP_PROXY || process.env.HTTPS_PROXY}\n`
    );
  } else {
    console.log("提示: 如果下载失败，请设置代理环境变量:");
    console.log(
      '$env:HTTP_PROXY="http://127.0.0.1:7897"; $env:HTTPS_PROXY="http://127.0.0.1:7897"\n'
    );
  }

  for (const file of FILES_TO_DOWNLOAD) {
    try {
      await downloadFile(file.url, file.path);
    } catch (error) {
      console.error(`\n下载 ${file.url} 失败:`, error.message);
      process.exit(1);
    }
  }

  console.log("=== 所有模型文件下载完成 ===");
}

main().catch((error) => {
  console.error("发生错误:", error);
  process.exit(1);
});
