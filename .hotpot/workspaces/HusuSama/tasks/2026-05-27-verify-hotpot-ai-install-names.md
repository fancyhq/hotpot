# Verify Hotpot AI Install Names

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| Done | true | 3 | medium |
:::

---

## Task

### Summary

::: info
验证并修复 Rust crate 从 `hotpot` 改为 `hotpot-ai` 后导致 release-please tag/component 变为 `hotpot-ai-v<version>` 的问题，确保 GitHub Release tag、release asset、npm、Chocolatey 和最终 CLI 命令名继续使用 `hotpot` 契约。
:::

### User Request

::: info 用户原始请求
目前应为 crates.io 里面已经存在了 hotpot 的包，所以需要更改为 `hotpot-ai`，目前已经进行了更改，现在你需要确认如下问题，并建立修复工作：

- 确认包名更改是否会影响 `npm` 等方式的安装，因为存在文件名拼接问题。
- 确认更改包名后，通过 `npm` 等安装是否依然会使用 `hotpot` 二进制名称，需要确保任何平台安装下来，都是使用的 `hotpot` 名称。

用户补充确认：当前执行 GitHub Action 时实际生成的二进制压缩包已经是 `hotpot-hotpot-ai-v0.3.4-linux-aarch64.tar.gz` 这种形式。原因大概率是 release-please 在 Rust package name 改为 `hotpot-ai` 后，把 release component/tag 也改成了 `hotpot-ai-v0.3.4`；workflow 仍按 `hotpot-${TAG}-${ASSET_LABEL}${EXT}` 拼接 archive 名，因此自然生成 `hotpot-hotpot-ai-v...`。

用户批准按推荐方案重新修改任务：不要只在 workflow 里硬改 archive 名，而是优先修复 `release-please-config.json`，显式固定 release-please component/tag 为 `hotpot`，让 crates.io package name `hotpot-ai` 只影响 Cargo 发布，不影响 GitHub Release tag、asset 名、npm/Chocolatey 下载 URL 或安装后的 `hotpot` 命令。
:::

### Approved Design

::: tip
本任务采用“修复 release-please component/tag，并补齐防回归验证”的设计，不在任务创建阶段直接修改应用代码。执行 agent 需要先用测试或脚本复现当前 `hotpot-ai-v<version>` tag/asset 问题，再做最小修复，使 release tag 回到 `hotpot-v<version>`。

已确认的核心契约：

