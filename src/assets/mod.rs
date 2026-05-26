//! Static asset catalog and install engine.
//!
//! This module owns everything Hotpot ships into a target project:
//! the [`Asset`] abstraction, every per-platform `ASSETS` registry, the
//! cross-platform [`SHARED_ASSETS`] catalog, the merge engine, and the
//! install dispatcher. Commands consume this module via [`install_for`],
//! [`detect_installed_platforms`], and [`shared_prompts`]; they should
//! **not** construct [`Asset`] values themselves — every declaration
//! lives in [`SHARED_ASSETS`] or in [`platforms`] so the catalog stays
//! the single source of truth.
//!
//! Each [`Asset`] declares an [`InstallStrategy`]:
//! - [`InstallStrategy::Owned`] — Hotpot-private files (single-file
//!   overwrite semantics; `--force` toggles overwrite on differing
//!   existing files).
//! - [`InstallStrategy::MergeJson`] / [`InstallStrategy::MergeToml`] /
//!   [`InstallStrategy::MergeText`] — platform main-config files that
//!   must coexist with user-authored content. The merge helpers in
//!   [`merge`] do an idempotent deep-merge or anchored line-block
//!   replacement.
//! - [`InstallStrategy::CreateIfMissing`] — user-owned config seeds
//!   that are only written on first install.
//!
//! 资源 catalog 与安装引擎。模块内集中维护 Hotpot 安装到目标项目的所有
//! 文件：[`Asset`] 抽象、各平台的 `ASSETS` 数组、跨平台共享的
//! [`SHARED_ASSETS`]、merge 引擎与安装分派器。其他命令通过
//! [`install_for`]、[`detect_installed_platforms`]、[`shared_prompts`]
//! 消费；**禁止**在模块外构造 [`Asset`]——所有声明都集中在
//! [`SHARED_ASSETS`] 或 [`platforms`] 子模块，保证 catalog 是唯一真相源。

mod merge;
mod platforms;
mod shared;
mod vuepress_hub;
mod vuepress_opt_in;

use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

pub use platforms::Platform;
pub(crate) use shared::SHARED_ASSETS;

/// Statistics returned by a single [`install_for`] call.
///
/// 单次 [`install_for`] 调用的返回统计。
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
pub(crate) enum InstallStrategy {
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
    /// 时跳过且 `--force` 也不覆盖。
    CreateIfMissing,
}

/// Static asset installed by Hotpot.
///
/// Hotpot 安装的静态资产单元。
pub(crate) struct Asset {
    pub(crate) path: &'static str,
    pub(crate) content: &'static str,
    pub(crate) strategy: InstallStrategy,
}

impl Asset {
    /// Constructs an [`InstallStrategy::Owned`] asset (Hotpot-private file).
    pub(crate) const fn owned(path: &'static str, content: &'static str) -> Self {
        Self {
            path,
            content,
            strategy: InstallStrategy::Owned,
        }
    }

    /// Constructs an [`InstallStrategy::MergeJson`] asset (JSON main-config).
    pub(crate) const fn merge_json(path: &'static str, content: &'static str) -> Self {
        Self {
            path,
            content,
            strategy: InstallStrategy::MergeJson,
        }
    }

    /// Constructs an [`InstallStrategy::MergeToml`] asset (TOML main-config).
    pub(crate) const fn merge_toml(path: &'static str, content: &'static str) -> Self {
        Self {
            path,
            content,
            strategy: InstallStrategy::MergeToml,
        }
    }

    /// Constructs an [`InstallStrategy::MergeText`] asset (line-block text
    /// file with `# hotpot:begin` / `# hotpot:end` anchors).
    pub(crate) const fn merge_text(path: &'static str, content: &'static str) -> Self {
        Self {
            path,
            content,
            strategy: InstallStrategy::MergeText,
        }
    }

    /// Constructs an [`InstallStrategy::CreateIfMissing`] asset (user-owned
    /// config seed that is never overwritten on re-install).
    pub(crate) const fn create_if_missing(path: &'static str, content: &'static str) -> Self {
        Self {
            path,
            content,
            strategy: InstallStrategy::CreateIfMissing,
        }
    }
}

