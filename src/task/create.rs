//! Task creation entrypoint and its two-tier active-conflict guards.
//!
//! Owns [`create_task`] plus its `_locked` body and the
//! `HOTPOT_ALLOW_DEFAULT_USERNAME` / `--allow-default` gate. The append
//! sequence is "classify existing rows → guard → cleanup rewrite → append
//! new row", all under the cross-process lock so a concurrent writer
//! cannot slip between guard and rewrite.
//!
//! 任务创建入口及其两层 active 守卫：先做协作者级（Tier-1）"default
//! workspace 拒绝静默共享"，再做模式级（Tier-2）"In-Progress 焦点
//! 不被静默抢占"。整个"分类 → 守卫 → 清理重写 → 追加新行"流程都在
//! 跨进程锁内串行，避免并发写绕过守卫。

use anyhow::{Result, bail};
use nanoid::nanoid;

use std::fs;

use super::storage::{
    append_task_line, ensure_overview_exists, get_task_list, rewrite_overview_with,
};
use super::{CreateMode, TaskInfo, TaskStatus};
use crate::lock::with_file_lock;
use crate::paths::{overview_file_path, task_dir_path};

/// 判断是否允许在 `default` workspace 中继续创建任务。
///
/// 优先级：`allow_default` CLI flag 命中即放行；否则查环境变量
/// `HOTPOT_ALLOW_DEFAULT_USERNAME`，识别 `1` / `true` / `yes`（不区分大小写）。
///
/// Decides whether the default-username gate should let creation through.
/// `allow_default=true` short-circuits the check; otherwise we consult
/// `HOTPOT_ALLOW_DEFAULT_USERNAME` for one of `1` / `true` / `yes`.
fn is_default_allowed(allow_default: bool) -> bool {
    if allow_default {
        return true;
    }
    matches!(
        std::env::var("HOTPOT_ALLOW_DEFAULT_USERNAME")
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("1") | Some("true") | Some("yes")
    )
}

/// Creates a new task row in `overview.jsonl`, reconciling existing
/// `active` rows according to `mode` (see [`CreateMode`]).
///
/// Semantics by mode (returning the appended `TaskInfo` on success):
/// - [`CreateMode::Default`]:
///   * If any row has `active=true && status=InProgress`: bail with an
///     `ACTIVE_CONFLICT:` prefix that names the conflicting ids and the
///     recovery flags. The ledger is **not** mutated in this branch.
///   * Otherwise: silently clear every `active=true` row whose `status`
///     is `Done` or `Cancelled` (stale rows from manual edits or legacy
///     bugs), then append the new row with `active=true`.
/// - [`CreateMode::Switch`]: atomically clear EVERY `active=true` row
///   (both In-Progress and stale) in one `rewrite_overview_with` pass,
///   then append the new row with `active=true`. The caller must have
///   already obtained user consent for this preemption.
/// - [`CreateMode::Inactive`]: leave In-Progress active rows alone;
///   still clear stale active rows; append the new row with
///   `active=false`. Use when the user wanted to record a task without
///   switching execution focus.
///
/// The conflict bail uses the literal English token `ACTIVE_CONFLICT:`
/// as the message prefix. Treat it as a machine-readable marker — the
/// orchestrator pattern-matches on it to drive the "ask user, retry
/// with a flag" loop. Do not localize the prefix even if the rest of
/// the message is translated.
///
/// 在 `overview.jsonl` 创建一行新任务，并按 `mode`（见 [`CreateMode`]）
/// 处理已有 active 行。
///
/// 按模式的语义（成功时返回追加的 `TaskInfo`）：
/// - [`CreateMode::Default`]:
///   * 若已有 `active=true && status=InProgress` 行：bail，错误以
///     `ACTIVE_CONFLICT:` 前缀开头，列出冲突 id 与可用 flag。此分支
///     **不会**改动台账。
///   * 否则：静默清掉所有 `active=true` 且 status 是 `Done` /
///     `Cancelled` 的陈旧行（手工编辑或旧版本残留），再追加新行
///     `active=true`。
/// - [`CreateMode::Switch`]：在一次 `rewrite_overview_with` 中原子清掉
///   **所有** `active=true` 行（含 In-Progress 与陈旧），再追加新行
///   `active=true`。调用方应已经从用户处获取了抢占许可。
/// - [`CreateMode::Inactive`]：保留 In-Progress active 行；仍清陈旧
///   active 行；追加新行 `active=false`。供"记录任务但不切换执行
///   焦点"场景。
///
/// 冲突 bail 的消息前缀是英文字面 `ACTIVE_CONFLICT:`——把它当作机器
/// 可读 token，orchestrator 通过该前缀触发"询问用户后带 flag 重试"
/// 流程。即便其余消息按 `config.toml::language` 翻译，**前缀也必须
/// 保持英文**。
pub fn create_task(
    root_dir: &str,
    username: &str,
    title: &str,
    commit: Option<&str>,
    mode: CreateMode,
    allow_default: bool,
) -> Result<TaskInfo> {
    let overview = overview_file_path(root_dir, username);
    with_file_lock(&overview, || {
        create_task_locked(root_dir, username, title, commit, mode, allow_default)
    })
}

