use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Ok, Result};
use indoc::indoc;
use serde::{Deserialize, Serialize};

use crate::lock::with_file_lock;
use crate::paths::{issue_candidates_file_path, issues_file_path};

#[derive(Debug, Serialize, Deserialize)]
/// Historical issue category used to separate correctness problems from quality improvements.
pub enum IssueKind {
    #[serde(rename = "bug")]
    Bug,
    #[serde(rename = "optimization")]
    Optimization,
}

impl IssueKind {
    fn as_str(&self) -> &'static str {
        match self {
            IssueKind::Bug => "bug",
            IssueKind::Optimization => "optimization",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Source metadata for a review memory item.
pub struct IssueSource {
    /// Files changed when the original issue was fixed.
    pub changed_files: Vec<String>,
    /// Short summary of the original fix; the full diff is intentionally not stored.
    pub summary: String,
}

#[derive(Debug, Serialize, Deserialize)]
/// A reusable review memory item loaded from `.hotpot/issues.jsonl`.
pub struct Issue {
    /// Date when the issue memory was created.
    pub date: chrono::NaiveDate,
    /// Short title describing the reusable problem pattern.
    pub title: String,
    /// Category of the historical issue.
    pub kind: IssueKind,
    /// Reusable keywords used to match future changes.
    pub tags: Vec<String>,
    /// Stable file paths or prefixes where this memory is relevant.
    pub paths: Vec<String>,
    /// Natural-language condition for when reviewers should consider this memory.
    pub scene: String,
    /// Description of the historical problem.
    pub description: String,
    /// Concrete check reviewers should perform when the scene matches.
    pub review_check: String,
    /// Preferred fix or prevention pattern.
    pub solution: String,
    /// Metadata about the change that produced this memory.
    pub source: IssueSource,
}

/// Lightweight facts about the current change used to match relevant review memories.
pub struct ChangeContext {
    /// Files changed in the current review target.
    pub changed_files: Vec<String>,
    /// Extracted keywords from filenames, symbols, summaries, or small relevant snippets.
    pub keywords: Vec<String>,
}

/// Temporary issue memory candidate written during a repair session.
///
/// Candidates are not long-term review memory yet. A later finish step should
/// deduplicate, merge, discard, or promote them into [`Issue`] records.
#[derive(Debug, Serialize, Deserialize)]
pub struct IssueCandidate {
    /// Creation timestamp supplied by the caller or AI.
    pub created_at: String,
    /// Why this candidate may be worth promoting later.
    pub reason: String,
    /// Files changed when the candidate was observed.
    pub changed_files: Vec<String>,
    /// Lightweight keywords extracted from the change.
    pub keywords: Vec<String>,
    /// Problem observed during the repair.
    pub problem: String,
    /// Fix that was applied or proposed.
    pub fix: String,
    /// Validation commands or checks that passed.
    pub validation: Vec<String>,
    /// Hint describing the likely long-term review memory to promote.
    pub promote_hint: String,
}

fn ensure_issues_exists(root_dir: &str) -> Result<PathBuf> {
    let path = issues_file_path(root_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建issue目录路径失败，路径：{}", path.display()))?;
    }

    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("创建issues文件失败，路径：{}", path.display()))?;

    Ok(path)
}

/// Ensures the project-shared candidates file exists and migrates legacy rows.
///
/// This is the single ensure/migration entry point for hook bootstrap,
/// init/update workspace bootstrap, and candidate CLI operations. The
/// `username` parameter is retained for compatibility with existing callers;
/// the resolved path is project-global.
///
/// 确保项目级共享 candidates 文件存在，并迁移旧版 per-user 行。
///
/// 这是 hook bootstrap、init/update workspace bootstrap 与 candidate CLI 共用的
/// ensure/migration 入口。`username` 参数仅为兼容既有调用方保留；解析出的路径是
/// 项目级全局文件。
pub fn ensure_issue_candidates_exists(root_dir: &str, username: &str) -> Result<PathBuf> {
    let path = issue_candidates_file_path(root_dir, username);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建issue候选目录路径失败，路径：{}", path.display()))?;
    }

    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("创建issue候选文件失败，路径：{}", path.display()))?;

    with_file_lock(&path, || migrate_legacy_issue_candidates(root_dir, &path))?;

    Ok(path)
}

