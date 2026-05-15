//! Git-worktree domain layer for Hotpot tasks.
//!
//! Owns the "filesystem + git" half of the worktree feature: resolving
//! the configurable base directory, spawning `git worktree add/remove`,
//! and translating those into [`crate::task::attach_worktree`] /
//! [`crate::task::detach_worktree`] calls on `overview.jsonl`.
//!
//! Locking discipline: every git invocation here happens **outside**
//! the cross-process lock that [`crate::task`] takes on `overview.jsonl`.
//! Spawning a subprocess while holding that lock would deadlock a
//! platform hook that re-enters `hotpot` (see `AGENTS.md`'s hard rule).
//! The flow is therefore: git mutation first, ledger update second —
//! and on partial failure we make a best-effort cleanup pass.
//!
//! Worktree git-worktree 领域层。
//!
//! 负责"文件系统 + git"那一半：解析可配置的 base 目录、调起
//! `git worktree add/remove`、最后通过 [`crate::task::attach_worktree`]
//! / [`crate::task::detach_worktree`] 把元信息落到 `overview.jsonl`。
//!
//! 加锁纪律：本模块的所有 git 调用都在 [`crate::task`] 对
//! `overview.jsonl` 的跨进程锁**之外**完成。持锁期间 spawn 子进程会与
//! 平台 hook 回调 `hotpot` 发生嵌套死锁（见 `AGENTS.md`）。流程因此固定
//! 为"先 git 改动、后写台账"，部分失败时尽力补偿清理。

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};

use crate::{paths, task};

/// Default location for per-task worktrees, relative to project root.
///
/// 默认 worktree 物理目录（相对项目根）。
const DEFAULT_BASE_DIR: &str = ".hotpot/worktrees";

/// Branch prefix Hotpot creates for every task worktree.
///
/// Hotpot 为每个任务 worktree 创建的分支前缀。
const BRANCH_PREFIX: &str = "hotpot/";

/// Aggregate view of a worktree-attached task for CLI output.
///
/// 用于 CLI 输出的 worktree 任务聚合视图。
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorktreeInfo {
    pub task_id: String,
    pub title: String,
    pub path: String,
    pub branch: String,
    pub base_branch: String,
}

/// Reads `.hotpot/config.toml`'s `[worktree] base_dir` if present, falling
/// back to [`DEFAULT_BASE_DIR`]. Relative values are joined with
/// `root_dir`; absolute values are returned as-is.
///
/// 读取 `.hotpot/config.toml` 的 `[worktree] base_dir`，缺省时回退到
/// [`DEFAULT_BASE_DIR`]。相对路径会基于 `root_dir` 解析为绝对路径，
/// 绝对路径原样保留。
pub fn resolve_base_dir(root_dir: &str) -> Result<PathBuf> {
    let configured = read_configured_base_dir(root_dir)?;
    let raw = configured.unwrap_or_else(|| DEFAULT_BASE_DIR.to_string());
    let p = PathBuf::from(&raw);
    Ok(if p.is_absolute() {
        p
    } else {
        PathBuf::from(root_dir).join(p)
    })
}

