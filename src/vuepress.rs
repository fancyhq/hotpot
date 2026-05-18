//! VuePress 集成的实现总入口。
//!
//! 模块包含：
//! - `assets/vuepress/` 模板部署到 `<project>/.hotpot-hub/` 的安装 / 卸载逻辑
//! - `.hotpot/config.toml::[vuepress]` 表的写入 / 关闭辅助
//! - 现有的 docs 软链工具（`sync_tasks_links` / `remove_vuepress_links` /
//!   `get_vuepress_user_dir_entries` / `get_repo_name`）
//! - `stop_if_running` 公共函数（供 `hotpot hook <platform> session-end`
//!   复用；当前为 stub，service start/stop 逻辑在阶段 2 后续 task 内填充）
//!
//! 设计约束：`enabled` 字段与 `.hotpot-hub/` 目录、`.hotpot/prompts/`
//! 下的 opt-in prompt 资产是绑定的**原子状态**。所有需要切换 enabled
//! 的操作都走 `install_hub` / `uninstall_hub`，不允许直接编辑 config.toml。
//!
//! VuePress integration entry point. Owns hub asset deployment, config
//! toggle helpers, the existing docs-symlink utilities, and the
//! `stop_if_running` hook shim. `enabled` is part of an atomic state
//! tied to the on-disk hub project and opt-in prompt assets — flipping
//! it without the install/uninstall path desyncs reality and is treated
//! as a config error by `verify_install_consistency` at start time.

use std::fs;
use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::assets;
use crate::paths::{hotpot_dir, hotpot_hub_dir};

/// Gets the repository name from the given root directory using `git remote -v`.
///
/// 通过 `git remote -v` 获取远程仓库名称
pub fn get_repo_name(root_dir: &str) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .current_dir(root_dir)
        .arg("remote")
        .arg("-v")
        .output()?;
    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to get repo name"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Failed to get repo name"))?;
    let url = first_line
        .split_whitespace()
        .find(|tok| tok.contains("git"))
        .ok_or_else(|| anyhow::anyhow!("No git-like token in remote line"))?;
    let last_segment = url.rsplit(['/', ':']).next().unwrap_or("");
    let repo_name = last_segment.trim_end_matches(".git").trim();
    if repo_name.is_empty() {
        return Err(anyhow::anyhow!("Failed to derive repo name from {url}"));
    }
    Ok(repo_name.to_string())
}

/// 获取 .hotpot-hub/docs 下所有的用户目录任务文件目录
pub fn get_vuepress_user_dir_entries(root_dir: &str) -> Vec<fs::DirEntry> {
    let vuepress_docs_dir = hotpot_hub_dir(root_dir).join("docs");
    if !vuepress_docs_dir.exists() {
        eprintln!("docs 目录不存在");
        return vec![];
    }
    let Ok(entries) = fs::read_dir(vuepress_docs_dir) else {
        return vec![];
    };
    entries
        .filter_map(|res| res.ok())
        .filter(|res| {
            res.path().is_dir() && !res.file_name().to_str().is_some_and(|f| f.starts_with("."))
        })
        .collect()
}

/// 把 `.hotpot/workspaces/<user>/tasks/` 同步软链到
/// `.hotpot-hub/docs/<user>/`，做幂等三件事：清掉 stale 链（hub docs 下
/// 有但 workspaces 里已无的 user）、保留已有链、为新加入的 user 建链。
///
/// 一致性保证（新用户加入场景）：
/// - 老用户的 link 已存在 → 跳过（保留 vuepress 可能的缓存关联）。
/// - workspaces 里新增的 user → 检测到 hub 缺对应 entry，按需创建。
/// - workspaces 里被删除的 user → 反向 diff 时清理对应 stale 链。
///
/// 调用入口：`hotpot vuepress install` 末段、`hotpot update` 在启用态
/// 的尾段都调用，使"新用户加入 → 跑 update → 自动获得软链"形成闭环。
///
/// Synchronizes `.hotpot/workspaces/<user>/tasks/` symlinks under
/// `.hotpot-hub/docs/<user>/` in three idempotent passes: prune stale
/// links (hub has an entry that workspaces no longer does), keep
/// existing links untouched, create missing links for newly-added
/// users. Called from `vuepress install` and the VuePress-enabled
/// branch of `hotpot update`, so a new collaborator gets their browse
/// view by simply running `hotpot update`.
pub fn sync_tasks_links(root_dir: &str) -> anyhow::Result<()> {
    let workspace_dir = hotpot_dir(root_dir).join("workspaces");
    let hub_docs_dir = hotpot_hub_dir(root_dir).join("docs");
    if !workspace_dir.exists() || !hub_docs_dir.exists() {
        return Err(anyhow::anyhow!("Not found workspace or hub directory."));
    }

    // (1) 当前 workspaces 下的 user 名集合（用 OsString 兼容非 UTF-8 路径）。
    // (1) Snapshot live users in workspaces; OsString covers non-UTF-8 names.
    let workspace_users: std::collections::HashSet<std::ffi::OsString> = fs::read_dir(&workspace_dir)?
        .filter_map(|res| res.ok())
        .filter(|res| {
            res.path().is_dir()
                && !res.file_name().to_str().is_some_and(|f| f.starts_with("."))
        })
        .map(|entry| entry.file_name())
        .collect();

    // (2) Prune stale：hub docs 下存在但 workspaces 已删除的 user link。
    //     单条 prune 失败不阻塞——继续处理其余 entry。
    // (2) Prune stale: hub entries whose user no longer exists in
    //     workspaces. Per-entry failures warn instead of aborting so a
    //     single orphaned permission denial doesn't block the rest.
    for entry in get_vuepress_user_dir_entries(root_dir) {
        if !workspace_users.contains(&entry.file_name()) {
            #[cfg(windows)]
            let result = fs::remove_dir(entry.path());
            #[cfg(unix)]
            let result = fs::remove_file(entry.path());
            if let Err(err) = result {
                eprintln!(
                    "Warning: failed to prune stale vuepress link {}: {err}",
                    entry.path().display()
                );
            }
        }
    }

    // (3) Add missing：为 workspaces 有但 hub 还没建 link 的 user 建链。
    //     已存在的 entry 跳过——保留任何 vuepress 缓存关联。源 tasks/ 不
    //     存在的 user（workspace 骨架被破坏的边角场景）也跳过。
    // (3) Add missing: link users present in workspaces but not yet in
    //     the hub. Existing entries are left alone to preserve any
    //     VuePress cache linkage. Users whose tasks/ directory is
    //     missing (broken skeleton) are also skipped.
    for user_name in &workspace_users {
        let vuepress_user_dir = hub_docs_dir.join(user_name);
        if vuepress_user_dir.symlink_metadata().is_ok() {
            continue;
        }
        let tasks_dir = workspace_dir.join(user_name).join("tasks");
        if !tasks_dir.is_dir() {
            continue;
        }

        #[cfg(windows)]
        junction::create(&tasks_dir, &vuepress_user_dir)?;

        #[cfg(unix)]
        std::os::unix::fs::symlink(&tasks_dir, &vuepress_user_dir)?;
    }

    Ok(())
}

