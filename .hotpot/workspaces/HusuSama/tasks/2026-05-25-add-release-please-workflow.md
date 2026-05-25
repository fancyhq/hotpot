---
title: add-release-please-workflow
description: 新增 release-please GitHub Actions 发布流程任务
date: 2026-05-25
category: [Task]
tag: [github-actions, release-please, release]
---

# Add Release Please Workflow

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | false | 3 | medium |
:::

---

## Task

### Summary

::: info
为 Hotpot 新增基于 `release-please` 的 GitHub Actions 发布流程，使 `main` 分支上的 conventional commits 自动汇总到同一个 Release PR 中；普通功能分支合并不会立即创建 tag 或 GitHub Release，只有维护者人工合并 Release PR 后才真正发布版本。
:::

### User Request

::: info 用户原话
帮我创建一个 git action，通过 release-please 来自动进行版本发布，但有一个问题需要讨论：我并不想每次其他分支合并到 main 都会直接创建版本，而是可以合并多个分支后再进行版本发布。
:::

用户已批准的关键决策：

- 采用“自动维护 Release PR”策略。
- `push` 到 `main` 时运行 `release-please`，但只创建或更新同一个 Release PR。
- 普通功能分支合并到 `main` 不立即发布版本。
- 维护者人工合并 Release PR 后，`release-please` 才创建 tag 和 GitHub Release。
- TDD 模式关闭：`tdd: false`。
- 执行策略为当前 checkout：`git-worktree: false`。

### Approved Design

::: tip
新增 GitHub Actions workflow 和 `release-please` 配置，目标是 Rust crate 项目。workflow 应在 `main` 分支 push 时运行 `googleapis/release-please-action`，并通过配置文件声明 Rust release 类型、changelog、manifest 和版本文件。`release-please` 的 Release PR 将承担“聚合多个分支后再发布”的闸门：它会随 `main` 上的新 conventional commits 更新，但不会在普通 feature PR merge 后立即 tag/release；真正发布发生在 Release PR 被人工合并时。

执行代理需要优先核对当前 `release-please-action` 推荐配置格式，避免使用已废弃参数。实现应尽量保持配置最小化，新增 `.github/workflows/release-please.yml`、`release-please-config.json`、`.release-please-manifest.json`，并在需要时补充 `CHANGELOG.md` 由 release-please 后续维护。
:::

### Alternatives Considered

- 自动维护 Release PR：每次 `main` 有新 conventional commit 时创建或更新一个 Release PR，人工合并该 PR 才发布。已选择，因为它满足“合并多个分支后再发布”，同时保留自动生成版本、changelog 和 tag 的能力。
- 完全手动触发：只在 `workflow_dispatch` 时创建或更新 Release PR。未选择，因为它降低自动化程度，容易忘记更新 Release PR。
- 定时或手动触发：按周或按需更新 Release PR。未选择，因为当前需求更像“持续累积、人工闸门发布”，不需要固定节奏。

### Requirements

- 新增 GitHub Actions workflow，使用 `release-please` 自动维护 Release PR。
- workflow 必须针对 `main` 分支运行，且不能让普通分支合并立即创建 tag 或 GitHub Release。
- Release PR 合并后应由 `release-please` 创建 GitHub Release/tag，并更新版本文件与 changelog。
- 配置必须适配当前 Rust crate：`Cargo.toml` 中 package 名称为 `hotpot`，当前版本为 `0.1.0`。
- 配置应覆盖 `Cargo.toml` 和 `Cargo.lock` 的版本更新需求，避免发布时版本文件不同步。
- 新增文件与注释需要遵守项目约定：代码或配置里的输出使用英文；如写自然语言注释，采用中英双语风格。
- 如发布流程会影响 README 或 ROADMAP，应同步更新相关说明或任务状态。

### Non-Goals

::: details Non-Goals
- 不实现 crates.io、npm、Homebrew、Scoop、Chocolatey 等包管理器发布。
- 不新增跨平台二进制构建和上传 release asset 的完整矩阵，除非 release-please 的最小配置必须依赖它。
- 不改变 Hotpot CLI 的执行逻辑、任务系统或架构流程。
- 不直接创建真实 GitHub Release 或 tag。
- 不在本任务中迁移仓库历史 commit message；只记录后续需要 conventional commits 才能自动生成合理 changelog。
:::

