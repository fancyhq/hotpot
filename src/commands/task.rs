use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};

use crate::{
    context,
    task::{self, TaskInfo},
};

/// Subcommands of `hotpot task`.
///
/// `hotpot task` 的子命令集合。
#[derive(Subcommand, Debug)]
pub enum TaskCommand {
    /// List all tasks in the current user's overview ledger.
    ///
    /// 列出当前用户 overview 台账中的全部任务。
    List(ListArgs),
    /// Create a new task row in the current user's overview ledger.
    ///
    /// 在当前用户的 overview 台账中创建一条新任务。
    Create(CreateArgs),
    /// Show the unique active task (path or count).
    ///
    /// 显示唯一 active 任务（路径或计数）。
    Active(ActiveArgs),
    /// Clear the `active` flag on tasks without changing their status.
    ///
    /// 清掉任务的 `active` 标记，但不改变其 status。
    Stop(StopArgs),
    /// Mark a task as `Done`, optionally backfilling its commit hash.
    ///
    /// 把任务标记为 `Done`，可选回填 commit hash。
    Done(DoneArgs),
    /// Mark a task as `Cancelled` and clear its `active` flag.
    ///
    /// 把任务标记为 `Cancelled` 并清掉 `active` 标记。
    Cancel(CancelArgs),
    /// Resume a task, atomically making it the single active row.
    ///
    /// 恢复任务并原子地把它置为唯一 active 行。
    Resume(ResumeArgs),
}

/// CLI arguments for `hotpot task list`.
///
/// Default output is the historical TSV form (`<task_id>\t<status>\t
/// <title>\t<date>`) which downstream slash commands and shell users
/// have come to rely on. `--json` switches to JSONL (one full
/// `TaskInfo` per line), letting orchestrators distinguish live actives
/// (`status="In Progress"`) from stale actives (`status="Done"` /
/// `"Cancelled"`) without parsing TSV columns.
///
/// `hotpot task list` 的 CLI 参数。默认输出沿用原 TSV 形式
/// （`<task_id>\t<status>\t<title>\t<date>`），保持对下游 slash 命令与
/// shell 用户的兼容；`--json` 切到 JSONL（每行一个完整 `TaskInfo`），
/// 让 orchestrator 能区分 live active（`status="In Progress"`）与
/// 陈旧 active（`status="Done"` / `"Cancelled"`），不必再解析 TSV。
#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(long)]
    status: Option<String>,
    /// Emit one JSON object per task line instead of the default TSV.
    ///
    /// 每行输出一个 TaskInfo JSON 对象，替代默认的 TSV 输出。
    #[arg(long)]
    json: bool,
}

/// CLI arguments for `hotpot task stop`.
///
/// `hotpot task stop` 的 CLI 参数。
#[derive(Debug, Args)]
pub struct StopArgs {
    /// Stop every active task for the current user.
    ///
    /// 清掉当前用户全部 active 任务的 active 标记。
    #[arg(long)]
    all: bool,
}

/// CLI arguments for `hotpot task done`.
///
/// Both fields are optional. When `task_id` is omitted, the active task is
/// resolved automatically (see [`task::mark_task_done`] for the rules). When
/// `commit` is omitted, the row's existing `commit` value is preserved so the
/// command stays useful even outside git repositories.
///
/// `hotpot task done` 的 CLI 参数。两个字段都可选：`--task-id` 不传时
/// 默认锁定唯一 active 任务；`--commit` 不传时保留原有 commit 值，方便
/// 在非 git 项目中也能收尾。
#[derive(Debug, Args)]
pub struct DoneArgs {
    /// Explicit task id; defaults to the unique active task.
    ///
    /// 显式指定 task id；不传则取唯一 active 的任务。
    #[arg(long = "task-id", value_name = "ID")]
    task_id: Option<String>,

    /// Commit hash to backfill into the row.
    ///
    /// 要回填到任务行的 commit hash。
    #[arg(long, value_name = "HASH")]
    commit: Option<String>,
}

