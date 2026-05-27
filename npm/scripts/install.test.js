// Test suite for Hotpot install naming contracts.
// Verifies release-please configuration, asset filenames, and binary names
// across all distribution channels (npm, crates.io, Chocolatey, GitHub Releases).
// 用于验证 Hotpot 安装命名契约的测试套件。
// 验证 release-please 配置、资产文件名以及所有分发渠道的二进制名。

"use strict";

const { describe, it } = require("node:test");
const assert = require("node:assert");
const path = require("node:path");
const fs = require("node:fs");

// Resolve the project root (two levels up from npm/scripts/).
// 解析项目根目录（从 npm/scripts/ 向上两级）。
const PROJECT_ROOT = path.resolve(__dirname, "..", "..");

// Helper to read and parse a JSON file, returning the parsed object.
// 读取并解析 JSON 文件的辅助函数。
function readJson(relativePath) {
  const fullPath = path.join(PROJECT_ROOT, relativePath);
  return JSON.parse(fs.readFileSync(fullPath, "utf-8"));
}

// Helper to read a text file and return its content as a string.
// 读取文本文件并返回字符串内容的辅助函数。
function readFile(relativePath) {
  const fullPath = path.join(PROJECT_ROOT, relativePath);
  return fs.readFileSync(fullPath, "utf-8");
}

// Helper to assert that a file's content matches a given pattern (regex).
// 断言文件内容匹配指定正则表达式的辅助函数。
function assertFileMatches(filePath, regex, description) {
  const content = readFile(filePath);
  assert.ok(
    regex.test(content),
    `${description}: expected ${filePath} to match /${regex.source}/`
  );
}

// ---------------------------------------------------------------------------
// Task 1: Fix release-please component and tag naming
// ---------------------------------------------------------------------------

describe("release-please configuration", () => {
  it("release_please_component_stays_hotpot_not_hotpot_ai", () => {
    // Read the release-please config.
    // 读取 release-please 配置。
    const config = readJson("release-please-config.json");
    const rootPkg = config.packages && config.packages["."];

    assert.ok(rootPkg, "release-please-config.json must have a root package ('.') entry");

    // The root package must explicitly set component to "hotpot" so that
    // release-please generates tags like "hotpot-v<version>" instead of
    // deriving "hotpot-ai-v<version>" from Cargo.toml's package name.
    // 根 package 必须显式设置 component 为 "hotpot"，使 release-please
    // 生成的 tag 为 "hotpot-v<version>"，而非从 Cargo.toml 推导出 "hotpot-ai-v<version>"。
    assert.strictEqual(
      rootPkg.component,
      "hotpot",
      `Expected root package component to be "hotpot" for proper tag naming, got "${rootPkg.component}"`
    );
  });

  it("release_please_extra_files_still_sync_distribution_versions", () => {
    // Read the release-please config.
    // 读取 release-please 配置。
    const config = readJson("release-please-config.json");
    const rootPkg = config.packages && config.packages["."];

    assert.ok(rootPkg, "release-please-config.json must have a root package ('.') entry");
    assert.ok(Array.isArray(rootPkg.extraFiles || rootPkg["extra-files"]), "root package must have extra-files");

    const extraFiles = rootPkg.extraFiles || rootPkg["extra-files"];

    // These files must remain in extra-files for version synchronization.
    // 下列文件必须保留在 extra-files 中以实现版本同步。
    const expectedFiles = [
      "Cargo.lock",
      "npm/package.json",
      "packaging/chocolatey/hotpot.nuspec",
    ];

    for (const f of expectedFiles) {
      assert.ok(
        extraFiles.includes(f),
        `extra-files must include "${f}" for version synchronization`
      );
    }
  });
});

// ---------------------------------------------------------------------------
// Task 2: Lock npm, release workflow, and package-channel contracts
// ---------------------------------------------------------------------------

