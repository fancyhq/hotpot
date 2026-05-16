use std::fs;

use crate::paths::{hotpot_dir, hotpot_hub_dir};

/// Gets the repository name from the given root directory using `git remote -v`.
///
/// 通过 `git remote -v` 获取远程仓库名称
pub fn get_repo_name(root_dir: &str) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .current_dir(root_dir)
        .arg("remote")
        .arg("-v")
        .output()?;
    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to get repo name"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Failed to get repo name"))?;
    let url = first_line
        .split_whitespace()
        .find(|tok| tok.contains("git"))
        .ok_or_else(|| anyhow::anyhow!("No git-like token in remote line"))?;
    let last_segment = url.rsplit(['/', ':']).next().unwrap_or("");
    let repo_name = last_segment.trim_end_matches(".git").trim();
    if repo_name.is_empty() {
        return Err(anyhow::anyhow!("Failed to derive repo name from {url}"));
    }
    Ok(repo_name.to_string())
}

/// 获取 .hotpot-hub/docs 下所有的用户目录任务文件目录
pub fn get_vuepress_user_dir_entries(root_dir: &str) -> Vec<fs::DirEntry> {
    let vuepress_docs_dir = hotpot_hub_dir(root_dir).join("docs");
    if !vuepress_docs_dir.exists() {
        eprintln!("docs 目录不存在");
        return vec![];
    }
    let Ok(entries) = fs::read_dir(vuepress_docs_dir) else {
        return vec![];
    };
    entries
        .filter_map(|res| res.ok())
        .filter(|res| {
            res.path().is_dir() && !res.file_name().to_str().is_some_and(|f| f.starts_with("."))
        })
        .collect()
}

/// 将 .hotpot/workspaces 下所有用户的任务文件软链接到 .hotpot-hub/docs 下，通过用户名称进行分组
pub fn mklink_tasks_dir(root_dir: &str) -> anyhow::Result<()> {
    // let workspace_dir = root_dir.join(".hotpot/workspaces");
    let workspace_dir = hotpot_dir(root_dir).join("workspaces");
    let hub_docs_dir = hotpot_hub_dir(root_dir).join("docs");
    if !workspace_dir.exists() || !hub_docs_dir.exists() {
        return Err(anyhow::anyhow!("Not found workspace or hub directory."));
    }
    let entries: Vec<fs::DirEntry> = fs::read_dir(workspace_dir)?
        .filter_map(|res| res.ok())
        .filter(|res| {
            res.path().is_dir() && !res.file_name().to_str().is_some_and(|f| f.starts_with("."))
        })
        .collect();
    for entry in entries {
        // Like HusuSama
        let user_dir = entry.file_name();
        let tasks_dir = entry.path().join("tasks");
        let vuepress_user_dir = hub_docs_dir.join(user_dir);

        #[cfg(windows)]
        junction::create(&tasks_dir, &vuepress_user_dir)?;

        #[cfg(unix)]
        std::os::unix::fs::symlink(&tasks_dir, &vuepress_user_dir)?;
    }

    Ok(())
}

/// 删除 .hotpot-hub/docs 下绑定的文档软链接，新增用户目录后需要新增软链接时，需要先进行删除操作，**手动创建的目录将会被一并删除**
pub fn remove_vuepress_links(root_dir: &str) -> anyhow::Result<()> {
    let link_entries = get_vuepress_user_dir_entries(root_dir);
    if link_entries.is_empty() {
        return Ok(());
    }
    for entry in link_entries {
        #[cfg(windows)]
        let result = fs::remove_dir(entry.path());

        #[cfg(unix)]
        let result = fs::remove_file(entry.path());

        result.map_err(|_| {
            anyhow::anyhow!("Remove link failed, link: {}", entry.file_name().display())
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mklink_tasks_dir() -> anyhow::Result<()> {
        let path = "D:\\RustProjects\\hotpot";
        mklink_tasks_dir(path).unwrap();

        Ok(())
    }

    #[test]
    fn test_get_vuepress_user_dir_entries() -> anyhow::Result<()> {
        let path = "D:\\RustProjects\\hotpot";
        let entries = get_vuepress_user_dir_entries(path);
        dbg!("entries: ", &entries);
        Ok(())
    }

    #[test]
    fn test_remove_vuepress_links() -> anyhow::Result<()> {
        let path = "D:\\RustProjects\\hotpot";
        remove_vuepress_links(path).unwrap();

        Ok(())
    }
}