/// 删除 .hotpot-hub/docs 下绑定的文档软链接，新增用户目录后需要新增软链接时，需要先进行删除操作，**手动创建的目录将会被一并删除**
pub fn remove_vuepress_links(root_dir: &str) -> anyhow::Result<()> {
    let link_entries = get_vuepress_user_dir_entries(root_dir);
    if link_entries.is_empty() {
        return Ok(());
    }
    for entry in link_entries {
        #[cfg(windows)]
        let result = fs::remove_dir(entry.path());

        #[cfg(unix)]
        let result = fs::remove_file(entry.path());

        result.map_err(|_| {
            anyhow::anyhow!("Remove link failed, link: {}", entry.file_name().display())
        })?;
    }

    Ok(())
}

// ============================================================================
// pnpm 检测
// pnpm detection
// ============================================================================

/// 在 PATH 中查找可用的 pnpm 命令名。
///
/// Windows 上 pnpm 通常通过 `pnpm.cmd` / `pnpm.exe` 发布，Rust 的
/// `Command::new("pnpm")` 不会自动尝试 `.cmd` / `.exe` 后缀；这里按候选
/// 顺序逐个 `--version` 探测，第一个成功的就是可用名。
///
/// Locates a usable pnpm command name on `PATH`. On Windows pnpm is
/// commonly installed as `pnpm.cmd` or `pnpm.exe`, and Rust's
/// `Command::new("pnpm")` does not auto-suffix. We probe each
/// candidate with `--version` and return the first one whose process
/// exited successfully. Returns a stable `&'static str` so callers can
/// hand it to `Command::new` without lifetime juggling.
pub fn find_pnpm() -> Result<&'static str> {
    let candidates: &[&str] = if cfg!(windows) {
        &["pnpm", "pnpm.cmd", "pnpm.exe"]
    } else {
        &["pnpm"]
    };
    for cmd in candidates {
        let result = std::process::Command::new(cmd).arg("--version").output();
        if let Ok(out) = result
            && out.status.success()
        {
            return Ok(cmd);
        }
    }
    bail!("pnpm not found on PATH; install pnpm (https://pnpm.io/installation) and retry")
}

// ============================================================================
// config.toml [vuepress] 表读写
// config.toml [vuepress] table read/write
// ============================================================================

/// `.hotpot/config.toml` 首次写入 `[vuepress]` 表时附带的警告注释模板。
///
/// 中英双语，明确告知用户 `enabled` 是受 install/uninstall 命令维护的
/// 原子状态字段，不要手动改。`{PORT}` 占位符会被 [`enable_in_config_toml`]
/// 替换为实际端口。
///
/// Bilingual warning comment block written above the `[vuepress]` table
/// on first config insertion. Tells users not to flip `enabled` by hand;
/// `{PORT}` is substituted with the actual port by
/// [`enable_in_config_toml`].
const VUEPRESS_TABLE_TEMPLATE: &str = r#"
# VuePress 集成配置 / VuePress integration settings.
#
# enabled 由 `hotpot vuepress install` / `hotpot vuepress uninstall` 共同
# 维护，**切勿手动修改**——它与 .hotpot-hub/ 目录及 .hotpot/prompts/
# vuepress*.md 是绑定的原子状态。手动改只会让 enabled 与磁盘资源不一致，
# 运行时找不到依赖。如需开/关 VuePress 请用：
#   hotpot vuepress install [--port <p>]
#   hotpot vuepress uninstall
#
# `enabled` is managed atomically by `hotpot vuepress install` /
# `hotpot vuepress uninstall`. Do NOT flip it by hand — it is tied to
# the `.hotpot-hub/` directory and opt-in prompt assets on disk, and a
# manual flip leaves the runtime looking for dependencies that don't
# exist. Use the install / uninstall subcommands above to toggle.
[vuepress]
enabled = true
port = {PORT}
"#;

