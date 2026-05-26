# Sync Task Overview Status

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | true | 4 | medium |
:::

---

## Task

### Summary

::: info
实现 Hotpot 任务完成后对任务 Markdown 顶部 VuePress Overview 状态表的同步更新：当 `hotpot task done` 成功把 ledger 行标记为 `Done` 后，如果任务文件包含 VuePress Overview 表，则把表格中的 `Status` 单元格从 `In Progress` 更新为 `Done`。未启用 VuePress、任务文件不存在或普通 Markdown 任务没有 Overview 表时必须安全跳过，不能影响 `overview.jsonl` 的完成状态。
:::

### User Request

::: info 用户原话
在 `task` 文档中，记录了任务状态，但是整体完成后，并没有修改这个 `In Progress` 的状态，要么移除这个状态显示，要么在整体完成后，再次更新这个状态，但是需要注意有 `vuepress` 和没有 `vuepress` 的情况
:::

后续决策：用户选择“完成后更新状态”，而不是移除状态显示；用户批准该设计，并选择 `tdd: true` 与 `git-worktree: true`。

### Approved Design

::: tip
采用“完成后同步任务文件 Overview 状态”的设计。执行代理需要在 Rust 层新增一个可测试的 Markdown 同步函数，并在 `hotpot task done` 成功返回更新后的 `TaskInfo` 后调用它。

核心行为：

1. 任务 ledger 仍然是生命周期状态的权威来源，`overview.jsonl` 的 `status` 先由现有 `task::mark_task_done` 更新为 `Done`。
2. Markdown 同步是成功 `task done` 之后的附加用户界面更新，只更新当前任务文件中的 VuePress Overview 表。
3. 只有当项目可观测地启用了 VuePress 且任务文件存在并包含标准 Overview 表时才改写 Markdown。
4. 未启用 VuePress、缺少任务文件、没有 Overview 表或表格形状不匹配时安全跳过；不要让这些情况使 `hotpot task done` 失败。
5. 如果文件存在且尝试写入时发生真实 I/O 错误，应向用户输出英文 warning，但不回滚已完成的 ledger 状态。
6. 更新 `.hotpot/prompts/hotpot-finish-work.md`，移除“finish-work 不修改 task file content”的过时限制，并说明 `task done` 之后的 Overview 状态同步。
7. 更新 `.hotpot/prompts/vuepress-style.md`，把 `Status` 字段语义改为：新建时为 `In Progress`，finish-work 成功后由 `hotpot task done` 路径同步为 `Done`，非 VuePress Markdown 无此要求。
8. 因该任务改变任务完成执行流，必须同步更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`。
:::

建议实现点：优先新增一个小型模块或函数，例如 `src/task/markdown.rs` 中的 `sync_task_markdown_status_after_done(root_dir, username, task: &TaskInfo) -> Result<SyncResult>` 或类似命名，再由 `src/commands/task.rs::done_task` 在 `mark_task_done` 成功后调用。具体命名可按现有模块边界调整，但必须保持函数有中英文 doc comments。

### Alternatives Considered

- 移除 VuePress Overview 的 `Status` 列：实现更小，但会减少任务页的状态信息，并且不能修复已有任务文档中的静态 `In Progress`。
- 只修改 prompt，让未来执行代理手动更新任务文件：不可靠，且当前 finish-work prompt 明确禁止修改 task file content，容易继续遗漏。
- 已批准方案：在 `task done` 成功后由 CLI 同步 Markdown Overview 状态。它把状态更新绑定到权威生命周期转换，兼容 VuePress 与非 VuePress 场景，并可通过单元测试覆盖。

### Requirements

- `hotpot task done` 成功后，ledger 返回 JSON 必须继续显示 `"status":"Done"` 与 `"active":false`。
- VuePress 启用且任务文件含标准 `::: info Overview` 状态表时，Markdown 表格中的 `Status` 数据单元格必须从 `In Progress` 更新为 `Done`。
- 未启用 VuePress 时，不应读取或改写任务 Markdown 状态表。
- VuePress 启用但任务文件不存在时，`hotpot task done` 仍应成功；应输出英文 warning 或静默跳过，具体以最少惊扰用户为准，但不能 panic 或回滚 ledger。
- 任务文件存在但没有 Overview 表或表格不匹配时，必须安全跳过且不破坏文件内容。
- 不要修改任务实现复选框状态；本任务只处理 Overview 状态表。
- 所有新增 Rust 函数、模块和重要参数必须有中英文 doc comments。
- 代码输出必须使用英文。

### Non-Goals

::: details Non-Goals
- 不实现 `Cancelled` 状态的 Markdown Overview 同步，除非执行中发现现有取消流程已需要同一通用函数且改动很小。
- 不重写 VuePress Overview 的整体格式。
- 不新增外部 Markdown parser 依赖；优先使用小范围文本处理。
- 不修改旧任务的批量迁移工具。
- 不改变 `overview.jsonl` 作为权威状态源的设计。
- 不改变 `/hotpot:new` 的任务创建流程。
:::

### Project Context

- `docs/ARCH.md` 说明任务生命周期为 `new -> execute -> finish-work`，且 finish-work 通过 `hotpot task done` 标记 `Done`。
- `src/commands/task.rs::done_task` 当前调用 `task::mark_task_done` 后直接把更新后的 `TaskInfo` JSON 打印到 stdout。
- `src/task/transitions.rs::mark_task_done` 持锁更新 `overview.jsonl`，把目标任务的 `status` 置为 `TaskStatus::Done`、`active=false`，并可选写入 commit。
- `src/task/storage.rs::get_task_filename` 根据 `TaskInfo` 构造 `<YYYY-MM-DD>-<title>` 文件名，`get_active_task_filepath` 会使用同一规则。
- `src/paths.rs` 提供 workspace 和 task 目录路径工具，执行代理应先检查可复用函数，避免手写路径拼接。
- `.hotpot/prompts/vuepress-style.md` 当前要求 VuePress 任务文件在 H1 与 `## Task` 之间写入 `::: info Overview`，其中表格数据行为 `| In Progress | <true|false> | <N> | <low|medium|high> |`。
- `.hotpot/prompts/hotpot-finish-work.md` 当前在约束中写着“Do not modify the task file content during finish-work; checkbox updates belong to the execute flow.”，这与新设计冲突，需要改成仅禁止复选框/任务正文进度更新，但允许 CLI 在 `task done` 后同步 Overview 状态。
- 项目当前没有顶层 `tests/` 目录，现有 task 模块测试位于 `src/task/mod.rs` 的 `#[cfg(test)] mod tests` 中。可以继续在模块内添加单元测试，或新增更合适的模块内测试。
- VuePress 启用判断在 `/hotpot:new` prompt 中使用文件存在门控：`.hotpot/prompts/vuepress.md` 与 `.hotpot/prompts/vuepress-style.md` 同时存在即视为启用。Rust 侧已有 `context::resolve_vuepress_enabled(root_dir)` 与 `src/vuepress.rs::verify_install_consistency` 等相关逻辑，执行代理必须检查现有函数后选择最兼容的门控。

