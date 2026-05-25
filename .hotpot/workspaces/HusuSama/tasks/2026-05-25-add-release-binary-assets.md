---
title: add-release-binary-assets
description: 为 release-please 发布流程增加跨平台 release 二进制产物上传
date: 2026-05-25
category: [Task]
tag: [github-actions, release-please, release, binary-assets]
---

# Add Release Binary Assets

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| Done | false | 4 | medium |
:::

---

## Task

### Summary

::: info
更新现有 `Release Please` GitHub Actions 发布流程，使维护者合并 Release PR 后，workflow 在 `release-please` 创建 GitHub Release/tag 的同一次运行中编译 `hotpot` 的 release 二进制，覆盖 Windows、macOS x86_64/aarch64、Linux x86_64/aarch64，并把压缩包与 SHA256 校验文件上传到对应 GitHub Release。
:::

### User Request

::: info 用户原话
帮我更新 github action，在发布版本的时候，需要编译 release 二进制文件，包含 windows、macos、linux，应该是如下流程：

1. 合并 PR
2. 手动发布版本
3. 触发 action，编译各个版本的 release 二进制文件
4. 发布到 release 中
:::

用户在 brainstorming 中确认的关键决策：

- “手动发布版本”指沿用现有 `release-please` 模式：普通 PR 合并到 `main` 只累计变更并更新 Release PR；维护者手动合并 Release PR 后，`release-please` 创建 GitHub Release/tag。
- Release assets 构建应挂在 Release PR 合并后的 `release_created` 分支上，而不是普通 PR 合并、普通 `main` push 或手动创建 tag。
- 产物目标需要包含 Windows、macOS x86_64、macOS aarch64、Linux x86_64、Linux aarch64。
- 需要生成 SHA256 校验文件并上传到 release。
- 本任务暂不包含 crates.io、Homebrew、Scoop、Chocolatey 发布；这些渠道需要单独评估和后续任务。
- TDD 模式关闭：`tdd: false`。
- 执行策略为当前 checkout：`git-worktree: false`。

### Approved Design

::: tip
沿用并增强现有 `.github/workflows/release-please.yml`，保留 `googleapis/release-please-action@v4` 和当前 `release-please-config.json` / `.release-please-manifest.json` 配置。workflow 仍在 `push` 到 `main` 时运行，用于创建或更新 Release PR；只有当 Release PR 被合并并由 release-please 创建 GitHub Release 时，后续构建 job 才运行。

实现应优先使用 `release-please-action` 的输出作为 gate，例如检查 `steps.release.outputs.release_created == 'true'`，并把发布 tag 传给后续 matrix job。执行代理必须先核对当前 action v4 文档中的准确输出名，避免使用过期示例。

构建 job 使用 GitHub Actions matrix 覆盖 5 个 release assets：`windows-x86_64`、`macos-x86_64`、`macos-aarch64`、`linux-x86_64`、`linux-aarch64`。每个 matrix entry 应指定 runner、Rust target、二进制文件名、压缩包名和 checksum 文件名。Windows 产物使用 `.zip`；macOS/Linux 产物使用 `.tar.gz`。上传建议使用 GitHub CLI `gh release upload <tag> <files> --clobber` 或等价官方/维护良好的 action，确保上传到 release-please 刚创建的 release。

Linux aarch64 交叉编译可能需要额外 linker 或专用 Rust cross build action。执行代理需要选择最小可靠方案，并在 workflow 中注释说明原因；不要引入包管理器发布、安装脚本或额外发布渠道。
:::

### Alternatives Considered

- 监听 `release.published` 事件单独构建：语义直观，但会把 release-please 创建 release 与二进制上传拆成两个 workflow，状态追踪和权限配置更分散；本次未选。
- 使用 `workflow_dispatch` 手动输入 tag/version 构建：可以人工控制，但绕开现有 Release PR 闸门，容易与 release-please 版本状态不一致；本次未选。
- 在现有 release-please workflow 中基于 `release_created` 输出追加构建上传：已选择，因为它最贴合“合并普通 PR -> 手动合并 Release PR -> 自动构建并上传 release assets”的目标，同时保持发布状态来源单一。
- 把 crates.io、Homebrew、Scoop、Chocolatey 发布一起纳入：暂不选择，因为各渠道需要独立凭据、manifest/仓库维护、安装验证和回滚策略，范围显著大于 GitHub Release assets。

### Requirements

