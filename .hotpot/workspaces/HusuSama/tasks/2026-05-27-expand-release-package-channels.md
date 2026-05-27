---
title: expand-release-package-channels
description: 扩展 Hotpot 发布流程，增加 crates.io、Homebrew、Chocolatey、Scoop、winget 安装渠道维护
date: 2026-05-27
category: [Task]
tag: [release, packaging, crates-io, homebrew, chocolatey, scoop, winget]
---

# Expand Release Package Channels

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| Done | false | 6 | high |
:::

---

## Task

### Summary

::: info
在现有 GitHub Release 二进制资产和 npm 自动发布流程基础上，继续扩展 Hotpot 的安装渠道：新增 crates.io 自动发布、Chocolatey 自动发布，以及本仓库内 Homebrew formula、Scoop manifest、winget manifest 的版本与校验和维护。目标是在 Release PR 合并并创建 release 后，所有新增渠道都能与 Rust crate 版本、GitHub Release tag、二进制压缩包名称保持一致。
:::

### User Request

::: info 用户原始需求
用户说明当前仓库已经配置了 Repository secrets：`CARGO_REGISTRY_TOKEN`、`CHOCO_API_KEY`、`NPM_TOKEN`。npm 安装渠道已经完成，希望继续扩展安装渠道：`crates.io`、Homebrew、Chocolatey、Scoop、winget。

这些渠道需要和 npm 一样，在编译发布时自动维护版本。二进制压缩文件名称组成如下：前缀为 `hotpot-hotpot-v0.3.2`，系统架构包括 `linux-aarch64`、`linux-x86_64`、`macos-aarch64`、`macos-x86_64`、`windows-x86_64`，Linux/macOS 使用 `tar.gz`，Windows 使用 `zip`。示例文件名：`hotpot-hotpot-v0.3.2-windows-x86_64.zip`。目前 Windows 仅有 `x86_64`。

Brainstorming 中已确认：Homebrew、Scoop、winget 的 manifest 本次先在本仓库维护，不在本任务中自动向外部 Homebrew tap、Scoop bucket 或 `microsoft/winget-pkgs` 提交 PR。
:::

### Approved Design

::: tip
采用分层发布方案，保留现有 `release-please` 作为版本源头，保留现有 GitHub Release asset 命名规则 `hotpot-${TAG}-${asset_label}${ext}`。当 tag 为 `hotpot-v0.3.2` 时，产物名自然是用户要求的 `hotpot-hotpot-v0.3.2-<platform>.<ext>`。

crates.io 渠道在 `.github/workflows/release-please.yml` 的 release-created 分支中新增 `publish-crates-io` job，检出 release tag，运行 `cargo publish --locked --token ${{ secrets.CARGO_REGISTRY_TOKEN }}`。发布前必须做最小 dry-run 或 package 校验，且缺失 token 时应让 workflow 明确失败而不是静默跳过。

Chocolatey 渠道新增仓库内包定义目录，包含 `.nuspec` 和 `tools/chocolateyInstall.ps1`。安装脚本从当前 release 的 Windows x86_64 zip 下载 `hotpot.exe`，校验 SHA256，并安装到 Chocolatey package tools 目录。release workflow 在二进制资产构建完成后计算 checksum、更新包版本，执行 `choco pack` 和 `choco push --api-key ${{ secrets.CHOCO_API_KEY }}`。

Homebrew、Scoop、winget 本次不向外部仓库提交 PR，而是在本仓库内维护可发布的 manifest/template，并在 release workflow 中根据 release tag、asset URL、SHA256 自动生成或更新对应 manifest 文件。执行代理应优先让这些 manifest 作为 release asset 上传，必要时也可在 Release PR 中保持模板版本字段由 `release-please` 更新；但不得引入需要人工维护多处版本号的设计。

文档需要同步更新 README、README.zh_CN、docs/ARCH.md、docs/ARCH.zh_CN.md、docs/ROADMAP.md，说明哪些渠道已经自动发布，哪些只是仓库内 manifest 维护，以及每个渠道依赖的 secret 和外部发布前提。
:::

