# Auto Commit Task Done Record

## Task

### Summary

在 `/hotpot:finish-work` 中，当用户已经选择自动创建任务 commit 后，`hotpot task done --commit <SHA>` 会把 commit hash 和 Done 状态写入 Hotpot ledger，产生新的 `overview.jsonl` diff。本任务要求 finish-work 在该写入成功后自动把这份 ledger diff 再提交一次，提交信息固定为 `chore: record task Done`，不再二次询问用户确认，避免用户手动处理收尾元数据 diff。

### User Request

用户原始请求："当前在 `hotpot:finish-work` 后，如果用户选择创建 commit，将会写入 commit hash，在填写 commit 会产生新的 diff，希望将这个写入的 diff 自动 commit 一次，无需提醒用户确认"。

已批准设计：采用 prompt 流程改造，不新增 Rust CLI 能力。执行阶段更新共享 finish-work prompt、当前已安装 prompt，以及中英文架构文档，使后续执行 agent 按新流程自动提交 ledger 写入 diff。

### Approved Design

在 `assets/prompts/hotpot-finish-work.md` 中扩展 finish-work 流程：当且仅当用户在 Optional Git Commit 步骤中选择由 finish-work 自动创建 commit，并且 `hotpot task done --commit <SHA>` 成功后，新增一个自动的 ledger 记录提交步骤。该步骤检查 `overview.jsonl` 是否因为写入 Done 状态和 commit hash 产生了尚未提交的 diff；如果存在，则只 stage 对应的 workspace ledger 文件并执行 `git commit`，subject 固定为 `chore: record task Done`。该操作继承用户前面"自动创建 commit"的确认，不再额外弹确认。若没有 ledger diff，则跳过。若提交失败，原始任务已标记 Done，不应回滚；需要在最终报告里说明剩余 blocker。

同步要求：

- 修改源资产 `assets/prompts/hotpot-finish-work.md`，保证 `hotpot init` / `hotpot update` 后新项目获得新流程。
- 同步修改当前项目已安装的 `.hotpot/prompts/hotpot-finish-work.md`，保证当前仓库立即可用。
- 更新 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md`，因为该改动改变了 finish-work 执行流程。
- 如 ROADMAP 中已有对应待办，可在完成实现时将其勾选或移除，保持路线图准确。

### Alternatives Considered

- 新增 Rust CLI 能力：可以把"标记 Done + 自动提交 ledger"做成机械命令，但改动范围更重，还会把 git 提交流程耦合进 CLI 状态机；本需求本质是 slash-command orchestration，因此未选。
- 只提醒用户手动提交：实现最小，但不满足"无需提醒用户确认"的核心诉求。
- Prompt 流程改造：改动集中在共享 finish-work prompt 和文档，跨平台 prompt 都引用共享主体，符合当前架构并保持 CLI 状态机简单，因此被批准。

### Requirements

- 当用户选择自动创建任务 commit 后，`hotpot task done --commit <SHA>` 产生的 ledger diff 必须自动提交一次。
- 自动 ledger 提交不再询问用户确认。
- 自动 ledger 提交信息必须使用 `chore: record task Done`。
- 自动 ledger 提交只能 stage Hotpot workspace ledger 相关文件，不能使用 `git add -A` 或提交无关用户改动。
- 如果用户跳过 commit，或选择"我已经手动提交，用 HEAD 作为任务 commit"，默认不触发自动 ledger 提交，除非执行时明确判断这也符合已确认的自动提交语义；本任务批准范围只覆盖用户选择 finish-work 自动 commit 的路径。
- `hotpot task done` 成功后若 ledger 自动提交失败，不得回滚任务状态；必须在最终报告中说明 blocker。
- 当前已安装 prompt 与源资产 prompt 必须保持一致。
- 因执行流程变化，必须更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`。

### Non-Goals

- 不新增 Rust CLI 子命令或 git 自动提交 API。
- 不改变 `hotpot task done` 的状态机语义或 `overview.jsonl` 写入格式。
- 不自动提交 issue promotion、候选清理或其他非 task-done ledger diff，除非它们已经包含在用户确认的首个任务 commit 中。
- 不改变 worktree disposal 的三选一确认要求。

### Project Context

- 共享 finish-work prompt 源文件位于 `assets/prompts/hotpot-finish-work.md`，会通过 `src/assets/shared.rs` 安装到 `.hotpot/prompts/hotpot-finish-work.md`。
- 当前已安装共享 prompt 位于 `.hotpot/prompts/hotpot-finish-work.md`，需要同步修改，保证当前仓库的 `/hotpot:finish-work` 立即生效。
- 平台薄壳如 `.opencode/commands/hotpot/finish-work.md` 只引用共享 prompt，不应复制完整逻辑。
- `docs/ARCH.md` / `docs/ARCH.zh_CN.md` 第 59 行附近描述 finish-work 为"optional git commit → task done [--commit <SHA>]"，第 78 行附近的流程图也需要补充自动 ledger commit。
- `docs/ROADMAP.md` 第 14 行已有待办：`在自动 commit 并写入 commit hash 后，再次将这个写入，提交一个 commit，使用信息：chore: record task Done`。
- `src/task/transitions.rs::mark_task_done` 已负责写入 Done 状态和 commit hash；本任务不要求修改该代码。
- `src/commands/task.rs::done_task` 调用 `task::mark_task_done` 并输出更新后的 JSON；本任务只要求 finish-work prompt 在该命令之后继续做 git orchestration。
- 当前 `cargo run -- task list --json` 显示新任务创建前没有 live active；本任务已用默认 active 创建。

## Plan

### Mode

- tdd: false

### File Map