- 更新 `.github/workflows/release-please.yml`，保留现有 release-please Release PR 聚合语义。
- 普通 PR 合并到 `main` 不得触发二进制 release assets 上传；只有 release-please 实际创建 GitHub Release/tag 时才运行构建上传 job。
- 构建 release 模式二进制，命令应等价于 `cargo build --release --target <target>`。
- 至少覆盖这些 Rust targets 或等价目标：`x86_64-pc-windows-msvc`、`x86_64-apple-darwin`、`aarch64-apple-darwin`、`x86_64-unknown-linux-gnu`、`aarch64-unknown-linux-gnu`。
- 每个目标上传一个压缩包和一个 SHA256 校验文件。
- Asset 命名必须清晰包含 `hotpot`、release tag 或 version、平台和架构；执行代理可根据 GitHub Actions 可用变量确定最终命名，但必须避免多个 matrix entry 互相覆盖。
- Windows 压缩包应包含 `hotpot.exe`；macOS/Linux 压缩包应包含 `hotpot`。
- workflow 权限必须足以创建 release 并上传 assets，通常需要 `contents: write`。
- 配置、脚本和注释遵守项目约定：输出文本使用 English；如写自然语言注释，使用中英双语风格。

### Non-Goals

::: details Non-Goals
- 不发布 crates.io。
- 不发布 Homebrew formula 或 tap。
- 不发布 Scoop manifest。
- 不发布 Chocolatey package。
- 不新增安装脚本、自动更新器或包管理器文档。
- 不改变 Hotpot CLI 的 Rust 业务逻辑、命令行为或 agent 执行流程。
- 不改变 release-please 的核心版本策略，除非构建上传必须修正现有配置。
- 不手动创建真实 tag 或 GitHub Release。
:::

### Project Context

- 当前仓库是 Rust CLI crate，`Cargo.toml` 中 `[package]` 为 `name = "hotpot"`、`version = "0.1.0"`、`edition = "2024"`、`repository = "https://github.com/fancyhq/hotpot"`。
- `Cargo.toml` 已配置 `[profile.release]`，目标是最小体积 release binary：`opt-level = "z"`、`lto = true`、`codegen-units = 1`、`strip = "symbols"`、`panic = "abort"`。
- 当前存在 `.github/workflows/release-please.yml`，workflow 名称为 `Release Please`，触发条件为 `push` 到 `main` 和 `workflow_dispatch`，job 调用 `googleapis/release-please-action@v4`，参数为 `config-file: release-please-config.json` 和 `manifest-file: .release-please-manifest.json`。
- `release-please-config.json` 当前根包配置为 `release-type: rust`，并把 `Cargo.lock` 放入 `extra-files`。
- `.release-please-manifest.json` 当前记录根包版本为 `0.1.0`。
- 之前的 Hotpot 任务 `2026-05-25-add-release-please-workflow.md` 已完成新增 release-please 基础流程；本任务是在该流程上增加 release binary assets。
- `AGENTS.md` 要求如果修改影响 Hotpot 执行流，需要更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`。本任务主要修改仓库发布 CI，不改变 Hotpot agent 执行流；通常不需要更新架构文档，除非执行中发现发布流程被项目架构文档显式描述。

---

## Plan

### Mode

- tdd: false

### Execution Strategy

- git-worktree: false
- rationale: 本任务是集中修改 GitHub Actions release workflow 和可能的发布说明，用户已确认直接在当前 checkout 执行；执行代理必须避免覆盖无关本地改动。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `.github/workflows/release-please.yml` | Modify | 增加 release-created gate、跨平台 matrix 构建、打包、SHA256 生成和 release asset 上传。 |
| `release-please-config.json` | Test | 确认现有 manifest 配置仍与 release-please 输出和 tag 命名兼容。 |
| `.release-please-manifest.json` | Test | 确认当前版本状态仍作为 release-please manifest 输入。 |
| `Cargo.toml` | Test | 确认 binary 名称、release profile 和 Rust target 构建假设。 |
| `Cargo.lock` | Test | 确认 lockfile 存在且 workflow 构建应使用锁定依赖。 |
| `README.zh_CN.md` | Modify | 如需要，补充维护者发布流程说明：合并 Release PR 后 assets 自动上传，包管理器发布暂未覆盖。 |
| `docs/ROADMAP.md` | Modify | 如存在相关发布 assets 或包管理器发布待办，更新状态或补充后续项。 |

### Implementation Tasks

#### Task 1: 确认 release-please 输出与构建矩阵方案

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.github/workflows/release-please.yml` | Test | 确认当前 workflow 结构和可插入 job 的位置。 |
| `release-please-config.json` | Test | 确认 manifest mode 配置与 action 输出语义。 |
| `.release-please-manifest.json` | Test | 确认根 package key 为 `.`。 |
| `Cargo.toml` | Test | 确认 binary 名称和 release profile。 |

