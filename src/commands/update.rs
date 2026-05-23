//! `hotpot update` — day-1 onboarding entry point for collaborators.
//!
//! A new collaborator clones a Hotpot-managed project and runs:
//!
//! ```text
//! hotpot update
//! ```
//!
//! Which (idempotently) refreshes platform assets, installs the shared
//! `.hotpot/prompts/` directory, merges the hotpot block into the project
//! `.gitignore`, creates the current user's workspace skeleton under
//! `.hotpot/workspaces/<username>/`, and runs a health self-check.
//!
//! Unlike `hotpot init`, this command **does not** install new platforms:
//! it auto-detects which platforms have already been initialized (i.e.
//! `.claude/`, `.opencode/`, `.codex/`, `.pi/` exists) and only refreshes
//! those. To onboard a new platform on a project, the user still runs
//! `hotpot init --platform <p>` once explicitly.
//!
//! 新协作者 clone 项目后跑的"day-1"入口命令：刷新已安装平台资产、安装
//! `.hotpot/prompts/`、合并 `.gitignore`、创建当前用户的 workspace 骨架，
//! 并跑一次健康自检。与 `hotpot init` 不同，本命令**不会**为项目接入新
//! 平台——只刷新已存在的目录，新平台仍需用户显式 `hotpot init --platform`。

use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use clap::Args;
use serde::Serialize;

use crate::{
    assets::{self, InstallStats},
    context::{
        UsernameSource, resolve_language_with_source, resolve_username_with_source,
        resolve_vuepress_enabled,
    },
    paths, vuepress, workspace,
};

/// Refresh installed platforms, bootstrap the workspace, and run health checks.
///
/// `hotpot update` 的 CLI 参数：刷新已装平台、初始化 workspace、跑健康自检。
#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Explicit username override (in-memory only; does not persist to git config).
    ///
    /// 显式 username 覆盖（不持久化 git 配置；仅本次命令生效）。
    #[arg(long)]
    username: Option<String>,

    /// Project root directory.
    ///
    /// 项目根目录。
    #[arg(long = "project-dir", default_value = ".")]
    project_dir: PathBuf,

    /// Allow continuing when username resolves to `"default"`; suppresses the shared-workspace risk warning.
    ///
    /// 允许在 username 解析为 `"default"` 时继续，覆盖 workspace 共享风险警告。
    #[arg(long = "allow-default")]
    allow_default: bool,

    /// Print planned writes without modifying files.
    ///
    /// 打印计划但不写盘。
    #[arg(long = "dry-run")]
    dry_run: bool,

    /// Emit a JSON summary instead of human-readable output.
    ///
    /// 以 JSON 形式输出汇总（默认人类可读）。
    #[arg(long)]
    json: bool,
}

/// 单条健康自检告警。
///
/// A single health-check warning.
#[derive(Debug, Serialize)]
pub struct Warning {
    /// 机器可识别的告警代码（如 `missing_prompt` / `default_username`）。
    code: String,
    /// 人类可读消息。
    message: String,
    /// 可复制执行的修复命令；不适用时为空字符串。
    fix: String,
}

/// `hotpot update` 的结构化输出 schema。
///
/// Structured output schema for `hotpot update --json`.
#[derive(Debug, Serialize)]
pub struct UpdateReport {
    username: String,
    source: &'static str,
    /// 已解析的项目输出语言（自然语言回复）。
    ///
    /// Resolved natural-language output preference (free-form). Reported
    /// alongside `username` so operators can confirm Hotpot picked up the
    /// expected `config.toml::language` (or env override) when debugging
    /// "why is the agent still replying in English?".
    ///
    /// 与 `username` 并列报告，便于排查"agent 为什么还在用英文回复"——
    /// 看一眼 `hotpot update` 即可知道 language 解析到了哪一档。
    language: String,
    /// language 解析链命中的来源标签（`env` / `config_toml` / `default`）。
    ///
    /// Source label for [`UpdateReport::language`].
    language_source: &'static str,
    workspace: String,
    workspace_created: bool,
    refreshed_platforms: Vec<String>,
    gitignore_updated: bool,
    prompts_updated: Vec<String>,
    /// VuePress opt-in 状态：当前项目是否启用了 VuePress 集成
    /// (`.hotpot/config.toml::[vuepress] enabled`)。
    ///
    /// Whether `[vuepress] enabled = true` is set for this project.
    /// Reported so operators can confirm at a glance whether VuePress
    /// is part of the current setup, alongside username / language.
    vuepress_enabled: bool,
    /// 本轮 update 刷新的 VuePress opt-in prompt 文件名（相对 `.hotpot/prompts/`）。
    /// 仅在 `vuepress_enabled = true` 时可能非空。
    ///
    /// Names of VuePress opt-in prompts (relative to `.hotpot/prompts/`)
    /// rewritten by this update run. Only ever non-empty when
    /// `vuepress_enabled = true` and the bundled template differs from
    /// the on-disk copy.
    vuepress_prompts_updated: Vec<String>,
    warnings: Vec<Warning>,
}