---

## Plan

### Mode

- tdd: true

### Execution Strategy

- git-worktree: true
- rationale: 当前主 checkout 已有未提交改动，且本任务会修改 Rust 状态逻辑、prompt 资产和架构文档；使用隔离 worktree 可以降低与现有改动互相干扰的风险。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/task/mod.rs` | Modify | 暴露或测试新增的任务 Markdown 状态同步能力，并保持模块文档完整。 |
| `src/task/markdown.rs` | Create | 建议新增文本同步逻辑，负责识别 VuePress Overview 表并更新 `Status` 单元格。 |
| `src/commands/task.rs` | Modify | 在 `done_task` 成功更新 ledger 后调用 Markdown 状态同步，并处理 warning。 |
| `.hotpot/prompts/hotpot-finish-work.md` | Modify | 更新 finish-work 工作流说明，允许完成后同步 Overview 状态，同时继续禁止复选框进度改写。 |
| `.hotpot/prompts/vuepress-style.md` | Modify | 更新 VuePress Overview `Status` 字段语义，说明完成后会同步为 `Done`。 |
| `docs/ARCH.md` | Modify | 英文架构文档记录 task done 后的 Markdown Overview 同步流程。 |
| `docs/ARCH.zh_CN.md` | Modify | 中文架构文档同步同等内容。 |
| `src/task/markdown.rs` or `src/task/mod.rs` tests | Test | 验证 Overview 状态更新、非 Overview 跳过、幂等和未启用 VuePress门控。 |

### Implementation Tasks

#### Task 1: Add Markdown Overview Status Parser

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/task/markdown.rs` | Create | 放置小范围 Markdown Overview 状态同步逻辑。 |
| `src/task/mod.rs` | Modify | 注册新模块并按需要暴露测试目标。 |

##### Red

- [ ] R1: 在 `src/task/markdown.rs` 的测试模块中新增测试 `updates_vuepress_overview_status_to_done`，输入包含 H1 后的 `::: info Overview` 与表格行 `| In Progress | true | 4 | medium |`，期望输出为 `| Done | true | 4 | medium |` 且其他内容保持不变。
- [ ] R2: 运行 `cargo test updates_vuepress_overview_status_to_done`；**expect failure**，失败原因应是函数或行为尚未实现。