/// 启用 VuePress：把 `[vuepress] enabled = true / port = <port>` 写入
/// `.hotpot/config.toml`。
///
/// 已存在 `[vuepress]` 表时只更新 `enabled` 与 `port`，其他字段（例如
/// `ttl_seconds`）原样保留；首次写入时整段带上 [`VUEPRESS_TABLE_TEMPLATE`]
/// 的警告注释。文件不存在时按需创建（含父目录）。
///
/// Writes `[vuepress] enabled = true / port = <port>` to
/// `.hotpot/config.toml`. If the table already exists only those two
/// keys are updated — sibling fields like `ttl_seconds` survive. On
/// first insertion the bilingual warning block from
/// [`VUEPRESS_TABLE_TEMPLATE`] is prepended.
pub fn enable_in_config_toml(root_dir: &str, port: u16) -> Result<()> {
    let config_path = hotpot_dir(root_dir).join("config.toml");
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let existing = fs::read_to_string(&config_path).ok();
    let mut doc: toml_edit::DocumentMut = match existing.as_deref() {
        Some(raw) => raw
            .parse()
            .with_context(|| format!("failed to parse {}", config_path.display()))?,
        None => toml_edit::DocumentMut::new(),
    };

    if let Some(table) = doc.get_mut("vuepress").and_then(|i| i.as_table_mut()) {
        // 已存在 [vuepress] 表：只动 enabled / port，保留其它字段与原有注释。
        // Existing table: update enabled / port, preserve siblings & comments.
        table["enabled"] = toml_edit::value(true);
        table["port"] = toml_edit::value(i64::from(port));
    } else {
        // 没有 [vuepress] 表：把带注释的模板拼到现有内容末尾后重新解析。
        // No table yet: append the commented template and re-parse so the
        // resulting document is well-formed before serializing back.
        let mut combined = existing.unwrap_or_default();
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        let rendered = VUEPRESS_TABLE_TEMPLATE.replace("{PORT}", &port.to_string());
        combined.push_str(rendered.trim_start_matches('\n'));
        doc = combined
            .parse()
            .with_context(|| format!("rendered config.toml is not valid TOML: {combined}"))?;
    }

    fs::write(&config_path, doc.to_string())
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    Ok(())
}

/// 关闭 VuePress：把 `[vuepress] enabled = false` 写入
/// `.hotpot/config.toml`，保留 `port` / `ttl_seconds` 等其它字段以便下次
/// install 时复用。
///
/// 文件不存在或 `[vuepress]` 表不存在时视为 already-disabled，静默返回
/// 成功——uninstall 的语义就是"使其禁用"，无需中途 bail。
///
/// Flips `[vuepress] enabled = false` in `.hotpot/config.toml`, leaving
/// `port` / `ttl_seconds` etc. intact for a future re-install. If the
/// file or table is missing the function returns Ok silently — disable
/// means "make sure it's disabled," and missing-equals-disabled.
pub fn disable_in_config_toml(root_dir: &str) -> Result<()> {
    let config_path = hotpot_dir(root_dir).join("config.toml");
    let Ok(raw) = fs::read_to_string(&config_path) else {
        return Ok(());
    };
    let mut doc: toml_edit::DocumentMut = raw
        .parse()
        .with_context(|| format!("failed to parse {}", config_path.display()))?;
    let Some(table) = doc.get_mut("vuepress").and_then(|i| i.as_table_mut()) else {
        return Ok(());
    };
    table["enabled"] = toml_edit::value(false);
    fs::write(&config_path, doc.to_string())
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    Ok(())
}

// ============================================================================
// install / uninstall 编排
// install / uninstall orchestration
// ============================================================================

/// 一站式安装 VuePress 集成：部署 hub 模板 + 运行 pnpm install + 建立
/// docs 软链 + 写 config.toml 启用开关。
///
/// 执行顺序经过精心编排：先做能失败的、对用户磁盘无副作用的检查（pnpm
/// 检测），再做有副作用但可幂等回放的步骤（资产部署），最后才写最显著
/// 的状态（config.toml）。任何步骤报错都会保留之前的副作用以便用户
/// 手动 inspect，但调用方可以紧接着调 [`uninstall_hub`] 做强制清理。
///
/// One-shot VuePress install: deploy hub template → `pnpm install` →
/// `sync_tasks_links` → write `[vuepress] enabled = true` to
/// `.hotpot/config.toml`. Ordered so failure-prone checks (pnpm
/// availability) happen before any disk writes, and the most visible
/// state (config.toml) is the last thing flipped. Idempotent re-runs
/// only no-op the steps whose target is already at the desired state.
pub fn install_hub(root_dir: &str, port: u16, force: bool) -> Result<()> {
    // (1) pnpm 必须可用。
    // (1) pnpm must be available.
    let pnpm = find_pnpm()?;
    println!("Using pnpm: {pnpm}");

    let project_dir = PathBuf::from(root_dir);
    let hub_dir = hotpot_hub_dir(root_dir);

    // (2) 部署 hub 模板文件到 .hotpot-hub/。
    // (2) Deploy the hub template to .hotpot-hub/.
    let stats = assets::install_vuepress_hub(&project_dir, force, false, true)?;
    println!(
        "Hub assets: {} written, {} skipped.",
        stats.written.len(),
        stats.skipped.len()
    );

    // (3) pnpm install (长操作)。
    // (3) `pnpm install` (long-running).
    println!("Running `{pnpm} install` in {} ...", hub_dir.display());
    let status = std::process::Command::new(pnpm)
        .current_dir(&hub_dir)
        .arg("install")
        .status()
        .with_context(|| format!("failed to invoke `{pnpm} install`"))?;
    if !status.success() {
        bail!("`{pnpm} install` exited with status {status}");
    }

    // (4) 建立 docs 软链（尽力而为：用户还没创建任何任务时 workspace 不存在，
    //     此时不报错，等用户首个任务建好再补软链是合理的）。
    // (4) Symlink user task dirs into the hub docs root. Best-effort:
    //     missing workspace (no tasks created yet) is fine and ignored;
    //     we'll re-link when the user actually creates tasks.
    match sync_tasks_links(root_dir) {
        Ok(()) => println!("Docs symlinks created."),
        Err(err) => {
            // 现有 mklink 在 workspace 不存在时也返回 Err；这里软化为 info。
            // The existing mklink also errors when the workspace dir
            // simply doesn't exist yet; demote that to an informational
            // line instead of a hard failure.
            println!("Skipping docs symlinks (no workspace tasks yet, or hub missing): {err}");
        }
    }

    // (5) 安装 opt-in prompt 资产到 .hotpot/prompts/。先于 config 翻转，
    //     这样若 prompt 写盘失败，enabled 仍是 false，verify 校验拦得住。
    // (5) Install the opt-in prompt assets BEFORE flipping the config
    //     switch — if prompt write fails, `enabled` stays false and the
    //     atomic state remains consistent.
    let prompt_stats = assets::install_vuepress_prompts(&project_dir, force, false, true)?;
    println!(
        "Opt-in prompts: {} written, {} skipped.",
        prompt_stats.written.len(),
        prompt_stats.skipped.len()
    );

    // (6) 翻转 config.toml 的 enabled 开关。最后做，确保前面任一失败时
    //     enabled 仍是 false，避免出现"enabled=true 但 hub / 提示词残缺"
    //     的状态。
    // (6) Flip the config.toml switch LAST, so any failure above leaves
    //     enabled=false (i.e. we never lie about install completeness).
    enable_in_config_toml(root_dir, port)?;
    println!("Set [vuepress] enabled = true / port = {port} in .hotpot/config.toml.");

    println!("VuePress installed. Start with `hotpot vuepress start`.");
    Ok(())
}