/// Installs the asset groups associated with `platform` under `project_dir`.
///
/// Public entry point shared by `hotpot init` and `hotpot update`: pass a
/// concrete [`Platform`] (or [`Platform::All`]) and the helper iterates
/// through the matching groups, installing each asset via [`install_one`].
/// Pass `verbose=true` to keep the legacy `write {path}` /
/// `skip unchanged {path}` per-asset stdout lines; `false` keeps stdout
/// silent so the caller can render its own summary (e.g.
/// `hotpot update --json`).
///
/// 安装指定平台的资产到 `project_dir`，供 `hotpot init` 与 `hotpot update` 共用。
/// `verbose=true` 保留逐资产 stdout 输出；`false` 静默以便调用方自行渲染。
pub fn install_for(
    project_dir: &Path,
    platform: Platform,
    force: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<InstallStats> {
    let mut stats = InstallStats::default();
    for group in platforms::asset_groups(&platform) {
        for asset in *group {
            let result = install_one(project_dir, asset, force, dry_run, verbose)?;
            match result {
                InstallResult::Written => stats.written.push(asset.path.to_string()),
                InstallResult::Skipped => stats.skipped.push(asset.path.to_string()),
            }
        }
    }

    // One-shot retirement of deprecated Pi prompt-template thin shells. The
    // asset engine only installs/refreshes; it does not retire removed
    // assets, so this runs whenever the Pi platform is in scope and we are
    // not in dry-run mode. Idempotent: missing paths are skipped silently.
    //
    // 废弃 Pi prompt thin shell 的一次性清理。资产引擎只装不卸，所以
    // 在 Pi（或 All）平台范围内、非 dry-run 时主动调用 cleanup。幂等：
    // 文件不在则跳过。
    if !dry_run && matches!(platform, Platform::Pi | Platform::All) {
        platforms::cleanup_deprecated_pi_prompts(project_dir)?;
    }

    Ok(stats)
}

/// Detects which platforms have already been initialized under `project_dir`
/// by checking each platform's `config_dir`. Returns concrete platforms only
/// (never [`Platform::All`]); order matches [`Platform::ALL_CONCRETE`].
///
/// 通过检查每个平台的配置目录判断哪些平台已被初始化；返回具体平台列表，
/// 不含 `All`，顺序与 [`Platform::ALL_CONCRETE`] 一致。
pub fn detect_installed_platforms(project_dir: &Path) -> Vec<Platform> {
    Platform::ALL_CONCRETE
        .iter()
        .copied()
        .filter(|p| match p.config_dir() {
            Some(dir) => project_dir.join(dir).is_dir(),
            None => false,
        })
        .collect()
}

/// Installs the VuePress hub template under `<project_dir>/.hotpot-hub/`.
///
/// Thin wrapper around [`install_one`] driven by
/// [`vuepress_hub::VUEPRESS_HUB_ASSETS`]. Used by `hotpot vuepress install`
/// (and indirectly by `hotpot init --enable-vuepress`) to deploy the
/// VuePress project template that backs the dev-server preview. Idempotent:
/// re-running over an existing hub skips files whose content already
/// matches; differing files bail unless `force` is set.
///
/// **Does NOT run `pnpm install`** — the caller is responsible for that
/// step (and for `sync_tasks_links` / writing `[vuepress] enabled = true`
/// to `.hotpot/config.toml`). Splitting concerns this way keeps the
/// asset-install path identical to the existing platform installs.
///
/// 把 VuePress hub 模板部署到 `<project_dir>/.hotpot-hub/`。`hotpot vuepress
/// install` 与 `hotpot init --enable-vuepress` 共用此入口。幂等：内容相同
/// 跳过；内容不同时 `force=false` 会 bail。**不**负责 `pnpm install` /
/// `sync_tasks_links` / 写 `config.toml`——这些由调用方编排。
pub fn install_vuepress_hub(
    project_dir: &Path,
    force: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<InstallStats> {
    install_assets_slice(
        project_dir,
        vuepress_hub::VUEPRESS_HUB_ASSETS,
        force,
        dry_run,
        verbose,
    )
}

/// Installs the VuePress opt-in prompt assets under
/// `<project_dir>/.hotpot/prompts/`.
///
/// Thin wrapper around [`install_one`] driven by
/// [`vuepress_opt_in::VUEPRESS_OPT_IN_ASSETS`]. Called by
/// `hotpot vuepress install` (and indirectly by
/// `hotpot init --enable-vuepress`) after the hub deployment succeeds.
/// Mirrors [`install_vuepress_hub`] semantics: idempotent re-runs skip
/// matching content; differing files bail unless `force` is set.
///
/// Keeping these assets **out** of [`SHARED_ASSETS`] is what guarantees
/// disabled projects have a clean prompt directory with no VuePress
/// references in the AI's context. The file-existence gate in
/// `hotpot-new.md` relies on this absence to keep the disabled branch
/// silent: when both files are missing, the AI's Bash probe returns
/// `disabled` and the VuePress section is skipped.
///
/// 仅启用 VuePress 时安装 opt-in prompt 资产到 `.hotpot/prompts/`。
/// 通过这种 opt-in 安排，禁用项目里 `.hotpot/prompts/` 不含任何
/// vuepress 相关文件，`hotpot-new.md` 的 file-existence gate 探测到
/// 文件不在盘上即返回 `disabled`，自然跳过 VuePress 段。
pub fn install_vuepress_prompts(
    project_dir: &Path,
    force: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<InstallStats> {
    install_assets_slice(
        project_dir,
        vuepress_opt_in::VUEPRESS_OPT_IN_ASSETS,
        force,
        dry_run,
        verbose,
    )
}

/// Returns the project-relative paths registered in
/// [`vuepress_opt_in::VUEPRESS_OPT_IN_ASSETS`].
///
/// `hotpot vuepress uninstall` iterates over these to delete each
/// installed opt-in prompt, keeping the cleanup in sync with the
/// install catalog without a duplicate hardcoded list in `vuepress.rs`.
///
/// 返回 [`vuepress_opt_in::VUEPRESS_OPT_IN_ASSETS`] 中所有资产的项目
/// 相对路径，供 `hotpot vuepress uninstall` 精确反向删除。
pub fn vuepress_prompt_paths() -> impl Iterator<Item = &'static str> {
    vuepress_opt_in::VUEPRESS_OPT_IN_ASSETS
        .iter()
        .map(|asset| asset.path)
}

/// Shared body for slice-driven installs.
///
/// Hidden helper: kept module-private so external callers go through the
/// purpose-named wrappers ([`install_vuepress_hub`] etc.) and Asset
/// declarations remain centralized in [`shared`] / [`platforms`] /
/// [`vuepress_hub`].
///
/// 内部辅助：保持私有，让外部调用走具名包装函数，资产声明仍集中在
/// `shared` / `platforms` / `vuepress_hub` 三处。
fn install_assets_slice(
    project_dir: &Path,
    assets: &[Asset],
    force: bool,
    dry_run: bool,
    verbose: bool,
) -> Result<InstallStats> {
    let mut stats = InstallStats::default();
    for asset in assets {
        let result = install_one(project_dir, asset, force, dry_run, verbose)?;
        match result {
            InstallResult::Written => stats.written.push(asset.path.to_string()),
            InstallResult::Skipped => stats.skipped.push(asset.path.to_string()),
        }
    }
    Ok(stats)
}

/// Returns the filenames (relative to `.hotpot/prompts/`) of every shared
/// prompt currently in [`SHARED_ASSETS`].
///
/// Consumers like `hotpot update` use this to drive health checks without
/// maintaining a duplicate list, so the catalog stays the single source of
/// truth: adding a `Asset::owned(".hotpot/prompts/<name>.md", ...)` entry
/// to [`SHARED_ASSETS`] is sufficient to make `hotpot update` cover it.
///
/// 返回 [`SHARED_ASSETS`] 中所有 `.hotpot/prompts/<name>.md` 资产的文件名。
/// 给 `hotpot update` 等消费者推导校验清单用，让 catalog 成为唯一真相源——
/// 添加一份新 prompt 只需在 [`SHARED_ASSETS`] 登记即可，无需同步其它列表。
pub fn shared_prompts() -> impl Iterator<Item = &'static str> {
    SHARED_ASSETS
        .iter()
        .filter_map(|asset| asset.path.strip_prefix(".hotpot/prompts/"))
}

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
/// - `MergeJson` / `MergeToml` / `MergeText`: if the target doesn't exist,
///   write `asset.content` verbatim; otherwise compute the merged content
///   via the [`merge`] module and write it. If the merged content equals
///   the existing bytes, skip.
/// - `CreateIfMissing`: existing → skip (even with `--force`); missing →
///   write `asset.content` verbatim.
///
/// 单文件安装入口。私有文件维持原语义，主配置文件做幂等合并。`--force`
/// 不影响 Merge* 文件的合并行为（合并本身就是非破坏的），也不会覆盖
/// `CreateIfMissing` 已落地的用户文件。
fn install_one(
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
            // 用户自有配置：已存在则跳过，--force 也不覆盖。沿用 "skip
            // unchanged" 输出让操作者看到 seed 被识别。
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
    // Skip writes when merged content equals existing bytes to avoid
    // mtime churn.
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
