//! Markdown sync helpers for VuePress Overview status tables.
//!
//! Provides small-scope text transformations that update the `Status` column
//! inside a `::: info Overview` container in a task markdown file. The
//! transformations are pure string operations on a single `&str` — no file I/O,
//! no external parser dependencies.
//!
//! VuePress Overview 状态表 Markdown 同步工具。
//!
//! 提供小范围文本转换函数，用于更新任务 Markdown 中 `::: info Overview`
//! 容器内的 `Status` 单元格。所有转换都是纯字符串操作，不涉及文件 I/O，
//! 不依赖外部 Markdown 解析器。

use std::fs;

use anyhow::{Context, Result};

use crate::context::resolve_vuepress_enabled;
use crate::paths::task_dir_path;
use crate::task::{TaskInfo, TaskStatus, get_task_filename};

/// Result of attempting to sync a task markdown string's Overview status.
///
/// 同步任务 Markdown Overview 状态的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverviewSyncOutcome {
    /// The Overview status cell was updated to the new value.
    /// Overview 状态单元格已更新为新值。
    Updated(String),
    /// No `::: info Overview` container was found; content unchanged.
    /// 未找到 `::: info Overview` 容器，内容未变化。
    SkippedNoOverview,
    /// VuePress is not enabled for this project; status sync skipped.
    /// 项目未启用 VuePress，跳过状态同步。
    SkippedVuePressDisabled,
    /// The task file does not exist on disk; status sync skipped.
    /// 任务文件不存在，跳过状态同步。
    SkippedMissingFile,
    /// The status was already set to the target value; no change needed.
    /// 状态已经是目标值，无需改动。
    AlreadyCurrent,
}

