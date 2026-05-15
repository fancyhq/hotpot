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
/// 九个 `#[serde(rename = "...")]` 字段名是公共契约：OpenCode 的
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

    /// TDD 协议 prompt 路径。
    ///
    /// 当 `/hotpot:execute` 检测到任务文件 `## Plan > ### Mode` 含
    /// `tdd: true` 时，orchestrator 会把这份 prompt 注入到执行子代理
    /// 与 review 子代理的 prompt 中。Codex/Pi 通过该环境变量定位文件，
    /// Claude/OpenCode 通过 `@.hotpot/prompts/tdd-protocol.md` 引用。
    #[serde(rename = "HOTPOT_TDD_PROTOCOL_PROMPT")]
    pub tdd_protocol_prompt: String,

    /// `/hotpot:new` 工作流共享主体路径。
    ///
    /// Claude/OpenCode 通过 `@.hotpot/prompts/hotpot-new.md` 引用，
    /// Codex/Pi 通过该环境变量定位文件后 `Read` 加载。
    #[serde(rename = "HOTPOT_NEW_PROMPT")]
    pub new_prompt: String,

    /// `/hotpot:execute` 工作流共享主体路径。
    ///
    /// Claude/OpenCode 通过 `@.hotpot/prompts/hotpot-execute.md` 引用，
    /// Codex/Pi 通过该环境变量定位文件后 `Read` 加载。
    #[serde(rename = "HOTPOT_EXECUTE_PROMPT")]
    pub execute_prompt: String,

    /// `/hotpot:finish-work` 工作流共享主体路径。
    ///
    /// Claude/OpenCode 通过 `@.hotpot/prompts/hotpot-finish-work.md` 引用，
    /// Codex/Pi 通过该环境变量定位文件后 `Read` 加载。
    #[serde(rename = "HOTPOT_FINISH_WORK_PROMPT")]
    pub finish_work_prompt: String,
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
/// 优先级：`root_override` → env `ROOT_DIR` → **若 cwd 处于 git
/// worktree 内则回退到主仓 toplevel** → `env::current_dir()`。
/// 最后尝试 `canonicalize`；当路径不存在时回退到原始路径。
///
/// 第三层 worktree 兜底是为了让 `/hotpot:execute` 创建的任务 worktree
/// 内运行 `hotpot …` 时仍然把 `.hotpot/` 解析到主仓，而不是在 worktree
/// 自己的子目录里制造一份重复的台账。`ROOT_DIR` 仍是最高优先级显式
/// 覆盖，符合既有公共契约——只有在 env 与 override 都缺失时才查询 git。
///
/// Root-dir resolution for business commands.
///
/// Priority: `root_override` → env `ROOT_DIR` → **main repo toplevel
/// when cwd is inside a git worktree** → `env::current_dir()`. Then
/// `canonicalize`; falls back to the raw path if it does not exist.
///
/// The new third tier makes `hotpot …` calls made from inside a task
/// worktree still resolve `.hotpot/` against the main repo instead of
/// silently creating a parallel ledger under the worktree. `ROOT_DIR`
/// remains the highest-priority explicit override, preserving the
/// existing public contract — git discovery only kicks in when both
/// override and env are missing.
pub fn resolve_root_dir(root_override: Option<PathBuf>) -> Result<String> {
    let explicit = root_override.or_else(|| env::var("ROOT_DIR").ok().map(PathBuf::from));
    let candidate = explicit.or_else(main_repo_root_if_in_worktree);
    let resolved = canonicalize_or_cwd(candidate)?;
    Ok(resolved.display().to_string())
}