/// 一站式卸载 VuePress 集成：停服务 + 拆软链 + 删 .hotpot-hub/ + 关
/// config.toml 开关。
///
/// 反向编排：先做最可见的状态翻转（config.toml 关 enabled）让后续 hook
/// 立即停止注入 VuePress 相关 env，再清理资源以避免任何"enabled=true
/// 但磁盘已空"的中间态被读到。停服务幂等失败也只是 warn，不阻塞清理。
///
/// One-shot VuePress uninstall: stop service → flip
/// `[vuepress] enabled = false` (so hooks stop injecting VP env
/// immediately) → tear down docs symlinks → remove `.hotpot-hub/`.
/// Stops are best-effort: a failure to stop merely warns and we
/// continue with the rest of the teardown.
pub fn uninstall_hub(root_dir: &str) -> Result<()> {
    // (1) 先停服务（容错：如果 service lifecycle 还没接入，本次只是 noop）。
    // (1) Stop first (tolerates the lifecycle stubs we'll fill in later).
    if let Err(err) = stop_if_running(root_dir) {
        eprintln!("Warning: vuepress stop failed: {err}");
    }

    // (2) 翻转 config.toml enabled = false。如果 config 不存在则跳过。
    //     必须先于资源清理，让 hooks 在下一轮就停止注入 env。
    // (2) Flip the switch first so any hook firing during the rest of
    //     this command sees `enabled=false` and stops injecting VP env.
    disable_in_config_toml(root_dir)?;
    println!("Set [vuepress] enabled = false in .hotpot/config.toml.");

    // (3) 删 opt-in prompt 资产（用 catalog 驱动，避免重复维护文件清单）。
    //     单个文件缺失不算错——uninstall 语义是"使其不存在"。
    // (3) Remove opt-in prompt assets, catalog-driven to keep this in
    //     sync with installs. A missing file is fine — uninstall just
    //     wants the file gone.
    let project_dir = PathBuf::from(root_dir);
    for rel in assets::vuepress_prompt_paths() {
        let target = project_dir.join(rel);
        match fs::remove_file(&target) {
            Ok(()) => println!("Removed {}.", target.display()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => eprintln!("Warning: failed to remove {}: {err}", target.display()),
        }
    }

    // (4) 拆 docs 软链。
    // (4) Tear down docs symlinks.
    if let Err(err) = remove_vuepress_links(root_dir) {
        eprintln!("Warning: failed to remove docs symlinks: {err}");
    }

    // (5) 整体删 .hotpot-hub/。runtime.json 也住在里面，自然连带消失。
    // (5) Remove the .hotpot-hub/ tree. runtime.json lives inside it so
    //     it is cleaned up automatically.
    let hub_dir = hotpot_hub_dir(root_dir);
    if hub_dir.exists() {
        fs::remove_dir_all(&hub_dir)
            .with_context(|| format!("failed to remove {}", hub_dir.display()))?;
        println!("Removed {}.", hub_dir.display());
    }

    println!("VuePress uninstalled. Run `hotpot vuepress install` to re-enable.");
    Ok(())
}

/// 启动前的三者一致性校验。
///
/// 拦截"用户无视警告手动改了 `[vuepress] enabled`"的情况：要求 (a)
/// `[vuepress] enabled = true`、(b) `.hotpot-hub/package.json` 存在、
/// (c) `.hotpot/prompts/vuepress.md` 存在。任意失败都 bail，并提示
/// 用户跑 install 修复。
///
/// Pre-start sanity check. Catches the "user flipped enabled by hand"
/// case by requiring all three pieces of the atomic state to agree:
/// (a) `[vuepress] enabled = true` in config.toml, (b) the hub
/// `package.json` exists on disk, and (c) the opt-in `vuepress.md`
/// prompt exists. Any failure bails with a repair hint.
pub fn verify_install_consistency(root_dir: &str) -> Result<()> {
    if !crate::context::resolve_vuepress_enabled(root_dir) {
        bail!(
            "[vuepress] enabled is not true in .hotpot/config.toml; run `hotpot vuepress install` first"
        );
    }
    let hub_pkg = hotpot_hub_dir(root_dir).join("package.json");
    if !hub_pkg.exists() {
        bail!(
            "{} is missing; the hub project is out of sync with config.toml. \
             Run `hotpot vuepress uninstall` then `hotpot vuepress install` to repair.",
            hub_pkg.display()
        );
    }
    let prompt = hotpot_dir(root_dir).join("prompts").join("vuepress.md");
    if !prompt.exists() {
        bail!(
            "{} is missing; opt-in prompts are out of sync with config.toml. \
             Run `hotpot vuepress uninstall` then `hotpot vuepress install` to repair.",
            prompt.display()
        );
    }
    Ok(())
}

// ============================================================================
// 服务管理 stub（Task #7 填充实际逻辑）
// Service-management stubs (real logic lands in task #7)
// ============================================================================

/// 幂等地停止当前可能在跑的 vuepress dev server。
///
/// 供 `/hotpot:execute` 入口与各平台 SessionEnd hook 调用——无论 runtime
/// 文件是否存在、进程是否还活着，函数都返回 `Ok(())`，调用方不需要分支。
/// 等价于 `stop(root_dir, true)`。供 `/hotpot:execute` 入口与各平台
/// SessionEnd hook 调用——runtime.json 不存在 / pid 已死时均返回成功，
/// 调用方无需分支。
///
/// Idempotent shorthand for `stop(root_dir, true)`. Used by the
/// `/hotpot:execute` pre-flight directive and by every platform's
/// SessionEnd hook so they can call vuepress cleanup unconditionally
/// without first checking whether anything is running.
pub fn stop_if_running(root_dir: &str) -> Result<()> {
    stop(root_dir, true)
}

// ============================================================================
// 服务管理：runtime.json + start / stop / status
// Service lifecycle: runtime.json + start / stop / status
// ============================================================================

/// `.hotpot-hub/vuepress.runtime.json` 的内容 schema。
///
/// 写出由 [`start`]，读取由 [`stop`] / [`status`] / [`stop_if_running`]
/// 复用；运行时进程的"活着没"语义通过 [`is_pid_alive`] 单独验证。
///
/// Schema of `.hotpot-hub/vuepress.runtime.json`. The file is the
/// single source of truth for what we believe is running; liveness is
/// independently verified through [`is_pid_alive`] before trusting any
/// field.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeState {
    pid: u32,
    port: u16,
    url: String,
    started_at: DateTime<Utc>,
    /// `None` 表示无过期（用户传 `--ttl 0`）；`Some` 表示到期后视为 stale。
    /// `None` means no expiry (user passed `--ttl 0`); `Some` is treated
    /// as stale once reached and triggers lazy cleanup.
    expires_at: Option<DateTime<Utc>>,
}

