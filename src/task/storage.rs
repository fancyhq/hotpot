//! `overview.jsonl` I/O primitives and read-only queries.
//!
//! Provides the atomic-rewrite helper [`rewrite_overview_with`] that all
//! persistent state transitions in the `task` module funnel through, plus
//! the read-side queries (`get_task_list`, `get_active_task_count`,
//! `get_task_filename`, `get_active_task_filepath`). Cross-process safety
//! is the caller's responsibility — wrap mutating sequences in
//! [`crate::lock::with_file_lock`] before invoking these helpers.
//!
//! 封装 `overview.jsonl` 的读写原语与只读查询：原子重写
//! [`rewrite_overview_with`] 是 `task` 模块内所有持久化状态变更的唯一入口；
//! 读侧只读查询包括 `get_task_list` / `get_active_task_count` /
//! `get_task_filename` / `get_active_task_filepath`。跨进程并发安全由调用方
//! 在外层通过 [`crate::lock::with_file_lock`] 守护。

use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Ok, Result, bail};

use super::TaskInfo;
use crate::paths::{self, overview_file_path};

/// 确保 overview.jsonl 存在，不存在则创建（含父目录）
/// 返回文件路径，方便链式使用
pub(super) fn ensure_overview_exists(root_dir: &str, username: &str) -> Result<PathBuf> {
    let path = overview_file_path(root_dir, username);
    if !path.exists() {
        // 1. 父目录不存在则递归创建
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建任务目录失败，路径：{}", parent.display()))?;
        }
        // 2. 创建空文件
        fs::write(&path, "")
            .with_context(|| format!("创建任务文件失败，路径：{}", path.display()))?;
    }
    Ok(path)
}

/// 获取 overview 中的所有任务列表
pub fn get_task_list(root_dir: &str, username: &str) -> Result<Vec<TaskInfo>> {
    let task_file_path = ensure_overview_exists(root_dir, username)?;
    let overview_content = fs::read_to_string(&task_file_path).with_context(|| {
        format!(
            "读取任务文件失败，文件可能不存在或获取数据有误，文件路径：{}",
            task_file_path.display()
        )
    })?;
    if overview_content.trim().is_empty() {
        return Ok(Vec::new());
    }

    overview_content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<TaskInfo>(line).with_context(|| {
                format!(
                    "解析 overview.jsonl 第 {} 行失败，文件路径：{}",
                    index + 1,
                    task_file_path.display()
                )
            })
        })
        .collect()
}

/// 获取当前 overview.jsonl 中，active 状态为true的任务总数，当创建新任务，需判断小于1
pub fn get_active_task_count(root_dir: &str, username: &str) -> Result<usize> {
    let task_list = get_task_list(root_dir, username)?;
    Ok(task_list.iter().filter(|info| info.active).count())
}

/// Rewrites `overview.jsonl` after applying `mutator` to every parsed row.
///
/// The new content is first written to a sibling `.tmp` file and then renamed
/// over the original, so a crash mid-write cannot leave the ledger in a
/// half-written state. All persistent state transitions in this module are
/// expected to flow through this helper to keep that invariant in one place.
///
/// 以原子方式重写 `overview.jsonl`：把每一行解析为 `TaskInfo`，调用
/// `mutator` 就地修改后，先写入同目录下的 `.tmp` 文件，再通过 `rename`
/// 覆盖原文件，避免写入过程中崩溃导致台账损坏。本模块所有持久化状态
/// 变更都应走这个助手，以便把"原子写"这件事集中在一个地方。
pub(super) fn rewrite_overview_with<F>(root_dir: &str, username: &str, mut mutator: F) -> Result<()>
where
    F: FnMut(&mut TaskInfo),
{
    // `ensure_overview_exists` guarantees both the file and its parent directory.
    // 这里同时保证父目录与空文件已经存在，后续临时文件与 rename 才不会失败。
    let path = ensure_overview_exists(root_dir, username)?;
    let mut task_list = get_task_list(root_dir, username)?;

    for task in &mut task_list {
        mutator(task);
    }

    let mut content = String::new();
    for task in &task_list {
        let line = serde_json::to_string(task)
            .with_context(|| format!("序列化任务信息失败，任务ID：{}", task.task_id))?;
        content.push_str(&line);
        content.push('\n');
    }

    // tmp_path lives next to the real file so the rename stays on the same
    // filesystem, which is what makes `fs::rename` atomic on POSIX and Windows.
    // 临时文件放在同一目录下，保证 rename 是同卷内操作，从而具备原子性。
    let tmp_path = path.with_extension("jsonl.tmp");
    fs::write(&tmp_path, content)
        .with_context(|| format!("写入临时任务文件失败，路径：{}", tmp_path.display()))?;
    fs::rename(&tmp_path, &path).with_context(|| {
        format!(
            "重命名临时任务文件失败，原路径：{}，目标路径：{}",
            tmp_path.display(),
            path.display()
        )
    })?;

    Ok(())
}