/// Parses `.hotpot/config.toml` and returns the `[worktree] base_dir`
/// string if present. Missing file / missing key both yield `Ok(None)`.
///
/// 解析 `.hotpot/config.toml`，命中 `[worktree] base_dir` 时返回字符串；
/// 文件缺失或键缺失都返回 `Ok(None)`，让上层用默认值兜底。
fn read_configured_base_dir(root_dir: &str) -> Result<Option<String>> {
    let cfg_path = paths::hotpot_dir(root_dir).join("config.toml");
    if !cfg_path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&cfg_path)
        .with_context(|| format!("读取 {} 失败", cfg_path.display()))?;
    let doc: toml_edit::DocumentMut = raw
        .parse()
        .with_context(|| format!("解析 {} 失败（TOML 语法错误）", cfg_path.display()))?;
    let value = doc
        .get("worktree")
        .and_then(|t| t.as_table())
        .and_then(|t| t.get("base_dir"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(value)
}

/// Resolves the absolute, canonicalized git toplevel for `root_dir`.
///
/// Used as the cwd for every `git worktree …` call so the spawned
/// command never inherits an unrelated parent cwd. Falls back to
/// `root_dir` verbatim if `git rev-parse` fails (e.g. on a project not
/// yet inside a git repo) so the caller's error surface stays
/// actionable.
///
/// 解析项目所在 git 仓库的 toplevel，作为 `git worktree …` 的工作目录，
/// 避免继承到无关 cwd。`git rev-parse` 失败时回退到 `root_dir` 原值，
/// 把错误推到 git 命令本身的报错里，方便用户定位。
fn git_toplevel(root_dir: &str) -> PathBuf {
    Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(root_dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|| PathBuf::from(root_dir))
}

/// Returns the current branch name (e.g. `main`), or an error if HEAD is
/// detached. Used at create time to record `worktree_base_branch` so
/// `finish-work`'s "merge back" choice has a deterministic target.
///
/// 返回当前所在分支名（如 `main`）。HEAD 处于 detached 时报错——
/// 没有确定的回流目标，强迫调用方先 checkout 一个分支再重试，
/// 比静默把 SHA 当作 base_branch 更安全。
fn current_branch(cwd: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
        .context("调用 `git rev-parse --abbrev-ref HEAD` 失败")?;
    if !output.status.success() {
        bail!(
            "`git rev-parse --abbrev-ref HEAD` 退出码非零：{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name == "HEAD" {
        bail!(
            "当前 HEAD 处于 detached 状态，无法记录 worktree 的 base 分支。\
             请先 `git checkout <branch>` 后重试。"
        );
    }
    Ok(name)
}

/// Builds the absolute worktree path for a given task id.
///
/// 计算指定 task_id 对应的 worktree 绝对路径。
fn worktree_path_for(root_dir: &str, task_id: &str) -> Result<PathBuf> {
    Ok(resolve_base_dir(root_dir)?.join(task_id))
}

/// Builds the canonical branch name Hotpot uses for a given task id.
///
/// 计算指定 task_id 对应的 Hotpot 规约分支名。
fn branch_for(task_id: &str) -> String {
    format!("{BRANCH_PREFIX}{task_id}")
}

/// Creates a git worktree for `task_id` and records its metadata on the
/// task row.
///
/// Steps (each may fail with a clear error surface):
/// 1. Load the task; refuse if missing, not `InProgress`, or already
///    attached.
/// 2. Resolve the configured base dir, ensure its parent exists.
/// 3. Snapshot the current branch as `base_branch`.
/// 4. `git worktree add -b hotpot/<id> <path>` from the project root.
/// 5. Call [`task::attach_worktree`]. If that fails (race or schema
///    drift), best-effort `git worktree remove --force <path>` so we do
///    not leave a half-attached state behind.
///
/// 创建任务专属 worktree，并把元信息写回任务行：
/// 1. 读任务；缺失 / 非 In Progress / 已挂载都拒绝。
/// 2. 解析配置中的 base 目录，确保父目录存在。
/// 3. 快照当前分支作为 base 分支。
/// 4. 在项目根执行 `git worktree add -b hotpot/<id> <path>`。
/// 5. 调 [`task::attach_worktree`] 落盘。失败时尽力 `git worktree remove
///    --force <path>` 回滚，避免遗留半挂载状态。
pub fn create_worktree(
    root_dir: &str,
    username: &str,
    task_id: &str,
) -> Result<WorktreeInfo> {
    let task_row = task::get_task_by_id(root_dir, username, task_id)?
        .ok_or_else(|| anyhow::anyhow!("未找到 task_id = {task_id} 的任务"))?;

    let target_path = worktree_path_for(root_dir, task_id)?;
    let branch = branch_for(task_id);
    let toplevel = git_toplevel(root_dir);
    let base_branch = current_branch(&toplevel)?;

    // Ensure the parent of <base>/<task_id>/ exists so `git worktree add`
    // does not fail on a missing intermediate component.
    // 提前确保 `<base>` 父目录存在，避免 `git worktree add` 因路径残缺失败。
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建 worktree 父目录失败：{}", parent.display()))?;
    }

    // Run `git worktree add`. We do NOT hold the overview lock here:
    // attach_worktree below acquires it.
    // 这里不持锁；下面的 attach_worktree 会自己取锁。
    let output = Command::new("git")
        .args(["worktree", "add", "-b", &branch])
        .arg(&target_path)
        .current_dir(&toplevel)
        .output()
        .context("调用 `git worktree add` 失败")?;
    if !output.status.success() {
        bail!(
            "`git worktree add -b {branch} {path}` 失败：{stderr}",
            path = target_path.display(),
            stderr = String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let path_str = target_path.display().to_string();
    match task::attach_worktree(root_dir, username, task_id, &path_str, &branch, &base_branch) {
        Ok(_) => Ok(WorktreeInfo {
            task_id: task_row.task_id,
            title: task_row.title,
            path: path_str,
            branch,
            base_branch,
        }),
        Err(e) => {
            // Best-effort rollback: try to remove the worktree we just
            // created, then surface the original ledger error. We swallow
            // rollback failures because the user-actionable signal is the
            // original attach failure.
            // 尽力回滚：刚创建的 worktree 试着删掉，向用户呈现原始的
            // attach 错误。回滚自身的失败不再向上抛——用户该排查的还是
            // attach 的原因。
            let _ = Command::new("git")
                .args(["worktree", "remove", "--force"])
                .arg(&target_path)
                .current_dir(&toplevel)
                .output();
            Err(e)
        }
    }
}

/// Removes a previously created worktree and clears its task-row metadata.
///
/// Order:
/// 1. Load task; the worktree-related fields may be `None` if the user
///    edited `overview.jsonl` by hand — in that case fall back to the
///    computed defaults so cleanup is still possible.
/// 2. `git worktree remove [--force] <path>`. If `force` is true, also
///    survive a missing/dirty worktree.
/// 3. If `keep_branch` is false, `git branch -D <branch>` (silently
///    skipped on failure since the branch may already be gone).
/// 4. [`task::detach_worktree`] clears the row metadata.
///
/// 清理 worktree 并摘除任务行 worktree 元信息。
/// 1. 取任务行；若用户手动改过 overview，worktree_* 可能缺失，则按规约
///    回退到默认值，保证仍然可以清理。
/// 2. `git worktree remove [--force] <path>`；`force=true` 时容忍缺失 /
///    脏 worktree。
/// 3. `keep_branch=false` 时 `git branch -D <branch>`，失败静默跳过
///    （分支可能已被删）。
/// 4. 最后 [`task::detach_worktree`] 清零三列。
pub fn remove_worktree(
    root_dir: &str,
    username: &str,
    task_id: &str,
    keep_branch: bool,
    force: bool,
) -> Result<()> {
    let task_row = task::get_task_by_id(root_dir, username, task_id)?
        .ok_or_else(|| anyhow::anyhow!("未找到 task_id = {task_id} 的任务"))?;

    // Idempotent short-circuit: a task row whose worktree fields are
    // already cleared has nothing to undo. Do NOT shell out to git for
    // this case — issuing `git worktree remove` on a missing path would
    // fail loudly. The defensive `--force` path below still applies if
    // the user passed it (in case the filesystem actually has a stale
    // worktree the ledger doesn't know about).
    // 幂等短路：worktree 三列已清的行没什么可清理的，不去 spawn
    // `git worktree remove`（否则会因路径不存在大声报错）。下面 `--force`
    // 分支仍保留：用于"台账没记但文件系统还残留 worktree"的极端场景。
    if task_row.worktree_path.is_none() && !force {
        // detach_worktree is itself idempotent — short-circuit on None.
        // detach_worktree 自身在 None 时也短路。
        task::detach_worktree(root_dir, username, task_id)?;
        return Ok(());
    }

    // Prefer stored values; fall back to canonical defaults if user-edited.
    // 优先取存盘值；用户改过台账时回退到规约默认值，确保仍可清理。
    let path = task_row
        .worktree_path
        .clone()
        .unwrap_or_else(|| worktree_path_for(root_dir, task_id).map(|p| p.display().to_string()).unwrap_or_default());
    let branch = task_row.worktree_branch.clone().unwrap_or_else(|| branch_for(task_id));
    let toplevel = git_toplevel(root_dir);

    if !path.is_empty() {
        let mut args = vec!["worktree".to_string(), "remove".to_string()];
        if force {
            args.push("--force".to_string());
        }
        args.push(path.clone());
        let output = Command::new("git")
            .args(&args)
            .current_dir(&toplevel)
            .output()
            .context("调用 `git worktree remove` 失败")?;
        if !output.status.success() && !force {
            bail!(
                "`git worktree remove {path}` 失败：{stderr}\n如需强制清理，请加 --force。",
                stderr = String::from_utf8_lossy(&output.stderr).trim()
            );
        }
    }

    if !keep_branch {
        // Silent best-effort: branch deletion failures (e.g. branch already
        // removed) should not block ledger cleanup.
        // 静默尽力：分支早已删除等情况不该阻塞台账清理。
        let _ = Command::new("git")
            .args(["branch", "-D", &branch])
            .current_dir(&toplevel)
            .output();
    }

    task::detach_worktree(root_dir, username, task_id)?;
    Ok(())
}

/// Lists every task that currently has a worktree attached.
///
/// 列出所有当前已挂载 worktree 的任务。
pub fn list_attached(root_dir: &str, username: &str) -> Result<Vec<WorktreeInfo>> {
    let tasks = task::get_task_list(root_dir, username)?;
    Ok(tasks
        .into_iter()
        .filter_map(|t| {
            let path = t.worktree_path.clone()?;
            let branch = t.worktree_branch.clone()?;
            let base_branch = t.worktree_base_branch.clone().unwrap_or_default();
            Some(WorktreeInfo {
                task_id: t.task_id,
                title: t.title,
                path,
                branch,
                base_branch,
            })
        })
        .collect())
}

/// Returns the attached worktree info for a specific task, or `None` if
/// the task is unattached or missing.
///
/// 返回指定任务挂载的 worktree 元信息；任务不存在或未挂载时返回 `None`。
pub fn get_attached(
    root_dir: &str,
    username: &str,
    task_id: &str,
) -> Result<Option<WorktreeInfo>> {
    let Some(t) = task::get_task_by_id(root_dir, username, task_id)? else {
        return Ok(None);
    };
    let (Some(path), Some(branch)) = (t.worktree_path.clone(), t.worktree_branch.clone()) else {
        return Ok(None);
    };
    Ok(Some(WorktreeInfo {
        task_id: t.task_id,
        title: t.title,
        path,
        branch,
        base_branch: t.worktree_base_branch.clone().unwrap_or_default(),
    }))
}