/// Migrates legacy per-user candidate JSONL rows into the global candidates file.
///
/// The caller must hold the advisory lock for `global_path`. Each legacy file
/// under `.hotpot/workspaces/*/issue-candidates.jsonl` contributes its non-empty
/// rows once. Before appending, the legacy file is atomically moved to a staging
/// path that is not scanned by future migrations. If global append fails, the
/// staging file is restored to the original path; after append succeeds, an
/// empty legacy file is recreated and the staging file is removed.
///
/// 将旧版 per-user candidate JSONL 行迁移到全局候选文件。
///
/// 调用方必须已持有 `global_path` 的 advisory lock。每个旧文件位于
/// `.hotpot/workspaces/*/issue-candidates.jsonl`，只迁移非空行。追加前先将旧文件
/// 原子移动到不会被后续扫描的 staging 路径；若全局追加失败则恢复原路径，追加成功后
/// 重建空的旧文件并删除 staging 文件，避免清理失败导致后续重复迁移。
fn migrate_legacy_issue_candidates(root_dir: &str, global_path: &PathBuf) -> Result<()> {
    let workspaces_dir = crate::paths::hotpot_dir(root_dir).join("workspaces");
    if !workspaces_dir.is_dir() {
        return Ok(());
    }

    restore_orphaned_legacy_staging_files(&workspaces_dir, global_path)?;

    let mut candidates: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for entry in fs::read_dir(&workspaces_dir)
        .with_context(|| format!("读取workspaces目录失败，路径：{}", workspaces_dir.display()))?
    {
        let entry = entry.with_context(|| {
            format!(
                "读取workspace目录项失败，路径：{}",
                workspaces_dir.display()
            )
        })?;
        let legacy_path = entry.path().join("issue-candidates.jsonl");
        if legacy_path == *global_path || !legacy_path.is_file() {
            continue;
        }

        let content = fs::read_to_string(&legacy_path)
            .with_context(|| format!("读取旧issue候选文件失败，路径：{}", legacy_path.display()))?;
        let lines: Vec<String> = content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_string())
            })
            .collect();
        if lines.is_empty() {
            continue;
        }

        candidates.push((legacy_path, lines));
    }

    if candidates.is_empty() {
        return Ok(());
    }

    let mut migrations: Vec<(PathBuf, PathBuf, Vec<String>)> = Vec::new();
    for (legacy_path, lines) in candidates {
        let staging_path = legacy_unique_staging_path(&legacy_path);
        if let Err(err) = fs::rename(&legacy_path, &staging_path) {
            for (restored_legacy_path, restored_staging_path, _) in &migrations {
                if restored_staging_path.is_file() && !restored_legacy_path.exists() {
                    fs::rename(restored_staging_path, restored_legacy_path).with_context(|| {
                        format!(
                            "恢复旧issue候选文件失败，路径：{}",
                            restored_legacy_path.display()
                        )
                    })?;
                }
            }
            return Err(err).with_context(|| {
                format!("暂存旧issue候选文件失败，路径：{}", legacy_path.display())
            });
        }

        migrations.push((legacy_path, staging_path, lines));
    }

    let original_len = fs::metadata(global_path)
        .with_context(|| {
            format!(
                "读取issue候选文件元数据失败，路径：{}",
                global_path.display()
            )
        })?
        .len();
    let append_result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(global_path)
            .with_context(|| format!("打开issue候选文件失败，路径：{}", global_path.display()))?;
        write_missing_separator(global_path, &mut file)?;
        for (_, _, lines) in &migrations {
            for line in lines {
                file.write_all(line.as_bytes()).with_context(|| {
                    format!("写入issue候选文件失败，路径：{}", global_path.display())
                })?;
                file.write_all(b"\n").with_context(|| {
                    format!("写入issue候选文件失败，路径：{}", global_path.display())
                })?;
            }
        }
        file.flush()
            .with_context(|| format!("刷新issue候选文件失败，路径：{}", global_path.display()))
    })();

    if let Err(err) = append_result {
        rollback_global_append(global_path, original_len)?;
        restore_staged_migrations(&migrations)?;
        return Err(err);
    }

    let mut committed_migrations: Vec<(PathBuf, PathBuf, PathBuf)> = Vec::new();
    for (legacy_path, staging_path, _) in migrations {
        let done_path = legacy_unique_done_path(&legacy_path);
        if let Err(err) = fs::rename(&staging_path, &done_path) {
            rollback_global_append(global_path, original_len)?;
            if staging_path.is_file() && !legacy_path.exists() {
                fs::rename(&staging_path, &legacy_path).with_context(|| {
                    format!("恢复旧issue候选文件失败，路径：{}", legacy_path.display())
                })?;
            }
            for (committed_legacy_path, _, committed_done_path) in &committed_migrations {
                if committed_done_path.is_file() && !committed_legacy_path.exists() {
                    fs::rename(committed_done_path, committed_legacy_path).with_context(|| {
                        format!(
                            "恢复旧issue候选文件失败，路径：{}",
                            committed_legacy_path.display()
                        )
                    })?;
                }
            }
            return Err(err).with_context(|| {
                format!(
                    "标记旧issue候选文件已迁移失败，路径：{}",
                    legacy_path.display()
                )
            });
        }

        committed_migrations.push((legacy_path, staging_path, done_path));
    }

    for (legacy_path, _, done_path) in committed_migrations {
        fs::write(&legacy_path, "").with_context(|| {
            format!("重建空旧issue候选文件失败，路径：{}", legacy_path.display())
        })?;
        fs::remove_file(&done_path).with_context(|| {
            format!(
                "清理旧issue候选已迁移文件失败，路径：{}",
                done_path.display()
            )
        })?;
    }

    Ok(())
}

/// Returns a unique hidden staging path for a legacy candidates file.
///
/// The name intentionally does not equal `issue-candidates.jsonl`, so normal
/// legacy scans will skip it. The unique suffix prevents stale staging files
/// from being overwritten on Unix or blocking retries on Windows.
///
/// 返回旧候选文件迁移期间使用的唯一隐藏 staging 路径。
///
/// 文件名故意不等于 `issue-candidates.jsonl`，这样后续旧路径扫描不会误迁移它。唯一后缀
/// 避免 Unix 覆盖 stale staging 文件，也避免 Windows 因目标已存在而反复失败。
fn legacy_unique_staging_path(legacy_path: &PathBuf) -> PathBuf {
    legacy_path.with_file_name(format!(
        ".issue-candidates.jsonl.hotpot-staging.{}",
        unique_migration_suffix()
    ))
}

/// Returns a unique hidden committed-staging path for a migrated legacy file.
///
/// A file at this path has already been appended to the global candidates file,
/// so future migration runs must not restore it to the scanned legacy path. The
/// unique suffix prevents stale committed markers from blocking later migrations
/// on Windows.
///
/// 返回旧候选文件已完成追加后的唯一隐藏暂存路径。
///
/// 该路径下的文件已经追加到全局 candidates 文件，后续迁移不能把它恢复到会被扫描的旧路径。
/// 唯一后缀避免 stale committed marker 在 Windows 上阻塞后续迁移。
fn legacy_unique_done_path(legacy_path: &PathBuf) -> PathBuf {
    legacy_path.with_file_name(format!(
        ".issue-candidates.jsonl.hotpot-migrated.{}",
        unique_migration_suffix()
    ))
}

/// Returns a suffix for hidden migration files.
///
/// 返回隐藏迁移文件的唯一后缀。
fn unique_migration_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