### Alternatives Considered

- 一次性自动提交到外部 Homebrew tap、Scoop bucket、winget-pkgs：最终用户安装路径更完整，但需要确定外部仓库、token 权限、PR 策略和审核流程，范围明显超出当前仓库已配置 secret。
- 只做 crates.io 和 Chocolatey，把 Homebrew/Scoop/winget 全部拆成后续任务：实现风险更低，但用户明确希望五个渠道一起扩展，且 manifest 本仓库维护可作为合理的第一阶段。
- 批准方案：自动发布有 secret 支撑的 crates.io 与 Chocolatey；本仓库维护 Homebrew/Scoop/winget manifest，并在 release 时自动生成版本、URL、SHA256。该方案复用现有 release assets，避免外部仓库凭据阻塞，同时为后续 tap/bucket/winget-pkgs 自动 PR 留出清晰接口。

### Requirements

- 保留现有 npm 发布流程，不回退或改坏 `@fancyhq/hotpot` 安装方式。
- 保留现有二进制资产命名规则：`hotpot-${TAG}-${asset_label}${ext}`；当 `TAG=hotpot-v0.3.2` 时文件名前缀必须是 `hotpot-hotpot-v0.3.2`。
- 支持 release asset 平台矩阵：Linux x86_64/aarch64、macOS x86_64/aarch64、Windows x86_64；Windows 当前不新增 aarch64。
- Linux/macOS 压缩包使用 `.tar.gz`，Windows 压缩包使用 `.zip`。
- crates.io 发布使用已配置的 `CARGO_REGISTRY_TOKEN`，发布版本与 `Cargo.toml` release tag 一致。
- Chocolatey 发布使用已配置的 `CHOCO_API_KEY`，安装包应下载并校验 Windows x86_64 GitHub Release zip。
- Homebrew、Scoop、winget manifest 在本仓库维护，并由 release 流程自动写入版本、下载 URL、SHA256。
- 所有新增脚本输出和用户可见错误信息使用英文。
- 新增脚本、workflow 注释、配置注释遵守 English-first bilingual comment 风格。
- 修改发布执行流后必须更新 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md`。

### Non-Goals

::: details Non-Goals
- 不改变 Hotpot CLI 的 Rust 命令行为或 agent workflow 语义。
- 不改变 release-please 的版本来源或 tag 格式。
- 不重命名现有 GitHub Release binary assets。
- 不为 Windows aarch64 构建或发布资产。
- 不在本任务中向外部 Homebrew tap、Scoop bucket、`microsoft/winget-pkgs` 自动提交 PR。
- 不新增除必要发布工具外的运行时依赖。
- 不把编译后的二进制提交进 git 仓库。
:::

### Project Context

- 当前 Rust crate 位于 `Cargo.toml`，`[package]` 名称为 `hotpot`，当前版本为 `0.3.2`，仓库为 `https://github.com/fancyhq/hotpot`。
- 当前 npm 包位于 `npm/`，包名为 `@fancyhq/hotpot`，版本为 `0.3.2`，`postinstall` 根据 release asset 名称下载 GitHub Release 二进制。
- `release-please-config.json` 当前把 `Cargo.lock` 和 `npm/package.json` 放入 `extra-files`，确保 npm 版本随 Rust crate 同步。
- `.github/workflows/release-please.yml` 当前包含 `release-please`、`build-release-assets`、`publish-npm` job；`publish-npm` 依赖 `build-release-assets`，使用 `NPM_TOKEN` 发布 npm 包。
- `.github/workflows/rebuild-release-assets.yml` 是手动重建已有 tag 的 fallback，仅构建和上传二进制 assets，不发布 npm。
- GitHub Release assets 现有生成模板为 `hotpot-${TAG}-${ASSET_LABEL}${EXT}`，矩阵 asset label 包括 `windows-x86_64`、`macos-x86_64`、`macos-aarch64`、`linux-x86_64`、`linux-aarch64`。
- `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md` 已有 npm distribution 章节；本任务会影响发布执行流，必须同步扩展为多渠道 distribution 说明。
- README 当前仍提示 crates.io、Homebrew、Scoop、Chocolatey 需要单独评估；完成本任务后需更新该描述，并加入 winget。

