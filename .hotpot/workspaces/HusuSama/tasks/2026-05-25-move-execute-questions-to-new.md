# Move Execute Questions To New

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | false | 3 | medium |
:::

---

## Task

### Summary

::: info
调整 Hotpot 的任务创建与执行契约：所有会影响执行方式的问题，尤其是是否使用隔离 `git-worktree`，必须在 `/hotpot:new` 阶段问清并写入任务文件；之后 `/hotpot:execute` 不再向用户提问，而是只根据已有任务文件执行。
:::

### User Request

::: info 用户原始需求
目前执行任务时有点割裂，在使用 `hotpot:execute` 后，不应该询问问题，而是直接根据已存在的任务文件执行，在编写计划文件前应该询问清所有的问题，比如是否使用 `git-worktree` 等，这类问题应该在执行前就确定，并在任务文件中说明是否使用 `git-worktree`，之后执行 `agent` 再根据此说明来确定。
:::

补充决策：

- 本任务采用“严格按任务文件”行为：`/hotpot:execute` 缺失或读到非法执行策略时停止并要求重新规划或修订任务文件，不在执行阶段现场询问。
- 本任务自身执行时不使用隔离 `git-worktree`，任务文件写明 `git-worktree: false`。
- 本任务使用默认 review-driven 流程，`tdd: false`。

### Approved Design

::: tip
把“执行策略”前移到 `/hotpot:new` 生成任务文件之前确定，并将策略写入任务文件的 `## Plan > ### Execution Strategy` 区块。该区块至少包含机器可读的 `- git-worktree: true|false`。`/hotpot:execute` 启动时只读取这个区块来决定是否创建或复用 worktree；如果区块缺失、值非法或与现有 worktree 附着状态冲突到无法安全执行，则停止并提示用户重新运行或修订 `/hotpot:new` 产物。

执行阶段不再包含“Ask The User”式 worktree 决策。它仍可在 `git-worktree: true` 时探测是否已有 worktree 附着并复用；如果没有附着则直接运行 `hotpot worktree create`。如果 `git-worktree: false` 但已有 worktree 附着，执行提示应采用保守策略：停止并报告任务文件策略与 ledger 状态不一致，避免在错误目录执行。

同步更新架构文档，说明任务文件现在也是执行策略的来源，并把生命周期描述从“execute 询问 worktree”改为“new 记录策略，execute 消费策略”。
:::

### Alternatives Considered

- 保留 `/hotpot:execute` 对旧任务的兜底询问：兼容性更强，但与用户明确要求的“execute 后不应该询问问题”冲突，容易继续产生割裂体验。
- 只在任务文件自然语言中说明 worktree 决策：改动小，但执行提示难以可靠解析，容易退回猜测或提问。
- 推荐并批准的方案：新增固定 `### Execution Strategy` 区块与 `git-worktree: true|false` 字段，让 `new` 负责决策、`execute` 负责执行，职责边界清晰且跨平台一致。

### Requirements