- Modify: `assets/prompts/hotpot-finish-work.md` - 源 finish-work 流程主体，新增 task-done ledger 自动提交步骤和约束。
- Modify: `.hotpot/prompts/hotpot-finish-work.md` - 当前仓库已安装 prompt，需要与源资产同步。
- Modify: `docs/ARCH.md` - 英文架构说明，记录 finish-work 现在会在任务 commit 后自动提交 ledger 写入 diff。
- Modify: `docs/ARCH.zh_CN.md` - 中文架构说明，与英文文档保持等价。
- Modify: `docs/ROADMAP.md` - 将已实现的对应待办标记完成或移除。

### Implementation Tasks

#### Task 1: Update Finish-Work Prompt Flow

**Files:**

- Modify: `assets/prompts/hotpot-finish-work.md`
- Modify: `.hotpot/prompts/hotpot-finish-work.md`

- [x] Step 1: 在 `assets/prompts/hotpot-finish-work.md` 中检查 Required Flow、Optional Git Commit、Mark the Task Done、Final Response、Constraints 段落，确定最小插入点。
- [x] Step 2: 新增一个独立章节，例如 `## Auto-Commit Task-Done Ledger Diff`，放在 `Mark the Task Done` 之后、`Offer to Resume Next Task` 之前。
- [x] Step 3: 更新 Required Flow：在 `hotpot task done [--commit <SHA>]` 成功后，如果前面是 finish-work 自动创建 commit，则执行自动 ledger commit；如果没有 commit 或用户选择手动 HEAD，则跳过。
- [x] Step 4: 在新增章节中写明具体命令策略：用 `git status --porcelain -- .hotpot/workspaces/<username>/overview.jsonl` 或从 `hotpot task done` 输出/active path 推导当前 workspace ledger 路径；只 `git add -- <overview-jsonl-path>`；运行 `git commit -m "chore: record task Done"`；提交成功后捕获 SHA 供最终报告使用。
- [x] Step 5: 写明 worktree 情况下的路径和命令位置：如果附加 worktree 存在，task work commit 在 worktree 中；`hotpot task done` 和 ledger 自动提交在 `hotpot worktree remove` 后回到主仓执行，不能再对已移除 worktree 运行 git 命令。
- [x] Step 6: 写明失败处理：ledger 自动提交失败不回滚 Done 状态；保留 stderr 给用户，并在最终报告列为 leftover blocker。
- [x] Step 7: 更新 Final Response，包含 task commit SHA 和 ledger-record commit SHA（如有）。
- [x] Step 8: 更新 Constraints，明确前置自动 commit 确认覆盖 ledger-record commit，禁止二次确认，且自动提交只允许 stage task-done ledger 文件。
- [x] Step 9: 将同样改动同步到 `.hotpot/prompts/hotpot-finish-work.md`，保持两个文件相关段落一致。

#### Task 2: Update Architecture And Roadmap

**Files:**

- Modify: `docs/ARCH.md`
- Modify: `docs/ARCH.zh_CN.md`
- Modify: `docs/ROADMAP.md`

- [x] Step 1: 在 `docs/ARCH.md` 的命令表和执行流程图中，把 finish-work 从"optional git commit → task done [--commit <SHA>]"更新为包含"auto commit task-done ledger diff"的流程。
- [x] Step 2: 在 `docs/ARCH.zh_CN.md` 做等价中文更新，保持结构与含义和英文版本一致。
- [x] Step 3: 更新 `docs/ROADMAP.md` 第 14 行对应待办：如果实现完成则改为 `- [x] ...`，或从未完成列表移除；优先使用勾选以保留历史。
- [x] Step 4: 确认文档中 structural tokens 如 `/hotpot:finish-work`、`hotpot task done --commit <SHA>`、`chore: record task Done` 保持英文原样。

#### Task 3: Validate Prompt Consistency

**Files:**

- Modify: `assets/prompts/hotpot-finish-work.md`
- Modify: `.hotpot/prompts/hotpot-finish-work.md`
- Modify: `docs/ARCH.md`
- Modify: `docs/ARCH.zh_CN.md`
- Modify: `docs/ROADMAP.md`

- [x] Step 1: Run `diff -u assets/prompts/hotpot-finish-work.md .hotpot/prompts/hotpot-finish-work.md` and expect no differences, or only explainable installation-context differences already present before the task.
- [x] Step 2: Run `cargo fmt --check` and expect success; no Rust source changes are expected, but this confirms formatting remains clean if execution touched Rust unexpectedly.
- [x] Step 3: Run `cargo test` and expect success, or if warnings remain, ensure there are no new failures caused by prompt/doc changes.
- [x] Step 4: Search for `record task Done` and verify it appears in the finish-work prompts and ROADMAP status consistently.

### Validation

- `diff -u assets/prompts/hotpot-finish-work.md .hotpot/prompts/hotpot-finish-work.md` - expected no unintentional divergence between source and installed prompt.
- `cargo fmt --check` - expected success.
- `cargo test` - expected success.
- Search for `chore: record task Done` - expected finish-work prompt documents the automatic ledger commit subject, and ROADMAP no longer shows the item as unfinished.

### Risks and Watchouts

- The automatic ledger commit must not accidentally stage user files or the task work changes; explicit-path `git add -- <overview-jsonl-path>` is mandatory.
- Worktree mode is subtle: task implementation commit happens on `hotpot/<task-id>`, but task metadata lives in the main workspace ledger; the ledger-record commit must be described in a way that avoids operating inside a removed worktree.
- The SHA passed to `hotpot task done --commit <SHA>` must remain the task work commit SHA, not the ledger-record commit SHA.
- If `overview.jsonl` was already included in the first user-approved commit, the second commit may have no diff and should skip cleanly.
- Keep code/output examples in English; prose in this task file is Chinese per project language preference.

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task.
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff.
- Run the validation commands before reporting completion.