- Rust crates.io package name 是 `hotpot-ai`，但 `Cargo.toml` 必须保留显式 `[[bin]] name = "hotpot"`，确保 `cargo install hotpot-ai` 安装后的命令是 `hotpot`。
- release-please 必须显式固定 component/tag 为 `hotpot`，使 GitHub Release tag 继续是 `hotpot-v<version>`，不要跟随 Cargo package name 变成 `hotpot-ai-v<version>`。
- npm package name 仍是 `@fancyhq/hotpot`，`npm/package.json` 必须保留 `bin.hotpot` 指向 `bin/hotpot.js`，安装后的命令是 `hotpot`。
- npm postinstall 下载 GitHub Release asset 时，文件名必须继续由 release tag `hotpot-v<version>` 和平台 label 拼接，即 `hotpot-hotpot-v<version>-<platform>.<ext>`。不要从 Cargo package name `hotpot-ai` 派生 asset 名称。
- release workflows 必须继续从 Cargo bin target 产物 `target/<target>/release/hotpot` 或 `hotpot.exe` staging 到 archive 根目录，archive 内部文件名保持 `hotpot` 或 `hotpot.exe`。
- Chocolatey installer 必须继续下载 `hotpot-$tag-windows-x86_64.zip`，解压出的 `hotpot.exe` 由 Chocolatey shim 暴露为 `hotpot`。
- 如执行中发现文档仍暗示错误命名或遗漏 `hotpot-ai` 与 `hotpot` 二进制名分离的说明，更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`。
:::

### Alternatives Considered

- 只改 GitHub Actions archive 文件名：可以把 archive 名强制改回 `hotpot-hotpot-v<version>-...`，但如果 release tag 仍是 `hotpot-ai-v<version>`，npm 和 Chocolatey 下载 URL 仍必须拆分 tag 与 asset name，安装脚本会更复杂。
- 接受 `hotpot-ai-v<version>` tag 并同步改 npm/Chocolatey：能修复下载，但会扩大兼容性影响，并让 GitHub Release tag 与用户安装命令 `hotpot` 不一致。
- 推荐方案：修复 `release-please-config.json`，显式固定 release component/tag 为 `hotpot`，使 tag、asset、npm、Chocolatey 继续使用既有 `hotpot` 契约；`hotpot-ai` 仅作为 crates.io package name。这是最小且兼容现有渠道的方案。

### Requirements

- npm 安装链路不得因为 `Cargo.toml [package].name = "hotpot-ai"` 而拼接出不存在的 GitHub Release asset 名称。
- release-please 创建的新 tag 必须保持 `hotpot-v<version>`，不得因为 Cargo package name 变为 `hotpot-ai` 而生成 `hotpot-ai-v<version>`。
- npm 全局安装后的命令名必须保持 `hotpot`，Windows 上 wrapper 期望包内原生二进制为 `hotpot.exe`，Unix 上为 `hotpot`。
- GitHub Release archive 名称必须与 release workflow 实际上传名称一致，并覆盖 Linux x86_64/aarch64、macOS x86_64/aarch64、Windows x86_64。
- GitHub Release archive 名称必须是 `hotpot-hotpot-v<version>-<platform>.<ext>`，不应是 `hotpot-hotpot-ai-v<version>-<platform>.<ext>`。
- Release archive 根目录内必须继续包含 `hotpot` 或 `hotpot.exe`，不能包含 `hotpot-ai`。
- `cargo install hotpot-ai` 必须通过 `[[bin]] name = "hotpot"` 安装 `hotpot` 命令。
- Chocolatey 安装 URL 和 shim 行为必须继续指向 `hotpot.exe`。
- 新增或调整的脚本、测试和代码注释必须使用英文输出，注释遵循英文优先、中文补充的双语风格。
- 如果新增 Rust 函数或文件，必须包含双语 doc comments；如果新增 JS 测试 helper，也应保留简洁双语注释。

### Non-Goals

::: details Non-Goals
- 不更改 npm package name `@fancyhq/hotpot`。
- 不接受 GitHub release tag 前缀变为 `hotpot-ai-v`；任务目标是固定回 `hotpot-v`。
- 不更改二进制命令名 `hotpot`。
- 不引入新的安装渠道，例如 Homebrew、Scoop 或 winget。
- 不发布 npm、crates.io、Chocolatey 或 GitHub Release。
- 不重构整个 release workflow；只做命名契约相关的最小修复和验证。
:::

### Project Context

- `Cargo.toml` 当前 `[package].name` 是 `hotpot-ai`，并已存在 `[[bin]] name = "hotpot" path = "src/main.rs"`。
- `release-please-config.json` 当前只有 `release-type: "rust"` 和 `extra-files`，没有显式 `component` / tag-name 固定配置；这可能导致 release-please 从 Cargo package name 推导出 `hotpot-ai` component，并生成 `hotpot-ai-v<version>` tag。
- `npm/package.json` 当前 package name 是 `@fancyhq/hotpot`，`bin` 映射为 `"hotpot": "bin/hotpot.js"`。
- `npm/scripts/install.js` 当前用 ``const TAG = `hotpot-v${pkg.version}``` 生成 release tag，并用 ``hotpot-${TAG}-${assetLabel}${ext}`` 生成 asset 文件名；该逻辑要求 GitHub Release tag 必须是 `hotpot-v<version>`。如果 Action 实际创建 `hotpot-ai-v0.3.4`，npm postinstall 会下载错误 tag 或找不到 asset。
- `npm/scripts/install.js::getBinaryName()` 和 `npm/bin/hotpot.js` 当前都期望 Windows 为 `hotpot.exe`，其他平台为 `hotpot`。
- `.github/workflows/release-please.yml` 和 `.github/workflows/rebuild-release-assets.yml` 当前都设置 `$binary = "hotpot${{ matrix.suffix }}"` 或 `BINARY="hotpot${{ matrix.suffix }}"`，并将 `target/.../release/${BINARY}` 放入 archive 根目录。
- `packaging/chocolatey/tools/chocolateyInstall.ps1` 当前使用 `$tag = "hotpot-v$version"` 和 URL `hotpot-$tag-windows-x86_64.zip`，解压 `hotpot.exe` 后依赖 Chocolatey 自动 shim。
- `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md` 已有 npm distribution、多渠道发布和 crates.io `hotpot-ai` 说明；执行中需要核对这些说明是否足够准确，并补充 release-please component/tag 必须固定为 `hotpot` 的原因。
- 仓库目前没有根目录 `package.json`，也没有现成 `npm/**/*.test.js` 或 `tests/**/*` 测试文件；执行 agent 需要选择轻量验证方式，例如新增 Node 内置 `node:test` 测试、shell 验证脚本，或 Rust 测试，优先最小依赖。

---

## Plan

### Mode

- tdd: true

### Execution Strategy

- git-worktree: false
- rationale: 任务主要修改安装命名验证、脚本或文档，范围集中且可通过当前 checkout 直接验证；不需要隔离 worktree。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `Cargo.toml` | Modify | 必要时确认或保留 `[[bin]] name = "hotpot"`，并避免 package name 影响二进制名。 |
| `release-please-config.json` | Modify | 固定 release-please component/tag 为 `hotpot`，避免 tag 变成 `hotpot-ai-v<version>`。 |
| `npm/package.json` | Modify | 必要时增加 npm 测试脚本或保留 `bin.hotpot` 契约。 |
| `npm/scripts/install.js` | Modify | 必要时导出或重构纯函数以便测试 asset filename、download URL 和 binary name，不改变生产行为。 |
| `npm/bin/hotpot.js` | Modify | 必要时确保 wrapper 仍查找 `hotpot` 或 `hotpot.exe`。 |
| `npm/scripts/install.test.js` | Create | 用 Node 内置测试覆盖 release asset 文件名和二进制名契约。 |
| `npm/bin/hotpot.test.js` | Create | 如有必要，用 Node 内置测试覆盖 wrapper 使用的二进制名契约。 |
| `.github/workflows/release-please.yml` | Modify | 必要时修复 release asset 名称或 archive 内部二进制名。 |
| `.github/workflows/rebuild-release-assets.yml` | Modify | 必要时与主 release workflow 保持同一命名契约。 |
| `packaging/chocolatey/tools/chocolateyInstall.ps1` | Modify | 必要时修复 Chocolatey 下载 URL 或说明，使其保持 `hotpot.exe`。 |
| `scripts/update-release-package-manifests.sh` | Modify | 必要时修复 checksum 文件名查找逻辑，使其匹配实际 release asset。 |
| `docs/ARCH.md` | Modify | 更新英文架构说明，明确 `hotpot-ai` crate 与 `hotpot` 二进制名/asset 名分离。 |
| `docs/ARCH.zh_CN.md` | Modify | 与 `docs/ARCH.md` 同步更新中文架构说明。 |

### Implementation Tasks

#### Task 1: Fix release-please component and tag naming

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `release-please-config.json` | Modify | 显式固定 release component/tag 为 `hotpot`。 |
| `npm/scripts/install.test.js` | Create | 验证 release-please 配置不会让 tag 变成 `hotpot-ai-v<version>`。 |
| `npm/package.json` | Modify | 必要时增加 `test` 或 `test:install-names` 脚本。 |

##### Red

- [x] R1: 在 `npm/scripts/install.test.js` 中使用 Node 内置 `node:test` 和 `assert` 添加测试 `release_please_component_stays_hotpot_not_hotpot_ai`，读取 `release-please-config.json`，断言根 package 配置显式包含能让 release tag/component 保持 `hotpot` 的配置，例如 `component: "hotpot"` 或 release-please 支持的等价配置。
- [x] R2: 在同一测试文件添加测试 `release_please_extra_files_still_sync_distribution_versions`，断言 `extra-files` 仍包含 `Cargo.lock`、`npm/package.json`、`packaging/chocolatey/hotpot.nuspec`。
- [x] R3: 运行 `node --test npm/scripts/install.test.js`；预期失败，失败原因应是 `release-please-config.json` 当前缺少显式 `hotpot` component/tag 固定配置。

##### Green

- [x] G1: 在 `release-please-config.json` 中做最小修复，显式固定 release-please component/tag 为 `hotpot`，并保持 `release-type: "rust"` 与现有 `extra-files`。
- [x] G2: 如需要，在 `npm/package.json` 添加脚本，例如 `"test:install-names": "node --test scripts/install.test.js"`；不要引入第三方测试依赖。
- [x] G3: 运行 `node --test npm/scripts/install.test.js`；预期通过。
- [x] G4: 检查 release-please 文档或现有项目配置语义，确认新增配置确实影响 tag/component，而不是只影响 changelog 标题；如发现 `component` 不是正确字段，改用 release-please v4 支持的正确字段并更新测试。

##### Refactor

- [x] F1: 检查 `release-please-config.json` 的配置是否保持最小；不要引入无关 release-please 选项。若无重复或命名问题，写 `no refactor needed`。
- [x] F2: 如果发生 refactor，重新运行 `node --test npm/scripts/install.test.js`；预期通过。否则标记 `skipped (no refactor)`。

:::

#### Task 2: Lock npm, release workflow, and package-channel contracts

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.github/workflows/release-please.yml` | Modify | 确保 release assets 和 archive 内部二进制名与 npm/Chocolatey 下载契约一致。 |
| `.github/workflows/rebuild-release-assets.yml` | Modify | 确保手动重建 workflow 与主 release workflow 命名一致。 |
| `packaging/chocolatey/tools/chocolateyInstall.ps1` | Modify | 确保 Chocolatey 下载和安装 `hotpot.exe`。 |
| `scripts/update-release-package-manifests.sh` | Modify | 确保 checksum 文件名查找匹配实际 release asset。 |
| `Cargo.toml` | Modify | 确保 `[[bin]] name = "hotpot"` 未被移除。 |
| `npm/scripts/install.js` | Modify | 必要时暴露或隔离可测试命名函数，确保 npm 仍下载 `hotpot-v<version>` tag 下的 asset。 |
| `npm/scripts/install.test.js` | Test | 可扩展为覆盖 release asset 文件名矩阵。 |

