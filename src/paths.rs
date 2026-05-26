use std::path::PathBuf;

/// Returns Hotpot's project metadata directory.
///
/// 返回 Hotpot 的项目元数据目录。
pub fn hotpot_dir(root_dir: &str) -> PathBuf {
    PathBuf::from(root_dir).join(".hotpot")
}

/// Returns the per-user workspace directory.
///
/// 返回指定用户的 workspace 目录。
pub fn workspace_dir(root_dir: &str, username: &str) -> PathBuf {
    hotpot_dir(root_dir).join("workspaces").join(username)
}

/// Returns the task directory inside a per-user workspace.
///
/// 返回指定用户 workspace 内的任务目录。
pub fn task_dir_path(root_dir: &str, username: &str) -> PathBuf {
    workspace_dir(root_dir, username).join("tasks")
}

/// Returns the per-user overview ledger path.
///
/// 返回指定用户的任务总览台账文件路径。
pub fn overview_file_path(root_dir: &str, username: &str) -> PathBuf {
    workspace_dir(root_dir, username).join("overview.jsonl")
}

/// Returns the project-shared long-term review memory file path.
///
/// 返回项目级长期 review memory 文件路径。
pub fn issues_file_path(root_dir: &str) -> PathBuf {
    hotpot_dir(root_dir).join("issues.jsonl")
}

/// Returns the project-shared temporary issue candidates file path.
///
/// The `username` parameter is kept only for call-site compatibility; it no
/// longer influences the path because candidates are now global project state.
///
/// 返回项目级共享的临时 issue 候选文件路径。
///
/// `username` 参数仅为兼容既有调用签名而保留；候选现在是项目级全局状态，
/// 不再由 username 决定路径。
pub fn issue_candidates_file_path(root_dir: &str, _username: &str) -> PathBuf {
    hotpot_dir(root_dir).join("issue-candidates.jsonl")
}

/// Returns the VuePress hub directory managed by Hotpot.
///
/// 返回 Hotpot 管理的 VuePress hub 目录。
pub fn hotpot_hub_dir(root_dir: &str) -> PathBuf {
    PathBuf::from(root_dir).join(".hotpot-hub")
}