/// 返回 runtime.json 的绝对路径。
///
/// Runtime state file lives inside `.hotpot-hub/` so `uninstall_hub` /
/// 用户手动 rm -rf `.hotpot-hub` 都能顺带把它清理掉，避免悬挂状态。
fn runtime_json_path(root_dir: &str) -> PathBuf {
    hotpot_hub_dir(root_dir).join("vuepress.runtime.json")
}

/// 写入 runtime.json（含父目录创建）。
fn write_runtime_state(root_dir: &str, state: &RuntimeState) -> Result<()> {
    let path = runtime_json_path(root_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let raw =
        serde_json::to_string_pretty(state).context("failed to serialize vuepress.runtime.json")?;
    fs::write(&path, raw).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

/// 读取 runtime.json；不存在或损坏均返回 `None`，由调用方按"未运行"
/// 语义处理。
fn read_runtime_state(root_dir: &str) -> Option<RuntimeState> {
    let path = runtime_json_path(root_dir);
    let raw = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// 静默清理 runtime.json；缺失不算错。stale 状态恢复时调用。
fn discard_runtime_state(root_dir: &str) {
    let path = runtime_json_path(root_dir);
    let _ = fs::remove_file(&path);
}

/// 跨平台进程活跃性检测。
///
/// Unix: `kill -0 <pid>` exit code 0 = alive; non-zero = dead/no
/// permission. Windows: `tasklist /FI "PID eq <pid>" /NH` stdout
/// contains the pid string iff a process with that pid exists.
/// 任意检测失败 → 视为已死，让 stale 清理流程接管。
fn is_pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output();
        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // tasklist 无匹配时仍返回 success 但输出 "INFO: No tasks ..."；
                // 命中时输出行包含 pid 数字。简单 contains 判定足够。
                // tasklist exits successfully even when nothing matches
                // ("INFO: No tasks..."), so we look for the pid literal
                // in stdout — present iff a row was emitted.
                stdout.contains(&pid.to_string())
            }
            _ => false,
        }
    }
}

/// 跨平台进程杀进程。
///
/// `force=false` 走 graceful（Unix SIGTERM / Windows taskkill 不带 /F）；
/// `force=true` 走 SIGKILL / `taskkill /F`。`/T` 让 Windows 也把子进程
/// 树一起 kill（pnpm spawn 出 node + vuepress，必须 tree-kill）。
fn terminate_pid(pid: u32, force: bool) -> Result<()> {
    #[cfg(unix)]
    {
        let signal = if force { "-KILL" } else { "-TERM" };
        std::process::Command::new("kill")
            .args([signal, &pid.to_string()])
            .status()
            .with_context(|| format!("failed to invoke `kill {signal} {pid}`"))?;
    }
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T"]);
        if force {
            cmd.arg("/F");
        }
        cmd.status()
            .with_context(|| format!("failed to invoke `taskkill /PID {pid} /T`"))?;
    }
    Ok(())
}

/// `wait_for_port_ready` 的硬上限——pnpm + vuepress 首次构建经验
/// 上限。超过就 bail 并附 vuepress.log tail，由用户决定继续等还是
/// 排查。
///
/// Hard upper bound for `wait_for_port_ready`; the empirical ceiling
/// for a first pnpm + vuepress build of this hub. Defined above the
/// consumer per the module's "constants before usage" convention
/// (see `VUEPRESS_TABLE_TEMPLATE`).
const READINESS_TIMEOUT: Duration = Duration::from_secs(30);

/// 单次 `TcpStream::connect_timeout` 调用的超时——250ms 足以让
/// 内核把 SYN 走完三次握手，过短会假阴性、过长会让总轮询稀疏。
///
/// Per-attempt connect timeout (250 ms): long enough for the kernel
/// to complete a local SYN handshake, short enough to keep the poll
/// loop responsive.
const READINESS_CONNECT_TIMEOUT: Duration = Duration::from_millis(250);

