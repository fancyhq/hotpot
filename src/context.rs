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
/// 全部 `#[serde(rename = "...")]` 字段名是公共契约：OpenCode 的
/// `bash-before.ts` 与 Pi 的 `extensions/hotpot/index.ts` 都按这些大写
/// 下划线键名直接读取 JSON 输出，**字面量不能改动**。
///
/// VuePress 三字段（`HOTPOT_VUEPRESS_ENABLED` / `HOTPOT_VUEPRESS_PORT` /
/// `HOTPOT_VUEPRESS_URL`）受 `.hotpot/config.toml::[vuepress] enabled` 门
/// 控：禁用时 `HOTPOT_VUEPRESS_ENABLED` 始终输出 `"false"`，另两个空
/// 串通过 `skip_serializing_if` 从 JSON 中省略，AI 看到 `false` 即走未
/// 启用分支。
#[derive(Debug, serde::Serialize)]
pub struct Context {
    /// 绝对项目根目录。
    #[serde(rename = "ROOT_DIR")]
    pub root_dir: String,

    /// 当前会话使用的 Hotpot 用户名。
    #[serde(rename = "HOTPOT_USERNAME")]
    pub username: String,

    /// 项目配置的输出语言（自然语言回复使用）。
    ///
    /// Resolved natural-language preference (free-form user-authored
    /// string) read from `<root>/.hotpot/config.toml::language`. Hooks
    /// re-inject this value into every model turn so the directive
    /// cannot drift across long sessions / sub-agent boundaries.
    ///
    /// 由 hook 在每轮注入到 additionalContext / systemMessage，避免长
    /// 会话或子代理边界导致语言指令漂移。值为用户自由书写的字符串
    /// （例如 `English`、`简体中文`、`日本語`）；解析失败时回退到
    /// 字面量 `"English"`。结构性锚点无论该值如何都保持英文。
    #[serde(rename = "HOTPOT_LANGUAGE")]
    pub language: String,

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

    /// VuePress 启用开关，作为 hook / bootstrap 公共契约字段。
    ///
    /// Always serialized (`"true"` or `"false"`) as the public hook /
    /// bootstrap contract requires. **No longer used as the AI gate**:
    /// the prompt file-existence gate in `hotpot-new.md` probes
    /// `.hotpot/prompts/vuepress*.md` directly via Bash `test -f` so that
    /// every platform observes the same ground truth (OpenCode's plugin
    /// only injects this env-var into shell subprocesses via
    /// `shell.env` and never surfaces it into AI conversation context).
    /// Pulled from [`resolve_vuepress_enabled`]; never empty.
    ///
    /// 始终输出 `"true"` 或 `"false"`，作为 hook / bootstrap 公共契约
    /// 字段。**不再用作 AI 闸门**：`hotpot-new.md` 的 prompt
    /// file-existence gate 通过 Bash `test -f` 直接探测
    /// `.hotpot/prompts/vuepress*.md`，四个平台观察值一致（OpenCode
    /// 插件只把这个 env-var 经 `shell.env` 注入 shell 子进程，AI 对话
    /// 上下文里看不到它）。
    #[serde(rename = "HOTPOT_VUEPRESS_ENABLED")]
    pub vuepress_enabled: String,