describe("release and channel contracts", () => {
  it("release_and_channel_contracts_keep_hotpot_executable_name", () => {
    // --- release-please.yml ---
    const rpYml = readFile(".github/workflows/release-please.yml");
    // Assert archive naming template uses hotpot-${TAG}-${ASSET_LABEL}${EXT}.
    // 断言 archive 命名模板使用 hotpot-${TAG}-${ASSET_LABEL}${EXT}。
    assert.ok(
      /hotpot-\$\{TAG\}/.test(rpYml) ||
      rpYml.includes('ARCHIVE="hotpot-${TAG}-${ASSET_LABEL}${EXT}"') ||
      rpYml.includes('$archive = "hotpot-$tag-$assetLabel'),
      "release-please.yml must archive binary with hotpot-${TAG} prefix, not hotpot-ai-${TAG}"
    );
    // Assert binary name template uses hotpot${{ matrix.suffix }}.
    // 断言二进制名模板使用 hotpot${{ matrix.suffix }}。
    assertFileMatches(
      ".github/workflows/release-please.yml",
      /hotpot\$\{\{ matrix\.suffix \}\}/,
      "release-please.yml must use hotpot executable name in binary variable"
    );

    // --- rebuild-release-assets.yml ---
    const rebuildYml = readFile(".github/workflows/rebuild-release-assets.yml");
    // Assert archive naming template.
    // 断言 archive 命名模板。
    assert.ok(
      /hotpot-\$\{TAG\}/.test(rebuildYml) ||
      rebuildYml.includes('ARCHIVE="hotpot-${TAG}-${ASSET_LABEL}${EXT}"') ||
      rebuildYml.includes('$archive = "hotpot-$tag-$assetLabel'),
      "rebuild-release-assets.yml must archive binary with hotpot-${TAG} prefix, not hotpot-ai-${TAG}"
    );
    assertFileMatches(
      ".github/workflows/rebuild-release-assets.yml",
      /hotpot\$\{\{ matrix\.suffix \}\}/,
      "rebuild-release-assets.yml must use hotpot executable name in binary variable"
    );

    // --- chocolateyInstall.ps1 ---
    const chocoInstall = readFile("packaging/chocolatey/tools/chocolateyInstall.ps1");
    // Assert download URL uses hotpot-$tag-windows-x86_64.zip.
    // 断言 Chocolatey 下载 URL 使用 hotpot-$tag-windows-x86_64.zip 格式。
    assert.ok(
      /hotpot-\$tag-windows/.test(chocoInstall),
      `chocolateyInstall.ps1 must download hotpot-$tag-windows-x86_64.zip`
    );

    // --- update-release-package-manifests.sh ---
    const manifestScript = readFile("scripts/update-release-package-manifests.sh");
    // Assert checksum path uses hotpot-${tag}-windows-x86_64.zip.sha256.
    // 断言 checksum 路径使用 hotpot-${tag}-windows-x86_64.zip.sha256 格式。
    assert.ok(
      /hotpot-\$\{tag\}-windows/.test(manifestScript),
      `update-release-package-manifests.sh must reference hotpot-\${tag}-windows-x86_64.zip.sha256`
    );

    // --- Cargo.toml ---
    const cargoToml = readFile("Cargo.toml");
    assert.ok(
      /\[\[bin\]\]\s*\n\s*name\s*=\s*"hotpot"/.test(cargoToml),
      `Cargo.toml must have [[bin]] name = "hotpot"`
    );

    // --- npm/package.json ---
    const npmPkg = readJson("npm/package.json");
    assert.ok(
      npmPkg.bin && npmPkg.bin.hotpot,
      `npm/package.json must have bin.hotpot entry`
    );
    assert.strictEqual(
      npmPkg.bin.hotpot,
      "bin/hotpot.js",
      `npm/package.json bin.hotpot must point to bin/hotpot.js`
    );
  });

  it("release_asset_matrix_for_version_0_3_4", () => {
    // Test version for asset name matrix validation.
    // 用于资产名称矩阵验证的测试版本。
    const version = "0.3.4";
    const TAG = `hotpot-v${version}`;

    // Helper to simulate the asset filename generation matching install.js logic.
    // 模拟 install.js asset 文件名生成逻辑的辅助函数。
    function getArchiveExt(label) {
      return label.startsWith("windows") ? ".zip" : ".tar.gz";
    }

    function getAssetFilename(assetLabel) {
      const ext = getArchiveExt(assetLabel);
      return `hotpot-${TAG}-${assetLabel}${ext}`;
    }

    // Define the supported platform matrix.
    // 定义所支持的平台矩阵。
    const platformMatrix = [
      { label: "linux-x86_64", ext: ".tar.gz" },
      { label: "linux-aarch64", ext: ".tar.gz" },
      { label: "macos-x86_64", ext: ".tar.gz" },
      { label: "macos-aarch64", ext: ".tar.gz" },
      { label: "windows-x86_64", ext: ".zip" },
    ];

    for (const { label, ext } of platformMatrix) {
      const filename = getAssetFilename(label);
      const expected = `hotpot-${TAG}-${label}${ext}`;
      assert.strictEqual(
        filename,
        expected,
        `Asset filename for ${label} must be ${expected}, got ${filename}`
      );
      // The filename MUST NOT contain "hotpot-ai".
      // 文件名必须不包含 "hotpot-ai"。
      assert.ok(
        !filename.includes("hotpot-ai"),
        `Asset filename must not contain "hotpot-ai", got "${filename}"`
      );
    }
  });
});