##### Red

- [x] R1: 增加或扩展一个确定性验证，测试名为 `release_and_channel_contracts_keep_hotpot_executable_name`。可放在 `npm/scripts/install.test.js` 或新建轻量脚本测试；必须读取 `.github/workflows/release-please.yml`、`.github/workflows/rebuild-release-assets.yml`、`packaging/chocolatey/tools/chocolateyInstall.ps1`、`scripts/update-release-package-manifests.sh`、`Cargo.toml`、`npm/package.json`，断言这些文件包含当前契约：`hotpot-${TAG}-${ASSET_LABEL}${EXT}` 或等价 PowerShell 模板、`hotpot${{ matrix.suffix }}` 或等价 Windows 模板、Chocolatey URL `hotpot-$tag-windows-x86_64.zip`、checksum 路径 `hotpot-${tag}-windows-x86_64.zip.sha256`、`[[bin]] name = "hotpot"`、`bin.hotpot`。
- [x] R2: 在同一验证中覆盖 version `0.3.4` 的 npm asset 矩阵，断言 Linux aarch64/x86_64、macOS aarch64/x86_64、Windows x86_64 生成的文件名分别为 `hotpot-hotpot-v0.3.4-<platform>.<ext>`，且不包含 `hotpot-ai`。
- [x] R3: 运行对应测试命令，例如 `node --test npm/scripts/install.test.js`；如果新增断言因为测试尚未实现生产可观测接口而失败，捕获失败信息。若现有代码已满足契约，则 Red 阶段可通过临时反向断言或先写一个缺失检查以证明测试能失败，然后立即恢复为正确断言再进入 Green；不要把临时反向断言留在最终代码中。