/// Updates the first data-row's Status column inside a `::: info Overview` table.
///
/// The input content is searched for a `::: info Overview` container. If found,
/// the first markdown table inside it is inspected: if it has a header row
/// containing `| Status` and a separator row containing `---`, the first cell
/// of the very next data row is replaced with the new status value.
/// Returns `OverviewSyncOutcome::Updated(new_content)` on success,
/// or a skip variant when no matching table exists.
///
/// 在 `::: info Overview` 容器内查找第一个 Markdown 表格，将其数据行第一列
/// 替换为目标状态。找不到匹配表格时返回跳过变体。
pub fn update_overview_status(content: &str, new_status: &TaskStatus) -> OverviewSyncOutcome {
    let target = new_status.as_str();
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };

    // Collect lines but track the exact byte positions in the original content.
    // 收集行并追踪它们在原始内容中的精确字节位置。
    // We cannot simply use `.lines()` and rebuild because we need to preserve
    // the original newline convention exactly.
    // 我们不能简单地使用 `.lines()` 再重建，因为需要保留原始换行符。
    let mut line_spans: Vec<(usize, usize)> = Vec::new();
    let mut pos = 0usize;
    let nl_len = newline.len();
    for line in content.lines() {
        let start = pos;
        let line_len = line.len();
        let end = start + line_len;
        line_spans.push((start, end));
        // Advance past line content + newline
        pos = end + nl_len;
    }

    // 1. Find the `::: info Overview` marker line index.
    // 1. 找到 `::: info Overview` 标记行的索引。
    let lines: Vec<&str> = content.lines().collect();
    let marker_idx = match lines.iter().position(|l| l.contains("::: info Overview")) {
        Some(i) => i,
        None => return OverviewSyncOutcome::SkippedNoOverview,
    };

    // 2. Find the closing `:::` line after the marker.
    // 2. 找到关闭标记行。
    let close_idx = match lines[marker_idx + 1..]
        .iter()
        .position(|l| l.trim() == ":::")
    {
        Some(rel) => marker_idx + 1 + rel,
        None => return OverviewSyncOutcome::SkippedNoOverview,
    };

    // Body lines are between marker (exclusive) and close (exclusive).
    // 容器体是标记行之后、关闭行之前的行。
    let body_lines = &lines[marker_idx + 1..close_idx];

    // 3. Find header row containing `| Status`.
    // 3. 查找含 `| Status` 的表头行。
    let header_rel = match body_lines.iter().position(|l| l.contains("| Status")) {
        Some(i) => i,
        None => return OverviewSyncOutcome::SkippedNoOverview,
    };

    // 4. The next line must be a separator (contains `---`).
    // 4. 下一行必须是分隔行。
    if header_rel + 1 >= body_lines.len() {
        return OverviewSyncOutcome::SkippedNoOverview;
    }
    if !body_lines[header_rel + 1].contains("---") {
        return OverviewSyncOutcome::SkippedNoOverview;
    }

    // 5. Find data line: first non-empty line after separator starting with `|`.
    // 5. 找数据行。
    let data_rel = match body_lines[header_rel + 2..]
        .iter()
        .position(|l| !l.trim().is_empty() && l.trim().starts_with('|'))
    {
        Some(i) => i,
        None => return OverviewSyncOutcome::SkippedNoOverview,
    };
    let data_body_rel = header_rel + 2 + data_rel;
    let data_orig_line = body_lines[data_body_rel];

    // 6. Parse the first cell of the data line.
    // 6. 解析数据行第一列。
    let pipes: Vec<usize> = data_orig_line
        .char_indices()
        .filter(|&(_, c)| c == '|')
        .map(|(i, _)| i)
        .collect();
    if pipes.len() < 2 {
        return OverviewSyncOutcome::SkippedNoOverview;
    }
    let cell_start = pipes[0] + 1;
    let cell_end = pipes[1];
    let cell_raw = &data_orig_line[cell_start..cell_end];
    let cell_trimmed = cell_raw.trim();

    if cell_trimmed == target {
        return OverviewSyncOutcome::AlreadyCurrent;
    }

    // 7. Build replacement: preserve original leading/trailing whitespace.
    // 7. 构造替换：保留原空白。
    let leading_ws = cell_raw.len() - cell_raw.trim_start().len();
    let trailing_ws = cell_raw.len() - cell_raw.trim_end().len();
    let new_cell = format!(
        "{}{}{}",
        " ".repeat(leading_ws),
        target,
        " ".repeat(trailing_ws)
    );

    // 8. Build the new data line.
    // 8. 构造新数据行。
    let mut new_data_line = String::with_capacity(data_orig_line.len());
    new_data_line.push_str(&data_orig_line[..cell_start]);
    new_data_line.push_str(&new_cell);
    new_data_line.push_str(&data_orig_line[cell_end..]);

    // 9. Replace the original data line in the full content using byte spans.
    // 9. 在完整内容中替换原始数据行。
    let data_line_idx_in_full = marker_idx + 1 + data_body_rel;
    let (data_span_start, data_span_end) = line_spans[data_line_idx_in_full];

    let mut output = String::with_capacity(content.len());
    output.push_str(&content[..data_span_start]);
    output.push_str(&new_data_line);
    output.push_str(&content[data_span_end..]);

    OverviewSyncOutcome::Updated(output)
}