/// Restores pre-append orphaned staging files from a previous failed migration.
///
/// Only `hotpot-staging` files are restored. `hotpot-migrated` files represent
/// rows that were already appended globally and are intentionally ignored to
/// avoid duplicate migration.
///
/// 恢复上一次失败迁移留下的 pre-append staging 文件。
///
/// 这里只恢复 `hotpot-staging` 文件；`hotpot-migrated` 表示已追加到全局文件，必须忽略以避免重复迁移。
fn restore_orphaned_legacy_staging_files(
    workspaces_dir: &PathBuf,
    global_path: &PathBuf,
) -> Result<()> {
    let global_lines = read_non_empty_lines(global_path).unwrap_or_default();
    let mut committed_counts = line_counts(&global_lines);
    for entry in fs::read_dir(workspaces_dir)
        .with_context(|| format!("读取workspaces目录失败，路径：{}", workspaces_dir.display()))?
    {
        let entry = entry.with_context(|| {
            format!(
                "读取workspace目录项失败，路径：{}",
                workspaces_dir.display()
            )
        })?;
        let workspace_path = entry.path();
        if !workspace_path.is_dir() {
            continue;
        }

        let legacy_path = workspace_path.join("issue-candidates.jsonl");
        let legacy_lines = if legacy_path.is_file() {
            read_non_empty_lines(&legacy_path)?
        } else {
            Vec::new()
        };
        let mut legacy_counts = line_counts(&legacy_lines);
        for workspace_entry in fs::read_dir(&workspace_path)
            .with_context(|| format!("读取workspace目录失败，路径：{}", workspace_path.display()))?
        {
            let workspace_entry = workspace_entry.with_context(|| {
                format!(
                    "读取workspace目录项失败，路径：{}",
                    workspace_path.display()
                )
            })?;
            let staging_path = workspace_entry.path();
            if !is_legacy_staging_file(&staging_path) {
                continue;
            }

            let staging_lines = read_non_empty_lines(&staging_path)?;
            let missing_lines = missing_line_occurrences_from_counts(
                &mut committed_counts,
                &mut legacy_counts,
                &staging_lines,
            );
            if missing_lines.is_empty() {
                fs::remove_file(&staging_path).with_context(|| {
                    format!(
                        "清理旧issue候选暂存文件失败，路径：{}",
                        staging_path.display()
                    )
                })?;
            } else {
                merge_staging_into_legacy(&staging_path, &legacy_path, &missing_lines)?;
            }
        }
    }

    Ok(())
}

/// Returns whether a path is a pre-append legacy staging file.
///
/// 判断路径是否是 pre-append legacy staging 文件。
fn is_legacy_staging_file(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(".issue-candidates.jsonl.hotpot-staging."))
}

/// Merges an orphan staging file back into a legacy file.
///
/// Missing rows are appended to the legacy file. If the append fails after a
/// partial write, the file is truncated back to its original length and the
/// staging file remains available for the next retry.
///
/// 将 orphan staging 文件合并回旧文件。
///
/// 只追加缺失行；若追加中途失败，则把旧文件截断回原长度，并保留 staging 文件供下次重试。
fn merge_staging_into_legacy(
    staging_path: &PathBuf,
    legacy_path: &PathBuf,
    missing_lines: &[String],
) -> Result<()> {
    let mut legacy_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(legacy_path)
        .with_context(|| format!("打开旧issue候选文件失败，路径：{}", legacy_path.display()))?;
    let original_len = legacy_file
        .metadata()
        .with_context(|| {
            format!(
                "读取旧issue候选文件元数据失败，路径：{}",
                legacy_path.display()
            )
        })?
        .len();
    let append_result = (|| -> Result<()> {
        write_missing_separator(legacy_path, &mut legacy_file)?;
        for line in missing_lines {
            legacy_file.write_all(line.as_bytes()).with_context(|| {
                format!("恢复旧issue候选文件失败，路径：{}", legacy_path.display())
            })?;
            legacy_file.write_all(b"\n").with_context(|| {
                format!("恢复旧issue候选文件失败，路径：{}", legacy_path.display())
            })?;
        }
        legacy_file
            .flush()
            .with_context(|| format!("恢复旧issue候选文件失败，路径：{}", legacy_path.display()))
    })();

    if let Err(err) = append_result {
        fs::OpenOptions::new()
            .write(true)
            .open(legacy_path)
            .with_context(|| format!("打开旧issue候选文件失败，路径：{}", legacy_path.display()))?
            .set_len(original_len)
            .with_context(|| format!("回滚旧issue候选文件失败，路径：{}", legacy_path.display()))?;
        return Err(err);
    }

    fs::remove_file(staging_path).with_context(|| {
        format!(
            "清理旧issue候选暂存文件失败，路径：{}",
            staging_path.display()
        )
    })
}

/// Returns required row occurrences not covered by committed or legacy counts.
///
/// Covered occurrences are consumed so multiple orphan staging files cannot use
/// the same already-committed row more than once.
///
/// 返回 committed 或 legacy 计数无法覆盖的 required 行出现次数。
///
/// 已覆盖的出现次数会被消耗，避免多个 orphan staging 文件复用同一条已提交行。
fn missing_line_occurrences_from_counts(
    committed_counts: &mut HashMap<String, usize>,
    legacy_counts: &mut HashMap<String, usize>,
    required: &[String],
) -> Vec<String> {
    let mut missing = Vec::new();
    for line in required {
        match committed_counts.get_mut(line) {
            Some(count) if *count > 0 => *count -= 1,
            _ => match legacy_counts.get_mut(line) {
                Some(count) if *count > 0 => *count -= 1,
                _ => missing.push(line.clone()),
            },
        }
    }

    missing
}

/// Counts exact JSONL row occurrences.
///
/// 统计精确 JSONL 行出现次数。
fn line_counts(lines: &[String]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for line in lines {
        *counts.entry(line.clone()).or_insert(0) += 1;
    }

    counts
}

/// Reads non-empty trimmed JSONL rows from a file.
///
/// 从文件中读取非空 JSONL 行。
fn read_non_empty_lines(path: &PathBuf) -> Result<Vec<String>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("读取issue候选文件失败，路径：{}", path.display()))?;
    Ok(content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .collect())
}

/// Writes a separator newline before appending when the target lacks one.
///
/// This keeps JSONL valid when an existing file ends with a final row that has
/// no trailing newline. Callers record the original file length first, so this
/// separator is included in failure rollback.
///
/// 当目标文件缺少尾随换行时，追加前先写入分隔换行。
///
/// 调用方会先记录原文件长度，因此该分隔换行也会被失败回滚覆盖。
fn write_missing_separator(path: &PathBuf, file: &mut fs::File) -> Result<()> {
    if file_has_trailing_newline(path)? {
        return Ok(());
    }

    file.write_all(b"\n")
        .with_context(|| format!("写入issue候选文件失败，路径：{}", path.display()))
}

/// Returns whether a file is empty or already ends with `\n`.
///
/// 判断文件是否为空或已经以 `\n` 结尾。
fn file_has_trailing_newline(path: &PathBuf) -> Result<bool> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .open(path)
        .with_context(|| format!("打开issue候选文件失败，路径：{}", path.display()))?;
    let len = file
        .metadata()
        .with_context(|| format!("读取issue候选文件元数据失败，路径：{}", path.display()))?
        .len();
    if len == 0 {
        return Ok(true);
    }

    file.seek(SeekFrom::End(-1))
        .with_context(|| format!("读取issue候选文件失败，路径：{}", path.display()))?;
    let mut byte = [0_u8; 1];
    file.read_exact(&mut byte)
        .with_context(|| format!("读取issue候选文件失败，路径：{}", path.display()))?;
    Ok(byte[0] == b'\n')
}

