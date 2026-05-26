//! `hotpot worktree` subcommand CLI surface.
//!
//! Thin shell over [`crate::worktree`]: parse flags, resolve
//! `root_dir` / `username`, dispatch. Output style is symmetric with
//! `hotpot task …`: state-changing subcommands emit one JSON line per
//! affected row on success; pure queries emit a single path or JSONL.
//!
//! Subcommands:
//! - `create [--task-id <id>]` — create the worktree, attach metadata,
//!   emit a JSON line on stdout (so slash commands can capture the
//!   absolute path without parsing `git worktree list`).
//! - `remove [--task-id <id>] [--keep-branch] [--force]` — remove the
//!   worktree and (by default) its branch.
//! - `path [--task-id <id>]` — print the attached path; exits 0 with
//!   empty stdout when nothing is attached, so prompt bodies can probe
//!   cheaply without a JSON parse.
//! - `list [--json]` — list all attached worktrees (TSV by default,
//!   JSONL with `--json`), matching `hotpot task list`.
//!
//! `hotpot worktree` 子命令的 CLI 入口。形态对齐 `hotpot task …`：
//! 写操作单行 JSON 输出，读操作输出纯路径或 JSONL。
//!
//! 子命令清单：
//! - `create [--task-id <id>]`：创建 worktree 并挂载，stdout 输出一行
//!   JSON（路径可直接被 slash command 抓取，避免再解析 `git worktree list`）。
//! - `remove [--task-id <id>] [--keep-branch] [--force]`：删除 worktree
//!   并默认删除分支。
//! - `path [--task-id <id>]`：输出已挂载的绝对路径；未挂载时退出码为 0
//!   且 stdout 为空，方便 prompt body 用一行 shell 探测。
//! - `list [--json]`：列出全部已挂载 worktree，默认 TSV，`--json` 切换
//!   JSONL；与 `hotpot task list` 行为一致。

use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use crate::{context, task, worktree};

/// Subcommands of `hotpot worktree`.
///
/// `hotpot worktree` 的子命令集合。
#[derive(Subcommand, Debug)]
#[command(about = "Manage git worktrees (create, remove, path, list).")]
pub enum WorktreeCommand {
    /// Create a worktree for the resolved task and attach metadata.
    ///
    /// 为目标任务创建 worktree 并挂载元信息。
    #[command(
        about = "Create a worktree for the resolved task and attach metadata.",
        long_about = None
    )]
    Create(CreateArgs),
    /// Remove the resolved task's worktree (and by default its branch).
    ///
    /// 删除目标任务的 worktree（默认连同分支一起删除）。
    #[command(
        about = "Remove the resolved task's worktree (and by default its branch).",
        long_about = None
    )]
    Remove(RemoveArgs),
    /// Print the attached worktree path for the resolved task.
    ///
    /// 输出目标任务已挂载的 worktree 路径.
    #[command(
        about = "Print the attached worktree path for the resolved task.",
        long_about = None
    )]
    Path(PathArgs),
    /// List every attached worktree for the current user.
    ///
    /// 列出当前用户所有已挂载的 worktree.
    #[command(
        about = "List every attached worktree for the current user.",
        long_about = None
    )]
    List(ListArgs),
}

/// Shared `--task-id <id>` argument with active-task fallback.
///
/// 共用的 `--task-id <id>` 选项，未传时回退到唯一 active 任务。
#[derive(Debug, Args)]
#[command(
    about = "Create a worktree for the resolved task.",
    long_about = None
)]
pub struct CreateArgs {
    /// Explicit task id; defaults to the unique active task.
    ///
    /// 显式指定 task id；不传则取唯一 active 任务。
    #[arg(
        long = "task-id",
        value_name = "ID",
        help = "Explicit task id; defaults to the unique active task.",
        long_help = None
    )]
    task_id: Option<String>,
}

#[derive(Debug, Args)]
#[command(
    about = "Remove the resolved task's worktree (and by default its branch).",
    long_about = None
)]
pub struct RemoveArgs {
    /// Explicit task id; defaults to the unique active task.
    ///
    /// 显式指定 task id；不传则取唯一 active 任务。
    #[arg(
        long = "task-id",
        value_name = "ID",
        help = "Explicit task id; defaults to the unique active task.",
        long_help = None
    )]
    task_id: Option<String>,

    /// Keep the `hotpot/<id>` branch after removing the worktree.
    ///
    /// 删除 worktree 但保留 `hotpot/<id>` 分支。
    #[arg(
        long = "keep-branch",
        help = "Keep the hotpot/<id> branch after removing the worktree.",
        long_help = None
    )]
    keep_branch: bool,

    /// Force removal even if the worktree is missing or dirty.
    ///
    /// 即便 worktree 缺失或带有未提交改动，也强制清理。
    #[arg(
        long,
        help = "Force removal even if the worktree is missing or dirty.",
        long_help = None
    )]
    force: bool,
}

