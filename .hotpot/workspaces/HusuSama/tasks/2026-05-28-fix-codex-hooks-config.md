# Fix Codex Hooks Config

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| Done | true | 3 | medium |
:::

---

## Task

### Summary

::: info
修复 Hotpot 的 Codex 集成说明与 hook 运行时输出兼容性。当前 Codex 配置已经从旧的 `codex_hooks` feature 迁移到 `hooks`，但 `docs/platforms/codex.md` 仍记录旧字段；同时 Codex 执行时报告 `SessionStart`、`UserPromptSubmit`、`PreToolUse` hook 返回 invalid JSON output，需要让 hook 输出符合当前 Codex 接受的 schema。
:::

### User Request

::: info 用户原始请求
当前在 codex 中使用有如下问题：

1. codex 的配置中，移除了 `codex_hooks`，需要改成 `hooks`，用户已手动更改，需要在 `docs/platforms/codex.md` 更新此内容。
2. 当 codex 执行时，会出现错误：`SessionStart hook (failed) error: hook returned invalid session start JSON output`、`UserPromptSubmit hook (failed) error: hook returned invalid user prompt submit JSON output`、`PreToolUse hook (failed) error: hook returned invalid pre-tool-use JSON output`。

用户批准按推荐方案创建任务：同时修正文档和 Codex hook JSON 输出兼容性。用户选择 `tdd: true`，执行策略选择 `git-worktree: false`。
:::

### Approved Design

::: tip
本任务采用同步修复方案：先用测试锁定当前 Hotpot 生成的 Codex 配置和 hook JSON 输出契约，再修正实现与文档。

执行 agent 需要重点验证两类问题：

1. Codex 配置文档必须使用当前 `[features] hooks = true`，不能继续建议 `[features] codex_hooks = true`。
2. `src/commands/hook.rs::codex` 三个事件输出必须是 Codex 当前版本接受的 JSON。用户贴出的错误说明 Codex 能执行命令但拒绝 hook 输出结构，因此需要用最小变更调整输出字段，而不是改变 Hotpot 的上下文解析逻辑。

已确认仓库当前模板 `assets/platforms/codex/config.toml` 与项目 `.codex/config.toml` 都已使用：

```toml
[features]
hooks = true
```

已观察到 `src/assets/merge.rs` 的 Codex merge 测试内仍有旧 fixture `codex_hooks = true`，执行 agent 应将其纳入测试修复范围。
:::

### Alternatives Considered

- 只更新 `docs/platforms/codex.md`：改动最小，但不能解决用户看到的 hook 运行时报错。
- 只修改 hook 输出：可能解决运行错误，但会留下平台参考文档过期，后续适配容易回退到旧字段。
- 同步修正文档、测试与 hook 输出：范围仍可控，并覆盖用户的两个问题；这是批准方案。

### Requirements

- `docs/platforms/codex.md` 必须把 Codex hooks feature 说明更新为 `[features] hooks = true`。
- `docs/platforms/codex.md` 的 Tools 配置示例不能再出现 `codex_hooks = true`。
- `assets/platforms/codex/config.toml` 当前已是 `hooks = true`，执行 agent 应保持该模板不回退。
- `src/assets/merge.rs` 中 Codex config merge 测试 fixture 与断言必须同步到 `hooks = true`，并保持 merge/idempotent 行为。
- `src/commands/hook.rs::codex` 的 `PreToolUse`、`SessionStart`、`UserPromptSubmit` 输出必须是有效 JSON，并符合当前 Codex hook 输出 schema。
- 若 Codex hook 输出 schema 修复影响 Hotpot 执行流或平台契约，必须同步更新 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md`。
- 代码输出内容必须保持英文；新增或修改 Rust 注释必须遵守 English-first bilingual style。

### Non-Goals

::: details Non-Goals
- 不重新设计 Hotpot 的 Codex skill 或 subagent 结构。
- 不改变 OpenCode、Claude Code、Pi 的 hook 行为。
- 不引入新的非 Rust 依赖。
- 不把 Codex 的 `Stop` 事件改造成 session cleanup；当前架构说明 Codex 没有合适的 session-close event。
- 不创建额外 plan 文件；本任务文件就是执行 handoff。
:::

### Project Context

- 必读架构文件：`docs/ARCH.md`；若执行流变更，按 `AGENTS.md` 同步 `docs/ARCH.zh_CN.md`。
- 平台参考文件：`docs/platforms/codex.md` 当前仍写着 `Codex hooks are lifecycle hooks behind the codex_hooks feature flag`，并在 Hooks 与 Tools 示例中使用 `codex_hooks = true`。
- 当前 Codex 配置模板：`assets/platforms/codex/config.toml` 已使用 `[features] hooks = true`，并配置 `PreToolUse`、`SessionStart`、`UserPromptSubmit` 三个 hook。
- 当前项目 Codex 配置：`.codex/config.toml` 与模板一致，已使用 `hooks = true`。
- Hook 实现入口：`src/commands/hook.rs::codex(command: CodexHookCommand)`。
- 当前 `PreToolUse` 输出包含 `systemMessage` 与 `hookSpecificOutput.permissionDecision = "allow"`、`hookSpecificOutput.additionalContext`。
- 当前 `SessionStart` 与 `UserPromptSubmit` 输出包含顶层 `systemMessage` 与 `additionalContext`。
- 用户遇到的 Codex 错误明确指向 hook returned invalid JSON output，执行 agent 需要根据 Codex 当前 hook schema 判断字段是否应被包进 `hookSpecificOutput`、改名、或删除。
- 相关 merge 测试：`src/assets/merge.rs` Codex fixture 仍包含 `codex_hooks = true`，需要更新为 `hooks = true` 并保持用户 feature key merge 行为。

---

## Plan

### Mode

- tdd: true

### Execution Strategy

- git-worktree: false
- rationale: 任务范围集中在 Codex 文档、hook JSON 输出和 Rust 测试，直接在当前 checkout 执行即可；不需要隔离 worktree。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/commands/hook.rs` | Modify | 修正 Codex hook 事件输出 JSON，使 Codex 当前版本接受。 |
| `src/assets/merge.rs` | Test | 更新 Codex config merge fixture 与断言，覆盖 `hooks = true`。 |
| `docs/platforms/codex.md` | Modify | 更新 Codex hooks feature 文档和 Tools 示例。 |
| `docs/ARCH.md` | Modify | 仅当 hook 输出契约或执行流说明受影响时同步英文架构文档。 |
| `docs/ARCH.zh_CN.md` | Modify | 仅当 `docs/ARCH.md` 更新时同步中文架构文档。 |
| `assets/platforms/codex/config.toml` | Test | 验证模板已经是 `hooks = true` 且不回退。 |