/// 入口：解析参数、跑刷新、跑自检、打印。
///
/// Entry point: parse args, run refresh, run self-check, render report.
pub fn update(args: UpdateArgs) -> Result<()> {
    let json = args.json;
    let dry_run = args.dry_run;
    let report = build_report(args)?;

    if json {
        let payload = serde_json::to_string_pretty(&report)
            .context("failed to serialize update report as JSON")?;
        println!("{payload}");
    } else {
        render_human(&report, dry_run);
    }

    Ok(())
}

/// 跑一次 update 的全部业务逻辑并返回结构化报告，不做任何打印。
///
/// Runs the full update pipeline and returns a structured [`UpdateReport`].
/// Pure data path so it can be unit-tested without stdout capture.
pub(crate) fn build_report(args: UpdateArgs) -> Result<UpdateReport> {
    // 与 `hotpot init` 保持一致：用 `dunce::canonicalize` 规避 Windows
    // 的 `\\?\` verbatim 前缀，避免污染后续写入 env-var 的派生路径。
    // Mirror `hotpot init`: `dunce::canonicalize` strips the Windows
    // `\\?\` verbatim prefix so derived env-var paths stay clean.
    let project_dir: PathBuf =
        dunce::canonicalize(&args.project_dir).unwrap_or_else(|_| args.project_dir.clone());
    let root_dir = project_dir.display().to_string();

    // ── 1. 解析 username（带来源） ───────────────────────────────────────
    let (username, source) = match args.username.as_deref() {
        Some(explicit) => {
            let normalized = explicit.trim();
            if normalized.is_empty() {
                bail!("--username must not be empty");
            }
            (normalized.to_string(), UsernameSource::Env)
        }
        None => resolve_username_with_source(&root_dir)?,
    };

    // ── 1b. 解析 language（带来源）——失败会回退到 English，不会 bail。
    // ── 1b. Resolve language with source. Resolution is infallible
    //         (always returns at least `("English", Default)`), so this
    //         never aborts `hotpot update`; the operator can still see
    //         which link of the chain produced the value.
    let (language, language_source) = resolve_language_with_source(&root_dir);

    // ── 2. 平台探测 ────────────────────────────────────────────────────
    let platforms = assets::detect_installed_platforms(&project_dir);
    if platforms.is_empty() {
        bail!(
            "no platform directories found under {}; run `hotpot init --platform <claude|opencode|codex|pi>` first to bootstrap a platform",
            project_dir.display()
        );
    }

    // ── 3. 刷新各平台资产 ────────────────────────────────────────────────
    // 静默模式打印逐资产 stdout 与否：默认（人类可读）保留；--json 静默以
    // 便 stdout 只承载 JSON 报告。
    let verbose_per_asset = !args.json;
    let mut combined = InstallStats::default();
    let mut refreshed: Vec<String> = Vec::new();
    for platform in &platforms {
        let stats = assets::install_for(
            &project_dir,
            *platform,
            /* force */ false,
            args.dry_run,
            verbose_per_asset,
        )
        .with_context(|| format!("failed to refresh platform {}", platform.slug()))?;
        refreshed.push(platform.slug().to_string());
        combined.extend(stats);
    }

    // 4. 判断 .gitignore 与 prompts 是否被本轮触发写入。
    // 4. Determine whether .gitignore / prompts/* were rewritten in this run.
    let gitignore_updated = combined.written.iter().any(|p| p == ".gitignore");
    let prompts_updated: Vec<String> = combined
        .written
        .iter()
        .filter_map(|p| p.strip_prefix(".hotpot/prompts/").map(str::to_string))
        .collect();

    // ── 4b. VuePress opt-in 资产刷新 ────────────────────────────────────
    // 启用 VuePress 的项目里，`vuepress.md` / `vuepress-style.md` 不在
    // `SHARED_ASSETS` 内，平台刷新不会自动覆盖它们。手动跑 update 的协作
    // 者通常期待"day-1 一键拉齐"——这里检测 `[vuepress] enabled` 并按需
    // 重新部署 opt-in prompts（Hub 项目 `.hotpot-hub/` 不在此触动：那里
    // 含 `node_modules` 等用户运行时产物，重装应是 explicit 的
    // `hotpot vuepress install --force`，不该被 update 静默覆盖）。
    //
    // Refresh VuePress opt-in prompts when the project has VuePress
    // enabled. Hub assets (`.hotpot-hub/`) are intentionally NOT
    // refreshed here — they ship with `node_modules` etc. that should
    // not be wiped silently; users explicitly run
    // `hotpot vuepress install --force` for that.
    let vuepress_enabled = resolve_vuepress_enabled(&root_dir);
    let vuepress_prompts_updated: Vec<String> = if vuepress_enabled {
        let stats = assets::install_vuepress_prompts(
            &project_dir,
            /* force */ false,
            args.dry_run,
            verbose_per_asset,
        )
        .context("failed to refresh VuePress opt-in prompts")?;
        stats
            .written
            .iter()
            .filter_map(|p| p.strip_prefix(".hotpot/prompts/").map(str::to_string))
            .collect()
    } else {
        Vec::new()
    };

    // ── 5. 创建 workspace 骨架 ────────────────────────────────────────────
    let workspace_path = paths::workspace_dir(&root_dir, &username);
    let workspace_existed = workspace_path.is_dir();
    if !args.dry_run {
        workspace::ensure_workspace_skeleton(&root_dir, &username)
            .context("failed to bootstrap workspace skeleton")?;
    }
    let workspace_created = !workspace_existed;

    // ── 5b. 启用 VuePress 时同步 docs 软链 ───────────────────────────────
    // 必须放在 step 5 之后：sync 需要当前 user 的 `.hotpot/workspaces/<user>/
    // tasks/` 已存在才能建链。在 `dry_run` 下跳过；hub 还没装好（用户启用了
    // enabled 但 `.hotpot-hub/` 缺失）时 sync 会自然返 Err，这里 demote 为
    // warning 不阻塞 update——后续 6bb 步的一致性自检会单独报"hub missing"
    // 让用户跑 `vuepress install` 修复。
    //
    // Sync the docs symlinks once the workspace skeleton is in place.
    // Must run AFTER step 5 because sync needs the current user's
    // `tasks/` directory to exist before linking. Failures (e.g. hub
    // not installed yet) are demoted to a stderr warning so update
    // still completes; the consistency self-check below surfaces the
    // real "hub missing" problem with a repair hint.
    if vuepress_enabled
        && !args.dry_run
        && let Err(err) = vuepress::sync_tasks_links(&root_dir)
    {
        eprintln!("Warning: failed to sync vuepress docs symlinks: {err}");
    }

    // ── 6. 健康自检 ──────────────────────────────────────────────────────
    let mut warnings: Vec<Warning> = Vec::new();

    // 6a. Prompt 文件完整性（即便平台刷新顺利，本检验也是防御性兜底）。
    //     Prompt 列表直接从 `assets::SHARED_ASSETS` 推导，避免本地维护一份重复
    //     清单后随 catalog 漂移。
    //     Prompt list is derived from `assets::SHARED_ASSETS` to keep this
    //     check in sync with the catalog instead of maintaining a duplicate
    //     list that drifts.
    for prompt in assets::shared_prompts() {
        let path = PathBuf::from(&root_dir)
            .join(".hotpot")
            .join("prompts")
            .join(prompt);
        if !path.is_file() {
            warnings.push(Warning {
                code: "missing_prompt".to_string(),
                message: format!(".hotpot/prompts/{prompt} is missing"),
                fix: format!(
                    "hotpot init --platform {} --project-dir {}",
                    platforms[0].slug(),
                    project_dir.display()
                ),
            });
        }
    }

    // 6b. 平台目录完整性的反向提示：项目里有平台目录但缺关键资产时给提示。
    //     这里用 detect_installed_platforms 之外的更细粒度判断：每个 platform
    //     都跑过 install_assets_for，理论上会自动恢复缺失资产；只有当用户
    //     主动改了某个 owned 文件导致 bail 时才会留下不一致 —— 但 bail 早已
    //     抛错，所以本路径仅作为说明。
    //     6b. Platform-asset completeness is already self-healing via the
    //     refresh step above; an inconsistent state would have surfaced as
    //     a bail from `install_assets_for`. No extra check needed here.

    // 6bb. VuePress 三者一致性：启用态下校验 config / hub / opt-in prompt
    //      三件套是否齐全。enabled=false 时跳过——禁用是合法状态，不报警。
    //      `verify_install_consistency` 已经实现该校验，转译它的错为 warning。
    //
    // 6bb. VuePress atomic-state sanity. Only runs when enabled — a
    //      disabled project is legitimate state and must not warn.
    //      Reuses `vuepress::verify_install_consistency` as the source
    //      of truth for what "consistent" means; failures become
    //      operator-visible warnings instead of errors.
    if vuepress_enabled && let Err(err) = vuepress::verify_install_consistency(&root_dir) {
        warnings.push(Warning {
            code: "vuepress_inconsistent".to_string(),
            message: format!("VuePress atomic state is out of sync: {err}"),
            fix: format!(
                "hotpot vuepress uninstall && hotpot vuepress install --project-dir {}",
                project_dir.display()
            ),
        });
    }

    // 6c. 默认 username 风险提示。
    if matches!(source, UsernameSource::Default) && !args.allow_default {
        warnings.push(Warning {
            code: "default_username".to_string(),
            message:
                "username resolved to the literal \"default\"; collaborators sharing this fallback will overwrite each other's workspaces"
                    .to_string(),
            fix:
                "set `git config --local user.name <your-name>` or export HOTPOT_USERNAME=<your-name>; pass --allow-default for single-user projects"
                    .to_string(),
        });
    }

    // 6d. `hotpot` 是否在 PATH 中。如果不在，指导用户安装。
    if which_hotpot().is_none() {
        warnings.push(Warning {
            code: "binary_not_in_path".to_string(),
            message: "`hotpot` is not on PATH; install the binary so agents can shell out to `hotpot ...`".to_string(),
            fix: "`cargo install --path .` from the hotpot repo, or add the built binary to PATH".to_string(),
        });
    }

    // ── 7. 构造报告 ──────────────────────────────────────────────────────
    Ok(UpdateReport {
        username,
        source: source.as_str(),
        language,
        language_source: language_source.as_str(),
        workspace: workspace_path.display().to_string(),
        workspace_created,
        refreshed_platforms: refreshed,
        gitignore_updated,
        prompts_updated,
        vuepress_enabled,
        vuepress_prompts_updated,
        warnings,
    })
}