/// Returns the main-repo toplevel when the current cwd is inside a git
/// worktree (signalled by `.git` being a regular file rather than a
/// directory), otherwise `None`. Never returns an error — git discovery
/// is best-effort, and the caller falls back to cwd on `None`.
///
/// Detection rules:
/// - `<cwd>/.git` exists and is a regular file → linked worktree marker.
///   Parse it for `gitdir: <path>` to locate the worktree's git dir,
///   then resolve `commondir` next to it to find the main repo's git
///   dir, then take its parent as the main repo toplevel.
/// - `<cwd>/.git` is a directory → cwd is already the main repo (or an
///   independent repo). No worktree fallback needed; return `None` so
///   the caller's cwd path stays in effect.
/// - Anything else (no `.git`, unreadable, etc.) → `None`.
///
/// 当 cwd 处于 git linked worktree 内时（`.git` 是普通文件而非目录），
/// 返回主仓 toplevel；否则 `None`。git 发现是 best-effort：任何 IO 或
/// 解析失败都返回 `None`，把控制权交回上层 cwd 兜底。
///
/// 识别规则：
/// - `<cwd>/.git` 是普通文件：linked worktree 标记。读其内容拿到
///   `gitdir: <path>`，再读同目录下的 `commondir` 解析主仓 .git，
///   取其父目录作为主仓 toplevel。
/// - `<cwd>/.git` 是目录：cwd 已是主仓或独立仓库，不需要 worktree
///   回退，返回 `None` 沿用 cwd。
/// - 其他（无 `.git`、读不到等）：`None`。
fn main_repo_root_if_in_worktree() -> Option<PathBuf> {
    let cwd = env::current_dir().ok()?;
    main_repo_root_for(&cwd)
}

/// Pure helper that mirrors [`main_repo_root_if_in_worktree`] but accepts
/// an explicit starting directory. Kept separate so the implementation
/// is unit-testable without mutating the global cwd.
///
/// [`main_repo_root_if_in_worktree`] 的纯函数版本：显式接受起始目录，
/// 便于单元测试，不需要改全局 cwd。
fn main_repo_root_for(start: &std::path::Path) -> Option<PathBuf> {
    let dot_git = start.join(".git");
    let metadata = fs::metadata(&dot_git).ok()?;
    if metadata.is_dir() {
        // Already a real repo root; no worktree fallback to apply.
        // 已是真仓库根，无需 worktree 兜底。
        return None;
    }
    if !metadata.is_file() {
        return None;
    }

    // Read the gitdir pointer; format is `gitdir: <abs-or-rel-path>`.
    // 读 gitdir 指针；格式为 `gitdir: <绝对或相对路径>`。
    let content = fs::read_to_string(&dot_git).ok()?;
    let gitdir_line = content.lines().find(|l| l.starts_with("gitdir:"))?;
    let raw = gitdir_line.trim_start_matches("gitdir:").trim();
    let gitdir = if PathBuf::from(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        start.join(raw)
    };

    // The `commondir` file inside the worktree's gitdir points at the
    // main repo's `.git` (absolute) or, more commonly for a worktree of
    // the same repo, the relative form `../..`. Either way we need a
    // path that resolves to the main repo's `.git` directory.
    // worktree gitdir 里的 `commondir` 文件指向主仓 `.git`：可能是绝对
    // 路径，也可能是相对的 `../..`（更常见）。无论哪种形式，最终都要
    // 解析为主仓的 `.git` 目录。
    let common_dir_raw = match fs::read_to_string(gitdir.join("commondir")) {
        Ok(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                gitdir.clone()
            } else if PathBuf::from(trimmed).is_absolute() {
                PathBuf::from(trimmed)
            } else {
                gitdir.join(trimmed)
            }
        }
        Err(_) => gitdir.clone(),
    };

    // `PathBuf::parent` is purely lexical — it does NOT collapse `..`,
    // so a raw `.git/worktrees/<id>/../..` ends up with parent
    // `.git/worktrees/<id>/..`, which is wrong. Canonicalize first so
    // `..` segments are resolved, then take the parent. If canonicalize
    // fails (path missing / permission), return `None` and let the
    // caller fall back to cwd rather than emit a corrupt root.
    // `PathBuf::parent` 仅做字面层裁剪、不会消解 `..`，因此对
    // `.git/worktrees/<id>/../..` 直接取 parent 会得到
    // `.git/worktrees/<id>/..`，结果错误。这里先 canonicalize 再取
    // parent；canonicalize 失败时返回 `None`，让上层退回 cwd，避免落
    // 一个错误 root。
    let common_dir = common_dir_raw.canonicalize().ok()?;
    common_dir.parent().map(PathBuf::from)
}