/// 两次 connect 尝试之间的等待——让出 CPU + 给 vite 一点构建时间，
/// 总循环上限 30s / 250ms = 120 次，足够细。
///
/// Poll interval between connect attempts: yields CPU and gives Vite
/// a moment to make progress. With a 30s hard cap that's up to ~120
/// probes, which is plenty of granularity.
const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// 等 `127.0.0.1:<port>` 接受 TCP 连接，作为 vuepress 端口就绪的判据。
///
/// 每 [`READINESS_POLL_INTERVAL`] 探一次 connect，单次 connect 超时
/// [`READINESS_CONNECT_TIMEOUT`]；总时长上限 [`READINESS_TIMEOUT`]
/// 即 pnpm + vuepress 首次构建的经验上限——本仓库 hub 体量小，实测
/// 5–15s，30s 是给冷缓存 / 慢盘留余量。超时返回 `Err`，调用方应附带
/// `vuepress.log` 末尾做线索。
///
/// Polls `127.0.0.1:<port>` until vuepress accepts a TCP connection.
/// `READINESS_TIMEOUT` (30s) is the empirical upper bound for a first
/// pnpm+vuepress build of this hub. On timeout returns `Err`; the
/// caller should attach the tail of `vuepress.log` as a diagnostic
/// hint so the user can see what pnpm logged before bailing.
fn wait_for_port_ready(port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{port}")
        .parse()
        .with_context(|| format!("failed to parse readiness probe address 127.0.0.1:{port}"))?;
    let deadline = Instant::now() + READINESS_TIMEOUT;
    loop {
        if TcpStream::connect_timeout(&addr, READINESS_CONNECT_TIMEOUT).is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!(
                "vuepress port {port} did not become ready within {}s",
                READINESS_TIMEOUT.as_secs()
            );
        }
        thread::sleep(READINESS_POLL_INTERVAL);
    }
}

