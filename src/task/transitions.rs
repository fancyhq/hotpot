//! State transitions on `overview.jsonl` rows.
//!
//! Houses the one-way state-machine entrypoints: stop-all, mark-done,
//! mark-cancelled, mark-resumed. Each public wrapper acquires the
//! cross-process lock from [`crate::lock::with_file_lock`] and delegates
//! to a `_locked` body, mirroring the original monolithic layout so the
//! "lock scope = entire read-check-write critical section" invariant is
//! preserved.
//!
//! 任务状态切换的实体逻辑：stop-all / mark-done / mark-cancelled /
//! mark-resumed。每个对外函数先通过 [`crate::lock::with_file_lock`]
//! 取跨进程锁，再交给同名 `_locked` 子函数完成"读 → 校验 → 写"。
//! 拆分后锁作用域与原单文件版本完全一致——仍然覆盖整个临界区。

use anyhow::{Ok, Result, bail};

use super::storage::{get_active_task_count, get_task_list, rewrite_overview_with};
use super::{TaskInfo, TaskStatus};
use crate::lock::with_file_lock;
use crate::paths::overview_file_path;

/// Stop all active tasks, typically used during new task creation when other tasks are lingering.
///
/// 停止所有 active 的任务，通常在创建新任务时，有其他遗留时使用
pub fn stop_all_active_tasks(root_dir: &str, username: &str) -> Result<()> {
    // Cross-process lock spans the rewrite + verify cycle so a concurrent
    // mutator can't slip in between them.
    // 跨进程锁覆盖整个「重写 + 校验」逻辑。
    let overview = overview_file_path(root_dir, username);
    with_file_lock(&overview, || {
        rewrite_overview_with(root_dir, username, |task| {
            task.active = false;
        })?;

        let active_task_count = get_active_task_count(root_dir, username)?;
        if active_task_count > 0 {
            return Err(anyhow::anyhow!("Stop failed."));
        }
        Ok(())
    })
}

/// Marks a task as `Done`, clears its `active` flag, and optionally backfills the
/// `commit` hash, then returns the updated row.
///
/// Resolution:
/// - `task_id = Some(id)` targets that exact row; errors if the id is missing.
/// - `task_id = None` requires exactly one `active = true` row; errors with the
///   list of ambiguous ids when more than one active row exists, and errors
///   with a recovery hint when zero are active.
///
/// Idempotency:
/// - Calling this on an already-`Done` row succeeds when the supplied `commit`
///   is `None`, matches the stored commit, or fills in a previously empty
///   commit. It refuses to overwrite a non-empty stored commit with a
///   different non-empty value.
/// - Calling this on a `Cancelled` row is rejected; `finish-work` should not
///   resurrect cancelled work.
///
/// 将一条任务标记为 `Done` 并清掉 `active`，可选回填 `commit` hash，
/// 返回更新后的那一行。
///
/// 选行规则：
/// - `task_id = Some(id)` 时定位到精确行；找不到则报错。
/// - `task_id = None` 时要求恰好一条 `active = true`：>1 条会把所有候选
///   的 id 列出来让调用方加 `--task-id`；0 条会给出排查提示。
///
/// 幂等规则：
/// - 已经是 `Done` 的行允许再次调用：传入 commit 为 `None`、与已有
///   commit 一致、或填补此前为空的 commit 都视为成功；只有"已有非空
///   commit 与传入非空 commit 不同"才报错，避免覆盖历史。
/// - `Cancelled` 的行拒绝转回 `Done`，`finish-work` 不应让已取消任务复活。
pub fn mark_task_done(
    root_dir: &str,
    username: &str,
    task_id: Option<&str>,
    commit: Option<&str>,
) -> Result<TaskInfo> {
    let overview = overview_file_path(root_dir, username);
    with_file_lock(&overview, || {
        mark_task_done_locked(root_dir, username, task_id, commit)
    })
}