### Project Context

- 项目是单 crate Rust CLI，根目录包含 `Cargo.toml` 和 `Cargo.lock`。
- `Cargo.toml` 的 `[package]`：`name = "hotpot"`、`version = "0.1.0"`、`repository = "https://github.com/fancyhq/hotpot"`。
- 当前没有 `.github/workflows/` 目录，也没有现有 release workflow、`release-please` 配置或 `CHANGELOG.md`。
- `docs/ROADMAP.md` 第 42 行已有待办：增加 `github action` 机制，使用 `release-please` 完善发布流程，注意不能每次合并都创建新版本发布。
- `AGENTS.md` 要求在修改会影响执行流时更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`。本任务只新增仓库发布 CI，不改变 Hotpot agent 执行流，通常不需要更新架构文档；执行代理若发现新增发布流程需要被架构文档记录，应同步更新双语架构文档。

---

## Plan

### Mode

- tdd: false

### Execution Strategy

- git-worktree: false
- rationale: 用户选择直接在当前 checkout 执行；任务范围主要是新增 CI/release 配置，执行代理需注意不要覆盖无关本地改动。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `.github/workflows/release-please.yml` | Create | 定义 GitHub Actions workflow，调用 release-please 自动维护 Release PR 并在 Release PR 合并后发布。 |
| `release-please-config.json` | Create | 声明 Rust crate release 类型、包名、changelog、manifest 与版本文件策略。 |
| `.release-please-manifest.json` | Create | 记录当前 manifest 版本，初始应匹配 `Cargo.toml` 的 `0.1.0`。 |
| `CHANGELOG.md` | Create | 如 release-please 配置或最佳实践需要初始 changelog，则创建最小英文 changelog；否则让 release-please 首次 Release PR 创建。 |
| `docs/ROADMAP.md` | Modify | 将对应发布流程待办标记为完成或补充说明，前提是实现已经覆盖该需求。 |
| `README.zh_CN.md` | Modify | 如需要，补充发布流程说明，解释 Release PR 是人工发布闸门。 |
| `Cargo.toml` | Modify | 仅当 release-please 配置要求或验证发现版本字段格式需调整时修改；默认保持当前 `0.1.0`。 |
| `Cargo.lock` | Modify | 仅当版本同步或 cargo 命令导致 lockfile 必要更新时修改。 |

### Implementation Tasks

#### Task 1: 确认 release-please Rust 配置格式

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `Cargo.toml` | Test | 确认 crate 名称、版本、仓库信息。 |
| `Cargo.lock` | Test | 确认 lockfile 存在，后续版本同步需覆盖。 |
| `release-please-config.json` | Create | 根据确认后的 release-please schema 编写。 |
| `.release-please-manifest.json` | Create | 根据当前版本初始化 manifest。 |

**Steps:**

- [x] **Step 1**: 检查 `Cargo.toml` 的 `[package]` 字段，确认 `name = "hotpot"`、`version = "0.1.0"`、`repository` 指向 GitHub 仓库。
- [x] **Step 2**: 查阅当前 `googleapis/release-please-action` 与 release-please manifest 配置文档，确认 Rust crate 推荐的 `release-type`、manifest、`include-component-in-tag`、`packages`、`extra-files` 或等价字段写法。
- [x] **Step 3**: 创建 `release-please-config.json`，使用最小且明确的配置让根包作为 Rust crate 发布，并确保 changelog、`Cargo.toml`、`Cargo.lock` 版本更新可被 release-please 覆盖。
- [x] **Step 4**: 创建 `.release-please-manifest.json`，将根包初始版本设为 `0.1.0`。
- [x] **Step 5**: 如果文档显示 `CHANGELOG.md` 需要预置，创建最小英文 `CHANGELOG.md`；否则在任务记录中说明它将由首次 Release PR 创建或维护。

:::

#### Task 2: 新增 GitHub Actions workflow

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.github/workflows/release-please.yml` | Create | 执行 release-please 自动维护 Release PR。 |
| `release-please-config.json` | Modify | 与 workflow 参数保持一致。 |
| `.release-please-manifest.json` | Modify | 与 workflow 参数保持一致。 |

**Steps:**

