// postinstall script for the hotpot npm package.
// Downloads the correct platform-specific Rust binary from GitHub Releases,
// extracts it, and places it in the package's bin/ directory.
// npm postinstall 脚本：从 GitHub Releases 下载当前平台对应的 Rust 二进制，
// 解压并放入包的 bin/ 目录。

"use strict";

const fs = require("fs");
const path = require("path");
const https = require("https");
const http = require("http");
const zlib = require("zlib");
const { spawnSync } = require("child_process");

// Package version determines the GitHub Release tag.
// The tag format is "hotpot-v<version>".
// 包版本决定 GitHub Release tag，tag 格式为 "hotpot-v<version>"。
const pkg = require("../package.json");
const TAG = `hotpot-v${pkg.version}`;

// Owner, repo, and base URL for GitHub Releases.
// GitHub Releases 的 owner、repo 和基础 URL。
const OWNER = "fancyhq";
const REPO = "hotpot";

// Map Node's process.platform + process.arch to our release asset labels.
// 将 Node 的 process.platform + process.arch 映射到 release asset label。
function getAssetLabel() {
  const platform = process.platform;
  const arch = process.arch;

  if (platform === "linux" && arch === "x64") {
    return "linux-x86_64";
  }
  if (platform === "linux" && arch === "arm64") {
    return "linux-aarch64";
  }
  if (platform === "darwin" && arch === "x64") {
    return "macos-x86_64";
  }
  if (platform === "darwin" && arch === "arm64") {
    return "macos-aarch64";
  }
  if (platform === "win32" && arch === "x64") {
    return "windows-x86_64";
  }

  console.error(
    `Error: unsupported platform "${platform}-${arch}".\n` +
      `hotpot provides prebuilt binaries for:\n` +
      `  - Linux x86_64 / aarch64\n` +
      `  - macOS x86_64 / aarch64\n` +
      `  - Windows x86_64\n` +
      `To build from source, visit https://github.com/${OWNER}/${REPO}`
  );
  process.exit(1);
}

// Determine archive extension: .tar.gz for Unix, .zip for Windows.
// 确定压缩包扩展名：Unix 用 .tar.gz，Windows 用 .zip。
function getArchiveExt(label) {
  return label.startsWith("windows") ? ".zip" : ".tar.gz";
}

// Construct the release asset filename.
// Format: hotpot-${TAG}-${ASSET_LABEL}${EXT}
// Example: hotpot-hotpot-v0.3.1-linux-x86_64.tar.gz
// 构造 release asset 文件名。
function getAssetFilename(assetLabel) {
  const ext = getArchiveExt(assetLabel);
  return `hotpot-${TAG}-${assetLabel}${ext}`;
}

// Build the download URL for the release asset.
// 构造 release asset 的下载 URL。
function getDownloadUrl(assetFilename) {
  return `https://github.com/${OWNER}/${REPO}/releases/download/${TAG}/${assetFilename}`;
}

// Determine the directory where the native binary will be placed.
// 确定原生二进制的存放目录。
function getBinDir() {
  return path.resolve(__dirname, "..", "bin");
}

// Determine the native binary name (hotpot.exe on Windows, hotpot elsewhere).
// 确定原生二进制的文件名（Windows 为 hotpot.exe，其他为 hotpot）。
function getBinaryName() {
  return process.platform === "win32" ? "hotpot.exe" : "hotpot";
}

// Download a file from a URL and return its content as a Buffer.
// Returns a Promise that resolves with the Buffer.
// 从 URL 下载文件并返回 Buffer（Promise）。
function download(url) {
  return new Promise((resolve, reject) => {
    const protocol = url.startsWith("https:") ? https : http;

    protocol.get(url, (response) => {
      // Handle redirects (GitHub may redirect downloads).
      if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        download(response.headers.location).then(resolve).catch(reject);
        response.resume(); // Drain the response.
        return;
      }

      if (response.statusCode !== 200) {
        reject(
          new Error(
            `Download failed: HTTP ${response.statusCode} ${response.statusMessage} for ${url}`
          )
        );
        return;
      }

      const chunks = [];
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("end", () => resolve(Buffer.concat(chunks)));
      response.on("error", reject);
    }).on("error", reject);
  });
}

// Extract a .tar.gz archive buffer into the specified directory.
// Uses Node built-in zlib and tar (via child process tar command on Unix,
// or a manual approach). Since the archive contains a single binary at the
// root with no nesting, we use the system tar on Unix and a pure-Node
// approach for .zip on Windows via the system PowerShell.
// 将 .tar.gz 压缩包缓冲区解压到指定目录。
function extractTarGz(buffer, outputDir) {
  // Use system tar command (available on macOS and Linux).
  // Use stdin piping to avoid writing the archive to disk.
  const tar = spawnSync("tar", ["xzf", "-", "-C", outputDir], {
    input: buffer,
    stdio: ["pipe", "inherit", "inherit"],
  });

  if (tar.error) {
    throw new Error(
      `Failed to extract tar.gz: ${tar.error.message}. ` +
        "Ensure the 'tar' command is available on your system."
    );
  }
  if (tar.status !== 0) {
    throw new Error(
      `Failed to extract tar.gz: tar exited with code ${tar.status}.`
    );
  }
}