- `/hotpot:new` 工作流必须在写任务文件前询问或确定执行策略，至少包括是否使用隔离 `git-worktree`。
- 新任务文件必须在 `## Plan` 中写入 `### Execution Strategy`，并包含机器可读行 `- git-worktree: true|false`。
- `/hotpot:execute` 工作流不得在正常执行路径中询问是否使用 worktree。
- `/hotpot:execute` 必须从任务文件读取 `git-worktree` 决策；缺失或非法时停止并提示重新规划或修订任务文件。
- `git-worktree: true` 时，执行阶段应复用已附着 worktree；没有附着时直接创建 worktree 并继续。
- `git-worktree: false` 时，执行阶段应按当前仓库执行；如果 ledger 已附着 worktree，应停止并报告策略冲突。
- 更新 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md`，保持英文与中文架构说明同步。
- 若修改共享 prompt 源文件，也要意识到 `.hotpot/prompts/` 是已安装副本；执行 agent 需根据项目惯例判断是否同步更新源资产与当前安装副本，避免本仓内测试命令读取旧内容。

### Non-Goals

::: details Non-Goals
- 不实现新的 CLI 子命令或新的 task metadata 字段。
- 不改变 `hotpot worktree create/path/remove` 的 Rust 语义。
- 不迁移历史任务文件内容。
- 不改变 TDD 模式字段 `tdd: true|false` 的解析规则。
- 不改动 review/finish-work 的 worktree disposal 语义，除非文档中必须同步描述 execute 策略来源。
:::

### Project Context

- 共享 prompt 源文件位于 `assets/prompts/`，安装后的当前项目副本位于 `.hotpot/prompts/`。
- 当前 `assets/prompts/hotpot-execute.md` 和 `.hotpot/prompts/hotpot-execute.md` 的 `## Required Flow` Step 0 是 “Worktree decision (opt-in)”，并在 `## Worktree Decision (Step 0)` 中要求执行阶段询问用户。
- 当前 `assets/prompts/hotpot-new.md` 和 `.hotpot/prompts/hotpot-new.md` 的任务文件模板只强制 `### Mode`，没有执行策略字段。
- `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md` 都说明任务文件必须包含 `## Task`、`## Plan`、`## Execution Instructions`，且 execute 当前描述为解析 active task 后执行。
- `src/task/create.rs` 注释说明 worktree 三列在创建时为 `None`，目前由 `/hotpot:execute` 开头用户同意后调用 `hotpot worktree create` 回填；本任务需要让 prompt/文档语义改为“new 阶段决定，execute 阶段按任务文件创建或复用”。

---

## Plan

### Mode

- tdd: false

### Execution Strategy

- git-worktree: false
- rationale: 本任务是提示词与架构文档契约调整，范围集中，直接在当前仓库执行即可。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/hotpot-new.md` | Modify | 新增 new 阶段执行策略评估与任务文件模板字段。 |
| `.hotpot/prompts/hotpot-new.md` | Modify | 同步当前项目已安装共享 prompt，确保后续本仓 `/hotpot:new` 行为立即生效。 |
| `assets/prompts/hotpot-execute.md` | Modify | 移除执行阶段 worktree 询问，改为解析任务文件策略。 |
| `.hotpot/prompts/hotpot-execute.md` | Modify | 同步当前项目已安装执行 prompt。 |
| `docs/ARCH.md` | Modify | 更新英文架构说明中的任务文件契约与 execute 流程。 |
| `docs/ARCH.zh_CN.md` | Modify | 更新中文架构说明并保持与英文内容一致。 |

### Implementation Tasks

#### Task 1: Add execution strategy to new workflow

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/hotpot-new.md` | Modify | 在 brainstorming/planning 阶段问清执行策略并写入任务文件。 |
| `.hotpot/prompts/hotpot-new.md` | Modify | 同步安装副本，便于当前项目立即使用。 |

**Steps:**

- [x] **Step 1**: 在 `assets/prompts/hotpot-new.md` 中新增执行策略评估段落，放在设计批准后、最终 Planning Flow 写入任务文件之前；要求至少询问或确认 `git-worktree` 决策。
- [x] **Step 2**: 更新 Required Flow、Planning Flow、Task File Content、Constraints，使任务文件包含 `## Plan > ### Execution Strategy` 和 `- git-worktree: true|false`。
- [x] **Step 3**: 明确 `new` 阶段的 worktree 问题必须在创建任务记录前解决，并写入任务文件；结构性字段保持英文，不翻译 `git-worktree` 键和值。
- [x] **Step 4**: 将同等修改同步到 `.hotpot/prompts/hotpot-new.md`，保持源资产与当前安装副本一致。
- [x] **Step 5**: 搜索 `hotpot-new.md` 中的 `Execution Strategy`、`git-worktree`、`Worktree`，确认模板、约束、执行说明三处没有互相矛盾。

:::