- [x] **Step 1**: 新建 `.github/workflows/release-please.yml`，workflow 名称使用英文，例如 `Release Please`。
- [x] **Step 2**: 设置触发条件为 `push` 到 `main`，可附加 `workflow_dispatch` 作为手动补救入口，但不要让普通分支 push 触发发布流程。
- [x] **Step 3**: 设置最小必要 permissions，至少包含 `contents: write` 和 `pull-requests: write`；如当前 release-please 文档要求 `issues: write` 或 `id-token`，按文档添加并说明原因。
- [x] **Step 4**: 调用 `googleapis/release-please-action` 的当前稳定版本，传入 `config-file: release-please-config.json` 和 `manifest-file: .release-please-manifest.json` 或当前文档等价参数。
- [x] **Step 5**: 在 workflow 注释或 README 说明中明确语义：`main` push 只创建或更新 Release PR；合并 Release PR 才创建 tag 和 GitHub Release。
- [x] **Step 6**: 确认 workflow 不包含会在普通 feature PR 合并时额外打 tag、创建 release、上传二进制或发布包的步骤。

:::

#### Task 3: 文档、验证与风险检查

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ROADMAP.md` | Modify | 同步完成或解释发布流程待办。 |
| `README.zh_CN.md` | Modify | 如需要，给维护者说明如何通过 Release PR 聚合发布。 |
| `.github/workflows/release-please.yml` | Test | 审查 YAML 结构和触发条件。 |
| `release-please-config.json` | Test | 校验 JSON 格式和配置字段。 |
| `.release-please-manifest.json` | Test | 校验 JSON 格式与版本一致性。 |

**Steps:**

- [x] **Step 1**: 更新 `docs/ROADMAP.md` 中 release-please 发布流程待办，只有在实现已满足“不能每次合并都创建新版本发布”后才标记完成。
- [x] **Step 2**: 判断是否需要在 `README.zh_CN.md` 增加维护者发布说明；如增加，说明常规流程是合并多个功能 PR 后再人工合并 Release PR。
- [x] **Step 3**: 运行 `cargo check`；预期通过，且新增 CI 配置不影响 Rust 编译。
- [x] **Step 4**: 运行 `cargo test`；预期所有现有测试通过。
- [x] **Step 5**: 使用可用工具校验 JSON 文件格式，例如运行 `python -m json.tool release-please-config.json` 和 `python -m json.tool .release-please-manifest.json`；预期格式化解析成功且不修改文件。
- [x] **Step 6**: 人工审查 `.github/workflows/release-please.yml`：确认 trigger 只覆盖 `main` push 和可选手动触发，确认没有直接发布包管理器或构建二进制 asset 的额外步骤。
- [x] **Step 7**: 运行 `git diff --check`；预期没有 trailing whitespace 或 whitespace error。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo check` | Rust 项目编译检查通过，无新增编译错误。 |
| `cargo test` | 所有现有测试通过。 |
| `python -m json.tool release-please-config.json` | JSON 解析成功。 |
| `python -m json.tool .release-please-manifest.json` | JSON 解析成功，版本为 `0.1.0`。 |
| `git diff --check` | 无 whitespace error。 |

手动验证：检查 `.github/workflows/release-please.yml` 的触发条件与 release-please 配置，确认普通 feature 分支合并到 `main` 只会更新 Release PR；只有合并 Release PR 才会触发 release-please 创建 tag 和 GitHub Release。

### Risks and Watchouts

::: warning
- `release-please-action` 参数和 release-please 配置字段可能随版本变化；执行代理必须查阅当前文档，不要盲用旧示例。
- 如果仓库现有 commit message 不是 conventional commits，首次 Release PR 可能不会包含预期 changelog；需要在文档中说明后续应使用 conventional commits。
- `Cargo.lock` 是否由 release-please 自动更新依赖配置细节；必须显式验证配置覆盖 Rust lockfile 版本同步。
- GitHub 默认 `GITHUB_TOKEN` 权限受仓库设置影响；workflow 需声明必要 permissions，但实际运行仍可能被组织策略限制。
- 不要加入会在 Release PR 合并前发布二进制、crate 或其他包管理器产物的步骤，否则会破坏“人工合并 Release PR 才发布”的设计。
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
