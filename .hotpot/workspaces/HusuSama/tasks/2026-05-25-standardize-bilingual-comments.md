---
title: Standardize Bilingual Comments
description: Normalize bilingual comments and English-only CLI output across Hotpot.
date: 2026-05-25
category: [Task]
tag: [comments, i18n, cli-output, docs]
---

# Standardize Bilingual Comments

::: info Overview
| | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| Done | false | 5 | high |
:::

---

## Task

### Summary

::: info
规范 Hotpot 全项目的注释和用户可见输出：注释统一为 English first、中文简短补充；所有代码输出、错误消息和 command help/说明保持英文，避免 CLI 与自动化协议出现中英文混杂。
:::

### User Request

::: info 用户原话
优化整个项目注释，使用中英双语注释进行，如只有英文则需添加中文，整体注释为先英文，后中文的模式，中文只需要编写简短注释即可，有详情的注释仅使用英文即可。另外，所有输出，比如 println、bail 之类的，全部使用英文，commands 说明统一使用英文说明，不需要中文。
:::

### Approved Design

::: tip
采用一次全项目规范化任务，但按文件组拆分执行和 review。执行代理需要扫描 Rust、TypeScript、prompt/assets、docs 与配置文件中的注释，修复缺失双语或顺序不一致的注释；同时扫描 Rust CLI 输出路径，把 `println!`、`eprintln!`、`bail!`、`anyhow!`、clap command/arg help 等用户可见文案统一为英文。

注释规则：英文注释在前，中文简短注释在后；如果英文说明已经很详细，中文只写短句补充意图；不要为显而易见代码增加噪音注释；保持现有文档性长注释的技术细节主要使用英文。

输出规则：任何运行时输出、错误、warning、command 描述、clap help 文案必须使用英文；保留机器可读 token 和协议字面量，例如 `ACTIVE_CONFLICT:`、`Not found active task`、JSON keys、CLI flags。
:::

### Alternatives Considered

- 只处理 Rust 源码：范围较小，但会遗漏 `assets/platforms/pi/extensions/hotpot/index.ts`、prompt assets、`Cargo.toml`、`docs/ARCH*.md` 等当前已存在注释顺序不一致的位置，无法满足“整个项目”。
- 一次性全自动替换所有注释：速度快，但高风险，容易破坏长文档注释、prompt 示例、Markdown/VuePress 容器和机器可读 anchors。
- 批量扫描、分组手工修复并逐步验证：覆盖面完整，可控制风险，适合本任务；已批准。

### Requirements

- 所有新增或修改后的代码注释采用 English first，随后中文简短补充。
- 已有仅英文且有价值的注释补充简短中文；已有仅中文且有价值的注释补充英文并放在中文之前。
- 详细技术说明优先保留英文完整性，中文只需要概括用途或关键约束。
- 不为显而易见的赋值、简单分支或重复逻辑增加低价值注释。
- 所有代码输出、错误消息、warning、clap command/arg help、command 说明统一英文。
- 保留协议和结构性英文 token，不翻译 `ACTIVE_CONFLICT:`、`Not found active task`、JSON keys、CLI flags、Markdown anchors、`tdd: true|false`、`git-worktree: true|false`。
- 如果注释规范化影响执行流或架构约束，需要同步更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`。

### Non-Goals

::: details Non-Goals
- 不改变业务逻辑、任务状态机、锁机制、VuePress 行为或平台 hook 行为。
- 不重写 prompt 工作流内容，除非只是修复注释或明确的英文输出说明。
- 不引入新的 lint 工具或格式化依赖。
- 不迁移文件结构，不拆分超过 1000 行文件；若执行中认为必须拆分，先停止并询问用户。
- 不翻译自然语言文档主体为双语；本任务聚焦代码/配置/资产注释与代码输出。
:::

### Project Context

- 项目是 Rust CLI `hotpot`，架构要求见 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md`。
- `AGENTS.md` 已规定所有函数、文件等应有双语 doc comments，代码输出必须英文。
- 当前探索发现已有双语注释，但顺序不统一：例如 `src/task/create.rs` 大多 English first，`src/task/mod.rs`、`Cargo.toml` 中存在中文在前的注释。
- 当前探索发现仍有中文代码输出或错误，例如 `src/server.rs`、`src/task/transitions.rs`、`src/task/mod.rs` 测试输出、`src/vuepress.rs`、`src/commands/issues.rs`、`src/worktree/mod.rs` 等。
- 主要验证命令来自 Rust 项目：`cargo fmt --check`、`cargo test`、`cargo clippy --all-targets --all-features -- -D warnings`。

