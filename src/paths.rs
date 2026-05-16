use std::path::{Path, PathBuf};

/// hotpot 的管理根目录，root_dir 由 get_root_dir 获取
pub fn hotpot_dir(root_dir: &str) -> PathBuf {
    PathBuf::from(root_dir).join(".hotpot")
}

/// 当前所属用户的工作空间目录
pub fn workspace_dir(root_dir: &str, username: &str) -> PathBuf {
    hotpot_dir(root_dir).join("workspaces").join(username)
}

pub fn task_dir_path(root_dir: &str, username: &str) -> PathBuf {
    workspace_dir(root_dir, username).join("tasks")
}

/// 存放任务的总览文件路径
pub fn overview_file_path(root_dir: &str, username: &str) -> PathBuf {
    workspace_dir(root_dir, username).join("overview.jsonl")
}

pub fn issues_file_path(root_dir: &str) -> PathBuf {
    hotpot_dir(root_dir).join("issues.jsonl")
}

/// 存放当前用户临时 issue 候选的文件路径
pub fn issue_candidates_file_path(root_dir: &str, username: &str) -> PathBuf {
    workspace_dir(root_dir, username).join("issue-candidates.jsonl")
}

/// 存放 vuepress 生成的文档的目录
pub fn hotpot_hub_dir(root_dir: &str) -> PathBuf {
    PathBuf::from(root_dir).join(".hotpot-hub")
}
