//! Hotpot initialization command implementation.
//!
//! Writes embedded assets into the target project directory, grouped by
//! platform. Cross-platform shared prompts (currently `assets/prompts/`) are
//! installed via [`SHARED_ASSETS`] under every `--platform` selection so the
//! four platform `init/<platform>.rs::ASSETS` arrays don't have to register
//! them individually.
//!
//! Each [`Asset`] declares an [`InstallStrategy`]:
//! - [`InstallStrategy::Owned`] — Hotpot-private files (single-file overwrite
//!   semantics; `--force` toggles overwrite on differing existing files).
//! - [`InstallStrategy::MergeJson`] / [`InstallStrategy::MergeToml`] —
//!   platform main-config files that must coexist with user-authored content.
//!   The merge helpers in [`merge`] do an idempotent deep-merge, identifying
//!   hotpot-owned array entries via known anchors.
//!
//! 把内嵌的资产写入目标项目目录，按平台分组安装。跨平台共享的资产（目前为
//! `assets/prompts/` 下的 LLM 提示词）通过 [`SHARED_ASSETS`] 统一注入到
//! Hotpot 命名空间 `.hotpot/prompts/` 下，与项目根隔离。
//!
//! 每个 [`Asset`] 声明一个安装策略：私有文件（[`InstallStrategy::Owned`]）
//! 维持原「整文件写入 + `--force` 覆盖差异」语义；平台主配置文件（JSON /
//! TOML）走 [`InstallStrategy::MergeJson`] / [`InstallStrategy::MergeToml`]，
//! 由 [`merge`] 模块做幂等深合并，通过锚点识别 hotpot 段以避免重复 append。

mod claude;
mod codex;
mod merge;
mod opencode;
mod pi;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};

/// Install Hotpot platform assets and shared prompts into the project.
///
/// 把指定平台的资产与跨平台共享 prompt 安装到目标项目目录（幂等）。
#[derive(Args, Debug)]
pub struct InitArgs {
    /// Platform assets to install.
    #[arg(long, value_enum, default_value = "all")]
    platform: InitPlatform,

    /// Project directory to initialize.
    #[arg(long = "project-dir", default_value = ".")]
    project_dir: PathBuf,

    /// Overwrite existing Hotpot-private files when their contents differ.
    /// Merge-strategy files (platform main-config) always merge regardless
    /// of this flag.
    #[arg(long)]
    force: bool,

    /// Print planned writes without modifying files.
    #[arg(long = "dry-run")]
    dry_run: bool,
}

/// Supported platform targets for `hotpot init`.
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum InitPlatform {
    Opencode,
    Claude,
    Codex,
    Pi,
    All,
}

impl InitPlatform {
    /// Returns the project-relative directory each platform's config lives
    /// under, used by [`detect_installed_platforms`] to figure out which
    /// platforms an existing project has already been initialized for.
    /// `All` is a synthetic selector and intentionally returns `None`.
    ///
    /// 返回每个平台的项目相对目录名；`All` 是综合选项，返回 `None`。
    pub fn config_dir(&self) -> Option<&'static str> {
        match self {
            InitPlatform::Opencode => Some(".opencode"),
            InitPlatform::Claude => Some(".claude"),
            InitPlatform::Codex => Some(".codex"),
            InitPlatform::Pi => Some(".pi"),
            InitPlatform::All => None,
        }
    }

    /// Human-readable platform identifier used in summary / warning output.
    ///
    /// 人类可读的平台标识，用于摘要 / 警告输出。
    pub fn slug(&self) -> &'static str {
        match self {
            InitPlatform::Opencode => "opencode",
            InitPlatform::Claude => "claude",
            InitPlatform::Codex => "codex",
            InitPlatform::Pi => "pi",
            InitPlatform::All => "all",
        }
    }

    /// All concrete platforms (`All` excluded), in a stable display order.
    ///
    /// 所有具体平台（排除 `All`），按稳定顺序返回。
    pub const ALL_CONCRETE: &'static [InitPlatform] = &[
        InitPlatform::Claude,
        InitPlatform::Opencode,
        InitPlatform::Codex,
        InitPlatform::Pi,
    ];
}

