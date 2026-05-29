# Fix VuePress Runtime PID

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| Done | true | 4 | medium |
:::

## Task

### Summary

::: info
修复 `hotpot vuepress start` 在 Codex 等受限环境中可能记录错误 runtime `pid` 的问题，确保启动失败、端口未就绪、pid 已复用或 runtime pid 不属于 Hotpot VuePress 进程时，`stop` / `status` 不会信任错误 pid，也不会留下无法关闭的 VuePress/Vite 残留进程。
:::

### User Request

用户报告：当前 Codex 在启动 `vuepress` 时记录了错误的 `pid`，导致后续无法正常关闭 `pid`。需要确认这是否与 Codex 沙箱导致启动失败后记录错误 pid 有关，并修复该问题。

### Approved Design

::: tip
本任务聚焦 VuePress 服务生命周期，不修改 `/hotpot:new` 的任务创建流程。实现应先用测试固定 runtime pid 的可信边界，再调整 `src/vuepress.rs` 中 `start`、`stop`、`status` 共享的清理路径。

批准的方向：

1. `start` 不能只因为 `Command::spawn()` 返回 `child.id()` 就把该 pid 视为最终、可信、可直接 kill 的 VuePress 服务身份。
2. `stop` / `status` 在面对 runtime pid 仍存活但不属于当前 `.hotpot-hub` 的 Hotpot VuePress/Vite/pnpm 进程时，不应杀该 pid 或其进程组。
3. 当 runtime pid 不可信、已死亡或启动失败，但 runtime 端口仍被当前 hub 下可识别的 VuePress/Vite/pnpm 进程占用时，必须通过端口归属兜底清理该 Hotpot-owned 进程。
4. readiness timeout、spawn 后子进程快速退出、端口未绑定等失败路径必须清理 `vuepress.runtime.json`，并尽力清理当前 hub 的 VuePress 端口占用。
5. 如果修复改变执行流或服务生命周期契约，必须同步更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`。

实现时优先抽出可单元测试的规划函数，避免测试真实启动 `pnpm docs:dev`。真实进程命令的 I/O wrapper 可以保持很薄，复杂判断放进纯函数。
:::

### Alternatives Considered

- 只在 `start` 失败时删除 `vuepress.runtime.json`：能缓解部分启动失败场景，但不能处理 pid 复用或 runtime pid 指向非 Hotpot 进程时 `stop` 误杀的问题。
- 只依赖端口兜底清理：能杀掉占端口的 VuePress/Vite 进程，但 runtime pid 仍存活且不可信时，当前代码可能先错误地清理 runtime pid 或进程组。
- 推荐并批准：建立 runtime pid 归属校验 + 端口兜底的组合策略。runtime pid 只有在命令行可识别为当前 hub 的 VuePress/Vite/pnpm/docs:dev 进程时才作为清理目标；否则跳过 pid kill，只保留端口归属清理和 runtime 状态删除。

### Requirements

- `hotpot vuepress stop` 不得杀死命令行不属于当前 `.hotpot-hub` 的 runtime pid 或 Unix 进程组。
- `hotpot vuepress status` 在 runtime pid 不可信、已死或 ttl 过期时必须清理 stale runtime 状态。
- `hotpot vuepress start` readiness 失败后不得留下不可关闭的 `vuepress.runtime.json`。
- 端口兜底只能清理命令行包含当前 hub 路径且可识别为 VuePress/Vite/pnpm/docs:dev 的进程。
- 所有新增或修改的 Rust 函数必须带中英双语 doc comments；复杂参数和不明显逻辑也要有中英双语注释。
- CLI 输出和错误信息必须使用 English。

### Non-Goals

::: details Non-Goals
- 不重写 VuePress hub 资产、主题或文档渲染逻辑。
- 不改变 `hotpot vuepress install` / `uninstall` 的原子安装语义。
- 不修改 `/hotpot:new` 的浏览器打开提示流程。
- 不引入新的外部语言运行时依赖。
- 不要求测试真实运行 `pnpm docs:dev` 或依赖网络安装。
:::

### Project Context

当前相关实现集中在 `src/vuepress.rs`：

- `RuntimeState` 记录 `.hotpot-hub/vuepress.runtime.json` 的 `pid`、`port`、`url`、`started_at`、`expires_at`。
- `start` 使用 `Command::new(pnpm).arg("run").arg("docs:dev")...spawn()` 后立刻取 `child.id()` 写入 runtime state，再等待端口 ready。
- Unix 上 `start` 通过 `pre_exec(libc::setsid)` 让 spawn 出来的进程成为新 session / process group leader，`stop` 之后用 negative pid 清理进程组。
- `cleanup_runtime_process` 当前以 `is_pid_alive(state.pid)` 为主要信号，随后按 `cleanup_targets_for_runtime_pid(state.pid)` 清理 runtime pid / process group，再调用 `cleanup_runtime_port(root_dir, state.port)`。
- `hotpot_vuepress_port_cleanup_targets` 已有端口归属过滤：命令行必须包含当前 hub 路径，并包含 `vuepress` / `vite` / `pnpm` / `docs:dev`。
- 现有测试位于 `src/vuepress.rs` 的 `#[cfg(test)] mod tests`，已经覆盖 process group、端口兜底筛选、TTL cleanup、ARCH 文档关键描述。