/// Truncates the global candidates file back to its pre-append length.
///
/// This prevents partial write or flush failures from leaving duplicate rows or
/// corrupt JSONL fragments before legacy files are restored for retry.
///
/// 将全局 candidates 文件截断回追加前的长度。
///
/// 这样 write/flush 的部分失败不会在恢复 legacy 文件前留下重复行或损坏的 JSONL 片段。
fn rollback_global_append(global_path: &PathBuf, original_len: u64) -> Result<()> {
    if !global_path.is_file() {
        return Ok(());
    }

    fs::OpenOptions::new()
        .write(true)
        .open(global_path)
        .with_context(|| format!("打开issue候选文件失败，路径：{}", global_path.display()))?
        .set_len(original_len)
        .with_context(|| format!("回滚issue候选文件失败，路径：{}", global_path.display()))
}

/// Restores all pre-append staged legacy files to their scanned paths.
///
/// Used only before an append has committed; restoring after commit would make
/// the same legacy rows visible for migration again.
///
/// 将所有 pre-append staging 文件恢复到会被扫描的旧路径。
///
/// 仅在追加尚未提交时使用；提交后恢复会让同一批旧行再次参与迁移。
fn restore_staged_migrations(migrations: &[(PathBuf, PathBuf, Vec<String>)]) -> Result<()> {
    for (legacy_path, staging_path, _) in migrations {
        if staging_path.is_file() && !legacy_path.exists() {
            fs::rename(staging_path, legacy_path).with_context(|| {
                format!("恢复旧issue候选文件失败，路径：{}", legacy_path.display())
            })?;
        }
    }

    Ok(())
}

/// Reads `.hotpot/issues.jsonl` and deserializes each non-empty line into an [`Issue`].
pub fn get_issues_list(root_dir: &str) -> Result<Vec<Issue>> {
    let issues_file_path = ensure_issues_exists(root_dir)?;
    let issues_content = fs::read_to_string(&issues_file_path)
        .with_context(|| format!("读取issue文件失败，路径：{}", issues_file_path.display()))?;
    if issues_content.trim().is_empty() {
        return Ok(Vec::new());
    }
    issues_content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<Issue>(line).with_context(|| {
                format!(
                    "解析issues.jsonl第 {} 行失败，文件路径：{}",
                    index + 1,
                    issues_file_path.display()
                )
            })
        })
        .collect()
}

/// Reads project-shared temporary issue candidates from JSONL.
///
/// The `username` parameter is retained for API compatibility only; candidate
/// storage is project-global and does not vary by user.
///
/// 从项目级共享 JSONL 读取临时 issue 候选。
///
/// `username` 参数仅为兼容既有 API 保留；候选存储是项目级全局文件，不再随用户变化。
pub fn get_issue_candidates_list(root_dir: &str, username: &str) -> Result<Vec<IssueCandidate>> {
    let candidates_file_path = ensure_issue_candidates_exists(root_dir, username)?;
    let candidates_content = fs::read_to_string(&candidates_file_path).with_context(|| {
        format!(
            "读取issue候选文件失败，路径：{}",
            candidates_file_path.display()
        )
    })?;
    if candidates_content.trim().is_empty() {
        return Ok(Vec::new());
    }

    candidates_content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<IssueCandidate>(line).with_context(|| {
                format!(
                    "解析issue-candidates.jsonl第 {} 行失败，文件路径：{}",
                    index + 1,
                    candidates_file_path.display()
                )
            })
        })
        .collect()
}

/// Appends one temporary issue candidate to the project-shared JSONL file.
///
/// Holds the global candidates advisory lock so concurrent add, clear, and
/// legacy migration operations cannot clobber each other. The `username`
/// parameter is retained for API compatibility only.
///
/// 向项目级共享 JSONL 追加一条临时 issue 候选。
///
/// 持有全局 candidates 文件的 advisory 锁，使并发 add、clear 与旧文件迁移不会互相覆盖。
/// `username` 参数仅为兼容既有 API 保留。
pub fn append_issue_candidate(
    root_dir: &str,
    username: &str,
    candidate: &IssueCandidate,
) -> Result<()> {
    let candidates_file_path = ensure_issue_candidates_exists(root_dir, username)?;
    with_file_lock(&candidates_file_path, || {
        let mut line = serde_json::to_string(candidate).context("序列化issue候选失败")?;
        line.push('\n');

        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&candidates_file_path)
            .with_context(|| {
                format!(
                    "打开issue候选文件失败，路径：{}",
                    candidates_file_path.display()
                )
            })?;
        write_missing_separator(&candidates_file_path, &mut file)?;
        file.write_all(line.as_bytes()).with_context(|| {
            format!(
                "写入issue候选文件失败，路径：{}",
                candidates_file_path.display()
            )
        })
    })
}

/// Appends one promoted [`Issue`] to the project-shared `.hotpot/issues.jsonl`.
///
/// This is the only write entry point into long-term review memory. Callers
/// (currently `hotpot issues promote`) must obtain explicit user confirmation
/// before invoking it. Deduplication is intentionally not performed here:
/// merging near-duplicate candidates is the job of
/// `.hotpot/prompts/summarize-issue-candidates.md`, and re-implementing it in Rust
/// would create two sources of truth that drift apart.
///
/// 向项目级 `.hotpot/issues.jsonl` 追加一条已晋升的 [`Issue`]。这是写入
/// 长期 review 记忆的唯一入口，目前由 `hotpot issues promote` 调用，
/// 调用前必须获得用户确认。**这里不做去重**：近似候选的合并由
/// `.hotpot/prompts/summarize-issue-candidates.md` 的 LLM 流程负责，在 Rust 里
/// 再实现一遍只会让两边逻辑漂移。
pub fn append_issue(root_dir: &str, issue: &Issue) -> Result<()> {
    let issues_file_path = ensure_issues_exists(root_dir)?;
    // `.hotpot/issues.jsonl` 是项目级共享文件，最易遭并发写竞争——加锁
    // 是 FLOW.md 已知缺口 #8 的根治手段（参见 src/lock.rs 头注释）。
    // `.hotpot/issues.jsonl` is the project-shared file most likely to see
    // concurrent writes; the lock closes FLOW.md known-gap #8.
    with_file_lock(&issues_file_path, || {
        let mut line = serde_json::to_string(issue).context("序列化issue失败")?;
        line.push('\n');

        fs::OpenOptions::new()
            .append(true)
            .open(&issues_file_path)
            .with_context(|| format!("打开issues文件失败，路径：{}", issues_file_path.display()))?
            .write_all(line.as_bytes())
            .with_context(|| format!("写入issues文件失败，路径：{}", issues_file_path.display()))
    })
}