/// Build the on-disk filename stem for a task: `<time>-<sanitized-title>`.
///
/// The `title` field stays human-readable in `overview.jsonl`, but for the
/// filesystem we collapse any whitespace run into a single `-` so that
/// titles like `"add login retry"` still produce a shell-safe path
/// (`2026-05-15-add-login-retry`). This is a defensive layer only — the
/// `/hotpot:new` prompt asks the AI to produce kebab-case up front; this
/// function just guarantees the filename never contains raw spaces.
///
/// 根据 TaskInfo 生成磁盘文件名 stem：`<time>-<净化后的 title>`。
///
/// `overview.jsonl` 中的 title 保留人类可读形式，但写到文件系统时把
/// title 里的连续空白折叠为单个 `-`，保证类似 `"add login retry"` 的
/// 旧 title 也能生成 shell 安全的路径（`2026-05-15-add-login-retry`）。
/// 这是兜底层——`/hotpot:new` 的 prompt 已经要求 AI 给出 kebab-case
/// 标题；这里只保证最终文件名不会出现裸空格。
pub fn get_task_filename(task: &TaskInfo) -> String {
    // `split_whitespace` 同时处理首尾空白、连续空格、tab 等情况，
    // 再用 `-` 串起来即可得到 shell 安全的 title 段。
    // `split_whitespace` handles leading/trailing/runs/tabs in one shot;
    // joining with `-` yields a shell-safe title segment.
    let sanitized_title = task.title.split_whitespace().collect::<Vec<_>>().join("-");
    format!("{}-{}", task.time, sanitized_title)
}

/// Returns the task file path for the unique `active=true` row.
///
/// Enforces the single-active invariant at the read site:
/// - 0 active rows: `Err("Not found active task")` (preserved verbatim
///   for backwards compatibility with callers that match this string).
/// - 1 active row: returns its task file path.
/// - 2 or more active rows: bails with the list of conflicting ids so
///   the caller can disambiguate before `/hotpot:execute` runs on the
///   wrong task. Symmetric with [`super::mark_task_done`] and
///   [`super::mark_task_cancelled`] when they encounter ambiguous active sets.
///
/// 返回唯一 `active=true` 行的任务文件路径。
///
/// 在读路径上强制"单 active 不变式"：
/// - 0 条 active：`Err("Not found active task")`（错误文本保持原样，
///   兼容旧调用方对该字符串的匹配）。
/// - 1 条 active：返回该任务文件路径。
/// - 2 条及以上 active：bail 并列出冲突 id，避免 `/hotpot:execute`
///   静默挑首条 active 执行错任务。与 [`super::mark_task_done`] /
///   [`super::mark_task_cancelled`] 多 active 报错风格对称。
pub fn get_active_task_filepath(root_dir: &str, username: &str) -> Result<PathBuf> {
    let task_list = get_task_list(root_dir, username)?;
    let active: Vec<&TaskInfo> = task_list.iter().filter(|task| task.active).collect();
    match active.len() {
        0 => Err(anyhow::anyhow!("Not found active task")),
        1 => {
            let task = active[0];
            let task_filename = get_task_filename(task);
            let task_dir = paths::task_dir_path(root_dir, username);
            Ok(task_dir.join(format!("{task_filename}.md")))
        }
        n => {
            // 列出冲突 id 让用户排查；建议的恢复命令与 task list --json 配套。
            // List ambiguous ids; the recovery hint pairs with `task list --json`.
            let listed = active
                .iter()
                .map(|task| format!("{} ({})", task.task_id, task.title))
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "发现 {n} 条 active=true 的任务：{listed}。\
                 请通过 `hotpot task list --json` 检查后用 `hotpot task stop --all` 或 \
                 `hotpot task done --task-id <ID>` 收敛到单 active。"
            );
        }
    }
}