    /// VuePress dev server 端口字符串，仅启用态填充。
    ///
    /// Empty string when disabled (skipped from JSON via
    /// `skip_serializing_if`), so consumers reading the JSON cannot
    /// accidentally treat a stale port as live state.
    ///
    /// 禁用态为空串，通过 `skip_serializing_if` 从 JSON 中省略，避免
    /// 下游误把陈旧端口当成活动状态。
    #[serde(
        rename = "HOTPOT_VUEPRESS_PORT",
        skip_serializing_if = "String::is_empty"
    )]
    pub vuepress_port: String,

    /// VuePress dev server 根 URL，仅启用态填充。
    ///
    /// Pre-formatted `http://localhost:<port>` so the brainstorming
    /// closing flow can hand the URL straight to the user without
    /// stringly recomputing it.
    ///
    /// 已预先拼成 `http://localhost:<port>`，brainstorming 收尾流程
    /// 直接展示给用户即可，无需重复计算。
    #[serde(
        rename = "HOTPOT_VUEPRESS_URL",
        skip_serializing_if = "String::is_empty"
    )]
    pub vuepress_url: String,
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
        build_context(path_to_agent_string(&root_dir))
    }

    /// 显式确保临时 issue 候选 JSONL 文件存在，并迁移旧候选。
    ///
    /// hook bootstrap 调用此方法保证 OpenCode / Pi 的插件运行时拿到一个
    /// 已存在且已完成旧 per-user 候选迁移的 JSONL；业务命令不需要预创建文件，
    /// 按需调用即可。
    ///
    /// Ensures the temporary issue candidates JSONL exists and runs the legacy
    /// per-user migration path. Hook bootstrap calls this so OpenCode / Pi
    /// plugins see the same global candidates file as the CLI.
    pub fn ensure_issue_candidates_file(&self) -> Result<()> {
        crate::issues::ensure_issue_candidates_exists(&self.root_dir, &self.username).map(|_| ())
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
    Ok(path_to_agent_string(&resolved))
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
    // 同样改用 `dunce::canonicalize`：消解 `..` 段的同时避免 Windows 上
    // 给主仓 toplevel 加 `\\?\` verbatim 前缀。
    // Use `dunce::canonicalize` here too: it resolves `..` segments while
    // keeping the resulting main-repo toplevel free of the Windows
    // `\\?\` verbatim prefix.
    let common_dir = dunce::canonicalize(&common_dir_raw).ok()?;
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
    if let Some(username) = normalize_string(env::var("HOTPOT_USERNAME").ok()) {
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

/// 解析 Hotpot 自然语言输出偏好。
///
/// Resolves the project natural-language preference used for user-facing
/// output. Chain (matches `output-language.md::Recovery`):
/// 1. env `HOTPOT_LANGUAGE` (trimmed non-empty)
/// 2. `<root_dir>/.hotpot/config.toml` top-level `language = "..."`
/// 3. literal `"English"`
///
/// All failure modes (missing file, IO error, malformed TOML, missing
/// key, empty value) silently fall back to English — language resolution
/// must **never** abort a hook, since hooks run on every model turn.
///
/// 顺序：env `HOTPOT_LANGUAGE` → `<root_dir>/.hotpot/config.toml` 顶层
/// `language = "..."` → 字面量 `"English"`。任何解析失败一律静默回退到
/// `"English"`，绝不 `bail!` —— 该函数运行在每一轮 hook 上，必须无声。
pub fn resolve_language(root_dir: &str) -> String {
    resolve_language_with_source(root_dir).0
}

/// 解析 Hotpot 自然语言输出偏好并返回来源标签。
///
/// 与 [`resolve_language`] 顺序完全一致，但额外报告命中的链节，供
/// `hotpot update` 在向用户提示「输出语言已配置为 `<language>`」时说明
/// 解析来源（env / config / default），与 [`resolve_username_with_source`]
/// 的风格保持对齐。
///
/// Same chain as [`resolve_language`]; reports which link of the
/// resolution chain produced the value so `hotpot update` can explain
/// the source to the user.
pub fn resolve_language_with_source(root_dir: &str) -> (String, LanguageSource) {
    if let Some(language) = normalize_string(env::var("HOTPOT_LANGUAGE").ok()) {
        return (language, LanguageSource::Env);
    }

    if let Some(language) = read_language_from_config_toml(root_dir) {
        return (language, LanguageSource::ConfigToml);
    }

    ("English".to_string(), LanguageSource::Default)
}

/// 从 `<root_dir>/.hotpot/config.toml` 顶层 `language = "..."` 读取值。
///
/// Reads the top-level `language` string from `<root_dir>/.hotpot/config.toml`
/// using `toml_edit` (already a Hotpot dependency). Returns `None` for
/// any failure: missing file, IO error, malformed TOML, missing key,
/// non-string value, empty / whitespace-only value. Never panics.
///
/// 任何失败（文件不存在、IO 错误、TOML 损坏、缺字段、非字符串、空串）
/// 都返回 `None`，由调用方继续走链式回退。
fn read_language_from_config_toml(root_dir: &str) -> Option<String> {
    let config_path = PathBuf::from(root_dir).join(".hotpot").join("config.toml");
    let raw = fs::read_to_string(&config_path).ok()?;
    // toml_edit 容错地保留注释与原始格式；解析失败返回 None 即可。
    // toml_edit preserves comments/formatting and degrades gracefully on
    // parse error — falling through to None is the desired behavior here.
    let doc = raw.parse::<toml_edit::DocumentMut>().ok()?;
    let item = doc.get("language")?;
    let value = item.as_str()?;
    normalize_string(Some(value.to_string()))
}

/// 命中的 language 来源标签（用于上层报告，不影响解析顺序）。
///
/// Language resolution source label (reporting only; ordering is fixed in
/// [`resolve_language_with_source`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageSource {
    /// 命中 `HOTPOT_LANGUAGE` 环境变量。
    Env,
    /// 命中 `<root>/.hotpot/config.toml` 顶层 `language` 字段。
    ConfigToml,
    /// 两条链全部 miss，回退到字面量 `"English"`。
    Default,
}

impl LanguageSource {
    /// JSON / 摘要输出用的稳定字符串标签。
    pub fn as_str(&self) -> &'static str {
        match self {
            LanguageSource::Env => "env",
            LanguageSource::ConfigToml => "config_toml",
            LanguageSource::Default => "default",
        }
    }
}