/// Clears temporary issue candidates after they are promoted or deliberately discarded.
///
/// 与 [`append_issue_candidate`] 共用同一把 advisory 锁——清空操作期间
/// 必须独占文件，避免并发 add 把刚写入的候选连带清掉。
/// Shares the candidates advisory lock with [`append_issue_candidate`] to
/// stop a concurrent add from being clobbered by the truncate.
pub fn clear_issue_candidates(root_dir: &str, username: &str) -> Result<()> {
    let candidates_file_path = ensure_issue_candidates_exists(root_dir, username)?;
    with_file_lock(&candidates_file_path, || {
        fs::write(&candidates_file_path, "").with_context(|| {
            format!(
                "清空issue候选文件失败，路径：{}",
                candidates_file_path.display()
            )
        })
    })
}

fn render_markdown_header(title: &str) -> String {
    format!(
        indoc! {r#"
            # {title}

            这里记录的是历史 review 或实现中出现过的问题。执行代码 review 时，必须先根据每条记录的 `Scene` 判断当前变更是否相关。

            如果当前变更匹配某条 `Scene`，必须执行对应的 `Review Check`。如果发现同类问题，参考 `Solution` 给出修复建议，避免重复引入历史问题。

            示例：

            ```markdown
            ## 创建卡片显示优化

            - kind: optimization
            - date: 2026-05-11
            - tags: ui, card, border-radius
            - paths: src/components

            ### Scene

            当修改卡片、列表项、详情面板、弹窗内容块等 UI 容器时。

            ### Problem

            添加的卡片没有使用圆角形式，导致视觉风格不一致。

            ### Review Check

            检查新增或修改的卡片类 UI 是否遵循项目现有圆角规范。

            ### Solution

            给卡片添加 `10px` 的圆角边框。
            ```

        "#},
        title = title,
    )
}

fn push_issue_markdown(markdown: &mut String, issue: &Issue) {
    markdown.push_str(&format!(
        indoc! {"
            ## {title}
            - kind: {kind}
            - date: {date}
            - tags: {tags}
            - paths: {paths}

            ### Scene
            {scene}

            ### Problem
            {description}

            ### Review Check
            {review_check}

            ### Solution
            {solution}

        "},
        title = &issue.title,
        kind = issue.kind.as_str(),
        date = issue.date,
        tags = issue.tags.join(", "),
        paths = issue.paths.join(", "),
        scene = &issue.scene,
        description = &issue.description,
        review_check = &issue.review_check,
        solution = &issue.solution,
    ));
}

/// Renders already-selected issue references into markdown for AI review context.
pub fn render_issue_refs_to_markdown(title: &str, issues: &[&Issue]) -> String {
    let mut markdown = format!(
        indoc! {r#"
            # {title}

            以下历史问题与当前变更可能相关。请在 review 时逐条检查。

        "#},
        title = title,
    );

    if issues.is_empty() {
        markdown.push_str("No related historical issues matched this change.\n");
        return markdown;
    }

    for issue in issues {
        push_issue_markdown(&mut markdown, issue);
    }

    markdown
}

/// Renders all stored issue memories into markdown.
pub fn render_issues_to_markdown(root_dir: &str) -> Result<String> {
    let issues_list = get_issues_list(root_dir)?;
    let mut markdown = render_markdown_header("Hotpot Review Memory");

    for issue in &issues_list {
        push_issue_markdown(&mut markdown, issue);
    }

    Ok(markdown)
}

/// Scores how relevant an issue memory is to the current change context.
///
/// Path matches are weighted higher than tag matches because changed files are
/// usually a stronger signal than extracted keywords.
pub fn score_issue(issue: &Issue, context: &ChangeContext) -> usize {
    let keywords: Vec<String> = context
        .keywords
        .iter()
        .map(|keyword| keyword.to_lowercase())
        .collect();

    let path_score = issue
        .paths
        .iter()
        .filter(|issue_path| {
            context
                .changed_files
                .iter()
                .any(|changed_file| changed_file.starts_with(*issue_path))
        })
        .count()
        * 3;

    let tag_score = issue
        .tags
        .iter()
        .filter(|tag| {
            let tag = tag.to_lowercase();
            keywords.iter().any(|keyword| keyword == &tag)
        })
        .count()
        * 2;

    path_score + tag_score
}

/// Returns the top matching issue memories for a change context.
///
/// Issues with a zero score are excluded. The remaining issues are sorted by
/// descending relevance score and truncated to `limit`.
pub fn filter_relevant_issues<'a>(
    issues: &'a [Issue],
    context: &ChangeContext,
    limit: usize,
) -> Vec<&'a Issue> {
    let mut scored_issues: Vec<_> = issues
        .iter()
        .map(|issue| (score_issue(issue, context), issue))
        .filter(|(score, _)| *score > 0)
        .collect();

    scored_issues.sort_by(|(left_score, _), (right_score, _)| right_score.cmp(left_score));

    scored_issues
        .into_iter()
        .take(limit)
        .map(|(_, issue)| issue)
        .collect()
}

/// Loads issue memories, filters the most relevant entries, and renders them as markdown.
pub fn render_relevant_issues_to_markdown(
    root_dir: &str,
    context: &ChangeContext,
    limit: usize,
) -> Result<String> {
    let issues = get_issues_list(root_dir)?;
    let relevant_issues = filter_relevant_issues(&issues, context, limit);

    Ok(render_issue_refs_to_markdown(
        "Relevant Review Memory",
        &relevant_issues,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{Builder, TempDir};

    /// Creates a unique temporary project root for issue tests.
    ///
    /// 为 issue 测试创建唯一临时项目根目录，避免污染真实仓库状态。
    fn unique_issue_root(label: &str) -> TempDir {
        let root = Builder::new()
            .prefix(&format!("hotpot-issues-{label}-"))
            .tempdir()
            .unwrap();
        fs::create_dir_all(root.path().join(".hotpot")).unwrap();
        root
    }

    /// Returns a deterministic issue candidate for JSONL tests.
    ///
    /// 返回 JSONL 测试使用的确定性 issue candidate。
    fn test_candidate(reason: &str) -> IssueCandidate {
        IssueCandidate {
            created_at: String::from("2026-05-11T10:30:00Z"),
            reason: reason.to_string(),
            changed_files: vec![String::from("src/issues.rs")],
            keywords: vec![String::from("jsonl"), String::from("serde")],
            problem: String::from("issue data failed to deserialize"),
            fix: String::from("write valid JSONL data"),
            validation: vec![String::from("cargo test issues::tests -- --nocapture")],
            promote_hint: String::from("JSONL validation review memory"),
        }
    }

    #[test]
    fn test_filter_relevant_issues() {
        let issues = vec![
            Issue {
                date: chrono::NaiveDate::from_ymd_opt(2026, 5, 11).unwrap(),
                title: String::from("Relevant src issue"),
                kind: IssueKind::Bug,
                tags: vec![String::from("jsonl")],
                paths: vec![String::from("src/issues.rs")],
                scene: String::new(),
                description: String::new(),
                review_check: String::new(),
                solution: String::new(),
                source: IssueSource {
                    changed_files: vec![String::from("src/issues.rs")],
                    summary: String::new(),
                },
            },
            Issue {
                date: chrono::NaiveDate::from_ymd_opt(2026, 5, 11).unwrap(),
                title: String::from("Relevant src issue 2"),
                kind: IssueKind::Optimization,
                tags: vec![String::from("serde")],
                paths: vec![String::from("src/issues.rs")],
                scene: String::new(),
                description: String::new(),
                review_check: String::new(),
                solution: String::new(),
                source: IssueSource {
                    changed_files: vec![String::from("src/issues.rs")],
                    summary: String::new(),
                },
            },
            Issue {
                date: chrono::NaiveDate::from_ymd_opt(2026, 5, 11).unwrap(),
                title: String::from("Irrelevant docs issue"),
                kind: IssueKind::Bug,
                tags: vec![String::from("docs")],
                paths: vec![String::from("docs/")],
                scene: String::new(),
                description: String::new(),
                review_check: String::new(),
                solution: String::new(),
                source: IssueSource {
                    changed_files: vec![String::from("docs/ARCH.md")],
                    summary: String::new(),
                },
            },
        ];
        let context = ChangeContext {
            changed_files: vec![String::from("src/issues.rs")],
            keywords: vec![
                String::from("serde"),
                String::from("jsonl"),
                String::from("markdown"),
                String::from("rendering"),
            ],
        };

        let relevant_issues = filter_relevant_issues(&issues, &context, 2);

        assert_eq!(relevant_issues.len(), 2);
        assert!(
            relevant_issues
                .iter()
                .all(|issue| issue.paths.contains(&String::from("src/issues.rs")))
        );
    }

    #[test]
    fn test_issue_candidates_jsonl() {
        let root = unique_issue_root("jsonl");
        let root_dir = root.path().display().to_string();
        let username = "test_issue_candidates_jsonl";
        let candidate = test_candidate("user reported JSONL parsing failure");

        clear_issue_candidates(&root_dir, username).unwrap();
        append_issue_candidate(&root_dir, username, &candidate).unwrap();

        let candidates = get_issue_candidates_list(&root_dir, username).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].keywords,
            vec![String::from("jsonl"), String::from("serde")]
        );

        clear_issue_candidates(&root_dir, username).unwrap();
        let candidates = get_issue_candidates_list(&root_dir, username).unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn migrates_legacy_workspace_candidates_to_global_file() {
        let root = unique_issue_root("legacy-migration");
        let root_dir = root.path().display().to_string();
        let alice_legacy = root.path().join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        let bob_legacy = root.path().join(".hotpot/workspaces/bob/issue-candidates.jsonl");
        fs::create_dir_all(alice_legacy.parent().unwrap()).unwrap();
        fs::create_dir_all(bob_legacy.parent().unwrap()).unwrap();

        let alice = test_candidate("legacy alice candidate");
        let bob = test_candidate("legacy bob candidate");
        fs::write(
            &alice_legacy,
            format!("{}\n", serde_json::to_string(&alice).unwrap()),
        )
        .unwrap();
        fs::write(
            &bob_legacy,
            format!("{}\n", serde_json::to_string(&bob).unwrap()),
        )
        .unwrap();

        let candidates = get_issue_candidates_list(&root_dir, "alice").unwrap();
        assert_eq!(candidates.len(), 2);

        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        let global_content = fs::read_to_string(&global_path).unwrap();
        assert_eq!(
            global_content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            2
        );
        assert_eq!(fs::read_to_string(&alice_legacy).unwrap(), "");
        assert_eq!(fs::read_to_string(&bob_legacy).unwrap(), "");

        let candidates = get_issue_candidates_list(&root_dir, "alice").unwrap();
        assert_eq!(candidates.len(), 2);
        let global_content = fs::read_to_string(&global_path).unwrap();
        assert_eq!(
            global_content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            2,
            "legacy rows must not be migrated twice"
        );
    }

    #[test]
    fn failed_global_append_keeps_legacy_candidates() {
        let root = unique_issue_root("legacy-append-failure");
        let root_dir = root.path().display().to_string();
        let legacy = root.path().join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();

        let candidate = test_candidate("legacy candidate must survive append failure");
        let legacy_line = format!("{}\n", serde_json::to_string(&candidate).unwrap());
        fs::write(&legacy, &legacy_line).unwrap();

        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        fs::create_dir_all(&global_path).unwrap();

        let err = migrate_legacy_issue_candidates(&root_dir, &global_path)
            .expect_err("directory global path should make append fail");
        assert!(
            format!("{err:#}").contains("打开issue候选文件失败"),
            "unexpected error: {err:#}"
        );
        assert_eq!(
            fs::read_to_string(&legacy).unwrap(),
            legacy_line,
            "legacy rows must remain when global append fails"
        );
    }

    #[test]
    fn staged_legacy_candidate_is_not_migrated_twice_after_append_success() {
        let root = unique_issue_root("legacy-staged-idempotent");
        let root_dir = root.path().display().to_string();
        let legacy = root.path().join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();

        let candidate = test_candidate("staged legacy candidate migrates once");
        let legacy_line = format!("{}\n", serde_json::to_string(&candidate).unwrap());
        fs::write(&legacy, &legacy_line).unwrap();

        let done = legacy_unique_done_path(&legacy);
        fs::rename(&legacy, &done).unwrap();

        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        fs::write(&global_path, &legacy_line).unwrap();

        migrate_legacy_issue_candidates(&root_dir, &global_path).unwrap();

        let global_content = fs::read_to_string(&global_path).unwrap();
        assert_eq!(
            global_content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            1,
            "staged legacy rows must not be appended again after a successful append"
        );
        assert!(
            done.is_file(),
            "committed staging file should remain hidden from legacy scans if cleanup failed"
        );
    }

    #[test]
    fn orphan_staging_merges_with_existing_legacy_before_migration() {
        let root = unique_issue_root("legacy-orphan-staging-merge");
        let root_dir = root.path().display().to_string();
        let legacy = root.path().join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();

        let legacy_line = format!(
            "{}\n",
            serde_json::to_string(&test_candidate("legacy remains visible")).unwrap()
        );
        let staging_line = format!(
            "{}\n",
            serde_json::to_string(&test_candidate("staging is restored before scan")).unwrap()
        );
        fs::write(&legacy, &legacy_line).unwrap();
        let staging = legacy.with_file_name(".issue-candidates.jsonl.hotpot-staging.leftover");
        fs::write(&staging, &staging_line).unwrap();

        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        fs::write(&global_path, "").unwrap();

        migrate_legacy_issue_candidates(&root_dir, &global_path).unwrap();

        let global_content = fs::read_to_string(&global_path).unwrap();
        assert_eq!(
            global_content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            2,
            "legacy and orphan staging rows should both migrate exactly once"
        );
        assert!(
            !staging.exists(),
            "orphan staging file should be cleaned up"
        );
        assert_eq!(fs::read_to_string(&legacy).unwrap(), "");
    }

    #[test]
    fn stale_done_marker_does_not_block_later_legacy_migration() {
        let root = unique_issue_root("legacy-stale-done");
        let root_dir = root.path().display().to_string();
        let legacy = root.path().join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();

        let stale_done = legacy.with_file_name(".issue-candidates.jsonl.hotpot-migrated.leftover");
        fs::write(&stale_done, "already migrated\n").unwrap();
        let legacy_line = format!(
            "{}\n",
            serde_json::to_string(&test_candidate("new legacy after stale done")).unwrap()
        );
        fs::write(&legacy, &legacy_line).unwrap();

        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        fs::write(&global_path, "").unwrap();

        migrate_legacy_issue_candidates(&root_dir, &global_path).unwrap();

        let global_content = fs::read_to_string(&global_path).unwrap();
        assert_eq!(
            global_content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            1,
            "stale done marker should not block a later legacy migration"
        );
        assert!(
            stale_done.is_file(),
            "stale done marker may remain hidden, but must not block migration"
        );
    }

    #[test]
    fn orphan_staging_already_in_global_is_not_restored_or_duplicated() {
        let root = unique_issue_root("legacy-staging-already-global");
        let root_dir = root.path().display().to_string();
        let legacy = root.path().join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();

        let legacy_line = format!(
            "{}\n",
            serde_json::to_string(&test_candidate("already globally appended before crash"))
                .unwrap()
        );
        let staging = legacy_unique_staging_path(&legacy);
        fs::write(&staging, &legacy_line).unwrap();

        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        fs::write(&global_path, &legacy_line).unwrap();

        migrate_legacy_issue_candidates(&root_dir, &global_path).unwrap();

        let global_content = fs::read_to_string(&global_path).unwrap();
        assert_eq!(
            global_content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            1,
            "globally committed orphan staging row must not be duplicated"
        );
        assert!(
            !staging.exists(),
            "committed orphan staging should be cleaned"
        );
        assert!(
            !legacy.exists(),
            "committed orphan staging should not be restored"
        );
    }

    #[test]
    fn orphan_staging_commit_check_preserves_duplicate_row_counts() {
        let root = unique_issue_root("legacy-staging-duplicate-counts");
        let root_dir = root.path().display().to_string();
        let legacy = root.path().join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();

        let row = format!(
            "{}\n",
            serde_json::to_string(&test_candidate("duplicate row count must be preserved"))
                .unwrap()
        );
        let staging = legacy_unique_staging_path(&legacy);
        fs::write(&staging, format!("{row}{row}")).unwrap();

        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        fs::write(&global_path, &row).unwrap();

        migrate_legacy_issue_candidates(&root_dir, &global_path).unwrap();

        let global_content = fs::read_to_string(&global_path).unwrap();
        assert_eq!(
            global_content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            2,
            "only the missing duplicate occurrence should be appended"
        );
        assert!(
            !staging.exists(),
            "staging should be consumed after restoring missing count"
        );
    }

    #[test]
    fn multiple_orphan_staging_files_share_global_duplicate_counts() {
        let root = unique_issue_root("legacy-staging-shared-duplicate-counts");
        let root_dir = root.path().display().to_string();
        let alice_legacy = root.path().join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        let bob_legacy = root.path().join(".hotpot/workspaces/bob/issue-candidates.jsonl");
        fs::create_dir_all(alice_legacy.parent().unwrap()).unwrap();
        fs::create_dir_all(bob_legacy.parent().unwrap()).unwrap();

        let row = format!(
            "{}\n",
            serde_json::to_string(&test_candidate(
                "shared duplicate row count must be preserved"
            ))
            .unwrap()
        );
        let alice_staging = legacy_unique_staging_path(&alice_legacy);
        let bob_staging = legacy_unique_staging_path(&bob_legacy);
        fs::write(&alice_staging, &row).unwrap();
        fs::write(&bob_staging, &row).unwrap();

        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        fs::write(&global_path, &row).unwrap();

        migrate_legacy_issue_candidates(&root_dir, &global_path).unwrap();

        let global_content = fs::read_to_string(&global_path).unwrap();
        assert_eq!(
            global_content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            2,
            "one global row can satisfy only one orphan staging occurrence"
        );
        assert!(!alice_staging.exists());
        assert!(!bob_staging.exists());
    }

    #[test]
    fn orphan_staging_merge_preserves_duplicate_row_counts() {
        let root = unique_issue_root("legacy-orphan-duplicate-counts");
        let root_dir = root.path().display().to_string();
        let legacy = root.path().join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();

        let row = format!(
            "{}\n",
            serde_json::to_string(&test_candidate("legacy duplicate count must be preserved"))
                .unwrap()
        );
        fs::write(&legacy, &row).unwrap();
        let staging = legacy_unique_staging_path(&legacy);
        fs::write(&staging, format!("{row}{row}")).unwrap();

        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        fs::write(&global_path, "").unwrap();

        migrate_legacy_issue_candidates(&root_dir, &global_path).unwrap();

        let global_content = fs::read_to_string(&global_path).unwrap();
        assert_eq!(
            global_content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            2,
            "legacy plus staging duplicate occurrences should migrate exactly twice"
        );
        assert_eq!(fs::read_to_string(&legacy).unwrap(), "");
    }

    #[test]
    fn migration_appends_after_global_without_trailing_newline() {
        let root = unique_issue_root("legacy-global-no-newline");
        let root_dir = root.path().display().to_string();
        let legacy = root.path().join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();

        let existing = test_candidate("existing global without newline");
        let migrated = test_candidate("migrated row after missing newline");
        fs::write(
            &legacy,
            format!("{}\n", serde_json::to_string(&migrated).unwrap()),
        )
        .unwrap();

        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        fs::write(&global_path, serde_json::to_string(&existing).unwrap()).unwrap();

        let candidates = get_issue_candidates_list(&root_dir, "alice").unwrap();

        assert_eq!(candidates.len(), 2);
        assert_eq!(
            fs::read_to_string(&global_path)
                .unwrap()
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            2
        );
    }

    #[test]
    fn orphan_staging_appends_after_legacy_without_trailing_newline() {
        let root = unique_issue_root("legacy-orphan-no-newline");
        let root_dir = root.path().display().to_string();
        let legacy = root.path().join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();

        let existing = test_candidate("existing legacy without newline");
        let staged = test_candidate("staged row after missing newline");
        fs::write(&legacy, serde_json::to_string(&existing).unwrap()).unwrap();
        let staging = legacy_unique_staging_path(&legacy);
        fs::write(
            &staging,
            format!("{}\n", serde_json::to_string(&staged).unwrap()),
        )
        .unwrap();

        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        fs::write(&global_path, "").unwrap();

        let candidates = get_issue_candidates_list(&root_dir, "alice").unwrap();

        assert_eq!(candidates.len(), 2);
        assert_eq!(
            fs::read_to_string(&global_path)
                .unwrap()
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            2
        );
    }

    #[test]
    fn append_candidate_after_global_without_trailing_newline_keeps_jsonl_valid() {
        let root = unique_issue_root("candidate-append-no-newline");
        let root_dir = root.path().display().to_string();
        let username = "append-no-newline";
        let existing = test_candidate("existing candidate without newline");
        let appended = test_candidate("candidate appended after missing newline");
        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        fs::create_dir_all(global_path.parent().unwrap()).unwrap();
        fs::write(&global_path, serde_json::to_string(&existing).unwrap()).unwrap();

        append_issue_candidate(&root_dir, username, &appended).unwrap();

        let candidates = get_issue_candidates_list(&root_dir, username).unwrap();
        assert_eq!(candidates.len(), 2);
        assert_eq!(
            fs::read_to_string(&global_path)
                .unwrap()
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            2
        );
    }

    #[test]
    fn orphan_staging_restores_missing_legacy_before_migration() {
        let root = unique_issue_root("legacy-orphan-staging-restore");
        let root_dir = root.path().display().to_string();
        let legacy = root.path().join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();

        let legacy_line = format!(
            "{}\n",
            serde_json::to_string(&test_candidate("staging restores missing legacy")).unwrap()
        );
        let staging = legacy_unique_staging_path(&legacy);
        fs::write(&staging, &legacy_line).unwrap();

        let global_path = root.path().join(".hotpot/issue-candidates.jsonl");
        fs::write(&global_path, "").unwrap();

        migrate_legacy_issue_candidates(&root_dir, &global_path).unwrap();

        let global_content = fs::read_to_string(&global_path).unwrap();
        assert_eq!(
            global_content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            1,
            "restored orphan staging row should migrate exactly once"
        );
        assert!(!staging.exists(), "orphan staging file should be consumed");
        assert_eq!(fs::read_to_string(&legacy).unwrap(), "");
    }

    /// 多线程并发 `append_issue` 不丢更新：N 个线程各追加一条 Issue，
    /// 最终 `.hotpot/issues.jsonl` 应该恰好出现 N 条对应条目。
    ///
    /// Concurrent `append_issue` from N threads must persist exactly N rows.
    /// Without the cross-process lock this is the FLOW.md known-gap #8
    /// scenario for the project-shared file.
    #[test]
    fn test_concurrent_append_issue_does_not_lose_rows() {
        use std::sync::Arc;
        use std::thread;

        // 自有 tmp root 避免污染仓库根 `.hotpot/issues.jsonl`，并让本测试
        // 与其它并发 issue 测试互不干扰。
        // Isolated tmp root so we don't pollute the repo's issues.jsonl
        // and keep this test isolated from other concurrent issue writes.
        let tmp_root = unique_issue_root("concurrent");
        let root_dir = Arc::new(tmp_root.path().display().to_string());

        const THREADS: u32 = 8;
        let handles: Vec<_> = (0..THREADS)
            .map(|i| {
                let root_dir = Arc::clone(&root_dir);
                thread::spawn(move || {
                    let issue = Issue {
                        date: chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
                        title: format!("issue-{i}"),
                        kind: IssueKind::Bug,
                        tags: vec![],
                        paths: vec![],
                        scene: String::new(),
                        description: String::new(),
                        review_check: String::new(),
                        solution: String::new(),
                        source: IssueSource {
                            changed_files: vec![],
                            summary: format!("concurrent-{i}"),
                        },
                    };
                    append_issue(&root_dir, &issue).unwrap_or_else(|e| panic!("thread {i}: {e}"))
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread panicked");
        }

        let issues = get_issues_list(&root_dir).unwrap();
        assert_eq!(
            issues.len() as u32,
            THREADS,
            "expected {THREADS} rows in shared issues.jsonl, got {}: {:?}",
            issues.len(),
            issues.iter().map(|i| &i.title).collect::<Vec<_>>()
        );
        // 标题集合应恰好对应 0..THREADS。
        let mut titles: Vec<String> = issues.iter().map(|i| i.title.clone()).collect();
        titles.sort();
        let mut expected: Vec<String> = (0..THREADS).map(|i| format!("issue-{i}")).collect();
        expected.sort();
        assert_eq!(titles, expected, "row set mismatch under concurrency");
    }
}