/// Locked-body of [`mark_task_done`]. The outer wrapper holds the advisory
/// lock; this function performs the actual read/check/write logic. Split so
/// the lock scope is obvious and so the lock helper sees a single closure.
///
/// [`mark_task_done`] 的实体逻辑，已经在持锁状态下运行。拆出来让加锁范围
/// 一目了然。
fn mark_task_done_locked(
    root_dir: &str,
    username: &str,
    task_id: Option<&str>,
    commit: Option<&str>,
) -> Result<TaskInfo> {
    let tasks = get_task_list(root_dir, username)?;

    // Resolve the target task_id from either an explicit value or the
    // (singleton) active row. We capture the id by value so the mutator
    // closure does not borrow from `tasks`, which we drop before rewriting.
    // 先把目标 task_id 解析成一个独立的 String，避免后续的 mutator
    // 闭包跨过原 task_list 的借用。
    let target_id: String = match task_id {
        Some(id) => {
            let exists = tasks.iter().any(|task| task.task_id == id);
            if !exists {
                bail!("task_id = {id} not found; run `hotpot task list` to check available tasks.");
            }
            id.to_string()
        }
        None => {
            let active_rows: Vec<&TaskInfo> = tasks.iter().filter(|task| task.active).collect();
            match active_rows.len() {
                0 => bail!(
                    "No active task to mark as Done. Run `hotpot task list` to view existing tasks, or pass `--task-id` explicitly."
                ),
                1 => active_rows[0].task_id.clone(),
                _ => {
                    let listed = active_rows
                        .iter()
                        .map(|task| format!("{} ({})", task.task_id, task.title))
                        .collect::<Vec<_>>()
                        .join(", ");
                    bail!(
                        "Found {n} active=true tasks: {listed}. Use `--task-id` to specify which one to mark as Done.",
                        n = active_rows.len()
                    );
                }
            }
        }
    };

    // Pre-check status invariants before doing any write.
    // 写入前先校验状态约束，避免写到一半才发现违规。
    let current = tasks
        .iter()
        .find(|task| task.task_id == target_id)
        .expect("target_id must be in tasks; existence checked above");

    if current.status == TaskStatus::Cancelled {
        bail!(
            "Task {id} is Cancelled, cannot mark as Done.",
            id = current.task_id
        );
    }
    if current.status == TaskStatus::Done
        && let (Some(existing), Some(incoming)) = (current.commit.as_deref(), commit)
        && existing != incoming
    {
        bail!(
            "Task {id} is Done with commit={existing}, refusing to overwrite with {incoming}.",
            id = current.task_id
        );
    }

    let incoming_commit = commit.map(str::to_string);
    rewrite_overview_with(root_dir, username, |task| {
        if task.task_id == target_id {
            task.status = TaskStatus::Done;
            task.active = false;
            if let Some(new_commit) = &incoming_commit {
                task.commit = Some(new_commit.clone());
            }
        }
    })?;

    // Re-read once to return the canonical persisted form to the caller.
    // 重新读一次拿到落盘后的真实状态返回，避免内存里的值与磁盘漂移。
    let updated = get_task_list(root_dir, username)?
        .into_iter()
        .find(|task| task.task_id == target_id)
        .ok_or_else(|| anyhow::anyhow!("failed to re-locate task_id = {target_id} after update"))?;
    Ok(updated)
}