**Steps:**

- [x] **Step 1**: 查阅当前 `googleapis/release-please-action@v4` 文档，确认 manifest mode 下可用于 gating 的输出名：`release_created` 和 `tag_name`（在 root component `.` 下直接可用）。
- [x] **Step 2**: 检查 `.github/workflows/release-please.yml`，当前没有 `id: release`。将在 Task 2 中添加。
- [x] **Step 3**: 确认 matrix 目标：Windows 使用 `windows-latest` + `x86_64-pc-windows-msvc`；macOS x86_64 使用 `macos-13` + `x86_64-apple-darwin`；macOS aarch64 使用 `macos-latest` + `aarch64-apple-darwin`；Linux x86_64 使用 `ubuntu-latest` + `x86_64-unknown-linux-gnu`；Linux aarch64 也使用 `ubuntu-latest` + `aarch64-unknown-linux-gnu`（需交叉编译）。
- [x] **Step 4**: 对 Linux aarch64 选择最小可靠方案：使用 `gcc-aarch64-linux-gnu` 系统交叉链接器，通过 `apt-get install` 安装。项目无 OpenSSL 等复杂的本地 C 依赖（`libc`/`fs2` 仅使用标准系统调用），无需引入 `cross`、`actions-rust-cross` 等更重的工具。在 workflow 中通过 `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER` 环境变量指定链接器。
- [x] **Step 5**: 确定 asset 命名模板：`hotpot-<tag_name>-<asset_label>.<ext>`，例如 `hotpot-v0.1.0-linux-aarch64.tar.gz`，对应 `.sha256` 文件为 `hotpot-v0.1.0-linux-aarch64.tar.gz.sha256`。使用 shell 变量生成以避免 GitHub Actions 表达式嵌入文件名带来的限制。

:::

#### Task 2: 增强 release-please workflow

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.github/workflows/release-please.yml` | Modify | 添加 release gate、build job matrix、打包、checksum 和上传逻辑。 |

**Steps:**

- [x] **Step 1**: 在 `release-please` job 中为 action step 添加了 `id: release`，并声明了 job `outputs`（`release_created` 和 `tag_name`）。
- [x] **Step 2**: 新增了 `build-release-assets` job，设置 `needs: release-please`，用 `if: ${{ needs.release-please.outputs.release_created == 'true' }}` gate 确保只有 release-please 实际创建 release 时才运行。
- [x] **Step 3**: 为 build job 添加了 matrix，覆盖 5 个条目：`windows-x86_64`、`macos-x86_64`、`macos-aarch64`、`linux-x86_64`、`linux-aarch64`，每项包含 `os`、`target`、`suffix`、`archive_ext` 和 `asset_label`。
- [x] **Step 4**: 在每个 matrix entry 中 checkout 代码（指定 tag ref）、使用 `dtolnay/rust-toolchain@stable` 安装 stable Rust toolchain 并添加对应 target、执行 `cargo build --release --target ${{ matrix.target }}`。
- [x] **Step 5**: 添加了跨平台打包步骤：Windows 生成 `.zip` 且包含 `hotpot.exe`；macOS/Linux 生成 `.tar.gz` 且包含 `hotpot`。通过 staging 目录确保 archive 内部文件名对用户友好。
- [x] **Step 6**: 为每个 archive 生成 SHA256 文件。Windows runner 使用 PowerShell `Get-FileHash -Algorithm SHA256`；Unix runner 使用 `shasum -a 256`。输出格式为标准 `<hash>  <filename>`。
- [x] **Step 7**: 使用 `gh release upload "${TAG}" "${ARCHIVE}" "${ARCHIVE}.sha256" --clobber` 上传 archive 和 `.sha256`，通过 `GITHUB_TOKEN: ${{ github.token }}` 授权。
- [x] **Step 8**: 保持了现有 `contents: write`、`issues: write`、`pull-requests: write` 权限，未新增不需要的权限。

:::

#### Task 3: 更新维护说明和后续发布范围记录

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `README.zh_CN.md` | Modify | 如已有发布说明区域，补充维护者如何触发 release assets。 |
| `docs/ROADMAP.md` | Modify | 如有发布或包管理器相关条目，标记 GitHub Release assets 进展并保留包管理器后续项。 |
| `.github/workflows/release-please.yml` | Modify | 在复杂交叉编译或上传步骤旁补充必要双语注释。 |

**Steps:**

- [x] **Step 1**: 检查 `README.zh_CN.md`，已有发布流程说明（步骤 1-3）。在第 3 步后补充了第 4 步（自动构建）和说明：二进制 assets 仅在合并 Release PR 后自动构建上传，包管理器发布不在当前流程覆盖范围内。
- [x] **Step 2**: 检查 `docs/ROADMAP.md`，在已完成的 release-please 条目上方新增了 checked entry 标记 GitHub Release assets 已完成，并更新包管理器条目说明为 `crates.io / Homebrew / Scoop / Chocolatey 需后续单独评估`。
- [x] **Step 3**: workflow 的 Linux aarch64 交叉编译方案（`gcc-aarch64-linux-gnu`）已在步骤上方添加了双语注释（English + Chinese），解释了为什么不需要 `cross` 或 `actions-rust-cross` 等更重的工具。
- [x] **Step 4**: 未在 workflow 中添加任何包管理器发布步骤；README.zh_CN.md 和 ROADMAP.md 中说明了包管理器发布是后续工作。

:::

#### Task 4: 验证 workflow 语法与本地构建假设

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.github/workflows/release-please.yml` | Test | 校验 YAML、触发 gate、matrix 和上传逻辑。 |
| `release-please-config.json` | Test | 确认 JSON 格式仍有效。 |
| `.release-please-manifest.json` | Test | 确认 JSON 格式仍有效。 |
| `Cargo.toml` | Test | 本地 Rust 验证确保 workflow 改动没有掩盖编译问题。 |