// Extract a .zip archive buffer into the specified directory.
// On Windows, use PowerShell's Expand-Archive.
// 将 .zip 压缩包缓冲区解压到指定目录（Windows 用 PowerShell）。
function extractZip(buffer, outputDir) {
  // Write the zip to a temp file, then use PowerShell to extract it.
  const tmpDir = fs.mkdtempSync(path.join(require("os").tmpdir(), "hotpot-install-"));
  const zipPath = path.join(tmpDir, "archive.zip");

  try {
    fs.writeFileSync(zipPath, buffer);

    const ps = spawnSync(
      "powershell",
      [
        "-NoProfile",
        "-Command",
        `Expand-Archive -Path "${zipPath}" -DestinationPath "${outputDir}" -Force`,
      ],
      { stdio: ["pipe", "inherit", "inherit"] }
    );

    if (ps.error) {
      throw new Error(
        `Failed to extract zip: ${ps.error.message}. ` +
          "Ensure PowerShell is available on your system."
      );
    }
    if (ps.status !== 0) {
      throw new Error(
        `Failed to extract zip: Expand-Archive exited with code ${ps.status}.`
      );
    }
  } finally {
    // Clean up the temp file and directory.
    // 清理临时文件和目录。
    try {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    } catch {
      // Ignore cleanup errors.
    }
  }
}

// Set the executable permission on the binary (Unix only).
// Windows does not need a chmod equivalent.
// 为二进制文件设置可执行权限（仅 Unix）。
function setExecutable(filePath) {
  if (process.platform !== "win32") {
    fs.chmodSync(filePath, 0o755);
  }
}

async function main() {
  const assetLabel = getAssetLabel();
  const binDir = getBinDir();
  const binaryName = getBinaryName();
  const binaryPath = path.join(binDir, binaryName);

  // Ensure bin/ directory exists.
  // 确保 bin/ 目录存在。
  fs.mkdirSync(binDir, { recursive: true });

  // Check if the binary already exists and skip download.
  // This helps when the package is reinstalled or when the postinstall
  // script is re-run.
  // 检查二进制是否已存在，跳过下载（帮助重新安装场景）。
  if (fs.existsSync(binaryPath)) {
    console.log(`hotpot binary already exists at ${binaryPath}, skipping download.`);
    return;
  }

  const assetFilename = getAssetFilename(assetLabel);
  const downloadUrl = getDownloadUrl(assetFilename);

  console.log(`Downloading hotpot binary for ${assetLabel}...`);
  console.log(`  From: ${downloadUrl}`);

  let archiveBuffer;
  try {
    archiveBuffer = await download(downloadUrl);
  } catch (err) {
    console.error(
      `Error: failed to download hotpot binary for ${assetLabel}.\n` +
        `  URL: ${downloadUrl}\n` +
        `  Reason: ${err.message}\n\n` +
        `Possible causes:\n` +
        `  - Network connectivity issue or proxy requirement.\n` +
        `  - GitHub is not accessible from your current network.\n` +
        `  - The release tag "${TAG}" does not exist.\n` +
        `  - The asset "${assetFilename}" does not exist for this release.\n\n` +
        `Check existing releases at: https://github.com/${OWNER}/${REPO}/releases`
    );
    process.exit(1);
  }

  console.log(`Extracting ${assetFilename}...`);

  try {
    if (assetLabel.startsWith("windows")) {
      extractZip(archiveBuffer, binDir);
    } else {
      extractTarGz(archiveBuffer, binDir);
    }
  } catch (err) {
    console.error(`Error: failed to extract archive: ${err.message}`);
    process.exit(1);
  }

  if (!fs.existsSync(binaryPath)) {
    console.error(
      `Error: extracted archive does not contain the expected binary "${binaryName}".\n` +
        `  Expected path: ${binaryPath}\n` +
        `  The archive may have a different internal structure than expected.`
    );
    process.exit(1);
  }

  // Set executable permission on Unix.
  // Unix 上设置可执行权限。
  setExecutable(binaryPath);

  console.log(`hotpot binary installed successfully at ${binaryPath}`);
}

// Export pure helper functions for deterministic testing (no network I/O).
// 导出纯辅助函数供确定性的测试使用（无网络 I/O）。
module.exports = {
  setExecutable,
  getAssetLabel,
  getAssetFilename,
  getBinaryName,
  getBinDir,
  getDownloadUrl,
  getArchiveExt,
};

// Guard main() so that requiring this module for tests does not
// trigger network I/O.
// 保护 main()，使得测试中 require 本模块不会触发网络 I/O。
if (require.main === module) {
  main().catch((err) => {
    console.error(`Error: hotpot installation failed: ${err.message}`);
    process.exit(1);
  });
}