/// 解析 Hotpot 用户名。
///
/// 顺序：env `HOTPOT_USERNAME` → `git config --local user.name`（在
/// `root_dir` 中执行）→ `git config --global user.name` → 字面量
/// `"default"`。此顺序是公共契约，调整会改变已部署会话的归属目录。
pub fn resolve_username(root_dir: &str) -> Result<String> {
    resolve_username_with_source(root_dir).map(|(name, _src)| name)
}

/// 解析 Hotpot 用户名并返回来源标签。
///
/// 与 [`resolve_username`] 顺序完全一致，但额外报告命中的链节，供
/// `hotpot update` 在向用户提示「你将以 `<username>` 身份工作」时
/// 说明该 username 是怎么解析出来的。
///
/// Resolves the Hotpot username **and** the link of the resolution chain
/// that produced it. Same order as [`resolve_username`]; the extra source
/// label is used by `hotpot update` to explain to the user where their
/// identity came from.
pub fn resolve_username_with_source(root_dir: &str) -> Result<(String, UsernameSource)> {
    if let Some(username) = normalize_username(env::var("HOTPOT_USERNAME").ok()) {
        return Ok((username, UsernameSource::Env));
    }

    if let Some(username) = git_username(root_dir, &["config", "--local", "user.name"])? {
        return Ok((username, UsernameSource::GitLocal));
    }

    if let Some(username) = git_username(root_dir, &["config", "--global", "user.name"])? {
        return Ok((username, UsernameSource::GitGlobal));
    }

    Ok(("default".to_string(), UsernameSource::Default))
}

/// 命中的 username 来源标签（用于上层报告，不影响解析顺序）。
///
/// Username resolution source label (reporting only; ordering is fixed in
/// [`resolve_username_with_source`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsernameSource {
    /// 命中 `HOTPOT_USERNAME` 环境变量。
    Env,
    /// 命中 `git config --local user.name`。
    GitLocal,
    /// 命中 `git config --global user.name`。
    GitGlobal,
    /// 三条链全部 miss，回退到字面量 `"default"`。
    Default,
}

impl UsernameSource {
    /// JSON / 摘要输出用的稳定字符串标签。
    pub fn as_str(&self) -> &'static str {
        match self {
            UsernameSource::Env => "env",
            UsernameSource::GitLocal => "git_local",
            UsernameSource::GitGlobal => "git_global",
            UsernameSource::Default => "default",
        }
    }
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
    let tdd_protocol_prompt = prompt_path(&root_dir, "tdd-protocol.md")
        .display()
        .to_string();
    let new_prompt = prompt_path(&root_dir, "hotpot-new.md")
        .display()
        .to_string();
    let execute_prompt = prompt_path(&root_dir, "hotpot-execute.md")
        .display()
        .to_string();
    let finish_work_prompt = prompt_path(&root_dir, "hotpot-finish-work.md")
        .display()
        .to_string();

    Ok(Context {
        root_dir,
        username,
        issue_candidates_file,
        record_issue_candidate_prompt,
        summarize_issue_candidates_prompt,
        tdd_protocol_prompt,
        new_prompt,
        execute_prompt,
        finish_work_prompt,
    })
}

/// 返回 Hotpot 命名 prompt 的项目内绝对路径。
///
/// Prompt 文件由 `hotpot init` 从仓库内 `assets/prompts/` 安装到目标
/// 项目的 `.hotpot/prompts/<name>.md`——和 `.hotpot/issues.jsonl`、
/// `.hotpot/workspaces/...` 一样属于 Hotpot 命名空间下的内部文件，
/// 不应污染项目根目录。
fn prompt_path(root_dir: &str, name: &str) -> PathBuf {
    PathBuf::from(root_dir)
        .join(".hotpot")
        .join("prompts")
        .join(name)
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