/// Synchronizes the Overview `Status` cell in the task's markdown file after
/// a successful `mark_task_done` call.
///
/// Gate sequence:
/// 1. If VuePress is not enabled → returns `SkippedVuePressDisabled` immediately.
/// 2. If the task file does not exist → returns `SkippedMissingFile`.
/// 3. Reads the file and delegates to [`update_overview_status`].
/// 4. On `Updated(new_content)`, writes the new content through a same-directory
///    replacement helper.
/// 5. Real I/O errors on read or write are propagated as `Err` so the caller
///    can emit a warning without aborting the ledger update.
///
/// This function MUST NOT be called while holding the `overview.jsonl` lock —
/// it performs file I/O that could deadlock or delay the critical section.
///
/// 完成任务后同步任务 Markdown 文件中 Overview 状态列。
///
/// 门控顺序：
/// 1. 未启用 VuePress → 立即返回 `SkippedVuePressDisabled`。
/// 2. 任务文件不存在 → 返回 `SkippedMissingFile`。
/// 3. 读取文件并委托 [`update_overview_status`] 处理。
/// 4. 结果为 `Updated` 时通过同目录替换 helper 写回。
/// 5. 真实 I/O 错误以 `Err` 传播，由调用方输出 warning，不影响 ledger 状态。
///
/// **严禁在持有 `overview.jsonl` 锁期间调用此函数**——它会执行文件 I/O，
/// 可能导致死锁或延长临界区。
pub fn sync_task_file_status(
    root_dir: &str,
    username: &str,
    task: &TaskInfo,
) -> Result<OverviewSyncOutcome> {
    // 1. VuePress gate.
    // 1. VuePress 门控。
    if !resolve_vuepress_enabled(root_dir) {
        return Ok(OverviewSyncOutcome::SkippedVuePressDisabled);
    }

    // 2. Resolve the task file path.
    // 2. 解析任务文件路径。
    let task_filename = get_task_filename(task);
    let task_file = task_dir_path(root_dir, username).join(format!("{task_filename}.md"));

    // 3. Check file existence.
    // 3. 检查文件是否存在。
    if !task_file.exists() {
        return Ok(OverviewSyncOutcome::SkippedMissingFile);
    }

    // 4. Read current content.
    // 4. 读取当前内容。
    let content = fs::read_to_string(&task_file)
        .with_context(|| format!("failed to read task file: {}", task_file.display()))?;

    // 5. Delegate to the string-level transformer.
    // 5. 委托给字符串级转换函数。
    let outcome = update_overview_status(&content, &task.status);

    match outcome {
        OverviewSyncOutcome::Updated(ref new_content) => {
            replace_task_file_content(&task_file, new_content)?;
            Ok(OverviewSyncOutcome::Updated(new_content.clone()))
        }
        other => Ok(other),
    }
}

