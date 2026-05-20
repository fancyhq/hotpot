//! Per-user workspace bootstrap helpers.
//!
//! Shared between `hotpot init` (creates the workspace skeleton up-front
//! so VuePress symlinks and any subsequent `task create` find their
//! target directory) and `hotpot update` (day-1 collaborator entry).
//! Keeping these helpers in their own small module avoids a fragile
//! `init → update` dependency and gives `task::create` a single place
//! to look when it also wants to ensure the skeleton.
//!
//! The per-user skeleton contains only `overview.jsonl` and `tasks/`;
//! temporary issue candidates are project-shared at
//! `.hotpot/issue-candidates.jsonl`.
//!
//! 用户 workspace 骨架的创建逻辑。`hotpot init` 与 `hotpot update` 共用
//! 同一份实现——抽出独立模块避免 init 反向依赖 update，也给 `task create`
//! 等需要兜底创建骨架的命令一个统一入口。用户骨架只包含
//! `overview.jsonl` 与 `tasks/`；临时 issue 候选是项目级共享文件，位于
//! `.hotpot/issue-candidates.jsonl`。

use std::fs;

use anyhow::{Context, Result};

use crate::{issues, paths};

/// 创建指定 username 的 workspace 骨架，并确保项目级 candidates 文件存在。
///
/// 用户 workspace 只包含 `overview.jsonl` 与 `tasks/`；
/// `.hotpot/issue-candidates.jsonl` 是项目级共享文件，不位于用户 workspace 内。
/// 所有步骤幂等：目录或空文件已存在则 skip，不会覆盖既有内容。
///
/// Bootstraps the per-user workspace skeleton: `<workspace>/`,
/// `<workspace>/tasks/`, and `<workspace>/overview.jsonl`, then ensures the
/// project-shared `.hotpot/issue-candidates.jsonl` exists. Fully idempotent —
/// existing directories and non-empty JSONL files are left untouched.
pub fn ensure_workspace_skeleton(root_dir: &str, username: &str) -> Result<()> {
    let ws = paths::workspace_dir(root_dir, username);
    fs::create_dir_all(&ws).with_context(|| format!("failed to create {}", ws.display()))?;

    let tasks = paths::task_dir_path(root_dir, username);
    fs::create_dir_all(&tasks).with_context(|| format!("failed to create {}", tasks.display()))?;

    let overview = paths::overview_file_path(root_dir, username);
    if !overview.exists() {
        fs::write(&overview, b"")
            .with_context(|| format!("failed to create {}", overview.display()))?;
    }

    issues::ensure_issue_candidates_exists(root_dir, username)
        .context("failed to ensure project-shared issue candidates file")?;

    Ok(())
}