/// Marks a task as `Cancelled`, clears its `active` flag, and returns the
/// updated row.
///
/// Resolution:
/// - `task_id = Some(id)` targets that exact row; errors if the id is missing.
/// - `task_id = None` requires exactly one `active = true` row; errors with the
///   list of ambiguous ids when more than one active row exists, and errors
///   with a recovery hint when zero are active.
///
/// Status invariants (symmetric with [`mark_task_done`]):
/// - Calling this on a `Done` row is rejected; cancellation is meant for
///   abandoned work, not for retroactively un-shipping a finished task. This
///   mirrors `mark_task_done`'s refusal to flip a `Cancelled` row back to
///   `Done`, keeping the state machine one-way.
/// - Calling this on an already-`Cancelled` row is a silent no-op: it returns
///   the snapshot row clone without touching disk. Importantly, this means
///   `overview.jsonl`'s mtime is NOT bumped on a repeat cancel; downstream
///   tooling must not rely on mtime to observe duplicate cancel attempts.
///
/// No `commit` argument is accepted because cancellation semantically means
/// "this work never shipped".
///
/// 将一条任务标记为 `Cancelled` 并清掉 `active`，返回更新后的那一行。
///
/// 选行规则：
/// - `task_id = Some(id)` 精确定位；找不到则报错。
/// - `task_id = None` 要求恰好一条 `active = true`：>1 条会把所有候选 id
///   列出来让调用方加 `--task-id`；0 条会给出排查提示。
///
/// 状态约束（与 [`mark_task_done`] 对称）：
/// - 已经是 `Done` 的行拒绝转为 `Cancelled`：取消是给"放弃任务"用的，
///   不应让已完成的任务被反向撤销。与 `mark_task_done` 拒绝
///   `Cancelled → Done` 对称，保持状态机单向不可逆。
/// - 已经是 `Cancelled` 的行幂等静默返回：直接克隆快照行返回，**不写盘**。
///   注意这意味着重复 cancel 时 `overview.jsonl` 的 mtime 不会被更新——
///   下游若依赖 mtime 监听变化，需要自行处理。
///
/// 取消语义即"未发货"，因此不接收 `commit` 参数。
pub fn mark_task_cancelled(
    root_dir: &str,
    username: &str,
    task_id: Option<&str>,
) -> Result<TaskInfo> {
    let overview = overview_file_path(root_dir, username);
    with_file_lock(&overview, || {
        mark_task_cancelled_locked(root_dir, username, task_id)
    })
}

/// Locked body of [`mark_task_cancelled`]. See [`mark_task_done_locked`].
fn mark_task_cancelled_locked(
    root_dir: &str,
    username: &str,
    task_id: Option<&str>,
) -> Result<TaskInfo> {
    let tasks = get_task_list(root_dir, username)?;

    // Resolve the target task_id from either an explicit value or the
    // (singleton) active row. Mirrors mark_task_done's resolution structure.
    // 选行逻辑与 mark_task_done 完全对称，仅替换措辞。
    let target_id: String = match task_id {
        Some(id) => {
            let exists = tasks.iter().any(|task| task.task_id == id);
            if !exists {
                bail!("task_id = {id} not found; run `hotpot task list` to check available tasks.");
            }
            id.to_string()
        }
        None => {
            let active_rows: Vec<&TaskInfo> = tasks.iter().filter(|task| task.active).collect();
            match active_rows.len() {
                0 => bail!(
                    "No active task to mark as Cancelled. Run `hotpot task list` to view existing tasks, or pass `--task-id` explicitly."
                ),
                1 => active_rows[0].task_id.clone(),
                _ => {
                    let listed = active_rows
                        .iter()
                        .map(|task| format!("{} ({})", task.task_id, task.title))
                        .collect::<Vec<_>>()
                        .join(", ");
                    bail!(
                        "Found {n} active=true tasks: {listed}. Use `--task-id` to specify which one to cancel.",
                        n = active_rows.len()
                    );
                }
            }
        }
    };

    // Pre-check status invariants before doing any write.
    // 写入前先校验状态约束，避免写到一半才发现违规。
    let current = tasks
        .iter()
        .find(|task| task.task_id == target_id)
        .expect("target_id must be in tasks; existence checked above");

    if current.status == TaskStatus::Done {
        bail!(
            "Task {id} is Done, cannot mark as Cancelled.",
            id = current.task_id
        );
    }
    if current.status == TaskStatus::Cancelled {
        // Idempotent short-circuit: return the snapshot row clone without
        // calling rewrite_overview_with, so we do not bump the file's mtime
        // on repeat cancels.
        // 幂等短路：直接返回快照中的该行 clone，不调用 rewrite_overview_with，
        // 避免无意义的 tmp+rename 写入与 mtime 抖动。
        return Ok(current.clone());
    }

    rewrite_overview_with(root_dir, username, |task| {
        if task.task_id == target_id {
            task.status = TaskStatus::Cancelled;
            task.active = false;
        }
    })?;

    // Re-read once to return the canonical persisted form to the caller.
    // 重新读一次拿到落盘后的真实状态返回，避免内存里的值与磁盘漂移。
    let updated = get_task_list(root_dir, username)?
        .into_iter()
        .find(|task| task.task_id == target_id)
        .ok_or_else(|| anyhow::anyhow!("failed to re-locate task_id = {target_id} after update"))?;
    Ok(updated)
}

