use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, ValueEnum};

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

#[derive(Clone, Debug, ValueEnum)]
enum InitPlatform {
    Opencode,
    Claude,
    Codex,
    All,
}

struct Asset {
    platform: InitPlatform,
    path: &'static str,
    content: &'static str,
}

const ASSETS: &[Asset] = &[
    Asset {
        platform: InitPlatform::Opencode,
        path: ".opencode/agents/hotpot-execution.md",
        content: include_str!("../../assets/platforms/opencode/agents/hotpot-execution.md"),
    },
    Asset {
        platform: InitPlatform::Opencode,
        path: ".opencode/agents/hotpot-review.md",
        content: include_str!("../../assets/platforms/opencode/agents/hotpot-review.md"),
    },
    Asset {
        platform: InitPlatform::Opencode,
        path: ".opencode/commands/hotpot/execute.md",
        content: include_str!("../../commands/execute.md"),
    },
    Asset {
        platform: InitPlatform::Opencode,
        path: ".opencode/commands/hotpot/new.md",
        content: include_str!("../../commands/new.md"),
    },
    Asset {
        platform: InitPlatform::Opencode,
        path: ".opencode/commands/hotpot/finish-work.md",
        content: include_str!("../../assets/platforms/opencode/commands/hotpot/finish-work.md"),
    },
    Asset {
        platform: InitPlatform::Opencode,
        path: ".opencode/plugins/bash-before.ts",
        content: include_str!("../../assets/platforms/opencode/plugins/bash-before.ts"),
    },
    Asset {
        platform: InitPlatform::Opencode,
        path: ".opencode/plugins/review-memory.ts",
        content: include_str!("../../assets/platforms/opencode/plugins/review-memory.ts"),
    },
    Asset {
        platform: InitPlatform::Opencode,
        path: ".opencode/package.json",
        content: include_str!("../../assets/platforms/opencode/package.json"),
    },
    Asset {
        platform: InitPlatform::Opencode,
        path: ".opencode/tsconfig.json",
        content: include_str!("../../assets/platforms/opencode/tsconfig.json"),
    },
    Asset {
        platform: InitPlatform::Claude,
        path: ".claude/agents/hotpot-execution.md",
        content: include_str!("../../assets/platforms/claude/agents/hotpot-execution.md"),
    },
    Asset {
        platform: InitPlatform::Claude,
        path: ".claude/agents/hotpot-review.md",
        content: include_str!("../../assets/platforms/claude/agents/hotpot-review.md"),
    },
    Asset {
        platform: InitPlatform::Claude,
        path: ".claude/commands/hotpot/execute.md",
        content: include_str!("../../commands/execute.md"),
    },
    Asset {
        platform: InitPlatform::Claude,
        path: ".claude/commands/hotpot/new.md",
        content: include_str!("../../commands/new.md"),
    },
    Asset {
        platform: InitPlatform::Codex,
        path: ".codex/agents/hotpot-execution.toml",
        content: include_str!("../../assets/platforms/codex/agents/hotpot-execution.toml"),
    },
    Asset {
        platform: InitPlatform::Codex,
        path: ".codex/agents/hotpot-review.toml",
        content: include_str!("../../assets/platforms/codex/agents/hotpot-review.toml"),
    },
];

pub fn init(args: InitArgs) -> Result<()> {
    let project_dir = args
        .project_dir
        .canonicalize()
        .unwrap_or(args.project_dir.clone());
    let mut written = 0usize;
    let mut skipped = 0usize;

    for asset in ASSETS
        .iter()
        .filter(|asset| should_install(&args.platform, &asset.platform))
    {
        match install_asset(&project_dir, asset, args.force, args.dry_run)? {
            InstallResult::Written => written += 1,
            InstallResult::Skipped => skipped += 1,
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

fn should_install(requested: &InitPlatform, asset_platform: &InitPlatform) -> bool {
    matches!(requested, InitPlatform::All)
        || std::mem::discriminant(requested) == std::mem::discriminant(asset_platform)
}

enum InstallResult {
    Written,
    Skipped,
}

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