/// Looks up the `TaskInfo` row by id, returning `Ok(None)` when missing.
///
/// Used by worktree-related CLI handlers that need to inspect a task's
/// `worktree_*` fields without forcing every caller to re-grep the list.
/// Returns the full row so callers can read any combination of
/// `worktree_path` / `worktree_branch` / `worktree_base_branch`.
///
/// 按 task_id 查任务行，未命中返回 `Ok(None)`。
/// 用于 worktree 相关 CLI 子命令读取 `worktree_*` 字段，避免每个调用方
/// 都自行遍历列表。返回完整行以便组合取 path / branch / base_branch。
pub fn get_task_by_id(
    root_dir: &str,
    username: &str,
    task_id: &str,
) -> Result<Option<TaskInfo>> {
    let task_list = get_task_list(root_dir, username)?;
    Ok(task_list.into_iter().find(|task| task.task_id == task_id))
}

/// Returns the active task's `TaskInfo` row, applying the same single-active
/// invariant as [`get_active_task_filepath`].
///
/// Errors:
/// - `Err("Not found active task")` when there are zero active rows
///   (string preserved for backwards compatibility — slash-command
///   prompts pattern-match on this exact text).
/// - `bail!` with the list of conflicting ids when more than one row is
///   active.
///
/// 返回唯一 active 行的 `TaskInfo`，单 active 不变式与
/// [`get_active_task_filepath`] 共享。
/// 错误形式刻意保持与该函数对称：0 条 active 时返回
/// `Err("Not found active task")`，>1 条 active 时 bail 并列出冲突 id。
pub fn get_active_task(root_dir: &str, username: &str) -> Result<TaskInfo> {
    let task_list = get_task_list(root_dir, username)?;
    let mut active: Vec<TaskInfo> = task_list.into_iter().filter(|task| task.active).collect();
    match active.len() {
        0 => Err(anyhow::anyhow!("Not found active task")),
        1 => Ok(active.pop().expect("active.len() == 1")),
        n => {
            let listed = active
                .iter()
                .map(|task| format!("{} ({})", task.task_id, task.title))
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "发现 {n} 条 active=true 的任务：{listed}。\
                 请通过 `hotpot task list --json` 检查后用 `hotpot task stop --all` 或 \
                 `hotpot task done --task-id <ID>` 收敛到单 active。"
            );
        }
    }
}

/// Appends a single serialized `TaskInfo` line to `overview.jsonl`.
///
/// Extracted from the previous monolithic `create_task` so cleanup and
/// append remain separable. The caller is expected to have already run
/// `ensure_overview_exists` (or any function — like
/// [`rewrite_overview_with`] — that does).
///
/// 将单条 `TaskInfo` 序列化后追加到 `overview.jsonl`。从原 `create_task`
/// 抽出，把"清理"与"追加"两步拆开。调用方需先确保 overview 文件已存在
/// （直接调 [`rewrite_overview_with`] 或 `ensure_overview_exists` 都行）。
pub(super) fn append_task_line(path: &PathBuf, new_task: &TaskInfo) -> Result<()> {
    let mut line = serde_json::to_string(new_task)
        .with_context(|| format!("序列化任务信息失败，任务ID：{}", new_task.task_id))?;
    line.push('\n');
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("打开任务文件失败，路径：{}", path.display()))?
        .write_all(line.as_bytes())
        .with_context(|| format!("追加任务文件失败，路径：{}", path.display()))?;
    Ok(())
}