/// Attaches a git worktree to a task by writing `worktree_path`,
/// `worktree_branch`, and `worktree_base_branch` to that row.
///
/// Resolution & invariants:
/// - Target row is selected by exact `task_id`; missing id is rejected.
/// - The task must be `InProgress`. Worktrees attached to `Done` /
///   `Cancelled` rows have no consumer (execute & finish-work both
///   require an in-progress task) and would only accumulate as stale
///   metadata, so we refuse early.
/// - At most one worktree per task — a non-`None` `worktree_path` is
///   treated as the source of truth and the call is rejected. The
///   caller is expected to `detach_worktree` first (and clean up the
///   filesystem) before re-attaching. This refusal lets the CLI
///   handler return a clear "already attached" message instead of
///   silently re-pointing the field.
///
/// Note: this function only writes the ledger. It does **not** run
/// `git worktree add` — that is the domain layer's responsibility
/// (`src/worktree/mod.rs`) and must complete successfully **before**
/// the caller invokes this, since the cross-process lock here forbids
/// spawning subprocesses inside the locked region (see `AGENTS.md`).
///
/// 给一条任务挂上 worktree 元信息（写入 `worktree_path` /
/// `worktree_branch` / `worktree_base_branch` 三列）。
///
/// 选行与约束：
/// - 必须传 `task_id`（精确匹配）；找不到则报错。
/// - 任务状态必须是 `InProgress`：execute / finish-work 都只对
///   In-Progress 行有意义，把 worktree 挂到 Done / Cancelled 行只会
///   堆积陈旧元数据，故提前拒绝。
/// - 一条任务最多一份 worktree：已经挂载（`worktree_path` 非 `None`）
///   时拒绝再挂，调用方需先 `detach_worktree` 并清理文件系统再重挂。
///   该限制让 CLI 能返回明确的 "already attached" 提示，而不是
///   静默改写字段。
///
/// 注意：本函数只写台账，**不**调用 `git worktree add` —— 那是
/// `src/worktree/mod.rs` 的职责，且必须在调用本函数**之前**完成。
/// 持锁区域内严禁 spawn 子进程（见 `AGENTS.md`）。
pub fn attach_worktree(
    root_dir: &str,
    username: &str,
    task_id: &str,
    path: &str,
    branch: &str,
    base_branch: &str,
) -> Result<TaskInfo> {
    let overview = overview_file_path(root_dir, username);
    with_file_lock(&overview, || {
        attach_worktree_locked(root_dir, username, task_id, path, branch, base_branch)
    })
}

/// Locked body of [`attach_worktree`]. See [`mark_task_done_locked`].
fn attach_worktree_locked(
    root_dir: &str,
    username: &str,
    task_id: &str,
    path: &str,
    branch: &str,
    base_branch: &str,
) -> Result<TaskInfo> {
    let tasks = get_task_list(root_dir, username)?;
    let current = tasks
        .iter()
        .find(|task| task.task_id == task_id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "task_id = {task_id} not found; run `hotpot task list` to check available tasks."
            )
        })?;

    if current.status != TaskStatus::InProgress {
        bail!(
            "Task {id} status is {status}, cannot attach worktree (only In-Progress tasks qualify).",
            id = current.task_id,
            status = current.status.as_str()
        );
    }
    if let Some(existing) = current.worktree_path.as_deref() {
        bail!(
            "Task {id} already has a worktree at {existing}. Run `hotpot worktree remove --task-id {id}` first.",
            id = current.task_id
        );
    }

    let target_id = task_id.to_string();
    let path_owned = path.to_string();
    let branch_owned = branch.to_string();
    let base_branch_owned = base_branch.to_string();
    rewrite_overview_with(root_dir, username, |task| {
        if task.task_id == target_id {
            task.worktree_path = Some(path_owned.clone());
            task.worktree_branch = Some(branch_owned.clone());
            task.worktree_base_branch = Some(base_branch_owned.clone());
        }
    })?;

    let updated = get_task_list(root_dir, username)?
        .into_iter()
        .find(|task| task.task_id == target_id)
        .ok_or_else(|| anyhow::anyhow!("failed to re-locate task_id = {target_id} after update"))?;
    Ok(updated)
}