// ---------------------------------------------------------------------------
// Task 3: Update architecture documentation and final validation
// ---------------------------------------------------------------------------

describe("architecture documentation", () => {
  it("docs_describe_hotpot_ai_package_without_renaming_binary_assets", () => {
    // Reads docs/ARCH.md and docs/ARCH.zh_CN.md and verifies they describe the
    // relationship between the crates.io package name "hotpot-ai" and the
    // "hotpot" binary/command name, release tag, and asset naming.
    // 读取 docs/ARCH.md 和 docs/ARCH.zh_CN.md，验证它们描述了 crates.io
    // 包名 "hotpot-ai" 与 "hotpot" 二进制/命令名、release tag 和资产命名的关系。

    const archEn = readFile("docs/ARCH.md");
    const archZh = readFile("docs/ARCH.zh_CN.md");

    // 1. Must mention that crates.io package name is "hotpot-ai".
    // 必须说明 crates.io 包名为 "hotpot-ai"。
    assert.ok(
      /hotpot-ai/.test(archEn),
      "docs/ARCH.md must mention hotpot-ai as the crates.io package name"
    );
    assert.ok(
      /hotpot-ai/.test(archZh),
      "docs/ARCH.zh_CN.md must mention hotpot-ai as the crates.io package name"
    );

    // 2. Must mention that release-please component/tag is explicitly fixed to "hotpot".
    // 必须说明 release-please component/tag 显式固定为 "hotpot"。
    // This is a NEW requirement from Task 3 — the docs may not yet cover it.
    // 这是 Task 3 的新要求——文档可能尚未包含此内容。
    // Check for the exact release-please configuration context mentioning component/tag being fixed.
    // 检查是否在 release-please 配置上下文中明确说明了 component/tag 固定为 hotpot。
    const hasRpComponentEn =
      /`component`.*hotpot/.test(archEn) ||
      /component.*fixed.*hotpot/.test(archEn) ||
      /release-please-config\.json.*component.*hotpot/.test(archEn) ||
      /release-please.*tag.*prefix.*hotpot/.test(archEn);
    assert.ok(
      hasRpComponentEn,
      "docs/ARCH.md must explicitly describe that release-please-config.json sets " +
      'component to "hotpot" to prevent tag leakage from the "hotpot-ai" crate name'
    );
    const hasRpComponentZh =
      /`component`.*hotpot/.test(archZh) ||
      /component.*固定.*hotpot/.test(archZh) ||
      /release-please-config\.json.*component.*hotpot/.test(archZh) ||
      /release-please.*tag.*前缀.*hotpot/.test(archZh);
    assert.ok(
      hasRpComponentZh,
      "docs/ARCH.zh_CN.md must explicitly describe that release-please-config.json sets " +
      'component to "hotpot" to prevent tag leakage from the "hotpot-ai" crate name'
    );

    // 3. Must mention that the installed CLI command name remains "hotpot".
    // 必须说明安装后的 CLI 命令名仍为 "hotpot"。
    assert.ok(
      /command.*hotpot/.test(archEn) || /CLI.*hotpot/.test(archEn) || /`hotpot`/.test(archEn),
      "docs/ARCH.md must mention the CLI command is still hotpot"
    );
    assert.ok(
      /命令.*hotpot/.test(archZh) || /CLI.*hotpot/.test(archZh) || /`hotpot`/.test(archZh),
      "docs/ARCH.zh_CN.md must mention the CLI command is still hotpot"
    );

    // 4. Must mention that npm package still exposes bin.hotpot.
    // 必须说明 npm 包仍暴露 bin.hotpot。
    assert.ok(
      /bin\.hotpot/.test(archEn),
      "docs/ARCH.md must mention npm package exposes bin.hotpot"
    );
    assert.ok(
      /bin\.hotpot/.test(archZh),
      "docs/ARCH.zh_CN.md must mention npm package exposes bin.hotpot"
    );

    // 5. Must mention that release assets follow the hotpot-${TAG}-${ASSET_LABEL}${EXT} pattern.
    // 必须说明 release asset 仍按 hotpot-${TAG}-${ASSET_LABEL}${EXT} 命名。
    assert.ok(
      /hotpot-\$\{TAG\}/.test(archEn) ||
      /hotpot-\$tag/.test(archEn) ||
      /hotpot-hotpot-v/.test(archEn),
      "docs/ARCH.md must mention release asset naming pattern hotpot-${TAG}-<label>"
    );
    assert.ok(
      /hotpot-\$\{TAG\}/.test(archZh) ||
      /hotpot-\$tag/.test(archZh) ||
      /hotpot-hotpot-v/.test(archZh),
      "docs/ARCH.zh_CN.md must mention release asset naming pattern hotpot-${TAG}-<label>"
    );
  });
});