/// VuePress 启用状态的默认值（禁用）。
///
/// Default VuePress enabled state (disabled). Centralized so resolver
/// fallback and serde defaults agree on the same literal.
pub const VUEPRESS_ENABLED_DEFAULT: bool = false;

/// VuePress dev server 默认端口。
///
/// Default port for `pnpm docs:dev`. Centralized so resolver fallback,
/// config-template comments, and CLI `--port` defaults agree.
pub const VUEPRESS_PORT_DEFAULT: u16 = 8080;

/// 解析 VuePress 启用状态。
///
/// Resolves whether VuePress integration is currently enabled. Chain:
/// 1. env `HOTPOT_VUEPRESS_ENABLED` (`"true"` / `"false"`, case-insensitive)
/// 2. `<root_dir>/.hotpot/config.toml` `[vuepress] enabled = bool`
/// 3. literal `false` (disabled)
///
/// `enabled` is part of an *atomic* state — flipping it without running
/// `hotpot vuepress install` / `uninstall` will desync the `.hotpot-hub/`
/// directory and opt-in prompts. The resolver itself does not enforce
/// this; CLI commands cross-check on use.
///
/// 顺序：env `HOTPOT_VUEPRESS_ENABLED`（不区分大小写的 `"true"/"false"`）
/// → `.hotpot/config.toml::[vuepress] enabled` → 字面量 `false`。任何
/// 解析失败一律静默回退；该函数被 hook 每轮调用，必须无声。
pub fn resolve_vuepress_enabled(root_dir: &str) -> bool {
    resolve_vuepress_enabled_with_source(root_dir).0
}

/// 解析 VuePress 启用状态并返回来源标签。
///
/// 与 [`resolve_vuepress_enabled`] 顺序一致；额外报告命中链节，便于
/// `hotpot update` / 健康自检向用户说明 `enabled` 是从 env、config 还是
/// 默认值得来的。
pub fn resolve_vuepress_enabled_with_source(root_dir: &str) -> (bool, VuepressEnabledSource) {
    if let Some(enabled) = parse_bool(env::var("HOTPOT_VUEPRESS_ENABLED").ok()) {
        return (enabled, VuepressEnabledSource::Env);
    }

    if let Some(enabled) = read_vuepress_enabled_from_config_toml(root_dir) {
        return (enabled, VuepressEnabledSource::ConfigToml);
    }

    (VUEPRESS_ENABLED_DEFAULT, VuepressEnabledSource::Default)
}

/// 从 `<root_dir>/.hotpot/config.toml` 的 `[vuepress] enabled` 读取布尔值。
///
/// Returns `None` for any failure (missing file, IO error, malformed TOML,
/// missing `[vuepress]` table, missing `enabled` key, non-bool value).
///
/// 任何失败均返回 `None`，由调用方继续走链式回退。
fn read_vuepress_enabled_from_config_toml(root_dir: &str) -> Option<bool> {
    let config_path = PathBuf::from(root_dir).join(".hotpot").join("config.toml");
    let raw = fs::read_to_string(&config_path).ok()?;
    let doc = raw.parse::<toml_edit::DocumentMut>().ok()?;
    let table = doc.get("vuepress")?.as_table()?;
    let item = table.get("enabled")?;
    item.as_bool()
}

/// 命中的 VuePress enabled 来源标签（仅用于上层报告）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VuepressEnabledSource {
    /// 命中 `HOTPOT_VUEPRESS_ENABLED` 环境变量。
    Env,
    /// 命中 `<root>/.hotpot/config.toml::[vuepress] enabled`。
    ConfigToml,
    /// 链路全部 miss，回退到字面量 `false`。
    Default,
}