/// Detaches a git worktree from a task by clearing `worktree_path`,
/// `worktree_branch`, and `worktree_base_branch`.
///
/// Idempotent: a row whose `worktree_path` is already `None` returns
/// successfully without a rewrite (mtime unchanged), matching
/// [`mark_task_cancelled`]'s repeat-call semantics.
///
/// Like [`attach_worktree`], this function only touches the ledger —
/// the caller is responsible for running `git worktree remove`
/// **before** invoking this (the lock here forbids subprocess spawns).
///
/// 从任务行上摘除 worktree 元信息（清零三列）。
///
/// 幂等：`worktree_path` 已为 `None` 时直接返回，不调用 rewrite，
/// mtime 不抖动，与 [`mark_task_cancelled`] 的重复调用语义对称。
///
/// 与 [`attach_worktree`] 一样，本函数只写台账：调用方必须先在
/// 持锁区域**之外**完成 `git worktree remove`，再调用本函数。
pub fn detach_worktree(root_dir: &str, username: &str, task_id: &str) -> Result<TaskInfo> {
    let overview = overview_file_path(root_dir, username);
    with_file_lock(&overview, || {
        detach_worktree_locked(root_dir, username, task_id)
    })
}

/// Locked body of [`detach_worktree`]. See [`mark_task_done_locked`].
fn detach_worktree_locked(root_dir: &str, username: &str, task_id: &str) -> Result<TaskInfo> {
    let tasks = get_task_list(root_dir, username)?;
    let current = tasks
        .iter()
        .find(|task| task.task_id == task_id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "task_id = {task_id} not found; run `hotpot task list` to check available tasks."
            )
        })?;

    if current.worktree_path.is_none() {
        // Idempotent short-circuit; do not bump mtime on repeated detach.
        // 幂等短路：未挂载时不重写，避免无意义的 mtime 抖动。
        return Ok(current.clone());
    }

    let target_id = task_id.to_string();
    rewrite_overview_with(root_dir, username, |task| {
        if task.task_id == target_id {
            task.worktree_path = None;
            task.worktree_branch = None;
            task.worktree_base_branch = None;
        }
    })?;

    let updated = get_task_list(root_dir, username)?
        .into_iter()
        .find(|task| task.task_id == target_id)
        .ok_or_else(|| anyhow::anyhow!("failed to re-locate task_id = {target_id} after update"))?;
    Ok(updated)
}