#### Task 2: Make execute consume task strategy without asking

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/hotpot-execute.md` | Modify | 将 Step 0 改为读取任务文件执行策略，不再询问用户。 |
| `.hotpot/prompts/hotpot-execute.md` | Modify | 同步安装副本，便于当前项目立即使用。 |

**Steps:**

- [x] **Step 1**: 在 `assets/prompts/hotpot-execute.md` 中调整 Required Flow 顺序：先解析 active task path 并读取任务文件，再解析 `### Execution Strategy`，之后才决定 worktree 行为。
- [x] **Step 2**: 删除或重写 `## Worktree Decision (Step 0)` 中的 “Ask The User” 路径，替换为任务文件策略解析规则。
- [x] **Step 3**: 规定 `git-worktree: true` 时执行 `hotpot worktree path`，已有路径则复用，无路径则运行 `hotpot worktree create`；失败时报告 blocker，不询问降级。
- [x] **Step 4**: 规定 `git-worktree: false` 时不创建 worktree；若 `hotpot worktree path` 返回非空，应停止并报告任务策略与 ledger 状态冲突。
- [x] **Step 5**: 规定缺失 `### Execution Strategy`、缺失 `git-worktree` 或值不是 `true|false` 时停止，提示用户重新运行 `/hotpot:new` 或修订任务文件；不要现场询问。
- [x] **Step 6**: 同步修改 `.hotpot/prompts/hotpot-execute.md`。
- [x] **Step 7**: 搜索 `Ask The User`、`Use an isolated git worktree`、`Worktree Decision`，确认执行 prompt 中不再保留正常路径提问语义；保留错误报告语义可以，但不得要求用户在 execute 阶段选择 worktree。

:::

#### Task 3: Update architecture docs and validate prompt consistency

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify | 记录任务文件新增执行策略契约与 execute 行为变化。 |
| `docs/ARCH.zh_CN.md` | Modify | 中文同步说明相同架构变化。 |

**Steps:**

- [x] **Step 1**: 更新 `docs/ARCH.md` 的 Core Concepts、Commands 或 Execution Flow，说明任务文件 `## Plan > ### Execution Strategy` 包含 `git-worktree: true|false`，`/hotpot:execute` 只消费该策略。
- [x] **Step 2**: 更新 `docs/ARCH.zh_CN.md` 对应段落，语义与英文版一致。
- [x] **Step 3**: 运行 `cargo fmt --check`，预期通过；如果没有 Rust 改动也应通过。
- [x] **Step 4**: 运行 `cargo test`，预期通过或确认失败与本提示词/文档改动无关；如失败，记录具体失败。
- [x] **Step 5**: 运行针对性搜索，确认 `assets/prompts/hotpot-execute.md` 与 `.hotpot/prompts/hotpot-execute.md` 不再包含执行阶段 worktree 询问文案 `Use an isolated git worktree for this task?`。
- [x] **Step 6**: 运行针对性搜索，确认 `assets/prompts/hotpot-new.md` 与 `.hotpot/prompts/hotpot-new.md` 都包含 `### Execution Strategy` 和 `git-worktree: true|false` 模板说明。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo fmt --check` | 通过，无格式化差异。 |
| `cargo test` | 通过；若环境或既有测试失败，报告失败点并说明是否与本任务相关。 |
| `rg "Use an isolated git worktree for this task\?" assets/prompts .hotpot/prompts` | 不应在 `hotpot-execute.md` 中出现；如 finish-work 或历史说明出现，需确认不是 execute 阶段询问。 |
| `rg "Execution Strategy|git-worktree" assets/prompts/hotpot-new.md .hotpot/prompts/hotpot-new.md assets/prompts/hotpot-execute.md .hotpot/prompts/hotpot-execute.md docs/ARCH.md docs/ARCH.zh_CN.md` | 能看到 new、execute 与架构文档均有一致说明。 |

### Risks and Watchouts

::: warning
- `.hotpot/prompts/` 是安装副本，`assets/prompts/` 是源资产；只改其中一边会导致当前仓行为和未来安装行为不一致。
- Hotpot prompt 中的 markdown 标题和机器字段是解析契约，`## Task`、`## Plan`、`### Mode`、`### Execution Strategy`、`tdd: false`、`git-worktree: false` 等锚点和值不要翻译。
- `execute` 阶段仍需要处理已有 worktree 附着状态，但不能把这个处理重新变成用户选择问题。
- 如果 `cargo test` 因无关环境问题失败，最终报告必须明确失败命令、失败原因和与本任务的相关性。
:::

---

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Use the `### Execution Strategy` value `git-worktree: false`; do not create an isolated git worktree for this task.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task.
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff.
- Run the validation commands before reporting completion.