/// CLI arguments for `hotpot task cancel`.
///
/// Only `--task-id` is exposed; cancellation never ships a commit. When
/// `task_id` is omitted, the unique active task is resolved automatically
/// (see [`task::mark_task_cancelled`] for the rules).
///
/// `hotpot task cancel` 的 CLI 参数。仅暴露 `--task-id`；取消意味着
/// 未发货，因此不接收 `--commit`。`--task-id` 不传时默认锁定唯一
/// active 任务。
#[derive(Debug, Args)]
pub struct CancelArgs {
    /// Explicit task id; defaults to the unique active task.
    ///
    /// 显式指定 task id；不传则取唯一 active 的任务。
    #[arg(long = "task-id", value_name = "ID")]
    task_id: Option<String>,
}

/// CLI arguments for `hotpot task resume`.
///
/// `--task-id` is required because the resumable set (`status =
/// "In Progress"`) is generally not unique; there is no implicit
/// "active" heuristic to fall back on. Resume always rewrites the
/// ledger to enforce the single-active invariant, even when the
/// target is already `active = true`.
///
/// `hotpot task resume` 的 CLI 参数。`--task-id` 必传——可恢复
/// 集（`status = "In Progress"`）通常不唯一，没有"唯一 active"
/// 这种隐式推断可走。resume 永远会重写 overview，把目标置为唯一
/// active，即使目标已是 active 也不短路（参见决策 D3）。
#[derive(Debug, Args)]
pub struct ResumeArgs {
    /// Task id to resume; required.
    ///
    /// 要恢复并置为唯一 active 的 task id；必传。
    #[arg(long = "task-id", value_name = "ID")]
    task_id: String,
}

/// CLI arguments for `hotpot task active`.
///
/// `hotpot task active` 的 CLI 参数。
#[derive(Debug, Args)]
pub struct ActiveArgs {
    /// Print the number of active tasks instead of the path.
    ///
    /// 输出 active 任务数量，而非任务文件路径。
    #[arg(long)]
    count: bool,

    /// Print the active task file path.
    ///
    /// 输出 active 任务文件的绝对路径。
    #[arg(long)]
    path: bool,
}

/// CLI arguments for `hotpot task create`.
///
/// `--switch` and `--inactive` are mutually exclusive (enforced by
/// `clap::conflicts_with`) and together select the [`task::CreateMode`]:
/// - neither flag → [`task::CreateMode::Default`]: new row is
///   `active=true`; CLI bails with an `ACTIVE_CONFLICT:` prefix if any
///   existing row has `active=true && status=InProgress`.
/// - `--switch` → [`task::CreateMode::Switch`]: clears every existing
///   `active=true` row atomically, new row is `active=true`.
/// - `--inactive` → [`task::CreateMode::Inactive`]: leaves In-Progress
///   active rows alone, only clears stale active rows, new row is
///   `active=false`.
///
/// `hotpot task create` 的 CLI 参数。`--switch` 与 `--inactive` 互斥
/// （由 clap 的 `conflicts_with` 拦截），共同决定底层
/// [`task::CreateMode`]：不传 = Default（默认行为，遇 In-Progress
/// active 冲突 bail）；`--switch` = 抢占所有 active；`--inactive` =
/// 仅清陈旧 active 且新行 active=false。
#[derive(Args, Debug)]
pub struct CreateArgs {
    /// Positional task title (mutually exclusive with `--title`).
    ///
    /// 位置参数形式的任务标题，与 `--title` 互斥。
    #[arg(value_name = "TITLE")]
    title_arg: Option<String>,

    /// Positional commit hash (mutually exclusive with `--commit`).
    ///
    /// 位置参数形式的 commit hash，与 `--commit` 互斥。
    #[arg(value_name = "COMMIT")]
    commit_arg: Option<String>,

    /// Task title (mutually exclusive with the positional form).
    ///
    /// 任务标题（与位置参数形式互斥）。
    #[arg(long, value_name = "TITLE")]
    title: Option<String>,

    /// Commit hash to record on the new row.
    ///
    /// 写入新任务行的 commit hash。
    #[arg(long, value_name = "COMMIT")]
    commit: Option<String>,

    /// Take active focus: atomically clear every existing `active=true`
    /// row, then create with `active=true`.
    ///
    /// 抢占执行焦点：原子清掉所有现有 `active=true` 行，新行 active=true。
    #[arg(long, conflicts_with = "inactive")]
    switch: bool,