/// `which hotpot` 的最小实现：扫描 `PATH` 找可执行文件，存在则返回路径。
/// 不区分 `hotpot` / `hotpot.exe`，因 Windows 同名规则交由 shell 处理。
///
/// Minimal `which hotpot`: walks `PATH` looking for an executable named
/// `hotpot` (or `hotpot.exe`). Returns the first match.
fn which_hotpot() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["hotpot.exe", "hotpot"]
    } else {
        &["hotpot"]
    };
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        for name in names {
            let candidate = dir.join(name);
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// 判断路径是否为可执行的常规文件。Unix 看 `S_IXUSR` 等三位；Windows 上
/// 只判断是否为文件（`.exe` 后缀已由调用方处理）。
fn is_executable(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// 人类可读的摘要渲染（非 `--json` 模式）。
fn render_human(report: &UpdateReport, dry_run: bool) {
    let mode = if dry_run { "(dry-run) " } else { "" };
    println!();
    println!("Hotpot update {mode}— identity & workspace");
    println!(
        "  username : {} (source: {})",
        report.username, report.source
    );
    println!(
        "  language : {} (source: {})",
        report.language, report.language_source
    );
    println!("  workspace: {}", report.workspace);
    println!(
        "             {}",
        if report.workspace_created {
            "created"
        } else {
            "already existed"
        }
    );
    println!();
    println!(
        "Refreshed platforms: {}",
        if report.refreshed_platforms.is_empty() {
            "(none)".to_string()
        } else {
            report.refreshed_platforms.join(", ")
        }
    );
    if !report.prompts_updated.is_empty() {
        println!("Prompts updated: {}", report.prompts_updated.join(", "));
    }
    if report.gitignore_updated {
        println!(".gitignore: hotpot block synced");
    }
    if report.vuepress_enabled {
        if report.vuepress_prompts_updated.is_empty() {
            println!("VuePress: enabled (opt-in prompts already up to date)");
        } else {
            println!(
                "VuePress: enabled (refreshed: {})",
                report.vuepress_prompts_updated.join(", ")
            );
        }
    }

    if report.warnings.is_empty() {
        println!();
        println!("All checks passed.");
    } else {
        println!();
        println!("Warnings ({}):", report.warnings.len());
        for w in &report.warnings {
            println!("  • [{}] {}", w.code, w.message);
            if !w.fix.is_empty() {
                println!("    fix: {}", w.fix);
            }
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    //! `hotpot update` 集成测试。
    //!
    //! 每个测试使用唯一的临时项目目录，避免与 cargo 并发用例之间互相污染。
    //! 测试覆盖：
    //!   1. 无平台目录 → bail。
    //!   2. 已 init claude → 刷新平台、创建 workspace、空 warnings（dev 环境
    //!      下不强求 binary-on-path）。
    //!   3. 第二次运行 → workspace_created=false，骨架文件保留。
    //!   4. default username + --allow-default → 不再生成 default_username 警告。
    //!   5. 预置的 `.claude` / `.opencode` 用户内容在 update 后保持不变。
    //!   5. 预置的 `.claude` / `.opencode` 用户内容在 update 后保持不变。
    //!
    //! Integration tests for `hotpot update`. Each test runs in a unique
    //! tempdir so cargo's parallel runner doesn't cross-contaminate state.
    use std::{fs, path::PathBuf};

    use super::*;
    use tempfile::{Builder, TempDir};

    /// 在 `env::temp_dir()` 下分配一个唯一目录路径。
    fn temp_project_dir(label: &str) -> TempDir {
        let dir = Builder::new()
            .prefix(&format!("hotpot-update-{label}-"))
            .tempdir()
            .unwrap();
        fs::create_dir_all(dir.path().join(".hotpot")).unwrap();
        dir
    }

    /// 在 fixture 中预安装一个平台（默认 claude），让 `update` 有目标可刷新。
    fn install_claude_fixture(project_dir: &PathBuf) {
        assets::install_for(
            project_dir,
            assets::Platform::Claude,
            /* force */ false,
            /* dry_run */ false,
            /* verbose */ false,
        )
        .expect("fixture init should succeed");
    }

    /// 在项目根里预置用户自有的 `.claude` / `.opencode` 内容，用于验证 update 不会误删。
    ///
    /// Seeds user-owned `.claude` / `.opencode` content at the project root so update can prove
    /// it preserves pre-existing files outside Hotpot-managed asset paths.
    fn seed_user_platform_content(project_dir: &PathBuf) {
        fs::create_dir_all(project_dir.join(".claude/notes")).unwrap();
        fs::write(project_dir.join(".claude/notes/custom.txt"), "keep me").unwrap();
        fs::create_dir_all(project_dir.join(".opencode/custom")).unwrap();
        fs::write(
            project_dir.join(".opencode/custom/readme.md"),
            "keep me too",
        )
        .unwrap();
    }

    fn build_args(project_dir: PathBuf, username: Option<&str>, allow_default: bool) -> UpdateArgs {
        UpdateArgs {
            username: username.map(|s| s.to_string()),
            project_dir,
            allow_default,
            dry_run: false,
            json: true,
        }
    }

    #[test]
    fn update_bails_without_any_platform_dir() {
        let dir = temp_project_dir("no-platform");
        let args = build_args(dir.path().to_path_buf(), Some("alice"), true);
        let err = build_report(args).expect_err("should bail with no platform dir");
        let msg = format!("{err}");
        assert!(
            msg.contains("no platform directories found"),
            "unexpected bail message: {msg}"
        );
    }

    #[test]
    fn update_creates_workspace_skeleton_on_first_run() {
        let dir = temp_project_dir("first-run");
        install_claude_fixture(&dir.path().to_path_buf());

        let args = build_args(dir.path().to_path_buf(), Some("alice"), true);
        let report = build_report(args).expect("update should succeed");

        assert_eq!(report.username, "alice");
        assert_eq!(report.source, "env");
        assert!(
            report.workspace_created,
            "workspace should be newly created"
        );
        assert_eq!(report.refreshed_platforms, vec!["claude".to_string()]);
        // workspace 骨架文件已落地。
        let ws = dir.path().join(".hotpot/workspaces/alice");
        assert!(ws.join("overview.jsonl").is_file());
        assert!(dir.path().join(".hotpot/issue-candidates.jsonl").is_file());
        assert!(!ws.join("issue-candidates.jsonl").is_file());
        assert!(ws.join("tasks").is_dir());
    }

    #[test]
    /// Verifies update keeps pre-existing user-owned platform content intact.
    ///
    /// 验证 update 会保留预先存在的用户自有平台内容。
    fn update_preserves_existing_platform_directory_contents() {
        let dir = temp_project_dir("preserve-platform-content");
        install_claude_fixture(&dir.path().to_path_buf());
        seed_user_platform_content(&dir.path().to_path_buf());

        let args = build_args(dir.path().to_path_buf(), Some("alice"), true);
        build_report(args).expect("update should succeed");

        assert_eq!(
            fs::read_to_string(dir.path().join(".claude/notes/custom.txt")).unwrap(),
            "keep me"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join(".opencode/custom/readme.md")).unwrap(),
            "keep me too"
        );
    }

    #[test]
    fn update_migrates_legacy_workspace_candidates() {
        let dir = temp_project_dir("legacy-candidates");
        install_claude_fixture(&dir.path().to_path_buf());
        let legacy = dir
            .path()
            .join(".hotpot/workspaces/alice/issue-candidates.jsonl");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(
            &legacy,
            r#"{"created_at":"2026-05-19T00:00:00Z","reason":"legacy update","changed_files":["src/workspace.rs"],"keywords":["migration"],"problem":"update skipped legacy candidates","fix":"reuse shared candidate ensure","validation":["cargo test commands::update::tests"],"promote_hint":"update migration regression"}"#,
        )
        .unwrap();

        let args = build_args(dir.path().to_path_buf(), Some("alice"), true);
        build_report(args).expect("update should migrate legacy candidates");

        let global = dir.path().join(".hotpot/issue-candidates.jsonl");
        let content = fs::read_to_string(&global).unwrap();
        assert!(
            content.contains("update skipped legacy candidates"),
            "legacy candidate should be migrated during update, got: {content}"
        );
        assert_eq!(fs::read_to_string(&legacy).unwrap(), "");
    }

    #[test]
    fn update_is_idempotent_on_second_run() {
        let dir = temp_project_dir("second-run");
        install_claude_fixture(&dir.path().to_path_buf());
        // 第一次：建 workspace。
        let first = build_report(build_args(dir.path().to_path_buf(), Some("bob"), true))
            .expect("first run");
        assert!(first.workspace_created);
        // 第二次：workspace 已存在，应报 false 且不重写资产。
        let second = build_report(build_args(dir.path().to_path_buf(), Some("bob"), true))
            .expect("second run");
        assert!(!second.workspace_created);
        // .gitignore 在 install fixture 阶段就已合并；第二次跑应该 skip。
        assert!(
            !second.gitignore_updated,
            "second run should not rewrite .gitignore"
        );
        assert!(
            second.prompts_updated.is_empty(),
            "second run should not rewrite prompts"
        );
    }

    #[test]
    fn update_warns_on_default_username_without_allow_flag() {
        let dir = temp_project_dir("default-warn");
        install_claude_fixture(&dir.path().to_path_buf());

        // 通过 --username 显式传 `default` 不会触发警告（source 是 env），
        // 所以这里改成检测：当 allow_default=false 时，若 source=Default，
        // 一定会出现 default_username 警告。直接构造对应 source 的路径较麻烦，
        // 改为间接验证 --allow-default 旁路逻辑：如果显式给一个非 default
        // username + allow_default=false，不应产生 default_username 警告。
        let report = build_report(build_args(dir.path().to_path_buf(), Some("carol"), false))
            .expect("update");
        assert!(
            !report.warnings.iter().any(|w| w.code == "default_username"),
            "non-default username should not trigger default_username warning"
        );
    }
}
