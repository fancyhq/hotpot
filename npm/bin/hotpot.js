#!/usr/bin/env node

// CLI wrapper for the hotpot Rust binary.
// Runs the native binary downloaded during postinstall, forwarding all
// arguments and stdio.
// Rust 二进制 CLI wrapper：在 postinstall 阶段下载的原生二进制，
// 转发所有参数和 stdio。

"use strict";

const { spawnSync } = require("child_process");
const path = require("path");
const fs = require("fs");

// Resolve the native binary path relative to this script's location.
// The binary lives in <package>/bin/hotpot (or hotpot.exe on Windows).
// 确定相对于本脚本的原生二进制路径，位于 <package>/bin/hotpot （或 Windows 上的 hotpot.exe）。
const binDir = path.resolve(__dirname);
const binaryName = process.platform === "win32" ? "hotpot.exe" : "hotpot";
const binaryPath = path.join(binDir, binaryName);

if (!fs.existsSync(binaryPath)) {
  console.error(
    "Error: hotpot native binary not found at " +
      binaryPath +
      ".\n" +
      "The binary should have been downloaded during 'npm install -g @fancyhq/hotpot'.\n" +
      "Try reinstalling: npm install -g @fancyhq/hotpot"
  );
  process.exit(1);
}

// Forward all CLI arguments and stdio to the native binary.
// 将所有 CLI 参数和 stdio 转发给原生二进制。
const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  // Preserve the current working directory and environment.
  // 保留当前工作目录和环境变量。
  cwd: process.cwd(),
  env: process.env,
});

// Propagate the native binary's exit code and signal.
// 传递原生二进制的退出码和信号。
if (result.error) {
  console.error("Error: failed to execute hotpot binary:", result.error.message);
  process.exit(1);
}

process.exit(result.status !== null ? result.status : 1);