    /// Record without switching: create with `active=false`, leaving
    /// In-Progress active rows untouched. Stale active rows are still
    /// cleared as a fixed side effect.
    ///
    /// 仅记录不切换：新行 active=false，保留现有 In-Progress active 行；
    /// 陈旧 active 行仍会被清理。
    #[arg(long, conflicts_with = "switch")]
    inactive: bool,

    /// Bypass the `default` workspace collaborator guard.
    ///
    /// Allow this `create_task` invocation to proceed even when the
    /// resolved username is the literal `"default"` and the workspace
    /// already has an In-Progress row. Use only in genuine single-user
    /// projects; the safer fix is to set `git config --local user.name`.
    /// Equivalent to `HOTPOT_ALLOW_DEFAULT_USERNAME=1`.
    ///
    /// 旁路"default workspace 协作者守卫"。仅在确认单人项目时使用；
    /// 推荐改用 `git config --local user.name`。等价于
    /// `HOTPOT_ALLOW_DEFAULT_USERNAME=1`。
    #[arg(long = "allow-default")]
    allow_default: bool,
}

/// Lists all tasks from `overview.jsonl`.
///
/// Default output is TSV (`<task_id>\t<status>\t<title>\t<date>`),
/// preserved for backwards compatibility with existing slash commands.
/// `--json` switches to JSONL so orchestrators can read the full
/// `TaskInfo` (including `active`) without TSV parsing.
///
/// 列出 `overview.jsonl` 全部任务。默认输出 TSV
/// （`<task_id>\t<status>\t<title>\t<date>`），保持对既有 slash 命令
/// 的兼容。`--json` 切到 JSONL，方便 orchestrator 拿到完整 `TaskInfo`
/// （含 `active` 字段）而无需解析 TSV。
pub fn list_tasks(args: ListArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;
    let mut tasks = task::get_task_list(&root_dir, &username)?;
    if tasks.is_empty() {
        if !args.json {
            println!("No tasks found.");
        }
        // JSON 模式空集合输出空内容，保持调用方简单。
        // JSON mode: empty set emits no output to keep callers simple.
        return Ok(());
    }

    if let Some(status) = &args.status {
        tasks = filter_tasks_by_status(&tasks, status);
    }

    if args.json {
        for task in tasks {
            let line = serde_json::to_string(&task)
                .with_context(|| format!("序列化任务失败，任务ID：{}", task.task_id))?;
            println!("{line}");
        }
    } else {
        for task in tasks {
            println!(
                "{}\t{}\t{}\t{}",
                task.task_id,
                task.status.as_str(),
                task.title,
                task.time
            )
        }
    }
    Ok(())
}

pub fn filter_tasks_by_status(tasks: &[TaskInfo], status: &str) -> Vec<TaskInfo> {
    tasks
        .iter()
        .filter(|task| task.status.as_str() == status)
        .cloned()
        .collect()
}

/// Creates a new task, mapping CLI flags to the [`task::CreateMode`]
/// state machine and printing the resulting `TaskInfo` row as JSON on
/// stdout.
///
/// 创建任务：把 CLI flag 映射到 [`task::CreateMode`]，并把新行
/// `TaskInfo` JSON 输出到 stdout，与 done/cancel/resume 保持风格一致。
pub fn create_task(args: CreateArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;
    let has_positional_args = args.title_arg.is_some() || args.commit_arg.is_some();
    let has_named_args = args.title.is_some() || args.commit.is_some();

    if has_positional_args && has_named_args {
        bail!("Do not mix positional arguments with --title/--commit.");
    }

    let (title, commit) = if has_named_args {
        let title = args
            .title
            .ok_or_else(|| anyhow::anyhow!("--title is required."))?;
        (title, args.commit)
    } else {
        let title = args
            .title_arg
            .ok_or_else(|| anyhow::anyhow!("TITLE is required."))?;
        (title, args.commit_arg)
    };

    // clap 的 conflicts_with 已经拦截了 (true, true) 组合；这里只需把
    // 剩下三种组合映射到 CreateMode。
    // clap's conflicts_with already rejected (true, true); we only need
    // to map the remaining three combinations to CreateMode.
    let mode = match (args.switch, args.inactive) {
        (false, false) => task::CreateMode::Default,
        (true, false) => task::CreateMode::Switch,
        (false, true) => task::CreateMode::Inactive,
        (true, true) => unreachable!("clap conflicts_with 已拦截 --switch 与 --inactive 同时给出"),
    };

    let new_row = task::create_task(
        &root_dir,
        &username,
        &title,
        commit.as_deref(),
        mode,
        args.allow_default,
    )?;
    let line = serde_json::to_string(&new_row).context("序列化新任务失败")?;
    println!("{line}");
    Ok(())
}