---

## Plan

### Mode

- tdd: false

### Execution Strategy

- git-worktree: false
- rationale: 用户选择在当前 checkout 执行。任务范围较广，执行代理必须小步修改并在每组修改后检查 diff，避免误改用户并行变更。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/**/*.rs` | Modify | 统一 Rust doc comments、普通注释、CLI 输出、错误消息和 clap help 文案。 |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Modify | 统一 TypeScript 注释顺序，保持工具层输出英文。 |
| `assets/platforms/opencode/plugins/*.ts` | Modify | 检查并补齐插件注释的中文简短补充，保持工具输出英文。 |
| `assets/prompts/*.md` | Modify | 检查 prompt asset 中的注释块和 command-facing 说明，避免破坏结构 anchors。 |
| `assets/platforms/**/*.{md,toml,json}` | Modify | 检查平台资产注释和 command/agent 说明，保持 command 说明英文。 |
| `Cargo.toml` | Modify | 调整依赖与 profile 注释为 English first、中文简短补充。 |
| `docs/ARCH.md` | Modify | 如注释/输出语言约束或执行注意事项有变化，同步英文架构说明。 |
| `docs/ARCH.zh_CN.md` | Modify | 与 `docs/ARCH.md` 同步的中文架构说明。 |

### Implementation Tasks

#### Task 1: Establish Comment And Output Inventory

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/**/*.rs` | Modify | 找出中文在前、仅中文、仅英文、以及中文输出。 |
| `assets/**/*` | Modify | 找出资产注释和 command-facing 说明。 |
| `docs/ARCH*.md` | Modify | 确认需要同步记录的约束。 |
| `Cargo.toml` | Modify | 检查 TOML 注释顺序。 |

**Steps:**

- [x] **Step 1**: Run `rg --color never -n '^\s*(//!|///|//|#|<!--|/\*|\*)' src assets docs Cargo.toml` and use it as the comment inventory; expect matching comment lines across Rust, TS, Markdown, TOML, and assets.
- [x] **Step 2**: Run `rg --color never -n 'println!|eprintln!|bail!|anyhow!|\.about\(|\.help\(|Command::new|Arg::new|clap' src` and use it as the output/help inventory; expect all user-visible output sites to be reviewed.
- [x] **Step 3**: Identify machine-readable strings that must remain unchanged, especially `ACTIVE_CONFLICT:`, `Not found active task`, JSON keys, command names, env vars, and markdown anchors.
- [x] **Step 4**: Before editing any file, inspect `git status --short`; do not revert or overwrite unrelated user changes.

:::

#### Task 2: Normalize Rust Comments And English Outputs

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/**/*.rs` | Modify | Main implementation surface for comments, errors, output, and clap help. |

**Steps:**

- [x] **Step 1**: In every touched Rust module, ensure module docs `//!`, item docs `///`, and meaningful inline comments are English first, followed by short Chinese comments where useful.
- [x] **Step 2**: Reorder existing bilingual comments that currently put Chinese first, without changing semantics.
- [x] **Step 3**: Add short Chinese complements to valuable English-only comments; add English complements before valuable Chinese-only comments.
- [x] **Step 4**: Convert all user-visible `println!`, `eprintln!`, `bail!`, `anyhow!`, and clap help/about strings to English, except machine-readable protocol tokens that must remain exact.
- [x] **Step 5**: Pay special attention to the currently observed files: `src/server.rs`, `src/task/transitions.rs`, `src/task/mod.rs`, `src/vuepress.rs`, `src/commands/issues.rs`, `src/worktree/mod.rs`, `src/commands/task.rs`, `src/commands/update.rs`, and `src/assets/mod.rs`.
- [x] **Step 6**: Run `cargo fmt --check`; expect formatting to pass. If it fails, run `cargo fmt`, inspect the formatting-only diff, then rerun `cargo fmt --check`.

:::