##### Green

- [x] G1: 如测试发现 npm、workflow、Chocolatey、manifest 脚本或 `Cargo.toml` 与契约不一致，做最小修复；如果现有实现已正确，只保留测试。
- [x] G2: 运行 `node --test npm/scripts/install.test.js`；预期通过。
- [x] G3: 运行 `cargo metadata --no-deps --format-version 1`；预期成功，输出中 package name 是 `hotpot-ai`，target bin name 包含 `hotpot`。
- [x] G4: 运行 `bash scripts/update-release-package-manifests.sh hotpot-v0.3.4 <temp-sha256-dir>`，其中 `<temp-sha256-dir>` 包含文件 `hotpot-hotpot-v0.3.4-windows-x86_64.zip.sha256` 且内容为假 hash；预期脚本能定位 checksum 并更新 Chocolatey 文件。运行前保存相关文件状态，验证后只保留有意修改，不要提交临时 checksum 目录。

##### Refactor

- [x] F1: 检查是否存在重复 hard-coded 命名说明可以用更清晰注释替代；只在能减少歧义时微调。否则写 `no refactor needed`。
- [x] F2: 如果发生 refactor，重新运行 `node --test npm/scripts/install.test.js`、`cargo metadata --no-deps --format-version 1` 和 manifest 脚本验证；预期通过。否则标记 `skipped (no refactor)`。

:::