pub fn active_task(args: ActiveArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;

    if args.count {
        let count = task::get_active_task_count(&root_dir, &username)?;
        println!("{count}");
    } else if args.path {
        let path = task::get_active_task_filepath(&root_dir, &username)?;
        print!("{}", path.display())
    }

    Ok(())
}

pub fn stop_task(args: StopArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;

    if args.all {
        task::stop_all_active_tasks(&root_dir, &username)?;
    } else {
        todo!()
    }

    Ok(())
}

/// Marks a task as `Done`, optionally backfilling the commit hash.
///
/// Used by `/hotpot:finish-work` once the user has confirmed completion and
/// (when applicable) the git commit has been created. Prints the updated row
/// as a single JSON object on stdout so the slash command can inspect the
/// final state without re-reading the file.
///
/// After the ledger is updated, the function attempts to sync the task file's
/// VuePress Overview `Status` cell to `Done`. This is a best-effort UI update:
/// I/O errors are written to stderr as English warnings but do NOT cause the
/// command to fail. Skipped outcomes (VuePress disabled, missing file, no
/// Overview table) are silently ignored.
///
/// 把任务标记为 `Done`，可选回填 commit hash。由 `/hotpot:finish-work`
/// 在用户确认完成、可选创建好 git commit 后调用。stdout 输出更新后这
/// 一行 JSON，方便 slash command 直接核对落盘结果。
///
/// ledger 更新后额外同步任务文件的 VuePress Overview 状态列。该同步是
/// 尽力而为的 UI 更新：I/O 错误走 stderr 输出英文 warning，但不导致命令
/// 失败。跳过类结果（VuePress 未启用、文件缺失、无 Overview 表）静默忽略。
pub fn done_task(args: DoneArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;

    let updated = task::mark_task_done(
        &root_dir,
        &username,
        args.task_id.as_deref(),
        args.commit.as_deref(),
    )?;

    // Best-effort VuePress Overview status sync: warn on I/O error, skip
    // silently for non-Updated outcomes. stdout MUST remain the single JSON
    // line — warnings go to stderr only.
    // 尽力同步 VuePress Overview 状态：I/O 错误时 warn，非 Updated 静默跳过。
    // stdout 必须保持单行 JSON 语义，warning 只能走 stderr。
    if let Err(err) = task::sync_task_file_status(&root_dir, &username, &updated) {
        eprintln!("Warning: failed to sync task Markdown status: {err}");
    }

    let line = serde_json::to_string(&updated).context("序列化更新后的任务失败")?;
    println!("{line}");
    Ok(())
}