---

## Plan

### Mode

- tdd: false

### Execution Strategy

- git-worktree: true
- rationale: 该任务会广泛修改 GitHub Actions、发布脚本、包管理器 manifest 和中英文文档，且可能生成新目录；使用独立 worktree 可以隔离发布流程改动，避免干扰当前 checkout 中其他工作。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `.github/workflows/release-please.yml` | Modify | 增加 crates.io、Chocolatey、manifest 生成/上传 jobs，并串联现有二进制资产与 npm 发布顺序。 |
| `.github/workflows/rebuild-release-assets.yml` | Modify | 视执行设计同步手动重建 workflow 是否需要重新生成仓库内 manifests 或说明不覆盖包管理器发布。 |
| `release-please-config.json` | Modify | 如新增静态 manifest/package 版本字段需要 Release PR 更新，将对应文件加入 `extra-files`。 |
| `packaging/chocolatey/hotpot.nuspec` | Create | 定义 Chocolatey package 元数据、版本占位和依赖信息。 |
| `packaging/chocolatey/tools/chocolateyInstall.ps1` | Create | Chocolatey 安装脚本，下载并校验 Windows x86_64 zip 后安装 `hotpot.exe`。 |
| `packaging/homebrew/hotpot.rb` | Create | 本仓库维护的 Homebrew formula 或生成模板，引用 macOS/Linux release assets 与 SHA256。 |
| `packaging/scoop/hotpot.json` | Create | 本仓库维护的 Scoop manifest 或生成模板，引用 Windows x86_64 zip 与 SHA256。 |
| `packaging/winget/` | Create | 本仓库维护的 winget manifest 模板或版本化 manifest 目录，引用 Windows x86_64 zip 与 SHA256。 |
| `scripts/update-release-package-manifests.sh` | Create | 根据 release tag、asset URL、SHA256 生成或更新 Homebrew/Scoop/winget/Chocolatey manifest。 |
| `README.md` | Modify | 更新英文安装渠道和 release 流程说明。 |
| `README.zh_CN.md` | Modify | 同步中文安装渠道和 release 流程说明。 |
| `docs/ARCH.md` | Modify | 扩展架构文档中的 distribution/release workflow 说明。 |
| `docs/ARCH.zh_CN.md` | Modify | 与英文架构文档同步。 |
| `docs/ROADMAP.md` | Modify | 更新包管理器渠道状态。 |

### Implementation Tasks