##### Green

- [ ] G1: 在 `src/task/markdown.rs` 中实现最小文本转换函数，例如 `update_overview_status_markdown(content: &str, status: TaskStatus) -> Option<String>`；只在 `::: info Overview` 容器内找到 `| Status | ... |` 表头和下一条数据行时替换第一列。
- [ ] G2: 在 `src/task/mod.rs` 注册 `mod markdown;`，并按命名需要公开给命令层使用的函数。
- [ ] G3: 运行 `cargo test updates_vuepress_overview_status_to_done`；**expect pass**。
- [ ] G4: 运行 `cargo test task::`；**expect no regressions**。

##### Refactor

- [ ] F1: 检查解析函数命名和返回值是否能清晰表达“匹配则返回新内容，不匹配则跳过”；如无重复或命名问题，写 `no refactor needed`。
- [ ] F2: 如果发生重构，重新运行 `cargo test updates_vuepress_overview_status_to_done` 和 `cargo test task::`；**expect pass**。否则标记 `skipped (no refactor)`。

:::

#### Task 2: Gate File Sync By VuePress State And Task File Shape

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/task/markdown.rs` | Modify | 增加面向文件路径和 VuePress 门控的同步函数。 |
| `src/paths.rs` | Modify | 仅当现有路径工具不足时才修改；优先复用现有 API。 |

##### Red

- [ ] R1: 新增测试 `skips_status_sync_when_overview_is_absent`，输入普通 Hotpot 任务 Markdown，无 `::: info Overview`，期望同步函数返回 skipped/no-change 且内容完全不变。
- [ ] R2: 新增测试 `status_sync_is_idempotent_when_already_done`，输入 Overview 数据行为 `| Done | true | 4 | medium |`，期望不产生第二次变化或返回 already-up-to-date。
- [ ] R3: 运行 `cargo test skips_status_sync_when_overview_is_absent status_sync_is_idempotent_when_already_done`；**expect failure**，失败应指向缺失的跳过/幂等行为。

##### Green

- [ ] G1: 为同步结果定义小型 enum 或结构，例如 `TaskMarkdownStatusSync`，区分 `Updated`、`SkippedNoOverview`、`SkippedVuePressDisabled`、`SkippedMissingFile`、`AlreadyCurrent`；命名可调整但必须有中英文 doc comments。
- [ ] G2: 实现文件级函数，接收 `root_dir`、`username`、`TaskInfo` 或直接接收任务文件路径；在 VuePress 未启用时直接返回 skipped，不读写任务文件。
- [ ] G3: 任务文件不存在时返回 skipped/missing，不报错；文件存在但没有 Overview 表时返回 skipped/no-overview，不写盘。
- [ ] G4: 运行 `cargo test skips_status_sync_when_overview_is_absent status_sync_is_idempotent_when_already_done`；**expect pass**。
- [ ] G5: 运行 `cargo test task::`；**expect no regressions**。

##### Refactor

- [ ] F1: 检查文件级函数是否避免在持有 `overview.jsonl` 锁时执行文件 I/O；若命令层调用点在 `mark_task_done` 返回之后，则记录 `no refactor needed`。
- [ ] F2: 如果重构了门控或路径构造，重新运行 `cargo test task::`；**expect pass**。否则标记 `skipped (no refactor)`。

:::

#### Task 3: Wire Sync Into `hotpot task done`

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/commands/task.rs` | Modify | 在 CLI 完成任务后触发 Markdown Overview 状态同步。 |
| `src/task/markdown.rs` | Test | 可按需要增加命令调用路径依赖的单元测试或 helper 测试。 |

##### Red

- [ ] R1: 新增测试 `done_task_sync_helper_updates_task_file_after_ledger_done` 或等价测试，构造临时项目、启用 VuePress 门控资产、创建任务文件，调用同步入口后断言 Overview 状态变为 `Done`。
- [ ] R2: 运行 `cargo test done_task_sync_helper_updates_task_file_after_ledger_done`；**expect failure**，失败原因应是 CLI 完成路径尚未调用或 helper 尚未完成。

##### Green

- [ ] G1: 修改 `src/commands/task.rs::done_task`：保留现有 `task::mark_task_done(...)` 调用与 JSON 输出语义，在其成功返回 `updated` 后调用 Markdown 同步函数。
- [ ] G2: 同步失败为真实 I/O 错误时，用 `eprintln!` 输出英文 warning，例如 `Warning: failed to sync task Markdown status: ...`；不要改变 `hotpot task done` 的成功 JSON stdout。
- [ ] G3: 对 skipped/no-change 结果保持静默，避免普通非 VuePress 项目产生噪音。
- [ ] G4: 运行 `cargo test done_task_sync_helper_updates_task_file_after_ledger_done`；**expect pass**。
- [ ] G5: 运行 `cargo test task::`；**expect no regressions**。