/// 读 `vuepress.log` 末尾 `max_lines` 行，作为 readiness 探测超时时
/// 的诊断附录。读不到（文件不存在 / 权限错 / IO 错）返回空字符串而
/// 不是 propagate——线索本身是 best-effort，不应掩盖真正的 timeout
/// 错误。
///
/// Best-effort tail of `vuepress.log`. Any IO error returns an empty
/// string; the readiness timeout error is the primary signal.
fn read_log_tail(log_path: &Path, max_lines: usize) -> String {
    let file = match fs::File::open(log_path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let reader = BufReader::new(file);
    let mut buf: Vec<String> = Vec::with_capacity(max_lines);
    for line in reader.lines().map_while(Result::ok) {
        if buf.len() == max_lines {
            buf.remove(0);
        }
        buf.push(line);
    }
    buf.join("\n")
}

/// 启动 VuePress dev server。
///
/// 流程：[`verify_install_consistency`] → 检查既有 runtime.json
/// （活着 = bail，stale = 清理）→ [`find_pnpm`] → spawn `pnpm run
/// docs:dev -- --clean-cache --port <port>`（cwd 设为 `.hotpot-hub/`，
/// stdout/stderr 写到 `.hotpot-hub/vuepress.log`，stdin 重定向为 null
/// 避免子进程意外读父进程输入）→ 写 runtime.json → 输出单行 JSON
/// `{"url","pid"}` 给 AI 解析。
///
/// 平台 detach：Windows 通过 `creation_flags(CREATE_NO_WINDOW |
/// CREATE_NEW_PROCESS_GROUP)` 让 `cmd.exe`（pnpm.cmd 实际入口）拿到
/// 新的隐藏 console、自成进程组——既不弹黑窗、也让 conhost 与父
/// console 解绑；Unix 通过 `pre_exec(setsid)` 让子进程成为新会话
/// 首领，脱离父进程的控制终端。两层目的一致：阻断孙子进程
/// （`pnpm.cmd → node.exe`）继承 hotpot 从父 shell 拿到的 stdio pipe
/// 句柄，否则 Bash tool 这类等 EOF 的调用方会因为孙子进程不关 pipe
/// 而永远 hang 住。stdout/stderr 重定向到日志文件与 detach 是正交
/// 的两层防护，必须同时保留。
///
/// **不要**给 Windows 加 `DETACHED_PROCESS`——它会让 `CREATE_NO_WINDOW`
/// 被静默忽略，cmd.exe 退化为「allocate 自己 console 并显示窗口」，
/// 每次 start 都弹黑窗（已踩坑）。
///
/// `ttl_seconds = 0` 表示无过期；否则记 `expires_at`，由 [`status`]
/// 在过期后懒清理。
///
/// Starts the VuePress dev server. The spawned child is detached
/// from the parent: on Windows via `creation_flags(CREATE_NO_WINDOW
/// | CREATE_NEW_PROCESS_GROUP)` — `CREATE_NO_WINDOW` gives cmd.exe
/// (the actual entry for pnpm.cmd batch files) a fresh hidden
/// console so it doesn't pop a black window AND doesn't share the
/// parent's conhost; on Unix via `pre_exec(setsid)`. Together with
/// stdout/stderr redirection to `.hotpot-hub/vuepress.log` and
/// stdin null, this prevents the grandchild process (`pnpm.cmd →
/// node.exe`) from inheriting stdio pipe handles that hotpot
/// inherited from its parent shell — without detach, those handles
/// stay open after hotpot exits, and pipe-EOF readers (e.g. Claude
/// Code's Bash tool) hang forever waiting for them to close.
///
/// **Do NOT** add `DETACHED_PROCESS` on Windows: per Microsoft's
/// `CreateProcess` docs, setting `DETACHED_PROCESS` causes
/// `CREATE_NO_WINDOW` to be silently ignored, and cmd.exe (a
/// console application) then allocates and **shows** its own
/// console window for every start. We learned this the hard way.
///
/// Returns immediately after writing runtime.json; `stop` does the
/// actual kill later.
pub fn start(root_dir: &str, port: u16, ttl_seconds: u64) -> Result<()> {
    verify_install_consistency(root_dir)?;

    if let Some(existing) = read_runtime_state(root_dir) {
        if is_pid_alive(existing.pid) {
            bail!(
                "vuepress already running on port {} (pid {}); run `hotpot vuepress stop` first",
                existing.port,
                existing.pid
            );
        }
        // stale —— 静默清理后继续。
        // stale runtime.json — silently clean up and proceed.
        discard_runtime_state(root_dir);
    }

    let pnpm = find_pnpm()?;
    let hub_dir = hotpot_hub_dir(root_dir);
    if !hub_dir.exists() {
        bail!(
            "{} is missing; run `hotpot vuepress install` first",
            hub_dir.display()
        );
    }

    // 日志重定向：pnpm/vuepress 启动日志全部写 `.hotpot-hub/vuepress.log`，
    // 让父进程退出后子进程仍可写。stdin 设为 null 防止子进程因 pty 关闭
    // 退出。
    // Redirect stdout+stderr to a log file so the child keeps writing
    // after the parent exits, and null out stdin to keep the child from
    // ever blocking on terminal input.
    let log_path = hub_dir.join("vuepress.log");
    let log_file = fs::File::create(&log_path)
        .with_context(|| format!("failed to create {}", log_path.display()))?;
    let log_dup = log_file
        .try_clone()
        .context("failed to dup vuepress.log fd for stderr")?;

    let mut cmd = std::process::Command::new(pnpm);
    cmd.current_dir(&hub_dir);
    cmd.arg("run");
    cmd.arg("docs:dev");
    cmd.arg("--");
    cmd.arg("--clean-cache");
    cmd.arg("--port");
    cmd.arg(port.to_string());
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(log_file);
    cmd.stderr(log_dup);

    // Windows: 让子进程获得隐藏 console + 独立进程组，避免孙子进程
    // （pnpm.cmd → node.exe）继承 hotpot.exe 的 console / stdio pipe，
    // 父进程退出后 Bash tool 等不到 EOF 而卡死。
    //
    // 关键：**不能**加 `DETACHED_PROCESS`。微软 CreateProcess 文档明文：
    // 一旦设置 `DETACHED_PROCESS`，`CREATE_NO_WINDOW` 会被静默忽略。
    // 而我们 spawn 的是 `pnpm.cmd`（batch file），Windows 实际走
    // `cmd.exe /c pnpm.cmd ...`；cmd.exe 是 console app，没有继承的
    // console 时会 allocate 自己的并**默认显示窗口**。结果就是每次
    // start 都弹一个黑窗（参见 review 阶段 Low #1 后被实证）。
    //
    // 正确组合：`CREATE_NO_WINDOW`（cmd.exe 拿到新 console 但窗口
    // 隐藏，conhost 与父 console 解绑——bash-tool-hang 仍修好）
    // + `CREATE_NEW_PROCESS_GROUP`（Ctrl+C 隔离，与 console 分配
    // 正交，两者不互斥）。
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
        const DETACH_FLAGS: u32 = 0x0000_0200 | 0x0800_0000;
        cmd.creation_flags(DETACH_FLAGS);
    }

    // Unix: setsid() in the child after fork() (pre_exec) so it becomes
    // a new session leader, detaching from the parent's controlling
    // terminal and process group.
    // Unix: 在 fork 之后、exec 之前调 setsid() 让子进程成为新会话首领，
    // 脱离父进程的控制终端与进程组——nohup-style 后台进程的标准做法。
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: pre_exec runs in the child between fork and exec; setsid
        // is async-signal-safe and has no side effects that would corrupt
        // the soon-to-be-exec'd process image.
        // SAFETY: pre_exec 在 fork 后、exec 前的子进程中运行；setsid 是
        // async-signal-safe 的，不会影响即将被 exec 替换的进程映像。
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }

    let child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn `{pnpm} run docs:dev`"))?;

    let pid = child.id();

    // runtime.json 必须在 readiness probe 之前落盘：probe 期内 pnpm
    // 已经活着，若并发调用方（如 SessionEnd hook 或用户手动 `vuepress
    // status` / `vuepress stop`）此时来读，会因 runtime.json 不存在而
    // 错判 "未运行"——既泄漏当前 spawn 的 pnpm，又让 stop 误 bail
    // "not running"。把 runtime.json 写盘提前到这里关闭这个可见性窗口。
    //
    // Write runtime.json BEFORE the readiness probe: during the probe
    // window pnpm is already alive, and any concurrent reader (a
    // SessionEnd hook, a parallel `vuepress status`, or `vuepress
    // stop`) that finds no runtime.json wrongly concludes "not
    // running" — leaking the just-spawned process and making stop
    // bail. Hoisting the writeback closes that visibility gap.
    let started_at = Utc::now();
    let expires_at = if ttl_seconds == 0 {
        None
    } else {
        let dur = chrono::Duration::seconds(i64::try_from(ttl_seconds).unwrap_or(i64::MAX));
        Some(started_at + dur)
    };
    let url = format!("http://localhost:{port}");
    let state = RuntimeState {
        pid,
        port,
        url: url.clone(),
        started_at,
        expires_at,
    };
    write_runtime_state(root_dir, &state)?;

    // 端口就绪探测（P-A）：vuepress 的 vite dev server 是先 spawn、
    // 后才在异步任务里完成构建并 bind 端口；如果直接 println! URL，
    // 用户立刻点开浏览器会看到 connection refused / spin，被误读
    // 为「hotpot vuepress start 卡住」。在此处阻塞最多 30s 等
    // `127.0.0.1:<port>` accept TCP，端口可连了再让 start 返回 URL。
    // 超时则附 vuepress.log 末尾 20 行作为线索 bail —— 30s 是
    // pnpm + vuepress 首次构建的经验上限，本仓库 hub 实测 5–15s。
    //
    // Port-readiness probe (P-A): VuePress's Vite dev server forks
    // first and binds the port only after the build worker completes
    // an initial pass. Printing the URL before the bind would let the
    // user click into a connection-refused / spinning browser tab,
    // which they mis-read as "start hung". Block up to 30s here until
    // `127.0.0.1:<port>` accepts a TCP connect — this hub typically
    // becomes ready in 5–15s; 30s leaves margin for cold caches and
    // slow disks. On timeout we bail with the log tail attached so
    // the user has something concrete to debug.
    if let Err(err) = wait_for_port_ready(port) {
        let tail = read_log_tail(&log_path, 20);
        // 复用 stop 的 TERM → 3s 轮询 → KILL 升级 + runtime.json 清理：
        // 这样 pnpm 以及孙子进程（vuepress.js / esbuild）整棵 tree 都
        // 会被处理，而不仅 group leader；同时 runtime.json 与现实保持
        // 一致，不会留下 "enabled & runtime present 但进程已死" 的
        // 中间态。stop 本身的错误是 best-effort，readiness-timeout 才
        // 是主要信号。
        //
        // Reuse stop's TERM → 3 s poll → KILL escalation AND its
        // runtime.json cleanup so pnpm and its grandchildren
        // (vuepress.js / esbuild) are reaped together, not just the
        // group leader. This also keeps runtime.json consistent with
        // reality. Errors from stop are best-effort — the
        // readiness-timeout error is the primary signal.
        let _ = stop(root_dir, true);
        let tail_msg = if tail.is_empty() {
            "vuepress.log tail: <empty>".to_string()
        } else {
            format!("vuepress.log tail:\n{tail}")
        };
        return Err(err.context(tail_msg));
    }

    // 单行 JSON：AI 用 Bash 拿到 stdout 后直接 JSON.parse。
    // Single-line JSON: the brainstorming-closing prompt parses this
    // directly to extract the URL.
    let summary = serde_json::json!({ "url": url, "pid": pid });
    println!("{summary}");

    Ok(())
}