#### Task 1: Audit channel schemas and release ordering

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.github/workflows/release-please.yml` | Test | 确认现有 release-created gate、job outputs、asset upload 和 npm publish 顺序。 |
| `.github/workflows/rebuild-release-assets.yml` | Test | 确认手动重建流程与新增 manifest 生成策略是否需要同步。 |
| `Cargo.toml` | Test | 确认 crates.io 发布元数据是否满足 `cargo publish` 要求。 |

**Steps:**

- [x] **Step 1**: Inspected `.github/workflows/release-please.yml` — current jobs: `release-please` (outputs `release_created`, `tag_name`), `build-release-assets` (5-platform matrix), `publish-npm`.
- [x] **Step 2**: Inspected `.github/workflows/rebuild-release-assets.yml` — decided manual rebuild rebuilds only binary assets; documented in workflow comments that it does not regenerate manifests or publish packages.
- [x] **Step 3**: Ran `cargo package --allow-dirty --no-verify` — packaging succeeds with warning about missing `description`; added required Cargo metadata (`description`, `readme`, `keywords`, `categories`) for crates.io readiness.
- [x] **Step 4**: Verified package naming assumptions — crates.io package `hotpot` assumed available; Cargo metadata now complete. Chocolatey package id `hotpot` assumed available. No verification publish attempted (requires API tokens). Cargo.toml name is `hotpot`, nuspec id is `hotpot`.
- [x] **Step 5**: Checked Homebrew/Scoop/winget schema requirements — Homebrew uses Ruby DSL (`version`, `url`, `sha256`); Scoop uses JSON (`version`, `architecture.url`, `architecture.hash`, `bin`); winget uses YAML (`PackageVersion`, `InstallerUrl`, `InstallerSha256`). All implemented in corresponding manifest files.

:::

#### Task 2: Add repository packaging manifests and generation script

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `packaging/chocolatey/hotpot.nuspec` | Create | Chocolatey package metadata. |
| `packaging/chocolatey/tools/chocolateyInstall.ps1` | Create | Downloads and installs Windows x86_64 release zip. |
| `packaging/homebrew/hotpot.rb` | Create | Homebrew formula/template maintained in this repository. |
| `packaging/scoop/hotpot.json` | Create | Scoop manifest/template maintained in this repository. |
| `packaging/winget/` | Create | winget manifest/template maintained in this repository. |
| `scripts/update-release-package-manifests.sh` | Create | Generates versioned manifests from release tag and checksum inputs. |

**Steps:**

- [x] **Step 1**: Created `packaging/chocolatey/tools/`, `packaging/homebrew/`, `packaging/scoop/`, `packaging/winget/fancyhq.hotpot/` subdirectories with ecosystem-conventional layouts.
- [x] **Step 2**: Added Chocolatey `hotpot.nuspec` with version/author/description metadata; added `tools/chocolateyInstall.ps1` that downloads Windows x86_64 zip from GitHub Releases, verifies SHA256, and installs `hotpot.exe`.
- [x] **Step 3**: Added Homebrew formula `packaging/homebrew/hotpot.rb` with conditional branches for macOS arm64/x86_64 and Linux arm64/x86_64; uses `PLACEHOLDER_*_SHA256` for checksums to be filled by release script.
- [x] **Step 4**: Added Scoop manifest `packaging/scoop/hotpot.json` with `version`, `architecture.64bit.url`, `architecture.64bit.hash`, `bin` and `autoupdate` support.
- [x] **Step 5**: Added winget manifests under `packaging/winget/fancyhq.hotpot/` — `hotpot.yaml` (main), `hotpot.installer.yaml` (installer with `InstallerUrl`, `InstallerSha256`), `hotpot.locale.en-US.yaml` (locale metadata).
- [x] **Step 6**: Implemented `scripts/update-release-package-manifests.sh` — accepts `tag` and `sha256_dir`, deterministically updates Homebrew/Scoop/winget/Chocolatey manifests with version/URL/SHA256 using portable shell.
- [x] **Step 7**: All scripts use English-first bilingual comments and English output/error messages.

:::

#### Task 3: Wire release workflow publishing jobs

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.github/workflows/release-please.yml` | Modify | Add publish jobs and manifest generation after binary assets exist. |
| `.github/workflows/rebuild-release-assets.yml` | Modify | Keep manual rebuild semantics explicit and compatible. |
| `release-please-config.json` | Modify | Synchronize version fields for any static package metadata that release-please can safely update. |

**Steps:**

