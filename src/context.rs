//! Hotpot 运行时上下文解析。
//!
//! 该模块提供 hook 与业务命令共享的上下文解析逻辑。
//! 业务命令入口 (`Context::resolve` / `resolve_root_dir`) 走 env-first 链：
//! `ROOT_DIR` → cwd → canonicalize。
//! hook 子命令入口 (`Context::from_payload`) 走 payload-first 链：
//! payload.cwd → cwd 兜底 → canonicalize，**不读 env**，避免被 ambient
//! `ROOT_DIR` 污染（这是 hook 必须反映平台传入 cwd 的硬约束）。

use std::{env, fs, path::PathBuf};

use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::paths;

/// 已解析的 Hotpot 运行时上下文，供 hook 与业务命令共享。
///
/// 五个 `#[serde(rename = "...")]` 字段名是公共契约：OpenCode 的
/// `bash-before.ts` 与 Pi 的 `extensions/hotpot/index.ts` 都按这些大写
/// 下划线键名直接读取 JSON 输出，**字面量不能改动**。
#[derive(Debug, serde::Serialize)]
pub struct Context {
    /// 绝对项目根目录。
    #[serde(rename = "ROOT_DIR")]
    pub root_dir: String,

    /// 当前会话使用的 Hotpot 用户名。
    #[serde(rename = "HOTPOT_USERNAME")]
    pub username: String,

    /// 临时 issue 候选 JSONL 文件路径。
    #[serde(rename = "HOTPOT_ISSUE_CANDIDATES_FILE")]
    pub issue_candidates_file: String,

    /// 记录候选时使用的 prompt 路径。
    #[serde(rename = "HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT")]
    pub record_issue_candidate_prompt: String,

    /// 汇总候选时使用的 prompt 路径。
    #[serde(rename = "HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT")]
    pub summarize_issue_candidates_prompt: String,
}

impl Context {
    /// 业务命令入口：根据 env-first 链解析根目录后构建上下文。
    ///
    /// 当 `root_override` 为 `Some` 时优先使用其值；否则按
    /// `ROOT_DIR` → `current_dir()` 顺序回退，最后尝试 `canonicalize`。
    pub fn resolve(root_override: Option<PathBuf>) -> Result<Self> {
        let root_dir = resolve_root_dir(root_override)?;
        build_context(root_dir)
    }

    /// hook 入口：从平台 hook payload 中的 `cwd` 字段派生上下文。
    ///
    /// 不读 `ROOT_DIR` env，确保 hook 输出反映平台传入的 cwd。
    pub fn from_payload(payload: &Value) -> Result<Self> {
        let payload_cwd = payload
            .get("cwd")
            .and_then(Value::as_str)
            .map(PathBuf::from);
        let root_dir = canonicalize_or_cwd(payload_cwd)?;
        build_context(root_dir.display().to_string())
    }

    /// 显式创建临时 issue 候选 JSONL 文件（含其父目录）。
    ///
    /// hook bootstrap 调用此方法保证 OpenCode / Pi 的插件运行时拿到一个
    /// 已存在的 JSONL；业务命令不需要预创建文件，按需调用即可。
    pub fn ensure_issue_candidates_file(&self) -> Result<()> {
        let file_path = PathBuf::from(&self.issue_candidates_file);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        if !file_path.exists() {
            fs::File::create(&file_path)
                .with_context(|| format!("failed to create {}", file_path.display()))?;
        }

        Ok(())
    }
}

/// 业务命令入口的根目录解析。
///
/// 优先级：`root_override` → env `ROOT_DIR` → `env::current_dir()`。
/// 最后尝试 `canonicalize`；当路径不存在时回退到原始路径。
pub fn resolve_root_dir(root_override: Option<PathBuf>) -> Result<String> {
    let candidate = root_override.or_else(|| env::var("ROOT_DIR").ok().map(PathBuf::from));
    let resolved = canonicalize_or_cwd(candidate)?;
    Ok(resolved.display().to_string())
}

/// 解析 Hotpot 用户名。
///
/// 顺序：env `HOTPOT_USERNAME` → `git config --local user.name`（在
/// `root_dir` 中执行）→ `git config --global user.name` → 字面量
/// `"default"`。此顺序是公共契约，调整会改变已部署会话的归属目录。
pub fn resolve_username(root_dir: &str) -> Result<String> {
    if let Some(username) = normalize_username(env::var("HOTPOT_USERNAME").ok()) {
        return Ok(username);
    }

    if let Some(username) = git_username(root_dir, &["config", "--local", "user.name"])? {
        return Ok(username);
    }

    if let Some(username) = git_username(root_dir, &["config", "--global", "user.name"])? {
        return Ok(username);
    }

    Ok("default".to_string())
}

/// 把候选路径解析为绝对路径字符串：先选定 `candidate`，缺失则用 cwd 兜底，
/// 最后尝试 `canonicalize`，不存在时返回原路径以保持向后兼容。
fn canonicalize_or_cwd(candidate: Option<PathBuf>) -> Result<PathBuf> {
    let path = match candidate {
        Some(path) => path,
        None => env::current_dir().context("failed to resolve current directory")?,
    };
    Ok(path.canonicalize().unwrap_or(path))
}

/// 由已解析的根目录构建完整 `Context`。
fn build_context(root_dir: String) -> Result<Context> {
    let username = resolve_username(&root_dir)?;
    let issue_candidates_file = paths::issue_candidates_file_path(&root_dir, &username)
        .display()
        .to_string();
    let record_issue_candidate_prompt = prompt_path(&root_dir, "record-issue-candidate.md")
        .display()
        .to_string();
    let summarize_issue_candidates_prompt = prompt_path(&root_dir, "summarize-issue-candidates.md")
        .display()
        .to_string();

    Ok(Context {
        root_dir,
        username,
        issue_candidates_file,
        record_issue_candidate_prompt,
        summarize_issue_candidates_prompt,
    })
}

/// 返回 Hotpot 命名 prompt 的项目内绝对路径。
fn prompt_path(root_dir: &str, name: &str) -> PathBuf {
    PathBuf::from(root_dir).join("prompts").join(name)
}

/// 只在字符串非空时返回 `Some`。
fn normalize_username(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

/// 在 `root_dir` 中运行一条 git 命令并返回 trim 后的 stdout。
fn git_username(root_dir: &str, args: &[&str]) -> Result<Option<String>> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root_dir)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(normalize_username(Some(stdout)))
        }
        Ok(_) | Err(_) => Ok(None),
    }
}