/// 停止 VuePress dev server。
///
/// `if_running = true` 表示幂等模式：runtime.json 缺失或进程已死，都
/// 返回 `Ok(())`，不算错——这是 `/hotpot:execute` 入口与各平台 SessionEnd
/// hook 调用的形态。`if_running = false`（普通 `hotpot vuepress stop`
/// 调用）要求 runtime.json 存在，否则 bail "not running"。
///
/// 杀进程顺序：SIGTERM → 等 3 秒（每 200ms 轮询一次 pid 活否）→ 仍活则
/// SIGKILL → 删 runtime.json。
///
/// Stops the VuePress dev server. `if_running=true` is the idempotent
/// mode used by hook callers — missing runtime.json or dead pid both
/// resolve to Ok. `if_running=false` (normal CLI) bails when nothing is
/// running. Kill sequence: TERM → 3 s polling wait → KILL if still
/// alive → remove runtime.json.
pub fn stop(root_dir: &str, if_running: bool) -> Result<()> {
    let Some(state) = read_runtime_state(root_dir) else {
        if if_running {
            return Ok(());
        }
        bail!("vuepress is not running (no runtime.json found)");
    };

    if !is_pid_alive(state.pid) {
        // stale — 静默清理，无论 if_running 与否都不算错。
        // stale runtime — silently clean up regardless of flag.
        discard_runtime_state(root_dir);
        return Ok(());
    }

    // graceful TERM
    let _ = terminate_pid(state.pid, false);

    // poll up to 3 seconds for the pid to disappear
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && is_pid_alive(state.pid) {
        thread::sleep(Duration::from_millis(200));
    }

    // forceful KILL if still hanging
    if is_pid_alive(state.pid) {
        let _ = terminate_pid(state.pid, true);
    }

    discard_runtime_state(root_dir);
    println!(
        "Stopped vuepress (pid {}, was running on port {}).",
        state.pid, state.port
    );
    Ok(())
}

/// 查询当前 VuePress dev server 运行状态。
///
/// 输出单行 JSON：`{"running": bool, "port": ..., "url": ..., "pid": ...,
/// "expires_at": ...}`。stale（pid 已死或 ttl 过期）会被懒清理；ttl
/// 过期时会顺带杀进程，避免遗留。
///
/// Reports the dev server's running state as one-line JSON. Lazily
/// cleans up stale runtime files (dead pid or expired ttl); for the
/// ttl-expired case we also kill the process so the OS doesn't carry
/// the leak indefinitely.
pub fn status(root_dir: &str) -> Result<()> {
    let mut output = serde_json::Map::new();
    output.insert("running".into(), serde_json::Value::Bool(false));

    if let Some(state) = read_runtime_state(root_dir) {
        let alive = is_pid_alive(state.pid);
        let expired = match state.expires_at {
            Some(exp) => exp <= Utc::now(),
            None => false,
        };

        if !alive {
            // pid 已死 → 清状态。
            discard_runtime_state(root_dir);
        } else if expired {
            // ttl 过期 → 杀掉 + 清状态。
            let _ = terminate_pid(state.pid, false);
            discard_runtime_state(root_dir);
        } else {
            output.insert("running".into(), serde_json::Value::Bool(true));
            output.insert("port".into(), serde_json::Value::from(state.port));
            output.insert("url".into(), serde_json::Value::from(state.url));
            output.insert("pid".into(), serde_json::Value::from(state.pid));
            if let Some(exp) = state.expires_at {
                output.insert(
                    "expires_at".into(),
                    serde_json::Value::from(exp.to_rfc3339()),
                );
            }
        }
    }

    println!("{}", serde_json::Value::Object(output));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_tasks_links() -> anyhow::Result<()> {
        let path = "D:\\RustProjects\\hotpot";
        sync_tasks_links(path).unwrap();

        Ok(())
    }

    #[test]
    fn test_get_vuepress_user_dir_entries() -> anyhow::Result<()> {
        let path = "D:\\RustProjects\\hotpot";
        let entries = get_vuepress_user_dir_entries(path);
        dbg!("entries: ", &entries);
        Ok(())
    }

    #[test]
    fn test_remove_vuepress_links() -> anyhow::Result<()> {
        let path = "D:\\RustProjects\\hotpot";
        remove_vuepress_links(path).unwrap();

        Ok(())
    }
}
