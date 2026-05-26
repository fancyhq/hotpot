//! `hotpot init` command.
//!
//! Thin CLI shell that delegates to [`crate::assets`] for the actual catalog
//! and install engine. The asset declarations, merge logic, and platform
//! registries live entirely in that module so other commands (e.g.
//! `hotpot update`, or future resource-import commands) can reuse them
//! without going through `commands::init`.
//!
//! `hotpot init` 命令的 CLI 入口。本文件只承担参数解析与人类可读摘要，
//! 真正的资产 catalog 与安装引擎都在 [`crate::assets`]，便于其他命令
//! （`hotpot update`、未来的资源导入命令等）共用。

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use crate::assets::{self, Platform};
use crate::context;
use crate::vuepress;
use crate::workspace;

/// Install Hotpot platform assets and shared prompts into the project.
///
/// 把指定平台的资产与跨平台共享 prompt 安装到目标项目目录（幂等）。
#[derive(Args, Debug)]
#[command(
    about = "Install platform assets and shared prompts into the project",
    long_about = None
)]
pub struct InitArgs {
    /// Platform assets to install.
    #[arg(
        long,
        value_enum,
        default_value = "all",
        help = "Platform assets to install",
        long_help = None
    )]
    platform: Platform,

    /// Project directory to initialize.
    #[arg(
        long = "project-dir",
        default_value = ".",
        help = "Project directory to initialize",
        long_help = None
    )]
    project_dir: PathBuf,

    /// Overwrite existing Hotpot-private files when their contents differ.
    /// Merge-strategy files (platform main-config) always merge regardless
    /// of this flag.
    #[arg(
        long,
        help = "Overwrite existing Hotpot-private files when their contents differ. \
                 Merge-strategy files always merge regardless of this flag.",
        long_help = None
    )]
    force: bool,

    /// Print planned writes without modifying files.
    #[arg(long = "dry-run", help = "Print planned writes without modifying files", long_help = None)]
    dry_run: bool,

    /// Enable VuePress integration (`hotpot vuepress install`) at the
    /// end of init without prompting. Useful for CI / scripted setups.
    /// Without this flag, init asks interactively on a TTY and skips
    /// VuePress on a non-TTY stdin.
    ///
    /// 启动 VuePress 集成（等价于在 init 末尾跑 `hotpot vuepress install`），
    /// 跳过交互询问。适合 CI / 脚本场景。不传此 flag 时，TTY 下会交互
    /// 询问，非 TTY 默认跳过。
    #[arg(
        long = "enable-vuepress",
        help = "Enable VuePress integration (equivalent to `hotpot vuepress install`) at the end of init",
        long_help = None
    )]
    enable_vuepress: bool,
}

/// Installs platform assets into the target project directory.
///
/// 把指定平台的资产安装到目标项目目录。
pub fn init(args: InitArgs) -> Result<()> {
    // Prefer `dunce::canonicalize` so the resulting absolute path never
    // carries the Windows `\\?\` verbatim prefix; keeps init output in
    // sync with the cleaned paths emitted by `hotpot hook bootstrap`.
    // 用 `dunce::canonicalize` 规避 Windows 的 `\\?\` verbatim 前缀，
    // 保持与 `hotpot hook bootstrap` 等下游输出一致的「干净」绝对路径。
    let project_dir =
        dunce::canonicalize(&args.project_dir).unwrap_or_else(|_| args.project_dir.clone());

    let stats = assets::install_for(&project_dir, args.platform, args.force, args.dry_run, true)?;

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

    if matches!(args.platform, Platform::Opencode | Platform::All) {
        println!("OpenCode plugin dependencies are declared in .opencode/package.json.");
    }

    // ── Workspace skeleton + optional VuePress install ──────────────────
    // Disk-write steps; both gated on `!dry_run` so `--dry-run` keeps its
    // "no filesystem mutation" contract intact. The workspace skeleton
    // step is unconditional so the post-init project state matches the
    // user's mental model — i.e. `.hotpot/workspaces/<username>/` is
    // already in place before any subsequent `task create` or
    // `vuepress install` consumes it. This also lets `install_hub`'s
    // docs-symlink step always find its source directory instead of
    // falling back to its best-effort "missing workspace, link later"
    // branch.
    //
    // Workspace 骨架与可选 VuePress 安装（写盘步骤；`--dry-run` 一律跳过）。
    // Workspace 骨架在 init 主路径里无条件建立，让 init 后的项目状态
    // 与用户直觉一致——`.hotpot/workspaces/<username>/` 在任何后续
    // `task create` 或 `vuepress install` 消费它之前已经就位，install_hub
    // 的软链步骤也能稳定找到源目录，不必走"workspace 缺失，下次再补软链"
    // 的兜底分支。
    if !args.dry_run {
        let root_dir = project_dir.display().to_string();
        let username = context::resolve_username(&root_dir)?;
        workspace::ensure_workspace_skeleton(&root_dir, &username)?;
        println!("Workspace ready: .hotpot/workspaces/{username}/");

        if resolve_enable_vuepress(args.enable_vuepress) {
            let port = context::resolve_vuepress_port(&root_dir);
            vuepress::install_hub(&root_dir, port, args.force)?;
        }
    }

    Ok(())
}

/// Decides whether init should install VuePress at the end.
///
/// `--enable-vuepress` always wins; otherwise prompt on a TTY (default
/// No); silently skip on a non-TTY so unattended runs don't deadlock.
///
/// 决定 init 末尾是否启用 VuePress 集成。决策树：`--enable-vuepress`
/// flag 显式启用 → `true`；否则 stdin 是 TTY → 走交互询问（默认 No）；
/// 非 TTY 且无 flag → `false`（CI 安全默认）。
fn resolve_enable_vuepress(flag: bool) -> bool {
    if flag {
        return true;
    }
    if !io::stdin().is_terminal() {
        return false;
    }
    prompt_yes_no("Enable VuePress (browser preview for task files)?", false)
}

/// Bilingual TTY yes/no prompt.
///
/// Accepts `y`/`yes` / `n`/`no` (any case); falls back to `default`
/// for empty / unrecognized input and any IO error so callers can keep
/// this synchronous and infallible.
///
/// 控制台 yes/no 询问。`default=true` 时回车视为 Yes，否则 No。
/// `y` / `yes`（任意大小写）视为 Yes；`n` / `no` 视为 No；其它输入
/// 沿用 `default`。读 stdin 失败也回退 `default`，保证函数不会 panic。
fn prompt_yes_no(question: &str, default: bool) -> bool {
    let suffix = if default { "[Y/n]" } else { "[y/N]" };
    print!("{question} {suffix}: ");
    let _ = io::stdout().flush();

    let mut line = String::new();
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    if handle.read_line(&mut line).is_err() {
        return default;
    }
    match line.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
    }
}