impl VuepressEnabledSource {
    /// JSON / 摘要输出用的稳定字符串标签。
    pub fn as_str(&self) -> &'static str {
        match self {
            VuepressEnabledSource::Env => "env",
            VuepressEnabledSource::ConfigToml => "config_toml",
            VuepressEnabledSource::Default => "default",
        }
    }
}

/// 解析 VuePress dev server 端口。
///
/// Resolves the VuePress dev server port. Chain:
/// 1. env `HOTPOT_VUEPRESS_PORT` (numeric in `u16` range)
/// 2. `<root_dir>/.hotpot/config.toml::[vuepress] port = <int>`
/// 3. literal `8080`
///
/// Out-of-range or non-numeric values at any level fall through to the
/// next link — never `bail!`, since this runs on every hook turn.
///
/// 顺序：env `HOTPOT_VUEPRESS_PORT`（必须能解析为 u16）→
/// `.hotpot/config.toml::[vuepress] port` → 字面量 `8080`。任何失败
/// 静默回退。
pub fn resolve_vuepress_port(root_dir: &str) -> u16 {
    resolve_vuepress_port_with_source(root_dir).0
}

/// 解析 VuePress 端口并返回来源标签。
pub fn resolve_vuepress_port_with_source(root_dir: &str) -> (u16, VuepressPortSource) {
    if let Some(port) = env::var("HOTPOT_VUEPRESS_PORT")
        .ok()
        .and_then(|raw| raw.trim().parse::<u16>().ok())
    {
        return (port, VuepressPortSource::Env);
    }

    if let Some(port) = read_vuepress_port_from_config_toml(root_dir) {
        return (port, VuepressPortSource::ConfigToml);
    }

    (VUEPRESS_PORT_DEFAULT, VuepressPortSource::Default)
}

/// 从 `<root_dir>/.hotpot/config.toml` 的 `[vuepress] port` 读取端口。
///
/// Reads `[vuepress] port` as an integer in `u16` range. Returns `None`
/// on missing / IO / malformed / out-of-range value.
///
/// `[vuepress] port` 必须是 0..=65535 范围内的整数；越界或类型不匹配
/// 都返回 `None`，由调用方继续走链式回退。
fn read_vuepress_port_from_config_toml(root_dir: &str) -> Option<u16> {
    let config_path = PathBuf::from(root_dir).join(".hotpot").join("config.toml");
    let raw = fs::read_to_string(&config_path).ok()?;
    let doc = raw.parse::<toml_edit::DocumentMut>().ok()?;
    let table = doc.get("vuepress")?.as_table()?;
    let item = table.get("port")?;
    let value = item.as_integer()?;
    u16::try_from(value).ok()
}

/// 命中的 VuePress port 来源标签（仅用于上层报告）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VuepressPortSource {
    /// 命中 `HOTPOT_VUEPRESS_PORT` 环境变量。
    Env,
    /// 命中 `<root>/.hotpot/config.toml::[vuepress] port`。
    ConfigToml,
    /// 链路全部 miss，回退到字面量 `8080`。
    Default,
}

impl VuepressPortSource {
    /// JSON / 摘要输出用的稳定字符串标签。
    pub fn as_str(&self) -> &'static str {
        match self {
            VuepressPortSource::Env => "env",
            VuepressPortSource::ConfigToml => "config_toml",
            VuepressPortSource::Default => "default",
        }
    }
}