**Steps:**

- [x] **Step 1**: 运行 `cargo check`；通过。
- [x] **Step 2**: 运行 `cargo test`；101 个测试全部通过。
- [x] **Step 3**: 运行 `cargo build --release`；release binary 构建通过。
- [x] **Step 4**: 运行 `python -m json.tool release-please-config.json`；JSON 解析成功。
- [x] **Step 5**: 运行 `python -m json.tool .release-please-manifest.json`；JSON 解析成功。
- [x] **Step 6**: 运行 `ruby -e 'require "yaml"; YAML.load_file(".github/workflows/release-please.yml")'`；YAML 解析成功，显示 Jobs: `release-please, build-release-assets`。
- [x] **Step 7**: 人工审查：`build-release-assets` job 的 gate 是 `if: ${{ needs.release-please.outputs.release_created == 'true' }}`。当普通 PR 合并到 `main` 时 `release_created` 为 `false`，build job 跳过；仅当 Release PR 合并后 release-please 创建了 GitHub Release/tag 时 `release_created` 为 `true`，build job 运行。
- [x] **Step 8**: 运行 `git diff --check`；无 whitespace error。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo check` | Rust 项目编译检查通过，无新增编译错误。 |
| `cargo test` | 所有现有测试通过。 |
| `cargo build --release` | 当前平台 release binary 构建通过。 |
| `python -m json.tool release-please-config.json` | JSON 解析成功。 |
| `python -m json.tool .release-please-manifest.json` | JSON 解析成功。 |
| `ruby -e 'require "yaml"; YAML.load_file(".github/workflows/release-please.yml")'` | workflow YAML 可被解析；如果本机无 Ruby，使用等价 YAML linter 并记录替代命令。 |
| `git diff --check` | 无 whitespace error。 |

手动验证：检查 `.github/workflows/release-please.yml`，确认普通 PR 合并到 `main` 只更新 Release PR；只有 release-please 创建 GitHub Release/tag 时，release assets matrix 才构建并上传 archive 与 `.sha256` 文件。

### Risks and Watchouts

::: warning
- `release-please-action@v4` 在 manifest mode 下的输出名必须按当前文档确认；错误输出会导致 build job 永远跳过或错误触发。
- Linux aarch64 交叉编译可能因 linker、OpenSSL、glibc 或 runner 环境失败；当前项目依赖偏纯 Rust，但仍要选择可靠构建方案并在 workflow 中体现。
- macOS 双架构构建依赖 runner 和 Rust target 支持；执行代理需要确认 `x86_64-apple-darwin` 与 `aarch64-apple-darwin` 在所选 runner 上可用。
- `strip = "symbols"` 在 release profile 中由 cargo 处理；部分 cross target 对 strip/linker 支持可能不同，若失败应优先修正 workflow toolchain，而不是移除 release profile。
- `gh release upload --clobber` 会覆盖同名 asset；这是重跑 workflow 时有用的幂等行为，但 asset 命名必须唯一，避免不同平台互相覆盖。
- GitHub 仓库或组织级 `GITHUB_TOKEN` 权限可能限制 release asset 上传；workflow 应声明 `contents: write`，但实际失败仍可能需要仓库设置调整。
- 包管理器发布不在本任务范围内；不要因为用户询问过工作量就把 crates.io/Homebrew/Scoop/Chocolatey 加进本次 workflow。
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