/// Statistics returned by a single `install_assets_for` call.
///
/// 单次 `install_assets_for` 调用的返回统计。
#[derive(Debug, Default)]
pub struct InstallStats {
    /// Project-relative asset paths that were (or would be, in `--dry-run`) written.
    /// 本次实际写入（或 dry-run 下预定写入）的资产相对路径。
    pub written: Vec<String>,
    /// Project-relative asset paths skipped because content already matches.
    /// 因内容已一致而跳过的资产相对路径。
    pub skipped: Vec<String>,
}

impl InstallStats {
    /// Merges another stats result into this one. Used to aggregate across
    /// multiple platform installs in `hotpot update`.
    ///
    /// 把另一份统计合并进当前统计，用于 `hotpot update` 跨平台聚合。
    pub fn extend(&mut self, other: InstallStats) {
        self.written.extend(other.written);
        self.skipped.extend(other.skipped);
    }
}

/// Per-asset install strategy.
///
/// 资产安装策略。Hotpot 私有文件走 [`Owned`]，平台主配置文件走对应的
/// `Merge*` 变体。
#[derive(Clone, Copy)]
enum InstallStrategy {
    /// Hotpot-private file. Existing-and-equal → skip; existing-and-differs
    /// → bail (or overwrite when `--force`). Single-file write semantics.
    Owned,
    /// Platform main-config file in JSON form. Always deep-merged into the
    /// user's existing content via [`merge::merge_json`].
    MergeJson,
    /// Platform main-config file in TOML form. Always merged via
    /// [`merge::merge_toml`], which preserves user comments and key order.
    MergeToml,
    /// Plain-text file with a hotpot-managed line block. Always merged via
    /// [`merge::merge_text`], which only rewrites the region between
    /// `# hotpot:begin` and `# hotpot:end` anchor lines; user-authored
    /// content outside the markers is preserved byte-for-byte.
    ///
    /// 文本文件（如 `.gitignore`）专用：只重写 `# hotpot:begin` /
    /// `# hotpot:end` 锚点之间的内容，锚点外用户行字节不变。
    MergeText,
    /// User-owned config seed. Writes `asset.content` only when the target
    /// is missing; existing files are left untouched (and reported as
    /// skipped). `--force` does NOT override this — the strategy exists
    /// specifically so subsequent `hotpot init` / `hotpot update` runs
    /// never clobber user customizations.
    ///
    /// 用户自有配置的"首次播种"策略：目标文件缺失时写入模板，已存在
    /// 时跳过且 `--force` 也不覆盖。`.hotpot/config.toml` 这类用户可改
    /// 文件用此策略，保证 init / update 不会覆写用户的自定义值。
    CreateIfMissing,
}

/// Static asset installed by `hotpot init`.
struct Asset {
    path: &'static str,
    content: &'static str,
    strategy: InstallStrategy,
}

impl Asset {
    /// Constructs an [`InstallStrategy::Owned`] asset (Hotpot-private file).
    pub(super) const fn owned(path: &'static str, content: &'static str) -> Self {
        Self {
            path,
            content,
            strategy: InstallStrategy::Owned,
        }
    }

    /// Constructs an [`InstallStrategy::MergeJson`] asset (JSON main-config).
    pub(super) const fn merge_json(path: &'static str, content: &'static str) -> Self {
        Self {
            path,
            content,
            strategy: InstallStrategy::MergeJson,
        }
    }

    /// Constructs an [`InstallStrategy::MergeToml`] asset (TOML main-config).
    pub(super) const fn merge_toml(path: &'static str, content: &'static str) -> Self {
        Self {
            path,
            content,
            strategy: InstallStrategy::MergeToml,
        }
    }