/// Replaces a task Markdown file with new content using a same-directory temp file.
///
/// Unix-like platforms support replacing an existing destination with
/// `rename(2)`, so the final step is atomic there. Windows `std::fs::rename`
/// fails when the destination already exists, so Windows copies the complete
/// temp file over the existing destination and then removes the temp file. The
/// temp file lives next to the destination so Unix replacement does not cross
/// filesystems and Windows never depends on overwrite-by-rename semantics.
///
/// 使用同目录临时文件替换任务 Markdown 文件内容。
///
/// 类 Unix 平台的 `rename(2)` 可以原子覆盖已有目标；Windows 的
/// `std::fs::rename` 在目标已存在时会失败，因此 Windows 分支把完整临时文件
/// copy 覆盖到已有目标，然后删除临时文件。临时文件与目标文件位于同一目录，
/// Unix 替换不会跨文件系统，Windows 也不依赖 rename 覆盖语义。
fn replace_task_file_content(task_file: &std::path::Path, new_content: &str) -> Result<()> {
    // Same-directory temp path keeps replacement on the same filesystem.
    // 同目录临时路径保证替换不会跨文件系统。
    let tmp_path = task_file.with_extension("md.tmp");
    fs::write(&tmp_path, new_content)
        .with_context(|| format!("failed to write tmp file: {}", tmp_path.display()))?;

    #[cfg(unix)]
    fs::rename(&tmp_path, task_file).with_context(|| {
        format!(
            "failed to replace task file {} with {}",
            task_file.display(),
            tmp_path.display()
        )
    })?;

    #[cfg(windows)]
    {
        fs::copy(&tmp_path, task_file).with_context(|| {
            format!(
                "failed to copy tmp file {} over task file {}",
                tmp_path.display(),
                task_file.display()
            )
        })?;
        fs::remove_file(&tmp_path)
            .with_context(|| format!("failed to remove tmp file: {}", tmp_path.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 普通 VuePress 任务 Markdown：包含 `::: info Overview` 与 `| In Progress | true | 4 | medium |`，
    /// 期望 `In Progress` 被更新为 `Done`，其余内容完全不变。
    #[test]
    fn updates_vuepress_overview_status_to_done() {
        let input = r#"# My Task

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | true | 4 | medium |
:::

## Task
"#;
        let result = update_overview_status(input, &TaskStatus::Done);
        match result {
            OverviewSyncOutcome::Updated(output) => {
                assert!(
                    output.contains("| Done | true | 4 | medium |"),
                    "数据行第一列应从 'In Progress' 更新为 'Done'；实际输出：\n{output}"
                );
                // 验证 H1、容器声明、表头、`## Task` 等其他内容原样保留。
                assert!(output.starts_with("# My Task"), "H1 应原样保留");
                assert!(
                    output.contains("::: info Overview"),
                    "Overview 容器标记应保留"
                );
                assert!(
                    output.contains("| Status | TDD | Tasks | Risk |"),
                    "表头应保留"
                );
                assert!(output.contains("## Task"), "Task 节应保留");
                assert!(
                    !output.contains("| In Progress |"),
                    "旧状态 'In Progress' 应被替换，不应再出现"
                );
            }
            other => panic!("期望 Updated 变体，实际得到：{other:?}"),
        }
    }

    /// 输入不含 Overview 表时返回 SkippedNoOverview，内容不变。
    #[test]
    fn skips_status_sync_when_overview_is_absent() {
        let input = "# Plain Task\n\n## Task\n\nSome content.\n";
        let result = update_overview_status(input, &TaskStatus::Done);
        assert_eq!(
            result,
            OverviewSyncOutcome::SkippedNoOverview,
            "缺少 Overview 时应当跳过"
        );
    }

    /// 文件级同步：完整流程 — 创建任务文件，调用 sync_task_file_status，验证文件被更新。
    #[test]
    fn sync_task_file_status_updates_file_content() {
        use std::fs;
        use tempfile::Builder;

        let root = Builder::new()
            .prefix("hotpot-sync-file-test-")
            .tempdir()
            .unwrap();
        let root_dir = root.path().display().to_string();
        let username = "sync_user";

        // Create .hotpot/workspaces/<user>/tasks/ directory.
        let tasks_dir = root
            .path()
            .join(".hotpot/workspaces")
            .join(username)
            .join("tasks");
        fs::create_dir_all(&tasks_dir).unwrap();

        // Create .hotpot/config.toml with VuePress enabled.
        let config_dir = root.path().join(".hotpot");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.toml"),
            "[vuepress]\nenabled = true\nport = 8080\n",
        )
        .unwrap();

        // Create the task file with an Overview table.
        let task_file = tasks_dir.join("2026-05-25-test-task.md");
        fs::write(
            &task_file,
            "# Test Task\n\n::: info Overview\n| Status | TDD | Tasks | Risk |\n| ------ | --- | ----- | ---- |\n| In Progress | true | 4 | medium |\n:::\n\n## Task\n",
        )
        .unwrap();

        // Build a TaskInfo matching this task.
        let task = TaskInfo {
            time: chrono::NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            task_id: "test-id".to_string(),
            title: "test-task".to_string(),
            commit: None,
            status: TaskStatus::Done,
            active: false,
            worktree_path: None,
            worktree_branch: None,
            worktree_base_branch: None,
        };

        let result = sync_task_file_status(&root_dir, username, &task).unwrap();
        match result {
            OverviewSyncOutcome::Updated(_) => {
                let content = fs::read_to_string(&task_file).unwrap();
                assert!(
                    content.contains("| Done | true | 4 | medium |"),
                    "文件内容应更新为 Done，实际：\n{content}"
                );
            }
            other => panic!("期望 Updated，实际得到：{other:?}"),
        }
    }

    /// Existing task files are replaced through the same helper used by sync.
    ///
    /// This covers the production path where the destination already exists;
    /// on Windows the helper must not rely on `rename` overwriting the target.
    ///
    /// 已存在的任务文件通过同步逻辑使用的同一个 helper 替换。
    ///
    /// 该测试覆盖目标文件已经存在的生产路径；在 Windows 上 helper 不能依赖
    /// `rename` 覆盖目标文件。
    #[test]
    fn replace_task_file_content_replaces_existing_file() {
        use tempfile::Builder;

        let root = Builder::new()
            .prefix("hotpot-replace-existing-")
            .tempdir()
            .unwrap();
        let task_file = root.path().join("task.md");
        fs::write(&task_file, "old content").unwrap();

        replace_task_file_content(&task_file, "new content").unwrap();

        assert_eq!(fs::read_to_string(&task_file).unwrap(), "new content");
        assert!(
            !task_file.with_extension("md.tmp").exists(),
            "temporary file should be moved into place"
        );
    }

    /// 文件缺失时返回 SkippedMissingFile。
    #[test]
    fn sync_task_file_status_skips_when_file_missing() {
        use std::fs;
        use tempfile::Builder;

        let root = Builder::new()
            .prefix("hotpot-sync-missing-")
            .tempdir()
            .unwrap();
        let root_dir = root.path().display().to_string();
        let username = "missing_user";

        // Create .hotpot/config.toml with VuePress enabled.
        let config_dir = root.path().join(".hotpot");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.toml"),
            "[vuepress]\nenabled = true\nport = 8080\n",
        )
        .unwrap();

        // Don't create the task file.
        let task = TaskInfo {
            time: chrono::NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            task_id: "missing-id".to_string(),
            title: "nonexistent-task".to_string(),
            commit: None,
            status: TaskStatus::Done,
            active: false,
            worktree_path: None,
            worktree_branch: None,
            worktree_base_branch: None,
        };

        let result = sync_task_file_status(&root_dir, username, &task).unwrap();
        assert_eq!(
            result,
            OverviewSyncOutcome::SkippedMissingFile,
            "文件不存在时应返回 SkippedMissingFile"
        );
    }

    /// VuePress 未启用时返回 SkippedVuePressDisabled，不读写任务文件。
    /// 使用 crate 级 `ScopedEnvVar` 守卫串行化并 panic-safe 恢复。
    #[test]
    fn sync_task_file_status_skips_when_vuepress_disabled() {
        use tempfile::Builder;

        // Use the crate-wide ScopedEnvVar guard: removes HOTPOT_VUEPRESS_ENABLED
        // so resolve_vuepress_enabled returns the default (false), and restores
        // it in Drop (even on panic). The crate-wide mutex serialises all
        // env-mutating tests across modules.
        // 使用 crate 级 ScopedEnvVar 守卫：删除 HOTPOT_VUEPRESS_ENABLED 使
        // resolve_vuepress_enabled 返回默认值 false，析构时自动恢复。
        let _env = crate::test_support::ScopedEnvVar::new(&[("HOTPOT_VUEPRESS_ENABLED", None)]);

        let root = Builder::new()
            .prefix("hotpot-sync-vp-disabled-")
            .tempdir()
            .unwrap();
        let root_dir = root.path().display().to_string();
        let username = "vp_disabled_user";

        let task = TaskInfo {
            time: chrono::NaiveDate::from_ymd_opt(2026, 5, 25).unwrap(),
            task_id: "vp-disabled-id".to_string(),
            title: "vp-disabled-task".to_string(),
            commit: None,
            status: TaskStatus::Done,
            active: false,
            worktree_path: None,
            worktree_branch: None,
            worktree_base_branch: None,
        };

        let result = sync_task_file_status(&root_dir, username, &task).unwrap();
        assert_eq!(
            result,
            OverviewSyncOutcome::SkippedVuePressDisabled,
            "VuePress 未启用时应返回 SkippedVuePressDisabled"
        );

        // _env drops here, restoring HOTPOT_VUEPRESS_ENABLED — even on panic.
        // _env 在此析构，自动恢复 HOTPOT_VUEPRESS_ENABLED（含 panic 路径）。
    }

    /// 已经是 Done 时返回 AlreadyCurrent，内容不变。
    #[test]
    fn status_sync_is_idempotent_when_already_done() {
        let input = r#"# My Task

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| Done | true | 4 | medium |
:::

## Task
"#;
        let result = update_overview_status(input, &TaskStatus::Done);
        assert_eq!(
            result,
            OverviewSyncOutcome::AlreadyCurrent,
            "已经是 Done 时应返回 AlreadyCurrent"
        );
    }
}