#### Task 3: Normalize Asset, TypeScript, Prompt, And Config Comments

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Modify | Large TS file with many existing bilingual comments and command workflow notes. |
| `assets/platforms/opencode/plugins/*.ts` | Modify | Plugin comments should follow the same bilingual convention. |
| `assets/prompts/*.md` | Modify | Prompt comments and command-facing wording should stay consistent while preserving anchors. |
| `assets/platforms/**/*` | Modify | Platform assets may contain comments, agent instructions, TOML comments, and command descriptions. |
| `Cargo.toml` | Modify | Existing comments need English-first ordering. |

**Steps:**

- [x] **Step 1**: Update `Cargo.toml` comments so English paragraphs precede Chinese summaries.
- [x] **Step 2**: Update TypeScript comments in `assets/platforms/pi/extensions/hotpot/index.ts` and `assets/platforms/opencode/plugins/*.ts` to English first, then Chinese brief notes.
- [x] **Step 3**: Review `assets/prompts/*.md` and platform command/agent assets for HTML comments, TOML comments, and command descriptions; keep structural Markdown headings and machine-readable references unchanged.
- [x] **Step 4**: Ensure command descriptions and model/tool correction strings remain English-only where they are outputs, help text, or machine-consumed instructions.
- [x] **Step 5**: Do not translate examples that intentionally show literal command names, env vars, JSON keys, slash commands, or Hotpot protocol tokens.

:::

#### Task 4: Synchronize Architecture Documentation

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify | English agent-facing architecture constraints must mention finalized comment/output convention if changed. |
| `docs/ARCH.zh_CN.md` | Modify | Chinese architecture mirror must stay aligned. |
| `AGENTS.md` | Modify | Only if the existing convention text is incomplete or contradictory. |

**Steps:**

- [x] **Step 1**: Inspect existing `AGENTS.md` convention text and avoid duplicating it unnecessarily.
- [x] **Step 2**: If implementation clarifies the convention, update `docs/ARCH.md` Notes For Future Agents with English-first bilingual comment expectations and English-only code output expectations.
- [x] **Step 3**: Mirror the same content in Simplified Chinese in `docs/ARCH.zh_CN.md` while preserving English structural tokens and file paths.
- [x] **Step 4**: If `AGENTS.md` already fully covers the convention, leave it unchanged unless a contradiction is discovered.

:::

#### Task 5: Validate And Review Diff

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/**/*.rs` | Test | Rust formatting, tests, and clippy must pass after wording changes. |
| `assets/**/*` | Test | Diff review must confirm prompt/asset anchors were not broken. |
| `docs/ARCH*.md` | Test | Documentation mirrors must stay consistent. |

**Steps:**

- [x] **Step 1**: Run `cargo fmt --check`; expect success.
- [x] **Step 2**: Run `cargo test`; expect all tests to pass.
- [x] **Step 3**: Run `cargo clippy --all-targets --all-features -- -D warnings`; expect no warnings.
- [x] **Step 4**: Run `rg --color never -n 'println!|eprintln!|bail!|anyhow!' src` and manually confirm all user-visible messages are English.
- [x] **Step 5**: Run `rg --color never -n '任务|未找到|失败|目录不存在|获取任务|现在有|新任务|最后一条|停止 server|stdin 没有' src` and confirm no Chinese user-visible Rust output remains; Chinese comments are allowed.
- [x] **Step 6**: Review `git diff -- src assets docs Cargo.toml AGENTS.md` and confirm changes are comment/output-only unless explicitly justified in the final report.

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo fmt --check` | Passes with no formatting changes needed. |
| `cargo test` | All tests pass. |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passes with no warnings. |
| `rg --color never -n 'println!|eprintln!|bail!|anyhow!' src` | Remaining user-visible strings are English or machine-readable protocol tokens. |
| `rg --color never -n '任务|未找到|失败|目录不存在|获取任务|现在有|新任务|最后一条|停止 server|stdin 没有' src` | No Chinese user-visible Rust output remains; Chinese comments may still appear. |

### Risks and Watchouts

::: warning
- 范围较大，容易出现 noisy diff；执行代理应按文件组提交思路修改并频繁检查 diff。
- Prompt assets 和 agent assets 包含结构性 anchors，不能翻译或改名。
- VuePress task Markdown 对 `:::` 容器和 inline backtick 有额外约束；修改任务文件时避免破坏容器结构。
- 某些错误字符串可能被测试或 workflow 匹配；修改前需要搜索引用，特别是 `ACTIVE_CONFLICT:` 和 `Not found active task`。
- 注释规范化不应掩盖逻辑改动；如果发现必须改逻辑，应停止并向用户报告。
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