##### Refactor

- [ ] F1: 检查 stdout/stderr 分离：stdout 必须仍是一行 `TaskInfo` JSON，warning 只能走 stderr；如满足，写 `no refactor needed`。
- [ ] F2: 如果调整了错误处理，重新运行 `cargo test done_task_sync_helper_updates_task_file_after_ledger_done` 和 `cargo test task::`；**expect pass**。否则标记 `skipped (no refactor)`。

:::

#### Task 4: Update Prompts And Architecture Docs

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.hotpot/prompts/hotpot-finish-work.md` | Modify | 记录 finish-work 在 `task done` 后同步 Overview 状态，并调整约束。 |
| `.hotpot/prompts/vuepress-style.md` | Modify | 修正 Overview `Status` 字段语义。 |
| `docs/ARCH.md` | Modify | 英文架构说明新增任务 Markdown 状态同步。 |
| `docs/ARCH.zh_CN.md` | Modify | 中文架构说明同步更新。 |

##### Red

- [ ] R1: 运行 `cargo test task::` 作为代码回归基线；**expect pass**，文档修改前不应影响测试。
- [ ] R2: 搜索 `.hotpot/prompts/hotpot-finish-work.md` 中的旧约束 `Do not modify the task file content during finish-work`；**expect found**，这是需要修正的文档断言。

##### Green

- [ ] G1: 在 `.hotpot/prompts/hotpot-finish-work.md` 的 Required Flow、Mark the Task Done、Final Response 或 Constraints 中说明：`hotpot task done` 成功后 CLI 会同步 VuePress Overview 状态；finish-work 仍不得手动改写复选框进度或任务正文。
- [ ] G2: 在 `.hotpot/prompts/vuepress-style.md` 中把 `Status` 语义改为新建时 `In Progress`，完成后由 `hotpot task done` 路径更新为 `Done`；普通非 VuePress 任务没有该表时无需同步。
- [ ] G3: 在 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md` 的 Task file、finish-work 或 VuePress Integration 相关段落中记录该同步行为与跳过规则。
- [ ] G4: 运行 `cargo test task::`；**expect pass**。
- [ ] G5: 运行 `cargo fmt --check`；**expect pass**。如失败，运行 `cargo fmt` 后再运行 `cargo fmt --check`。

##### Refactor

- [ ] F1: 检查中英文架构文档内容是否等价，且 structural tokens 如 `hotpot task done`、`Done`、`In Progress`、`VuePress` 保持英文；如无问题，写 `no refactor needed`。
- [ ] F2: 如果文档措辞调整，重新运行 `cargo test task::` 和 `cargo fmt --check`；**expect pass**。否则标记 `skipped (no refactor)`。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo test updates_vuepress_overview_status_to_done` | 新增 Markdown Overview 状态转换测试通过。 |
| `cargo test skips_status_sync_when_overview_is_absent status_sync_is_idempotent_when_already_done` | 无 Overview 与幂等场景测试通过。 |
| `cargo test done_task_sync_helper_updates_task_file_after_ledger_done` | 文件级同步或命令路径 helper 测试通过。 |
| `cargo test task::` | task 模块现有与新增测试全部通过。 |
| `cargo fmt --check` | Rust 格式检查通过；如曾运行 `cargo fmt`，再次检查通过。 |

### Risks and Watchouts

::: warning
- 不要把 Markdown 文件同步放进 `mark_task_done` 的 `overview.jsonl` 持锁闭包内；持锁区域不能扩大到额外文件 I/O，更不能运行子进程。
- stdout 必须保持 `hotpot task done` 的单行 JSON 语义；任何同步 warning 只能写 stderr。
- 不要使用外部 Markdown parser 依赖，除非先证明手写小范围解析无法可靠满足需求。
- 不要让 VuePress 未启用或旧任务文件缺少 Overview 表导致完成流程失败。
- 当前工作区已有未提交修改；执行应在 `git-worktree: true` 的隔离 worktree 中进行，避免覆盖用户现有改动。
- 修改 `.hotpot/prompts/*` 会影响安装后的共享 prompt 资产；若仓库另有源资产镜像，执行代理必须检查是否还需要同步 `assets/prompts/*` 或资产注册文件。
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
- Because `tdd: true`, follow Red → Green → Refactor for every `#### Task N` and capture the failing-test evidence before implementation.
