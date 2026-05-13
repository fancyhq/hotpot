//! Hotpot initialization command implementation.

mod claude;
mod codex;
mod opencode;
mod pi;

use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};

/// Arguments for the `hotpot init` command.
#[derive(Args, Debug)]
pub struct InitArgs {
    /// Platform assets to install.
    #[arg(long, value_enum, default_value = "all")]
    platform: InitPlatform,

    /// Project directory to initialize.
    #[arg(long = "project-dir", default_value = ".")]
    project_dir: PathBuf,

    /// Overwrite existing files when their contents differ.
    #[arg(long)]
    force: bool,

    /// Print planned writes without modifying files.
    #[arg(long = "dry-run")]
    dry_run: bool,
}

/// Supported platform targets for `hotpot init`.
#[derive(Clone, Debug, ValueEnum)]
enum InitPlatform {
    Opencode,
    Claude,
    Codex,
    Pi,
    All,
}

/// Static asset installed by `hotpot init`.
struct Asset {
    path: &'static str,
    content: &'static str,
}

/// Installs platform assets into the target project directory.
pub fn init(args: InitArgs) -> Result<()> {
    let project_dir = args
        .project_dir
        .canonicalize()
        .unwrap_or(args.project_dir.clone());
    let mut written = 0usize;
    let mut skipped = 0usize;

    for group in asset_groups(&args.platform) {
        for asset in *group {
            match install_asset(&project_dir, asset, args.force, args.dry_run)? {
                InstallResult::Written => written += 1,
                InstallResult::Skipped => skipped += 1,
            }
        }
    }

    let mode = if args.dry_run {
        "would install"
    } else {
        "installed"
    };
    println!(
        "Hotpot init {mode} {} file(s), skipped {} unchanged file(s).",
        written, skipped
    );

    if matches!(args.platform, InitPlatform::Opencode | InitPlatform::All) {
        println!("OpenCode plugin dependencies are declared in .opencode/package.json.");
    }

    Ok(())
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

const OPENCODE_GROUPS: &[&[Asset]] = &[opencode::ASSETS];
const CLAUDE_GROUPS: &[&[Asset]] = &[claude::ASSETS];
const CODEX_GROUPS: &[&[Asset]] = &[codex::ASSETS];
const PI_GROUPS: &[&[Asset]] = &[pi::ASSETS];
const ALL_GROUPS: &[&[Asset]] = &[opencode::ASSETS, claude::ASSETS, codex::ASSETS, pi::ASSETS];

/// Result of installing an individual asset.
enum InstallResult {
    Written,
    Skipped,
}

/// Installs a single asset, respecting `--force` and `--dry-run`.
fn install_asset(
    project_dir: &PathBuf,
    asset: &Asset,
    force: bool,
    dry_run: bool,
) -> Result<InstallResult> {
    let target = project_dir.join(asset.path);

    if target.exists() {
        let existing = fs::read_to_string(&target)
            .with_context(|| format!("failed to read existing {}", target.display()))?;
        if existing == asset.content {
            println!("skip unchanged {}", asset.path);
            return Ok(InstallResult::Skipped);
        }

        if !force {
            bail!(
                "{} already exists and differs; rerun with --force to overwrite",
                target.display()
            );
        }
    }

    if dry_run {
        println!("write {}", asset.path);
        return Ok(InstallResult::Written);
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(&target, asset.content)
        .with_context(|| format!("failed to write {}", target.display()))?;
    println!("write {}", asset.path);
    Ok(InstallResult::Written)
}