## Plan

### Mode

- tdd: true

### Execution Strategy

- git-worktree: false
- rationale: 本任务范围集中在 `src/vuepress.rs`、`docs/ARCH.md`、`docs/ARCH.zh_CN.md`，属于可测试的服务生命周期修复；当前没有其他 active task，直接在当前 checkout 执行更简单。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/vuepress.rs` | Modify | 增加 runtime pid 归属校验、启动失败清理和可测试的 cleanup planning。 |
| `docs/ARCH.md` | Modify | 如果服务生命周期契约变化，更新英文架构说明。 |
| `docs/ARCH.zh_CN.md` | Modify | 与 `ARCH.md` 同步更新简体中文架构说明。 |

### Implementation Tasks

#### Task 1: Guard Runtime PID Cleanup With Ownership Checks

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/vuepress.rs` | Test | 固定 runtime pid 存活但不属于当前 hub 时不能被清理的行为。 |
| `src/vuepress.rs` | Modify | 抽出可测试的 runtime pid 清理目标规划逻辑。 |

##### Red

- [x] R1: In `src/vuepress.rs`, add test `runtime_pid_cleanup_skips_unowned_alive_pid` under `#[cfg(test)] mod tests`. The test should construct hub path `/repo/.hotpot-hub`, runtime pid `4242`, and command `python -m http.server 8080`; it should assert that the new planning helper returns no runtime pid/process-group cleanup targets for that command.
- [x] R2: In `src/vuepress.rs`, add test `runtime_pid_cleanup_keeps_owned_vuepress_pid` using command `/repo/.hotpot-hub/node_modules/.bin/vuepress dev docs --port 8080`; on Unix it should expect `ProcessGroup(4242)` and `Pid(4242)`, on Windows it should expect `ProcessTree(4242)`, and on other platforms it should expect `Pid(4242)`.
- [x] R3: Run `cargo test runtime_pid_cleanup_`; expect failure because the ownership-aware planning helper does not exist or the old implementation still trusts any live pid.

##### Green

- [x] G1: In `src/vuepress.rs`, add a documented helper such as `runtime_pid_cleanup_targets_for_command(hub_dir: &Path, pid: u32, command: &str) -> Vec<CleanupTarget>` that reuses the same ownership predicate as `hotpot_vuepress_port_cleanup_targets`.
- [x] G2: Update `cleanup_runtime_process` to read `process_command_line(state.pid)` and only apply runtime pid / process-group / process-tree cleanup when the command is owned by the current hub; always keep `cleanup_runtime_port(root_dir, state.port)` and `discard_runtime_state(root_dir)` after the pid phase.
- [x] G3: Run `cargo test runtime_pid_cleanup_`; expect both new tests to pass.
- [x] G4: Run `cargo test stop_if_running_cleans_runtime_when_pid_dead_but_port_has_vuepress_owner status_expired_ttl_reuses_stop_cleanup`; expect existing lifecycle tests to pass.