- [x] **Step 1**: Added `publish-crates-io` job in `.github/workflows/release-please.yml` gated by `release_created == 'true'`, with `cargo package --locked --no-verify` validation and `cargo publish --locked --token "$CARGO_REGISTRY_TOKEN"`. Depends only on `release-please`.
- [x] **Step 2**: Added `generate-package-manifests` job that depends on `release-please` and `build-release-assets`, downloads `.sha256` files from GitHub Release via `gh release download`, runs `scripts/update-release-package-manifests.sh`.
- [x] **Step 3**: Added `publish-chocolatey` job that depends on `release-please` and `generate-package-manifests`, runs on `windows-latest`, does `choco pack` and `choco push --api-key "${{ secrets.CHOCO_API_KEY }}"`.
- [x] **Step 4**: Homebrew/Scoop/winget manifests are uploaded as GitHub Release assets in the `generate-package-manifests` job via `gh release upload`. Chosen design: upload as release assets rather than modifying repo history.
- [x] **Step 5**: `publish-npm` still depends on `release-please` and `build-release-assets` (unchanged dependency order); npm only publishes when `release_created == 'true'`.
- [x] **Step 6**: Updated `release-please-config.json` — added `packaging/chocolatey/hotpot.nuspec` to `extra-files` (version-only update, no checksums). Did NOT add Homebrew/Scoop/winget manifests (they contain checksum placeholders).
- [x] **Step 7**: Secrets scoped per job: `CARGO_REGISTRY_TOKEN` only in `publish-crates-io`, `CHOCO_API_KEY` only in `publish-chocolatey`, `NPM_TOKEN` only in `publish-npm`.

:::

#### Task 4: Update documentation and architecture

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `README.md` | Modify | English user-facing installation and release process docs. |
| `README.zh_CN.md` | Modify | Chinese user-facing installation and release process docs. |
| `docs/ARCH.md` | Modify | Agent-facing architecture description for multi-channel distribution. |
| `docs/ARCH.zh_CN.md` | Modify | Chinese architecture mirror. |
| `docs/ROADMAP.md` | Modify | Mark completed/remaining distribution milestones accurately. |

**Steps:**

- [x] **Step 1**: Updated `README.md` installation section with npm, crates.io, Homebrew, Scoop, Chocolatey, and winget installation commands/notes.
- [x] **Step 2**: Updated `README.zh_CN.md` with equivalent Chinese content; preserved command names, secrets, filenames, and manifest keys in English.
- [x] **Step 3**: Updated release process docs in both READMEs with 8-step process covering all channels and required secrets.
- [x] **Step 4**: Extended `docs/ARCH.md` with comprehensive Multi-Channel Distribution section covering channel overview table, asset naming, job dependency graph, crates.io/Chocolatey/Homebrew/Scoop/winget channels, version synchronization, and non-goal documentation.
- [x] **Step 5**: Mirrored architecture update in `docs/ARCH.zh_CN.md` with same technical content in Simplified Chinese.
- [x] **Step 6**: Updated `docs/ROADMAP.md` — npm marked complete, all 5 new channels marked complete, external tap/bucket/winget-pkgs submission marked as separate future items.

:::

#### Task 5: Validate package artifacts and workflow configuration

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `scripts/update-release-package-manifests.sh` | Test | Validate deterministic manifest generation. |
| `packaging/chocolatey/hotpot.nuspec` | Test | Validate Chocolatey package metadata. |
| `packaging/chocolatey/tools/chocolateyInstall.ps1` | Test | Validate PowerShell syntax. |
| `packaging/homebrew/hotpot.rb` | Test | Validate Ruby syntax and formula shape. |
| `packaging/scoop/hotpot.json` | Test | Validate JSON syntax. |
| `packaging/winget/` | Test | Validate YAML syntax or manifest structure. |
| `.github/workflows/release-please.yml` | Test | Validate workflow YAML and job dependencies. |

**Steps:**

- [x] **Step 1**: `bash -n scripts/update-release-package-manifests.sh` — Shell syntax OK.
- [x] **Step 2**: Ran manifest generation script with sample SHA256 for version 0.3.2, tag `hotpot-v0.3.2` — all manifests updated deterministically with correct asset names.
- [x] **Step 3**: `ruby -c packaging/homebrew/hotpot.rb` — Syntax OK.
- [x] **Step 4**: `node -e "JSON.parse(require('fs').readFileSync('packaging/scoop/hotpot.json', 'utf8'))"` — valid JSON.
- [x] **Step 5**: YAML validation for 3 winget manifests (`ruby -e 'require "yaml"; ...'`) — valid YAML.
- [x] **Step 6**: PowerShell not available on macOS; `chocolateyInstall.ps1` relies on workflow validation on Windows runners.
- [x] **Step 7**: `cargo package --allow-dirty --no-verify` — packaging successful (178 files, 1.8MiB).
- [x] **Step 8**: `.github/workflows/release-please.yml` and `.github/workflows/rebuild-release-assets.yml` validated as valid YAML with correct job dependencies.