/// Marks a task as `Cancelled` and clears its `active` flag.
///
/// Used to explicitly retire abandoned work. Prints the updated row as a
/// single JSON object on stdout so the caller can verify the persisted
/// state. Calling this on an already-`Cancelled` row is a silent no-op
/// and does NOT bump `overview.jsonl`'s mtime.
///
/// 把任务标记为 `Cancelled` 并清掉 `active`，用于显式放弃任务。
/// stdout 输出更新后这一行 JSON，方便调用方核对落盘结果。已是
/// `Cancelled` 的行幂等静默返回，且**不会**触发 `overview.jsonl`
/// 写入（mtime 不变）。
pub fn cancel_task(args: CancelArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;

    let updated = task::mark_task_cancelled(&root_dir, &username, args.task_id.as_deref())?;

    let line = serde_json::to_string(&updated).context("序列化更新后的任务失败")?;
    println!("{line}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::Builder;

    /// Validates that `done_task` wires the VuePress Overview status sync
    /// after the ledger is updated: after marking a task Done, the
    /// task's Markdown file should have its Overview `Status` cell set
    /// to `Done`.
    ///
    /// 验证 `done_task` 正确接入 VuePress Overview 状态同步：将任务标记为
    /// Done 后，任务 Markdown 文件的 Overview `Status` 单元格应为 `Done`。
    #[test]
    fn done_task_sync_helper_updates_task_file_after_ledger_done() {
        let root = Builder::new()
            .prefix("hotpot-done-sync-")
            .tempdir()
            .unwrap();
        let root_dir = root.path().display().to_string();
        let username = "done_sync_test_user";

        // Use the crate-wide ScopedEnvVar guard: saves current values,
        // applies overrides, and restores everything in Drop (even on panic).
        // HOTPOT_VUEPRESS_ENABLED is UNSET so it falls through to the
        // .hotpot/config.toml detection — this avoids polluting the
        // process env for concurrent tests running in other modules.
        // 使用 crate 级 ScopedEnvVar 守卫：保存当前值、施加新值、析构时
        // 自动恢复（含 panic 路径）。不设 HOTPOT_VUEPRESS_ENABLED env var，
        // 让 resolve_vuepress_enabled 走 config.toml 路径。
        let _env = crate::test_support::ScopedEnvVar::new(&[
            ("ROOT_DIR", Some(&root_dir)),
            ("HOTPOT_USERNAME", Some(username)),
            ("HOTPOT_VUEPRESS_ENABLED", None),
        ]);

        // Create .hotpot/config.toml (VuePress enabled via config file).
        let config_dir = root.path().join(".hotpot");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.toml"),
            "[vuepress]\nenabled = true\nport = 8080\n",
        )
        .unwrap();

        // Create tasks directory.
        let tasks_dir = root
            .path()
            .join(".hotpot/workspaces")
            .join(username)
            .join("tasks");
        fs::create_dir_all(&tasks_dir).unwrap();

        // Create a task in the ledger.
        let task_row = task::create_task(
            &root_dir,
            username,
            "done-sync-test-task",
            None,
            task::CreateMode::Default,
            false,
        )
        .expect("create_task should succeed");

        // Create the task .md file with an Overview table ("In Progress").
        let task_filename = task::get_task_filename(&task_row);
        let task_file = tasks_dir.join(format!("{task_filename}.md"));
        let md_content = format!(
            r#"# {title}

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | true | 4 | medium |
:::

## Task
"#,
            title = task_row.title
        );
        fs::write(&task_file, &md_content).unwrap();

        // Call done_task — this must sync the Overview Status to "Done".
        let args = DoneArgs {
            task_id: Some(task_row.task_id.clone()),
            commit: None,
        };
        done_task(args).expect("done_task should succeed");

        // Verify: task file's Overview Status cell should be "Done".
        let updated_content = fs::read_to_string(&task_file).unwrap();
        assert!(
            updated_content.contains("| Done | true | 4 | medium |"),
            "Task file Overview Status should be 'Done' after done_task.\nFile content:\n{updated_content}"
        );
        assert!(
            !updated_content.contains("| In Progress |"),
            "Old status 'In Progress' should no longer appear.\nFile content:\n{updated_content}"
        );

        // _env drops here, restoring ROOT_DIR, HOTPOT_USERNAME, and
        // HOTPOT_VUEPRESS_ENABLED to their original values — even on panic.
        // _env 在此析构，自动恢复 ROOT_DIR、HOTPOT_USERNAME、
        // HOTPOT_VUEPRESS_ENABLED 到原始值（含 panic 路径）。
    }
}

/// Resumes a task by flipping its `active` flag to `true` and clearing the
/// `active` flag on every other row.
///
/// Used by `/hotpot:finish-work`'s "Offer to Resume Next Task" step. Prints
/// the updated `TaskInfo` row as a single JSON object on stdout so the slash
/// command can read the canonical state without re-querying.
///
/// 把任务 resume：将目标行 `active` 置 `true`，并把所有其他行
/// `active` 清零，强制维持单 active 不变式。由 `/hotpot:finish-work`
/// 的"Offer to Resume Next Task"步骤调用。stdout 输出更新后这一行 JSON。
pub fn resume_task(args: ResumeArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;

    let updated = task::mark_task_resumed(&root_dir, &username, &args.task_id)?;

    let line = serde_json::to_string(&updated).context("序列化更新后的任务失败")?;
    println!("{line}");
    Ok(())
}