##### Refactor

- [x] F1: Inspect `hotpot_vuepress_port_cleanup_targets` and the new runtime pid helper for duplicated ownership logic. If duplicated, extract a bilingual-documented predicate such as `is_hotpot_vuepress_command(hub_dir: &Path, command: &str) -> bool`; otherwise write `no refactor needed`.
- [x] F2: If a refactor happened, re-run `cargo test runtime_pid_cleanup_ stop_if_running_cleans_runtime_when_pid_dead_but_port_has_vuepress_owner`; expect pass. Otherwise mark `skipped (no refactor)`.

#### Task 2: Make Start Failure Cleanup Ignore Untrusted PID State

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/vuepress.rs` | Test | 固定 readiness failure / startup failure cleanup 不会因错误 runtime pid 而误杀或泄漏。 |
| `src/vuepress.rs` | Modify | 调整 `start` 失败路径复用 ownership-aware cleanup。 |

##### Red

- [x] R1: In `src/vuepress.rs`, add test `readiness_failure_cleanup_uses_owned_port_fallback_when_runtime_pid_unowned`. The test should exercise the pure cleanup planner from Task 1 with an unowned runtime pid command and a port owner command `/repo/.hotpot-hub/node_modules/.bin/vite --host 127.0.0.1 --port 8080`; it should assert that runtime targets are empty and port fallback targets include only the Hotpot-owned port owner.
- [x] R2: In `src/vuepress.rs`, add test `readiness_failure_cleanup_ignores_foreign_port_owner` with a port owner command `/other/.hotpot-hub/node_modules/.bin/vuepress dev docs --port 8080`; it should assert that no fallback pid is selected for the current hub `/repo/.hotpot-hub`.
- [x] R3: Run `cargo test readiness_failure_cleanup_`; expect failure until cleanup planning is exposed and wired consistently.

##### Green

- [x] G1: In `src/vuepress.rs`, ensure the readiness timeout path in `start` still calls `stop(root_dir, true)` or an equivalent shared cleanup path after writing runtime state, but that shared cleanup must use the ownership-aware runtime pid planning from Task 1.
- [x] G2: Keep the log-tail error context from `read_log_tail(&log_path, 20)` unchanged so users still see concrete startup diagnostics.
- [x] G3: Run `cargo test readiness_failure_cleanup_`; expect pass.
- [x] G4: Run `cargo test vuepress`; expect all VuePress unit tests to pass.

##### Refactor

- [x] F1: Inspect whether readiness failure cleanup and stale runtime cleanup now call the same internal cleanup function. If there are two divergent implementations, consolidate into one bilingual-documented helper; otherwise write `no refactor needed`.
- [x] F2: If a refactor happened, re-run `cargo test vuepress`; expect pass. Otherwise mark `skipped (no refactor)`.

#### Task 3: Preserve Status And Stop Semantics For Stale Runtime States

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/vuepress.rs` | Test | 防止修复 runtime ownership 时破坏 existing stop/status 行为。 |
| `src/vuepress.rs` | Modify | 保持 `--if-running` 幂等、TTL lazy cleanup 和 JSON status 输出稳定。 |

##### Red

- [x] R1: In `src/vuepress.rs`, add test `status_stale_unowned_runtime_pid_keeps_status_not_running`. Use the cleanup planning helpers to model a runtime state whose pid command is unowned and whose port has no Hotpot-owned owner; assert the planned state is equivalent to not running and runtime cleanup would discard `runtime.json`.
- [x] R2: In `src/vuepress.rs`, add test `stop_stale_unowned_runtime_pid_still_discards_runtime_state`. The test can target the same pure planner if direct filesystem process tests would be flaky; assert no runtime kill target is selected but cleanup still reaches the discard-runtime step.
- [x] R3: Run `cargo test stale_unowned_runtime_pid`; expect failure until the implementation exposes or records the discard-runtime decision separately from pid kill targets.

##### Green