#[derive(Debug, Args)]
#[command(
    about = "Print the attached worktree path for the resolved task.",
    long_about = None
)]
pub struct PathArgs {
    /// Explicit task id; defaults to the unique active task.
    ///
    /// 显式指定 task id；不传则取唯一 active 任务。
    #[arg(
        long = "task-id",
        value_name = "ID",
        help = "Explicit task id; defaults to the unique active task.",
        long_help = None
    )]
    task_id: Option<String>,
}

#[derive(Debug, Args)]
#[command(
    about = "List every attached worktree for the current user.",
    long_about = None
)]
pub struct ListArgs {
    /// Emit one JSON object per worktree line instead of TSV.
    ///
    /// 每行输出一个 WorktreeInfo JSON，替代默认的 TSV 输出。
    #[arg(
        long,
        help = "Emit one JSON object per worktree line instead of TSV.",
        long_help = None
    )]
    json: bool,
}

/// Resolves the target `task_id` for a subcommand: explicit value
/// wins, otherwise fall back to the unique active task. Errors with
/// the same wording style as `task::mark_task_done`'s active fallback.
///
/// 解析子命令的目标 task_id：显式 `--task-id` 优先，否则取唯一 active
/// 任务。错误措辞与 `task::mark_task_done` 的 active 回退一致。
fn resolve_target_task_id(
    root_dir: &str,
    username: &str,
    explicit: Option<&str>,
) -> Result<String> {
    if let Some(id) = explicit {
        return Ok(id.to_string());
    }
    let active = task::get_active_task(root_dir, username)?;
    Ok(active.task_id)
}

/// Creates a worktree for the resolved task and prints the new metadata
/// as a single JSON line on stdout.
///
/// 为目标任务创建 worktree，stdout 输出一行 JSON 元信息。
pub fn create(args: CreateArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;
    let task_id = resolve_target_task_id(&root_dir, &username, args.task_id.as_deref())?;
    let info = worktree::create_worktree(&root_dir, &username, &task_id)?;
    let line = serde_json::to_string(&info).context("failed to serialize worktree info")?;
    println!("{line}");
    Ok(())
}

/// Removes a worktree (and optionally its branch) for the resolved task.
///
/// 删除目标任务的 worktree（默认连分支一起删）。
pub fn remove(args: RemoveArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;
    let task_id = resolve_target_task_id(&root_dir, &username, args.task_id.as_deref())?;
    worktree::remove_worktree(&root_dir, &username, &task_id, args.keep_branch, args.force)?;
    Ok(())
}

/// Prints the attached worktree path for the resolved task. Emits empty
/// stdout (and exits 0) when nothing is attached, so prompt bodies can
/// branch on the result without parsing.
///
/// 输出目标任务的 worktree 路径；未挂载时 stdout 为空、退出码为 0，
/// 让 slash command 用一行 shell 探测。
pub fn path(args: PathArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;
    let task_id = resolve_target_task_id(&root_dir, &username, args.task_id.as_deref())?;
    if let Some(info) = worktree::get_attached(&root_dir, &username, &task_id)? {
        print!("{}", info.path);
    }
    Ok(())
}

/// Lists every attached worktree. Default TSV columns: `task_id, branch,
/// base_branch, path`. `--json` switches to JSONL.
///
/// 列出所有已挂载 worktree。默认 TSV：`task_id, branch, base_branch, path`；
/// `--json` 切到 JSONL。
pub fn list(args: ListArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;
    let attached = worktree::list_attached(&root_dir, &username)?;
    if attached.is_empty() {
        if !args.json {
            println!("No attached worktrees.");
        }
        return Ok(());
    }
    if args.json {
        for info in attached {
            let line = serde_json::to_string(&info).with_context(|| {
                format!("failed to serialize worktree, task_id: {}", info.task_id)
            })?;
            println!("{line}");
        }
    } else {
        for info in attached {
            println!(
                "{}\t{}\t{}\t{}",
                info.task_id, info.branch, info.base_branch, info.path
            );
        }
    }
    Ok(())
}