    /// Constructs an [`InstallStrategy::MergeText`] asset (line-block text
    /// file with `# hotpot:begin` / `# hotpot:end` anchors).
    pub(super) const fn merge_text(path: &'static str, content: &'static str) -> Self {
        Self {
            path,
            content,
            strategy: InstallStrategy::MergeText,
        }
    }

    /// Constructs an [`InstallStrategy::CreateIfMissing`] asset (user-owned
    /// config seed that is never overwritten on re-install).
    pub(super) const fn create_if_missing(path: &'static str, content: &'static str) -> Self {
        Self {
            path,
            content,
            strategy: InstallStrategy::CreateIfMissing,
        }
    }
}

/// Installs platform assets into the target project directory.
pub fn init(args: InitArgs) -> Result<()> {
    let project_dir = args
        .project_dir
        .canonicalize()
        .unwrap_or(args.project_dir.clone());

    let stats = install_assets_for(&project_dir, args.platform, args.force, args.dry_run, true)?;

    let mode = if args.dry_run {
        "would install"
    } else {
        "installed"
    };
    println!(
        "Hotpot init {mode} {} file(s), skipped {} unchanged file(s).",
        stats.written.len(),
        stats.skipped.len()
    );

    if matches!(args.platform, InitPlatform::Opencode | InitPlatform::All) {
        println!("OpenCode plugin dependencies are declared in .opencode/package.json.");
    }

    Ok(())
}

/// Installs the asset groups associated with `platform` under `project_dir`.
///
/// Public entry point shared by `hotpot init` and `hotpot update`: pass a
/// concrete `InitPlatform` (or `All`) and the helper iterates through the
/// matching groups, installing each asset via [`install_asset`]. Pass
/// `verbose=true` to keep the legacy `write {path}` / `skip unchanged {path}`
/// per-asset stdout lines; `false` keeps stdout silent so the caller can
/// render its own summary (e.g. `hotpot update --json`).
///
/// 安装指定平台的资产到 `project_dir`，供 `hotpot init` 与 `hotpot update` 共用。
/// `verbose=true` 保留逐资产 stdout 输出；`false` 静默以便调用方自行渲染。
pub fn install_assets_for(
    project_dir: &Path,
    platform: InitPlatform,
    force: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<InstallStats> {
    let mut stats = InstallStats::default();
    for group in asset_groups(&platform) {
        for asset in *group {
            let result = install_asset(project_dir, asset, force, dry_run, verbose)?;
            match result {
                InstallResult::Written => stats.written.push(asset.path.to_string()),
                InstallResult::Skipped => stats.skipped.push(asset.path.to_string()),
            }
        }
    }
    Ok(stats)
}

/// Detects which platforms have already been initialized under `project_dir`
/// by checking each platform's `config_dir`. Returns concrete platforms only
/// (never `All`); order matches [`InitPlatform::ALL_CONCRETE`].
///
/// 通过检查每个平台的配置目录判断哪些平台已被初始化；返回具体平台列表，
/// 不含 `All`，顺序与 [`InitPlatform::ALL_CONCRETE`] 一致。
pub fn detect_installed_platforms(project_dir: &Path) -> Vec<InitPlatform> {
    InitPlatform::ALL_CONCRETE
        .iter()
        .copied()
        .filter(|p| match p.config_dir() {
            Some(dir) => project_dir.join(dir).is_dir(),
            None => false,
        })
        .collect()
}

/// Returns the asset groups requested by a platform selection.
fn asset_groups(platform: &InitPlatform) -> &'static [&'static [Asset]] {
    match platform {
        InitPlatform::Opencode => OPENCODE_GROUPS,
        InitPlatform::Claude => CLAUDE_GROUPS,
        InitPlatform::Codex => CODEX_GROUPS,
        InitPlatform::Pi => PI_GROUPS,
        InitPlatform::All => ALL_GROUPS,
    }
}

