use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Error, Ok, Result};
use chrono::NaiveDate;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};

use crate::{
    paths::{self, overview_file_path},
    utils,
};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    #[serde(rename = "Done")]
    Done,
    #[serde(rename = "In Progress")]
    InProgress,
    #[serde(rename = "Cancelled")]
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &str {
        match self {
            TaskStatus::Done => "Done",
            TaskStatus::InProgress => "In Progress",
            TaskStatus::Cancelled => "Cancelled",
        }
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub struct TaskInfo {
    pub time: NaiveDate,
    pub task_id: String,
    pub title: String,
    pub commit: Option<String>,
    pub status: TaskStatus,
    pub active: bool,
}

/// 确保 overview.jsonl 存在，不存在则创建（含父目录）
/// 返回文件路径，方便链式使用
fn ensure_overview_exists(root_dir: &str, username: &str) -> Result<PathBuf> {
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

/// 停止所有 active 的任务，通常在创建新任务时，有其他遗留时使用
pub fn stop_all_active_tasks(root_dir: &str, username: &str) -> Result<()> {
    let path = ensure_overview_exists(root_dir, username)?;
    let mut task_list = get_task_list(root_dir, username)?;

    for task in &mut task_list {
        task.active = false;
    }

    let mut content = String::new();
    for task in task_list {
        let line = serde_json::to_string(&task)
            .with_context(|| format!("序列化任务信息失败，任务ID：{}", task.task_id))?;
        content.push_str(&line);
        content.push('\n');
    }

    fs::write(&path, content)
        .with_context(|| format!("写入任务文件失败，路径：{}", path.display()))?;

    let active_task_count = get_active_task_count(root_dir, username)?;
    if active_task_count > 0 {
        return Err(anyhow::anyhow!("Stop failed."));
    }
    Ok(())
}

/// 根据提供的 TaskInfo 内容生成文件名
pub fn get_task_filename(task: &TaskInfo) -> String {
    format!("{}-{}", task.time, task.title)
}

/// 通过此函数获取第一个被标记为 active 的任务
pub fn get_active_task_filepath(root_dir: &str, username: &str) -> Result<PathBuf> {
    let task_list = get_task_list(root_dir, username)?;
    for task in task_list {
        if task.active {
            let task_filename = get_task_filename(&task);
            let task_dir = paths::task_dir_path(root_dir, username);
            return Ok(task_dir.join(format!("{task_filename}.md")));
        }
    }
    Err(anyhow::anyhow!("Not found active task"))
}

/// 创建一个新的任务，新的任务将追加到 overview.jsonl 文件中，并给定初始Status为InProgress，给定active为true
/// 在调用 create_task 之前，需要先检查是否有重复的 active 为 true 的任务
pub fn create_task(
    root_dir: &str,
    username: &str,
    title: &str,
    commit: Option<&str>,
) -> Result<()> {
    let task_time = chrono::Local::now().date_naive();
    let new_task_info = TaskInfo {
        time: task_time,
        task_id: nanoid!(10),
        title: title.to_string(),
        commit: commit.map(str::to_string),
        status: TaskStatus::InProgress,
        active: true,
    };

    let path = ensure_overview_exists(root_dir, username)?;
    let mut line = serde_json::to_string(&new_task_info)
        .with_context(|| format!("序列化任务信息失败，任务ID：{}", new_task_info.task_id))?;
    line.push('\n');

    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("打开任务文件失败，路径：{}", path.display()))?
        .write_all(line.as_bytes())
        .with_context(|| format!("追加任务文件失败，路径：{}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::utils::{get_root_dir, get_username};

    use super::*;

    #[test]
    fn test_get_task_list() {
        let root_dir = get_root_dir().unwrap();
        let username = get_username().unwrap();
        let task_list = get_task_list(&root_dir, &username).unwrap();
        for info in &task_list {
            let result = info.status == TaskStatus::InProgress;
            println!("任务状态：{result}");
        }
        println!("获取任务列表：{task_list:#?}")
    }

    #[test]
    fn test_create_task() {
        let root_dir = get_root_dir().unwrap();
        let username = get_username().unwrap();
        create_task(&root_dir, &username, "我的新任务", Some("deadbeef")).unwrap();
        let list = get_task_list(&root_dir, &username).unwrap();
        println!("现在有 {} 条任务", list.len());
        println!("最后一条：{:#?}", list.last().unwrap());
    }
}