/// Resumes a task by flipping its `active` flag to `true` and clearing the
/// `active` flag on every other row in `overview.jsonl`.
///
/// Resolution:
/// - `task_id` is required (`&str`, not `Option<&str>`). The resumable set is
///   every row with `status = "In Progress"`, which is generally not unique;
///   there is no implicit "single active" heuristic to fall back on, unlike
///   [`mark_task_done`] / [`mark_task_cancelled`].
///
/// Status invariants:
/// - Only `InProgress` rows can be resumed.
/// - `Done` is rejected: shipped work must not be revived.
/// - `Cancelled` is rejected: abandoned work must not be revived.
/// - Both refusals are symmetric with `mark_task_done`'s `Cancelled → Done`
///   guard and `mark_task_cancelled`'s `Done → Cancelled` guard, keeping the
///   task state machine strictly one-way.
///
/// No idempotent short-circuit:
/// - Even when the target row is already `active = true`, this function still
///   calls [`rewrite_overview_with`] and rewrites the entire ledger. That is
///   intentional — resume's purpose is to *collapse* any stray `active` rows
///   back to a single-active invariant, so a repeat call is the cleanup
///   mechanism, not a bug to optimize away. Downstream tooling MAY observe
///   `overview.jsonl`'s mtime advancing on a repeat resume.
///
/// Commit:
/// - No `commit` argument is accepted. Resume is a pure activation switch and
///   has no ship-related semantics.
///
/// 把任务 resume：将目标行的 `active` 置为 `true`，并把**所有**其他行
/// 的 `active` 清为 `false`，强制维持"全工作区仅一条 active 任务"的
/// 不变式。
///
/// 选行规则：
/// - `task_id` 必传（`&str`，不是 `Option<&str>`）。可恢复集是所有
///   `status = "In Progress"` 的行，通常不唯一，没有"唯一 active"
///   这种隐式推断可走（与 [`mark_task_done`] / [`mark_task_cancelled`]
///   故意不同）。
///
/// 状态约束：
/// - 仅允许 `InProgress`。
/// - `Done` 拒绝：已发货的工作不能复活。
/// - `Cancelled` 拒绝：已放弃的工作不能复活。
/// - 两条拒绝规则与 `mark_task_done` 拒绝 `Cancelled → Done`、
///   `mark_task_cancelled` 拒绝 `Done → Cancelled` 对称，整体保持
///   任务状态机单向不可逆。
///
/// **不做幂等短路**：即便目标行已是 `active = true`，依旧走
/// [`rewrite_overview_with`] 重写整个台账。这是设计有意——resume 的
/// 价值正是把任何残留 active 行收敛回单 active 不变式；重复 resume
/// 是清理机制，不是性能 bug，请不要将其"优化"为短路返回。下游可能
/// 观察到重复 resume 时 `overview.jsonl` 的 mtime 被更新。
///
/// 不接收 `commit` 参数：resume 只是激活切换，不涉及发货语义。
pub fn mark_task_resumed(root_dir: &str, username: &str, task_id: &str) -> Result<TaskInfo> {
    let overview = overview_file_path(root_dir, username);
    with_file_lock(&overview, || {
        mark_task_resumed_locked(root_dir, username, task_id)
    })
}

/// Locked body of [`mark_task_resumed`]. See [`mark_task_done_locked`].
fn mark_task_resumed_locked(root_dir: &str, username: &str, task_id: &str) -> Result<TaskInfo> {
    let tasks = get_task_list(root_dir, username)?;

    // Existence check: bail with a recovery hint that names the missing id.
    // 存在性校验：未命中时报错并指引用户先跑 `hotpot task list` 核对。
    let exists = tasks.iter().any(|task| task.task_id == task_id);
    if !exists {
        bail!("task_id = {task_id} not found; run `hotpot task list` to check available tasks.");
    }

    // Pre-check status invariants before doing any write.
    // 写入前先校验状态约束，避免写到一半才发现违规。
    let current = tasks
        .iter()
        .find(|task| task.task_id == task_id)
        .expect("task_id must be in tasks; existence checked above");
    match current.status {
        TaskStatus::InProgress => {
            // Allowed; fall through to the rewrite.
            // 允许 resume，进入下方重写。
        }
        TaskStatus::Done => bail!("Task {id} is Done, cannot resume.", id = current.task_id),
        TaskStatus::Cancelled => bail!(
            "Task {id} is Cancelled, cannot resume.",
            id = current.task_id
        ),
    }

    // Enforce the single-active invariant: target row becomes active, every
    // other row's `active` is cleared. No idempotent short-circuit — this
    // rewrite is exactly how resume sweeps stray active rows.
    // 强制单 active 不变式：目标行置 active=true，其他行 active=false。
    // 不做幂等短路——这次重写就是 resume 清扫残留 active 的核心副作用。
    let target_id = task_id.to_string();
    rewrite_overview_with(root_dir, username, |task| {
        task.active = task.task_id == target_id;
    })?;

    // Re-read once to return the canonical persisted form to the caller.
    // 重新读一次拿到落盘后的真实状态返回，避免内存里的值与磁盘漂移。
    let updated = get_task_list(root_dir, username)?
        .into_iter()
        .find(|task| task.task_id == target_id)
        .ok_or_else(|| anyhow::anyhow!("failed to re-locate task_id = {target_id} after update"))?;
    Ok(updated)
}