/// Cross-platform assets installed alongside every `--platform` selection.
///
/// These are the LLM prompts that the four platforms reference at runtime via
/// either `@.hotpot/prompts/...` expansion (Claude/OpenCode) or the
/// `$HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT` / `$HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`
/// / `$HOTPOT_TDD_PROTOCOL_PROMPT` / `$HOTPOT_NEW_PROMPT` / `$HOTPOT_EXECUTE_PROMPT`
/// / `$HOTPOT_FINISH_WORK_PROMPT` environment variables (Codex/Pi). The
/// runtime path is resolved by `src/context.rs::prompt_path` as
/// `<ROOT_DIR>/.hotpot/prompts/<name>.md`, so the install target lives under
/// the Hotpot-owned `.hotpot/` namespace rather than the project root.
///
/// 跨平台共享资产：四个平台的 new / execute / finish-work 编排都依赖这几份
/// 提示词（Claude/OpenCode 走 `@.hotpot/prompts/...`，Codex/Pi 走环境变量），
/// 而运行时路径由 `src/context.rs::prompt_path` 硬编码为
/// `<ROOT_DIR>/.hotpot/prompts/<name>.md`——和 `.hotpot/issues.jsonl`
/// 等内部文件同属 Hotpot 命名空间，不放到用户项目根。
const SHARED_ASSETS: &[Asset] = &[
    Asset::owned(
        ".hotpot/prompts/get-issue.md",
        include_str!("../../../assets/prompts/get-issue.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/record-issue-candidate.md",
        include_str!("../../../assets/prompts/record-issue-candidate.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/summarize-issue-candidates.md",
        include_str!("../../../assets/prompts/summarize-issue-candidates.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/tdd-protocol.md",
        include_str!("../../../assets/prompts/tdd-protocol.md"),
    ),
    // 跨工作流共用的「按 `.hotpot/config.toml::language` 决定自然语言输出」
    // 指令。四份主工作流 prompt (hotpot-new / hotpot-execute / hotpot-finish-work /
    // tdd-protocol) 都通过 `@.hotpot/prompts/output-language.md`（Claude/OpenCode）
    // 或 `$ROOT_DIR/.hotpot/prompts/output-language.md`（Codex/Pi）引用。
    //
    // Shared "respect `.hotpot/config.toml::language` for natural-language
    // output" directive. The four main workflow prompts (hotpot-new /
    // hotpot-execute / hotpot-finish-work / tdd-protocol) reference this file
    // via `@.hotpot/prompts/output-language.md` (Claude/OpenCode) or the
    // resolved `$ROOT_DIR/.hotpot/prompts/output-language.md` (Codex/Pi).
    Asset::owned(
        ".hotpot/prompts/output-language.md",
        include_str!("../../../assets/prompts/output-language.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/hotpot-new.md",
        include_str!("../../../assets/prompts/hotpot-new.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/hotpot-execute.md",
        include_str!("../../../assets/prompts/hotpot-execute.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/hotpot-finish-work.md",
        include_str!("../../../assets/prompts/hotpot-finish-work.md"),
    ),
    // 项目根 .gitignore：用「# hotpot:begin / # hotpot:end」锚点行做行块合并。
    // 锚点外用户内容字节保留；锚点之间的内容由 hotpot 完全管理。
    Asset::merge_text(
        ".gitignore",
        include_str!("../../../assets/templates/gitignore.hotpot"),
    ),
    // `.hotpot/config.toml`：用户自有配置文件，首次 init 时播种一份带英文
    // 注释的模板说明可用参数；之后的 init / update 都不会覆盖用户的修改。
    //
    // `.hotpot/config.toml`: user-owned config seed. Hotpot writes a fully
    // commented template on first install so the user can see every
    // available parameter; subsequent runs skip the file unconditionally so
    // edits survive re-init / update.
    Asset::create_if_missing(
        ".hotpot/config.toml",
        include_str!("../../../assets/templates/hotpot-config.toml"),
    ),
];

const OPENCODE_GROUPS: &[&[Asset]] = &[opencode::ASSETS, SHARED_ASSETS];
const CLAUDE_GROUPS: &[&[Asset]] = &[claude::ASSETS, SHARED_ASSETS];
const CODEX_GROUPS: &[&[Asset]] = &[codex::ASSETS, SHARED_ASSETS];
const PI_GROUPS: &[&[Asset]] = &[pi::ASSETS, SHARED_ASSETS];
const ALL_GROUPS: &[&[Asset]] = &[
    opencode::ASSETS,
    claude::ASSETS,
    codex::ASSETS,
    pi::ASSETS,
    SHARED_ASSETS,
];

/// Result of installing an individual asset.
enum InstallResult {
    Written,
    Skipped,
}

/// Installs a single asset.
///
/// Dispatches on [`InstallStrategy`]:
/// - `Owned`: existing-and-equal → skip; existing-and-differs without
///   `--force` → bail; otherwise write `asset.content` verbatim.
/// - `MergeJson` / `MergeToml`: if the target doesn't exist, write
///   `asset.content` verbatim; otherwise compute the merged content via the
///   [`merge`] module and write it. If the merged content equals the existing
///   bytes, skip.
///
/// 单文件安装入口。私有文件维持原语义，主配置文件做幂等合并。`--force`
/// 不影响 Merge* 文件的合并行为（合并本身就是非破坏的）。
fn install_asset(
    project_dir: &Path,
    asset: &Asset,
    force: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<InstallResult> {
    let target = project_dir.join(asset.path);

    let existing: Option<String> = if target.exists() {
        Some(
            fs::read_to_string(&target)
                .with_context(|| format!("failed to read existing {}", target.display()))?,
        )
    } else {
        None
    };

    let next_content: String = match asset.strategy {
        InstallStrategy::Owned => match existing.as_deref() {
            Some(cur) if cur == asset.content => {
                if verbose {
                    println!("skip unchanged {}", asset.path);
                }
                return Ok(InstallResult::Skipped);
            }
            Some(_) if !force => {
                bail!(
                    "{} already exists and differs; rerun with --force to overwrite",
                    target.display()
                );
            }
            _ => asset.content.to_string(),
        },
        InstallStrategy::MergeJson => match existing.as_deref() {
            Some(cur) => merge::merge_json(cur, asset.content, &target)?,
            None => asset.content.to_string(),
        },
        InstallStrategy::MergeToml => match existing.as_deref() {
            Some(cur) => merge::merge_toml(cur, asset.content, &target)?,
            None => asset.content.to_string(),
        },
        InstallStrategy::MergeText => match existing.as_deref() {
            Some(cur) => merge::merge_text(cur, asset.content, &target)?,
            None => asset.content.to_string(),
        },
        InstallStrategy::CreateIfMissing => match existing.as_deref() {
            // User-owned seed: never touch an existing file, even with --force.
            // The skip is reported via the standard "skip unchanged" line so
            // operators can see the seed was acknowledged.
            // 用户自有配置：已存在则跳过，--force 也不覆盖。沿用"skip
            // unchanged"输出让操作者看到 seed 被识别。
            Some(_) => {
                if verbose {
                    println!("skip existing {}", asset.path);
                }
                return Ok(InstallResult::Skipped);
            }
            None => asset.content.to_string(),
        },
    };

    // 合并/写入结果与现状字节相同时跳过，避免无意义的 mtime 抖动。
    if existing.as_deref() == Some(next_content.as_str()) {
        if verbose {
            println!("skip unchanged {}", asset.path);
        }
        return Ok(InstallResult::Skipped);
    }

    if dry_run {
        if verbose {
            println!("write {}", asset.path);
        }
        return Ok(InstallResult::Written);
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(&target, &next_content)
        .with_context(|| format!("failed to write {}", target.display()))?;
    if verbose {
        println!("write {}", asset.path);
    }
    Ok(InstallResult::Written)
}