/// Locked body of [`create_task`]. See [`super::mark_task_done`] for the
/// rationale: the lock has to span "classify existing rows → guard → cleanup
/// rewrite → append new row" as a single critical section, otherwise a
/// concurrent writer could land between the guard check and the rewrite.
///
/// [`create_task`] 的实体逻辑，已在持锁状态下运行。"分类 → 守卫 → 清理重写
/// → 追加新行"必须在同一临界区内完成，否则并发写入会绕过守卫。
fn create_task_locked(
    root_dir: &str,
    username: &str,
    title: &str,
    commit: Option<&str>,
    mode: CreateMode,
    allow_default: bool,
) -> Result<TaskInfo> {
    let tasks = get_task_list(root_dir, username)?;

    // Classify existing active rows. Live = the genuine "currently
    // executing" set Hotpot must enforce ≤ 1 on; stale = defensive
    // cleanup target (manual edits / legacy bug residue).
    // 分类：live = 真"正在执行"，Hotpot 必须强制 ≤ 1；
    // stale = 防御性清理目标（手工编辑 / 旧 bug 残留）。
    let live_active: Vec<&TaskInfo> = tasks
        .iter()
        .filter(|task| task.active && task.status == TaskStatus::InProgress)
        .collect();
    let stale_active_count = tasks
        .iter()
        .filter(|task| task.active && task.status != TaskStatus::InProgress)
        .count();

    // ── Tier 1: default-username collaborator gate ────────────────────
    // When username == "default", any existing In-Progress row in the
    // shared workspace may belong to another collaborator who silently
    // landed on the same `"default"` fallback (no `HOTPOT_USERNAME`,
    // no `git config user.name`). Refuse to add a second task into that
    // workspace unless the operator explicitly opts in via flag or env,
    // since silent stomping is the failure mode this guard exists for.
    //
    // The bail uses the literal `ACTIVE_CONFLICT:` prefix so the same
    // slash-command machinery that handles mode-aware conflicts can
    // surface this message to the user verbatim. Bypass paths:
    //   - `--allow-default` CLI flag (plumbed via `allow_default`).
    //   - `HOTPOT_ALLOW_DEFAULT_USERNAME=1` env.
    //
    // 当 username 为 `"default"` 时，可能是协作者 B 也回退到了同一个
    // 默认工作区，会与协作者 A 互相覆盖。这条守卫只放过显式同意者
    // （`--allow-default` 或 `HOTPOT_ALLOW_DEFAULT_USERNAME=1`），
    // 拒绝默认场景。`ACTIVE_CONFLICT:` 前缀复用 slash command 的
    // 解析约定。
    if username == "default" && !live_active.is_empty() && !is_default_allowed(allow_default) {
        let listed = live_active
            .iter()
            .map(|task| format!("{} ({})", task.task_id, task.title))
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "ACTIVE_CONFLICT: 拒绝与其他协作者共享 \"default\" workspace（已有 In-Progress 任务：{listed}）。\
             请运行 `hotpot update --username <your-name>` 设置专属身份；\
             若确认为单人项目，设置 HOTPOT_ALLOW_DEFAULT_USERNAME=1 或加 `--allow-default` 旁路。"
        );
    }

    // ── Tier 2: mode-aware Default conflict gate ──────────────────────
    // Default 冲突闸：拒绝静默抢占 In-Progress 焦点。
    // Default conflict gate: refuse silent preemption of in-progress focus.
    if mode == CreateMode::Default && !live_active.is_empty() {
        let listed = live_active
            .iter()
            .map(|task| format!("{} ({})", task.task_id, task.title))
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "ACTIVE_CONFLICT: 发现 {n} 条 active=true 且 In Progress 的任务：{listed}。\
             要切换执行焦点请加 `--switch`；要仅记录不切换请加 `--inactive`。",
            n = live_active.len()
        );
    }

    // Decide post-create flags / cleanup scope based on mode.
    // 根据 mode 决定新行 active 与清理范围。
    let new_active = mode != CreateMode::Inactive;
    let clear_in_progress = mode == CreateMode::Switch;
    let must_clean = stale_active_count > 0 || (clear_in_progress && !live_active.is_empty());

    // Drop the borrow on `tasks` before any &mut closure: rewrite_overview_with
    // re-reads the file inside, and Rust can't see that they're disjoint.
    // 释放对 tasks 的借用，rewrite 内部会再读一次文件。
    drop(live_active);
    drop(tasks);

    let path = ensure_overview_exists(root_dir, username)?;

    if must_clean {
        rewrite_overview_with(root_dir, username, |task| {
            if task.active {
                let is_stale = task.status != TaskStatus::InProgress;
                // Always clear stale; clear in-progress only when Switch.
                // 陈旧总清；In-Progress 仅 Switch 模式下清。
                if is_stale || clear_in_progress {
                    task.active = false;
                }
            }
        })?;
    }

    let new_task = TaskInfo {
        time: chrono::Local::now().date_naive(),
        task_id: nanoid!(10),
        title: title.to_string(),
        commit: commit.map(str::to_string),
        status: TaskStatus::InProgress,
        active: new_active,
        // Worktree fields default to None at creation; they are populated by
        // `hotpot worktree create` only after the user opts in at the start
        // of `/hotpot:execute`. Keep them out of the create path so the
        // single-active invariant logic above stays focused on its job.
        // worktree 三列在创建时一律为 None：只有用户在 `/hotpot:execute` 开头
        // 明确同意后，才由 `hotpot worktree create` 回填这些字段。把它们留在
        // create 之外可以让单 active 不变式逻辑保持专注。
        worktree_path: None,
        worktree_branch: None,
        worktree_base_branch: None,
    };

    append_task_line(&path, &new_task)?;

    // Eagerly materialize `<workspace>/tasks/` so the slash command's first
    // `Write` on `<time>-<title>.md` does not have to rely on its create-file
    // tool's parent-dir auto-creation behavior. This also keeps `ls` results
    // consistent for users browsing the workspace right after `task create`.
    // Failures here are non-fatal: the row is already persisted, and the
    // create-file tool will surface its own ENOENT later if the directory
    // really cannot be created.
    //
    // 提前把 `<workspace>/tasks/` 物化出来：slash command 在收到 `task create`
    // 输出后第一时间 `Write` 任务 `.md`，不需要再依赖各平台 Write 工具自动建
    // 父目录的行为；也让用户用 `ls` 浏览 workspace 时立刻能看到 `tasks/`。
    // 这一步失败不致命：overview 行已经落盘，真要建不出来 Write 工具会自己
    // 报 ENOENT；不在这里 bail 是为了不把可恢复的目录问题升级成 `task create`
    // 整体失败。
    let _ = fs::create_dir_all(task_dir_path(root_dir, username));

    Ok(new_task)
}