### Implementation Tasks

#### Task 1: Lock Codex config feature tests

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/assets/merge.rs` | Test | 将 Codex merge 测试从旧 `codex_hooks` 契约迁移到当前 `hooks` 契约。 |
| `assets/platforms/codex/config.toml` | Test | 作为当前正确配置模板的对照。 |

##### Red

- [ ] R1: 在 `src/assets/merge.rs` 的 Codex config 测试区，将 `CODEX_HOTPOT` fixture 和相关断言改为期望 `hooks = true`，并确保测试会在仍断言 `codex_hooks = true` 的旧实现上失败。
- [ ] R2: Run `cargo test toml_codex --lib`; **expect failure** tied to the old `codex_hooks = true` assertions or fixture expectations. Capture the failing test names.

##### Green

- [ ] G1: 在 `src/assets/merge.rs` 中完成最小测试/fixture 更新：`codex_hooks = true` 全部替换为 `hooks = true`，保留用户自定义 `[features]` key 的 merge 覆盖。
- [ ] G2: Run `cargo test toml_codex --lib`; **expect pass** for all Codex TOML merge tests.
- [ ] G3: Run `cargo test --lib`; **expect no library test regressions**.

##### Refactor

- [ ] F1: 检查 Codex fixture 和断言命名是否仍清晰；如果只需字段替换，写 `no refactor needed`。
- [ ] F2: If a refactor happened, re-run `cargo test toml_codex --lib` and `cargo test --lib`; **expect pass**. Otherwise mark `skipped (no refactor)`.

:::

#### Task 2: Fix Codex hook response JSON

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/commands/hook.rs` | Test | 添加或调整测试，锁定 Codex hook JSON 输出 schema。 |
| `src/commands/hook.rs` | Modify | 最小修改 `codex` 输出字段，使 Codex 接受三个事件输出。 |

##### Red

- [ ] R1: 在 `src/commands/hook.rs` 的测试模块中添加或调整测试，覆盖 `CodexHookCommand::PreToolUse`、`SessionStart`、`UserPromptSubmit` 的 JSON 输出结构。测试名建议使用 `codex_pre_tool_use_outputs_valid_hook_json`、`codex_session_start_outputs_valid_hook_json`、`codex_user_prompt_submit_outputs_valid_hook_json`；如果文件尚无测试模块，创建同文件 `#[cfg(test)] mod tests` 并保持双语 doc/comment 约定。
- [ ] R2: 测试应断言当前 Codex schema 需要的字段，并反向防止导致 invalid output 的字段组合。例如根据当前 Codex 文档/行为确认 `additionalContext` 是否只允许在 `hookSpecificOutput` 内，`SessionStart` / `UserPromptSubmit` 是否接受顶层 `systemMessage`，以及 `PreToolUse` 是否允许显式 `permissionDecision = "allow"`。
- [ ] R3: Run `cargo test codex_ --lib`; **expect failure** on current `src/commands/hook.rs::codex` output if它仍包含 Codex 当前版本拒绝的字段结构。Capture the failing assertion.

##### Green

- [ ] G1: 在 `src/commands/hook.rs::codex` 中做最小实现修复，调整三个事件输出 JSON。不要改变 `Context::from_payload`、`review_memory_message`、`language_directive_message` 的核心语义，除非测试证明字段位置必须改变。
- [ ] G2: Run `cargo test codex_ --lib`; **expect pass** for the new Codex hook JSON tests.
- [ ] G3: Run these manual JSON checks with simulated stdin and **expect each command prints exactly one valid JSON object accepted by the tests**:

| Command | Expected |
| ------- | -------- |
| `printf '{"cwd":"/Users/bytedance/RustProjects/hotpot","hook_event_name":"SessionStart"}' \| cargo run -- hook codex session-start` | valid Codex `SessionStart` hook JSON |
| `printf '{"cwd":"/Users/bytedance/RustProjects/hotpot","hook_event_name":"UserPromptSubmit"}' \| cargo run -- hook codex user-prompt-submit` | valid Codex `UserPromptSubmit` hook JSON |
| `printf '{"cwd":"/Users/bytedance/RustProjects/hotpot","hook_event_name":"PreToolUse"}' \| cargo run -- hook codex pre-tool-use` | valid Codex `PreToolUse` hook JSON |

##### Refactor

- [ ] F1: Inspect whether a tiny helper is warranted for repeated Codex hook response construction. Prefer keeping logic inline unless it removes duplicated schema-critical structure.
- [ ] F2: If a refactor happened, re-run `cargo test codex_ --lib` and `cargo test --lib`; **expect pass**. Otherwise mark `skipped (no refactor)`.

:::

#### Task 3: Update Codex platform documentation and architecture notes

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/platforms/codex.md` | Modify | Replace obsolete `codex_hooks` feature references with `hooks`. |
| `docs/ARCH.md` | Modify | Sync architecture if Codex hook output contract changed materially. |
| `docs/ARCH.zh_CN.md` | Modify | Chinese counterpart for any architecture change. |

##### Red

- [ ] R1: Add or use a repository text check command to prove stale docs still contain `codex_hooks`. Use `rg "codex_hooks" docs/platforms/codex.md src/assets/merge.rs` as the exact failing inspection command; **expect matches** before docs/tests are fully updated.
- [ ] R2: Run `rg "codex_hooks" docs/platforms/codex.md src/assets/merge.rs`; **expect failure condition for this task**: at least one stale match remains before the Green edit.

##### Green

- [ ] G1: In `docs/platforms/codex.md`, update the Hooks section to say Codex hooks are enabled by `[features] hooks = true`, not `codex_hooks = true`.
- [ ] G2: In `docs/platforms/codex.md`, update the Tools config example so it uses `hooks = true`.
- [ ] G3: In `docs/platforms/codex.md`, adjust wording around hook output fields if Task 2 changes the accepted schema description.
- [ ] G4: If Task 2 changes architecture-level platform behavior, update `docs/ARCH.md` and mirror the same content in Simplified Chinese in `docs/ARCH.zh_CN.md`. If only JSON field placement changed without changing workflow semantics, write in the task progress notes that architecture docs did not need changes.
- [ ] G5: Run `rg "codex_hooks" docs/platforms/codex.md src/assets/merge.rs`; **expect no matches**.
- [ ] G6: Run `cargo test`; **expect all tests pass**.

##### Refactor

- [ ] F1: Review final docs for consistency with `assets/platforms/codex/config.toml` and `.codex/config.toml`; either make a concrete wording cleanup or write `no refactor needed`.
- [ ] F2: If a refactor happened, re-run `cargo test` and `rg "codex_hooks" docs/platforms/codex.md src/assets/merge.rs`; **expect pass/no matches**. Otherwise mark `skipped (no refactor)`.

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo test toml_codex --lib` | Codex TOML merge tests pass and assert `hooks = true`. |
| `cargo test codex_ --lib` | Codex hook JSON output tests pass. |
| `cargo test --lib` | Library tests pass without regressions. |
| `cargo test` | Full Rust test suite passes. |
| `rg "codex_hooks" docs/platforms/codex.md src/assets/merge.rs` | No matches after the fix. |
| `printf '{"cwd":"/Users/bytedance/RustProjects/hotpot","hook_event_name":"SessionStart"}' \| cargo run -- hook codex session-start` | Prints one valid Codex hook JSON object. |
| `printf '{"cwd":"/Users/bytedance/RustProjects/hotpot","hook_event_name":"UserPromptSubmit"}' \| cargo run -- hook codex user-prompt-submit` | Prints one valid Codex hook JSON object. |
| `printf '{"cwd":"/Users/bytedance/RustProjects/hotpot","hook_event_name":"PreToolUse"}' \| cargo run -- hook codex pre-tool-use` | Prints one valid Codex hook JSON object. |

### Risks and Watchouts

::: warning
- Codex hook schema may have event-specific accepted fields; do not assume Claude Code hook response shape is valid for Codex.
- Removing too much from hook output can make JSON valid but lose Hotpot's per-turn language or shell context injection; tests should preserve required message content.
- `src/assets/merge.rs` tests currently encode old `codex_hooks` behavior; update tests intentionally rather than deleting coverage.
- If architecture docs are updated, English and Simplified Chinese versions must stay semantically aligned.
- Avoid broad refactors in `src/commands/hook.rs`; the user requested a targeted Codex fix.
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
- Because `tdd: true`, follow Red → Green → Refactor for every `#### Task N` and capture the failing test/inspection before the implementation step.