#### Task 3: Update architecture documentation and final validation

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify | 英文说明需要准确描述 `hotpot-ai` package name 与 `hotpot` command/release asset 的关系。 |
| `docs/ARCH.zh_CN.md` | Modify | 中文说明需要与英文文档一致。 |
| `README.md` | Modify | 仅当发现安装说明不准确时更新。 |
| `README.zh_CN.md` | Modify | 仅当发现中文安装说明不准确时更新。 |
| `docs/ROADMAP.md` | Modify | 仅当发现完成项描述不准确时更新。 |

##### Red

- [x] R1: 添加或扩展测试 `docs_describe_hotpot_ai_package_without_renaming_binary_assets`，断言 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md` 同时说明：crates.io package 是 `hotpot-ai`、release-please component/tag 固定为 `hotpot`、二进制命令仍是 `hotpot`、npm package 仍暴露 `bin.hotpot`、release asset 仍按 `hotpot-${TAG}-${ASSET_LABEL}${EXT}` 命名。
- [x] R2: 运行 `node --test npm/scripts/install.test.js` 或对应文档验证命令；预期在文档说明不足时失败，或在文档已满足时通过并记录“测试覆盖现有正确行为”。

##### Green

- [x] G1: 更新 `docs/ARCH.md`，明确用户关心的问题：`hotpot-ai` 只影响 crates.io package name，不应影响 npm 下载 asset 拼接，也不应影响 archive 内部二进制名或安装后的命令名。
- [x] G2: 更新 `docs/ARCH.zh_CN.md`，与英文文档保持同等信息量。
- [x] G3: 如 `README.md`、`README.zh_CN.md` 或 `docs/ROADMAP.md` 有错误或含混表述，做最小同步修正；如果已经准确，不修改。
- [x] G4: 运行 `node --test npm/scripts/install.test.js`；预期通过。

##### Refactor

- [x] F1: 检查文档是否重复、是否错误暗示 release asset 应改为 `hotpot-hotpot-ai-v...`；保留对当前实际命名 `hotpot-hotpot-v...` 的清晰说明。若无需进一步清理，写 `no refactor needed`。
- [x] F2: 运行 `cargo test`；预期通过。如果耗时或环境失败，记录具体失败原因和已完成的替代验证。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `node --test npm/scripts/install.test.js` | 命名契约测试全部通过，覆盖 release-please component/tag 固定、npm asset 文件名、binary name、release workflow/Chocolatey/crates.io 文档契约。 |
| `npm pack --dry-run ./npm` | npm 包 dry-run 成功，仍包含 `bin/` 和 `scripts/`，并保留 `bin.hotpot`。 |
| `cargo metadata --no-deps --format-version 1` | 成功输出 metadata，package name 为 `hotpot-ai`，bin target name 包含 `hotpot`。 |
| `cargo test` | Rust 测试全部通过，无安装命名相关回归。 |
| `bash scripts/update-release-package-manifests.sh hotpot-v0.3.4 <temp-sha256-dir>` | 能读取 `hotpot-hotpot-v0.3.4-windows-x86_64.zip.sha256`，并更新 Chocolatey manifest 字段；临时目录不入库。 |

### Risks and Watchouts

::: warning
- 用户已经观察到 GitHub Action 实际生成 `hotpot-hotpot-ai-v0.3.4-...`，这正是本任务要修复的问题；不要把该现象记录为目标契约。
- 不要只在 GitHub Actions 中硬改 archive 名来掩盖问题。如果 release tag 仍是 `hotpot-ai-v<version>`，npm 和 Chocolatey 下载 URL 仍会分裂为 tag 与 asset name 两套规则。优先修复 release-please component/tag。
- 不要把 Cargo package name 当成 release asset prefix。`hotpot-ai` 是 crates.io package name；GitHub repository、release tag、archive prefix 和 binary command 仍是 `hotpot` 契约的一部分。
- `npm/scripts/install.js` 当前会在顶层调用 `main()`；为测试导出 helper 时必须使用 `if (require.main === module)` 或等价模式，避免测试导入时触发真实下载。
- manifest 脚本验证可能修改 `packaging/chocolatey/hotpot.nuspec` 和 `packaging/chocolatey/tools/chocolateyInstall.ps1`。执行后必须确保只保留任务需要的有意修改，不要留下临时版本号或 placeholder 损坏。
- 不要引入第三方 npm 测试依赖；优先使用 Node 18 内置 `node:test`。
:::

---

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Because `tdd: true`, follow Red → Green → Refactor for every `#### Task N` and capture the failing/passing validation summaries.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task.
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff.
- Run the validation commands before reporting completion.