- [x] G1: In `src/vuepress.rs`, adjust the cleanup planner or cleanup result type so tests can distinguish runtime pid targets, port fallback targets, and whether `runtime.json` is discarded.
- [x] G2: Ensure `stop(root_dir, true)` remains idempotent when runtime state is missing, corrupt, dead, or untrusted.
- [x] G3: Ensure `status(root_dir)` still prints one-line JSON with `"running": false` after stale cleanup, and still prints `port`, `url`, `pid`, and `expires_at` only for trusted live runtime state.
- [x] G4: Run `cargo test stale_unowned_runtime_pid`; expect pass.
- [x] G5: Run `cargo test vuepress`; expect pass.

##### Refactor

- [x] F1: Inspect test helper names and production helper visibility. Keep helpers private unless tests in the same module need them; avoid widening public API unnecessarily.
- [x] F2: Re-run `cargo test vuepress`; expect pass.

#### Task 4: Update Architecture Documentation And Full Validation

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify | Document that runtime pid cleanup is ownership-checked and port fallback is the trusted cleanup path for mismatched pid states. |
| `docs/ARCH.zh_CN.md` | Modify | Keep Chinese architecture documentation equivalent to `ARCH.md`. |
| `src/vuepress.rs` | Test | Update doc guard tests if they need stronger assertions for the new lifecycle contract. |

##### Red

- [x] R1: In `src/vuepress.rs`, update test `arch_documents_vuepress_prewrite_gate_and_process_cleanup` or add test `arch_documents_vuepress_runtime_pid_ownership_check` so it requires both docs to mention runtime pid ownership/归属 checks and runtime port fallback.
- [x] R2: Run `cargo test arch_documents_vuepress_runtime_pid_ownership_check arch_documents_vuepress_prewrite_gate_and_process_cleanup`; expect failure until both docs are updated.

##### Green

- [x] G1: Update `docs/ARCH.md` under VuePress service lifecycle to explain that `stop` first validates the runtime pid command belongs to the current `.hotpot-hub` before killing pid/process group/tree, and otherwise relies on runtime port fallback plus runtime state discard.
- [x] G2: Update `docs/ARCH.zh_CN.md` with equivalent Simplified Chinese content.
- [x] G3: Run `cargo test arch_documents_vuepress_runtime_pid_ownership_check arch_documents_vuepress_prewrite_gate_and_process_cleanup`; expect pass.
- [x] G4: Run `cargo test vuepress`; expect pass.
- [x] G5: Run `cargo test`; expect full suite pass.

##### Refactor

- [x] F1: Review updated docs for consistency with CLI behavior and avoid claiming background polling exists; cleanup remains lazy through `stop`, `status`, and failed `start`.
- [x] F2: Re-run `cargo test`; expect pass.

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo test runtime_pid_cleanup_` | 新增 runtime pid ownership 测试通过。 |
| `cargo test readiness_failure_cleanup_` | 启动失败 / readiness cleanup 规划测试通过。 |
| `cargo test stale_unowned_runtime_pid` | stale runtime 状态清理测试通过。 |
| `cargo test arch_documents_vuepress_runtime_pid_ownership_check arch_documents_vuepress_prewrite_gate_and_process_cleanup` | 中英文架构文档守卫测试通过。 |
| `cargo test vuepress` | VuePress 相关测试全部通过。 |
| `cargo test` | 全量 Rust 测试通过。 |

### Risks and Watchouts

::: warning
- Unix 上 negative pid 会杀进程组；必须先确认 runtime pid 命令属于当前 hub，避免 pid 复用时误杀无关进程组。
- 端口兜底必须保持保守：只清理命令行包含当前 `.hotpot-hub` 且可识别为 VuePress/Vite/pnpm/docs:dev 的进程。
- `process_command_line`、`lsof`、`kill`、`taskkill` 都可能失败；失败时应偏向“不信任 runtime pid”，再通过 runtime state discard 和端口兜底恢复。
- `start` readiness timeout 的主错误信息和 `vuepress.log` tail 对用户排障很重要，不能被清理失败吞掉。
- 新增函数、文件级说明和复杂逻辑注释必须遵守项目要求的中英双语 doc comment 风格。
:::

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task.
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff.
- Run the validation commands before reporting completion.