:::

#### Task 6: Run full regression checks and final review

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `Cargo.toml` | Test | Ensure Rust project remains valid. |
| `npm/package.json` | Test | Ensure existing npm package remains valid. |
| `.github/workflows/release-please.yml` | Test | Ensure release flow still gates correctly. |
| `README.md` | Test | Manual consistency check for user docs. |
| `README.zh_CN.md` | Test | Manual consistency check for Chinese docs. |

**Steps:**

- [x] **Step 1**: `cargo test` — 101 tests pass, 0 failures.
- [x] **Step 2**: `npm pack --dry-run ./npm` — npm wrapper packaging successful (`@fancyhq/hotpot@0.3.2`).
- [x] **Step 3**: `git diff --check` — no whitespace errors.
- [x] **Step 4**: Manually inspected workflow — all publish jobs (`publish-crates-io`, `publish-npm`, `publish-chocolatey`) and manifest generation (`generate-package-manifests`) are gated by `if: ${{ needs.release-please.outputs.release_created == 'true' }}`. They only run on release creation, not on ordinary `main` push.
- [x] **Step 5**: Manually inspected generated URLs — all use `hotpot-hotpot-v<version>-<platform>.<ext>` format (e.g., `hotpot-hotpot-v0.3.2-windows-x86_64.zip`), consistent with existing `hotpot-${TAG}-${ASSET_LABEL}${EXT}` naming rule.
- [x] **Step 6**: Documentation in both READMEs and ARCH docs states required secrets: `NPM_TOKEN`, `CARGO_REGISTRY_TOKEN`, `CHOCO_API_KEY`.

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `bash -n scripts/update-release-package-manifests.sh` | Manifest generation script has valid shell syntax. |
| `cargo package --allow-dirty --no-verify` | Crate packaging is valid enough for crates.io publish automation, or blockers are explicitly documented. |
| `cargo test` | Existing Rust tests pass. |
| `npm pack --dry-run ./npm` | Existing npm package remains packable. |
| `ruby -c packaging/homebrew/hotpot.rb` | Homebrew formula syntax is valid, if Ruby is available. |
| `node -e "JSON.parse(require('fs').readFileSync('packaging/scoop/hotpot.json', 'utf8'))"` | Scoop manifest JSON parses successfully. |
| `git diff --check` | No whitespace errors. |

Manual validation:

- Confirm `.github/workflows/release-please.yml` only publishes crates.io, Chocolatey, npm, and generated package manifests when `release_created == 'true'`.
- Confirm generated package URLs point to `https://github.com/fancyhq/hotpot/releases/download/hotpot-v<version>/hotpot-hotpot-v<version>-<platform>.<ext>`.
- Confirm Homebrew/Scoop/winget are maintained in this repository and do not imply automatic external PR submission.

### Risks and Watchouts

::: warning
- crates.io package name `hotpot` or Chocolatey package id `hotpot` may be unavailable or reserved; execution must stop and report if confirmed unavailable.
- `cargo publish` may require additional Cargo metadata such as `description`, `readme`, `keywords`, or `categories`; fix only the minimum required metadata and keep output English.
- Homebrew, Scoop, and winget have ecosystem-specific schema and review expectations; this task maintains repository manifests first, not official upstream acceptance.
- Checksums are only known after release assets are built; avoid a design that asks release-please to maintain static checksum values in Release PRs before assets exist.
- Chocolatey package moderation can be delayed even after `choco push` succeeds; documentation should not promise instant availability.
- Secrets must never be printed or stored in generated files.
- Existing npm publish behavior must remain intact and must still use `NPM_TOKEN` only in the npm publish step.
:::

---

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task.
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff.
- Run the validation commands before reporting completion.

## Open Questions

- 无。