/// 把可选字符串解析为布尔值，容忍大小写与首尾空白。
///
/// Parses an optional string into a boolean, tolerating mixed case and
/// surrounding whitespace. Used by VuePress env-var resolution where
/// shell may emit `"True"` / `"FALSE"` etc.
fn parse_bool(value: Option<String>) -> Option<bool> {
    let raw = value?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// 把任意 `Path` 转成给 agent / env-var / prompt 消费的字符串：在所有平台
/// 上把反斜杠统一替换为正斜杠，让 `Context` 输出的 path 字段始终是 POSIX
/// 形式。Windows 自身的 fs API 完全接受正斜杠分隔符，因此这层规范化是单向、
/// 零功能损失的；同时避免反斜杠在 markdown / URI / JSON 二次转义场景里被
/// 误解（`\\?\` → `%3F` 只是最显眼的症状，反斜杠本身也会在下游 markdown
/// 渲染、URI 拼接、JS 字符串展开里被吃掉或重新百分号编码）。
///
/// Normalizes any `Path` into the string form that gets fed to agents
/// (env-vars, prompt arguments, JSON payloads). Always replaces `\\`
/// with `/` so `Context`'s path fields are POSIX-shaped on every
/// platform — Windows fs APIs accept forward slashes natively, so the
/// conversion is lossless. The point is to keep backslashes out of
/// markdown / URI / JSON contexts downstream, where they get eaten,
/// double-escaped, or percent-encoded (the `\\?\` → `%3F` symptom is
/// just the loudest member of this family).
fn path_to_agent_string(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// 把候选路径解析为绝对路径字符串：先选定 `candidate`，缺失则用 cwd 兜底，
/// 最后尝试 `canonicalize`，不存在时返回原路径以保持向后兼容。
///
/// 使用 `dunce::canonicalize` 而非 `std::fs::canonicalize`：在 Windows 上
/// 会在不必要时剥离 `\\?\` verbatim 前缀，避免该前缀进入 `ROOT_DIR` 等
/// env-var、再被下游消费方（VuePress、浏览器、URI 拼接代码等）把 `?`
/// 编码为 `%3F`，产生 `file://%3F\D:\...` 这种畸形路径。
///
/// Uses `dunce::canonicalize` instead of `std::fs::canonicalize`: on
/// Windows it strips the `\\?\` verbatim prefix when not strictly
/// required, so that prefix never leaks into `ROOT_DIR` and other agent
/// env-vars where downstream consumers may URI-encode `?` into `%3F`
/// and yield malformed `file://%3F\D:\...` paths.
fn canonicalize_or_cwd(candidate: Option<PathBuf>) -> Result<PathBuf> {
    let path = match candidate {
        Some(path) => path,
        None => env::current_dir().context("failed to resolve current directory")?,
    };
    Ok(dunce::canonicalize(&path).unwrap_or(path))
}

/// 由已解析的根目录构建完整 `Context`。
fn build_context(root_dir: String) -> Result<Context> {
    let username = resolve_username(&root_dir)?;
    let language = resolve_language(&root_dir);
    let issue_candidates_file =
        path_to_agent_string(&paths::issue_candidates_file_path(&root_dir, &username));
    let record_issue_candidate_prompt =
        path_to_agent_string(&prompt_path(&root_dir, "record-issue-candidate.md"));
    let summarize_issue_candidates_prompt =
        path_to_agent_string(&prompt_path(&root_dir, "summarize-issue-candidates.md"));
    let tdd_protocol_prompt = path_to_agent_string(&prompt_path(&root_dir, "tdd-protocol.md"));
    let new_prompt = path_to_agent_string(&prompt_path(&root_dir, "hotpot-new.md"));
    let execute_prompt = path_to_agent_string(&prompt_path(&root_dir, "hotpot-execute.md"));
    let finish_work_prompt = path_to_agent_string(&prompt_path(&root_dir, "hotpot-finish-work.md"));

    // VuePress 三字段：enabled 始终输出作为 hook / bootstrap 公共契约字段
    // （AI 实际走分支用的是 hotpot-new.md 里的 file-existence gate，不再依
    // 赖该 env-var），port/url 仅启用态填充——禁用时为空串以触发
    // `skip_serializing_if`，从 JSON 中省略，防止下游把陈旧端口当成活动状态。
    //
    // VuePress trio: `enabled` is always serialized as the public hook /
    // bootstrap contract field — the AI's actual branch decision is now
    // driven by the prompt file-existence gate in `hotpot-new.md`, not by
    // this env-var. `port`/`url` are populated only when enabled,
    // otherwise left empty so `skip_serializing_if` drops them —
    // preventing stale values from being treated as live.
    let vuepress_enabled_bool = resolve_vuepress_enabled(&root_dir);
    let vuepress_enabled = if vuepress_enabled_bool {
        "true"
    } else {
        "false"
    }
    .to_string();
    let (vuepress_port, vuepress_url) = if vuepress_enabled_bool {
        let port = resolve_vuepress_port(&root_dir);
        (port.to_string(), format!("http://localhost:{port}"))
    } else {
        (String::new(), String::new())
    };

    Ok(Context {
        root_dir,
        username,
        language,
        issue_candidates_file,
        record_issue_candidate_prompt,
        summarize_issue_candidates_prompt,
        tdd_protocol_prompt,
        new_prompt,
        execute_prompt,
        finish_work_prompt,
        vuepress_enabled,
        vuepress_port,
        vuepress_url,
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

/// 只在 trim 后非空时返回 `Some`。
///
/// Generic "trim + non-empty" gate, shared by username / language /
/// future free-form string resolutions. Lifting it from
/// `normalize_username` to `normalize_string` keeps a single place to
/// audit what "blank value" means across all resolution chains.
///
/// 把"空白即空"的判定收敛到一个函数，方便统一审计 username、language
/// 以及未来其他自由文本字段的回退规则。
fn normalize_string(value: Option<String>) -> Option<String> {
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
            Ok(normalize_string(Some(stdout)))
        }
        Ok(_) | Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    //! `resolve_language*` 单元测试。
    //!
    //! 全部 env-mutating 测试通过 `env_lock()` 串行化（同 task/mod.rs 风格），
    //! 避免 cargo 默认并发让 `HOTPOT_LANGUAGE` 互相污染。配置文件写入到
    //! `env::temp_dir()` 下的唯一目录，避免与真实 `.hotpot/config.toml` 冲突。
    //!
    //! Unit tests for the language resolver. Env-mutating tests are
    //! serialized through `env_lock()` to keep `HOTPOT_LANGUAGE` writes
    //! from racing across cargo's parallel runner. Each test materializes
    //! its own `.hotpot/config.toml` in `env::temp_dir()` so it cannot
    //! collide with the real project state.
    use std::{
        fs,
        path::PathBuf,
        sync::{Mutex, MutexGuard},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    /// 串行化所有触碰 `HOTPOT_LANGUAGE` 的测试。
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 创建唯一临时项目根目录。
    fn unique_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!("hotpot-lang-{label}-{nanos}"));
        fs::create_dir_all(dir.join(".hotpot")).unwrap();
        dir
    }

    /// 在 `<root>/.hotpot/config.toml` 写指定原始内容。
    fn write_config(root: &PathBuf, contents: &str) {
        fs::write(root.join(".hotpot/config.toml"), contents).unwrap();
    }

    /// SAFETY: callers hold `env_lock()`; 2024 edition flags env mutators
    /// as unsafe due to global state.
    unsafe fn clear_lang_env() {
        unsafe { env::remove_var("HOTPOT_LANGUAGE") };
    }

    #[test]
    fn resolves_language_from_env_override() {
        let _guard = env_lock();
        let root = unique_root("env-override");
        // 无 config.toml，仅靠 env 命中。
        unsafe { env::set_var("HOTPOT_LANGUAGE", "简体中文") };

        let (lang, source) = resolve_language_with_source(&root.display().to_string());
        assert_eq!(lang, "简体中文");
        assert_eq!(source, LanguageSource::Env);

        unsafe { clear_lang_env() };
    }

    #[test]
    fn resolves_language_from_config_toml_top_level() {
        let _guard = env_lock();
        unsafe { clear_lang_env() };
        let root = unique_root("config-toml");
        write_config(&root, "language = \"日本語\"\n");

        let (lang, source) = resolve_language_with_source(&root.display().to_string());
        assert_eq!(lang, "日本語");
        assert_eq!(source, LanguageSource::ConfigToml);
    }

    #[test]
    fn commented_out_default_yields_english() {
        let _guard = env_lock();
        unsafe { clear_lang_env() };
        let root = unique_root("commented-default");
        // 模拟 hotpot-config.toml 模板原状：language 行被注释。
        write_config(
            &root,
            "# Hotpot project configuration.\n# language = \"English\"\n",
        );

        let (lang, source) = resolve_language_with_source(&root.display().to_string());
        assert_eq!(lang, "English");
        assert_eq!(source, LanguageSource::Default);
    }

    #[test]
    fn empty_value_yields_english() {
        let _guard = env_lock();
        unsafe { clear_lang_env() };
        let root = unique_root("empty-value");
        write_config(&root, "language = \"\"\n");

        let (lang, source) = resolve_language_with_source(&root.display().to_string());
        assert_eq!(lang, "English");
        assert_eq!(source, LanguageSource::Default);
    }

    #[test]
    fn corrupt_toml_yields_english_no_panic() {
        let _guard = env_lock();
        unsafe { clear_lang_env() };
        let root = unique_root("corrupt-toml");
        // 故意写入未闭合引号的 TOML，确认解析器 None 之后我们回退到 English。
        write_config(&root, "language = \"unterminated\nfoo = bar baz\n");

        let (lang, source) = resolve_language_with_source(&root.display().to_string());
        assert_eq!(lang, "English");
        assert_eq!(source, LanguageSource::Default);
    }

    #[test]
    fn env_beats_config() {
        let _guard = env_lock();
        let root = unique_root("env-beats-config");
        write_config(&root, "language = \"日本語\"\n");
        unsafe { env::set_var("HOTPOT_LANGUAGE", "Français") };

        let (lang, source) = resolve_language_with_source(&root.display().to_string());
        assert_eq!(lang, "Français");
        assert_eq!(source, LanguageSource::Env);

        unsafe { clear_lang_env() };
    }

    #[test]
    fn whitespace_is_trimmed() {
        let _guard = env_lock();
        unsafe { clear_lang_env() };
        let root = unique_root("whitespace");
        write_config(&root, "language = \"   简体中文   \"\n");

        let (lang, _source) = resolve_language_with_source(&root.display().to_string());
        assert_eq!(lang, "简体中文");
    }

    #[test]
    fn missing_file_yields_english() {
        let _guard = env_lock();
        unsafe { clear_lang_env() };
        let root = unique_root("missing-file");
        // 不写 config.toml，跑解析。
        let (lang, source) = resolve_language_with_source(&root.display().to_string());
        assert_eq!(lang, "English");
        assert_eq!(source, LanguageSource::Default);
    }

    #[test]
    fn context_uses_global_issue_candidates_file() {
        let _guard = env_lock();
        unsafe {
            clear_lang_env();
            env::remove_var("HOTPOT_USERNAME");
        }
        let root = unique_root("global-candidates");

        let context = Context::resolve(Some(root)).unwrap();

        assert!(
            context
                .issue_candidates_file
                .ends_with("/.hotpot/issue-candidates.jsonl"),
            "unexpected candidates path: {}",
            context.issue_candidates_file
        );
        assert!(
            !context.issue_candidates_file.contains("/workspaces/"),
            "candidates path must be project-global: {}",
            context.issue_candidates_file
        );
    }

    #[test]
    fn ensure_issue_candidates_file_migrates_legacy_candidates() {
        let _guard = env_lock();
        unsafe {
            clear_lang_env();
            env::remove_var("HOTPOT_USERNAME");
        }
        let root = unique_root("ensure-migrates-candidates");
        let root_dir = root.display().to_string();
        let legacy = root.join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(
            &legacy,
            r#"{"created_at":"2026-05-19T00:00:00Z","reason":"legacy","changed_files":["src/context.rs"],"keywords":["migration"],"problem":"legacy candidate invisible","fix":"migrate during bootstrap ensure","validation":["cargo test"],"promote_hint":"migration regression"}"#,
        )
        .unwrap();

        let context = Context::resolve(Some(root.clone())).unwrap();
        context.ensure_issue_candidates_file().unwrap();

        let global = root.join(".hotpot/issue-candidates.jsonl");
        let content = fs::read_to_string(&global).unwrap();
        assert!(
            content.contains("legacy candidate invisible"),
            "legacy candidate should be migrated into global file, got: {content}"
        );
        assert_eq!(fs::read_to_string(&legacy).unwrap(), "");

        context.ensure_issue_candidates_file().unwrap();
        let non_empty_lines = fs::read_to_string(&global)
            .unwrap()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count();
        assert_eq!(non_empty_lines, 1, "migration must be idempotent");
        assert!(
            context
                .issue_candidates_file
                .ends_with("/.hotpot/issue-candidates.jsonl")
        );
        assert!(root_dir.contains("hotpot-lang-ensure-migrates-candidates"));
    }

    #[test]
    fn non_string_value_yields_english() {
        let _guard = env_lock();
        unsafe { clear_lang_env() };
        let root = unique_root("non-string");
        write_config(&root, "language = 42\n");

        let (lang, source) = resolve_language_with_source(&root.display().to_string());
        assert_eq!(lang, "English");
        assert_eq!(source, LanguageSource::Default);
    }

    /// SAFETY: caller holds `env_lock()`; env mutators are unsafe in 2024 edition.
    unsafe fn clear_vuepress_env() {
        unsafe {
            env::remove_var("HOTPOT_VUEPRESS_ENABLED");
            env::remove_var("HOTPOT_VUEPRESS_PORT");
        }
    }

    #[test]
    fn vuepress_enabled_from_env_override() {
        let _guard = env_lock();
        let root = unique_root("vp-enabled-env");
        unsafe { env::set_var("HOTPOT_VUEPRESS_ENABLED", "true") };

        let (enabled, source) = resolve_vuepress_enabled_with_source(&root.display().to_string());
        assert!(enabled);
        assert_eq!(source, VuepressEnabledSource::Env);

        unsafe { clear_vuepress_env() };
    }

    #[test]
    fn vuepress_enabled_from_config_toml() {
        let _guard = env_lock();
        unsafe { clear_vuepress_env() };
        let root = unique_root("vp-enabled-config");
        write_config(&root, "[vuepress]\nenabled = true\nport = 8080\n");

        let (enabled, source) = resolve_vuepress_enabled_with_source(&root.display().to_string());
        assert!(enabled);
        assert_eq!(source, VuepressEnabledSource::ConfigToml);
    }

    #[test]
    fn vuepress_enabled_defaults_to_false() {
        let _guard = env_lock();
        unsafe { clear_vuepress_env() };
        let root = unique_root("vp-enabled-default");
        // 既无 env 也无 config.toml，应回退到字面量 false。
        let (enabled, source) = resolve_vuepress_enabled_with_source(&root.display().to_string());
        assert!(!enabled);
        assert_eq!(source, VuepressEnabledSource::Default);
    }

    #[test]
    fn vuepress_enabled_env_case_insensitive() {
        let _guard = env_lock();
        let root = unique_root("vp-enabled-case");
        unsafe { env::set_var("HOTPOT_VUEPRESS_ENABLED", "  TRUE  ") };

        let (enabled, _source) = resolve_vuepress_enabled_with_source(&root.display().to_string());
        assert!(enabled);

        unsafe { clear_vuepress_env() };
    }

    #[test]
    fn vuepress_enabled_garbage_env_falls_through() {
        let _guard = env_lock();
        let root = unique_root("vp-enabled-garbage");
        // 模拟用户把 enabled 写成 "yes"——不在 {"true","false"} 集合内应当
        // 当作 None，让链路落到 config.toml；这里 config.toml 也缺，最终为
        // Default(false)。
        write_config(&root, "# no vuepress table\n");
        unsafe { env::set_var("HOTPOT_VUEPRESS_ENABLED", "yes") };

        let (enabled, source) = resolve_vuepress_enabled_with_source(&root.display().to_string());
        assert!(!enabled);
        assert_eq!(source, VuepressEnabledSource::Default);

        unsafe { clear_vuepress_env() };
    }

    #[test]
    fn vuepress_port_from_env_override() {
        let _guard = env_lock();
        let root = unique_root("vp-port-env");
        unsafe { env::set_var("HOTPOT_VUEPRESS_PORT", "9527") };

        let (port, source) = resolve_vuepress_port_with_source(&root.display().to_string());
        assert_eq!(port, 9527);
        assert_eq!(source, VuepressPortSource::Env);

        unsafe { clear_vuepress_env() };
    }

    #[test]
    fn vuepress_port_from_config_toml() {
        let _guard = env_lock();
        unsafe { clear_vuepress_env() };
        let root = unique_root("vp-port-config");
        write_config(&root, "[vuepress]\nenabled = true\nport = 4321\n");

        let (port, source) = resolve_vuepress_port_with_source(&root.display().to_string());
        assert_eq!(port, 4321);
        assert_eq!(source, VuepressPortSource::ConfigToml);
    }

    #[test]
    fn vuepress_port_defaults_to_8080() {
        let _guard = env_lock();
        unsafe { clear_vuepress_env() };
        let root = unique_root("vp-port-default");
        let (port, source) = resolve_vuepress_port_with_source(&root.display().to_string());
        assert_eq!(port, VUEPRESS_PORT_DEFAULT);
        assert_eq!(source, VuepressPortSource::Default);
    }

    #[test]
    fn vuepress_port_out_of_range_falls_through() {
        let _guard = env_lock();
        unsafe { clear_vuepress_env() };
        let root = unique_root("vp-port-overflow");
        // 70000 超出 u16 范围，应当被解析器拒绝并继续走链路；config.toml
        // 也无该字段，最终落到 Default(8080)。
        write_config(&root, "[vuepress]\nport = 70000\n");

        let (port, source) = resolve_vuepress_port_with_source(&root.display().to_string());
        assert_eq!(port, VUEPRESS_PORT_DEFAULT);
        assert_eq!(source, VuepressPortSource::Default);
    }
}
