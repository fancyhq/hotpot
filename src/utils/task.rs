use std::fs;

use anyhow::{Context, Ok, Result};
use chrono::NaiveDate;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use toml_edit::{ArrayOfTables, DocumentMut, Table, value};

pub struct Task {
    root_dir: String,
    username: String,
}

#[derive(Debug, Deserialize)]
pub struct Overview {
    tasks: Vec<TaskInfo>,
}

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
    fn as_str(self) -> &'static str {
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
    pub commit: String,
    pub status: TaskStatus,
    pub active: bool,
}

impl Task {
    fn new(root_dir: String, username: String) -> Self {
        Task { root_dir, username }
    }

    fn overview_path(&self) -> String {
        format!(
            "{}/.hotpot/workspaces/{}/overview.toml",
            self.root_dir, self.username
        )
        // 验证 overview.toml 文件是否存在，不存在则创建一个
    }

    /// 确保 overview.toml 存在，不存在则创建（含父目录）
    /// 返回文件路径，方便链式使用
    fn ensure_overview_exists(&self) -> Result<String> {
        let path = self.overview_path();
        if !std::path::Path::new(&path).exists() {
            // 1. 父目录不存在则递归创建
            if let Some(parent) = std::path::Path::new(&path).parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("创建任务目录失败，路径：{}", parent.display()))?;
            }
            // 2. 创建空文件
            fs::write(&path, "").with_context(|| format!("创建任务文件失败，路径：{path}"))?;
        }
        Ok(path)
    }

    /// 获取overview中的所有任务列表
    fn get_task_list(&self) -> Result<Vec<TaskInfo>> {
        let task_file_path = self.overview_path();
        let overview_content = fs::read_to_string(&task_file_path).with_context(|| {
            format!("读取任务文件失败，文件可能不存在或获取数据有误，文件路径：{task_file_path}")
        })?;
        if overview_content.trim().is_empty() {
            return Ok(Vec::new());
        }
        let overview: Overview = toml::from_str(overview_content.as_ref())
            .with_context(|| format!("解析toml文件失败，文件路径：{task_file_path}"))?;
        Ok(overview.tasks)
    }

    /// 判断是否有多个active的任务，程序中只能有一个任务是active为true，多个应该进行处理
    fn has_multi_active_task(&self) -> Result<bool> {
        let task_list = self.get_task_list()?;
        Ok(task_list.iter().filter(|info| info.active).count() > 1)
    }

    /// 创建一个新的任务，新的任务将追加到 overview.toml 文件中，并给定初始Status为InProgress，给定active为true
    /// 在调用 create_task 之前，需要先检查是否有重复的 active 为 true 的任务
    fn create_task(&self, title: &str, commit: &str) -> Result<()> {
        let task_time = chrono::Local::now().date_naive();
        let new_task_info = TaskInfo {
            time: task_time,
            task_id: nanoid!(10),
            title: title.to_string(),
            commit: commit.to_string(),
            status: TaskStatus::InProgress,
            active: true,
        };

        let path = self.overview_path();

        // 读取overview.toml文件
        let content = fs::read_to_string(&path)
            .with_context(|| format!("读取overviwe.toml文件失败，路径：{path}"))?;
        // 解析toml文件内容为可变的Document（DocumentMut）
        let mut doc = content
            .parse::<DocumentMut>()
            .with_context(|| format!("解析toml文件失败，路径：{path}"))?;

        // 创建新的 Table
        let mut table = Table::new();
        table["time"] = value(new_task_info.time.to_string());
        table["task_id"] = value(&new_task_info.task_id);
        table["title"] = value(new_task_info.title);
        table["commit"] = value(&new_task_info.commit);
        table["status"] = value(new_task_info.status.as_str());
        table["active"] = value(new_task_info.active);

        // 检查是否有 tasks 数组，没有则创建一个，然后将 table 插入
        let tasks = doc
            .entry("tasks")
            .or_insert(toml_edit::Item::ArrayOfTables(ArrayOfTables::new()))
            .as_array_of_tables_mut()
            .context("overview中不是 [[tasks]] 数组")?;
        tasks.push(table);

        // 写入，使用临时文件写入，之后进行重命名，避免失败导致文件损坏
        let tmp = format!("{path}.tmp");
        fs::write(&tmp, doc.to_string()).with_context(|| format!("写入临时文件 {tmp} 失败"))?;
        fs::rename(&tmp, &path).with_context(|| format!("覆盖任务文件失败，路径：{path}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::base::{get_root_dir, get_username};

    use super::*;

    #[test]
    fn test_get_task_list() {
        let root_dir = get_root_dir().unwrap();
        let username = get_username().unwrap();
        let task = Task::new(root_dir, username);
        let task_list = task.get_task_list().unwrap();
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
        let task = Task::new(root_dir, username);
        task.create_task("我的新任务", "deadbeef").unwrap();
        let list = task.get_task_list().unwrap();
        println!("现在有 {} 条任务", list.len());
        println!("最后一条：{:#?}", list.last().unwrap());
    }
}
