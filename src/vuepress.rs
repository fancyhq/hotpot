//! VuePress integration entry point. Owns hub asset deployment, config
//! toggle helpers, the existing docs-symlink utilities, and the
//! `stop_if_running` hook shim. `enabled` is part of an atomic state
//! tied to the on-disk hub project and opt-in prompt assets — flipping
//! it without the install/uninstall path desyncs reality and is treated
//! as a config error by `verify_install_consistency` at start time.
//!
//! VuePress 集成的实现总入口。`enabled` 字段与 `.hotpot-hub/` 目录、
//! opt-in prompt 资产绑定；必须通过 install/uninstall 切换。

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

/// Collect user-directory task-file dirs under `.hotpot-hub/docs/`.
///
/// 获取 .hotpot-hub/docs 下所有的用户目录任务文件目录。
pub fn get_vuepress_user_dir_entries(root_dir: &str) -> Vec<fs::DirEntry> {
    let vuepress_docs_dir = hotpot_hub_dir(root_dir).join("docs");
    if !vuepress_docs_dir.exists() {
        eprintln!("docs directory does not exist");
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

/// Synchronizes `.hotpot/workspaces/<user>/tasks/` symlinks under
/// `.hotpot-hub/docs/<user>/` in three idempotent passes: prune stale
/// links (hub has an entry that workspaces no longer does), keep
/// existing links untouched, create missing links for newly-added
/// users. Called from `vuepress install` and the VuePress-enabled
/// branch of `hotpot update`, so a new collaborator gets their browse
/// view by simply running `hotpot update`.
///
/// 把 `.hotpot/workspaces/<user>/tasks/` 同步软链到
/// `.hotpot-hub/docs/<user>/`，做幂等三件事：清掉 stale 链、保留已有链、为新加入的 user 建链。
/// 调用入口：`hotpot vuepress install` 末段、`hotpot update` 在启用态的尾段都调用，
/// 使"新用户加入 → 跑 update → 自动获得软链"形成闭环。
pub fn sync_tasks_links(root_dir: &str) -> anyhow::Result<()> {
    let workspace_dir = hotpot_dir(root_dir).join("workspaces");
    let hub_docs_dir = hotpot_hub_dir(root_dir).join("docs");
    if !workspace_dir.exists() || !hub_docs_dir.exists() {
        return Err(anyhow::anyhow!("Not found workspace or hub directory."));
    }

    // (1) Snapshot live users in workspaces; OsString covers non-UTF-8 names.
    //     当前 workspaces 下的 user 名集合（用 OsString 兼容非 UTF-8 路径）。
    let workspace_users: std::collections::HashSet<std::ffi::OsString> =
        fs::read_dir(&workspace_dir)?
            .filter_map(|res| res.ok())
            .filter(|res| {
                res.path().is_dir() && !res.file_name().to_str().is_some_and(|f| f.starts_with("."))
            })
            .map(|entry| entry.file_name())
            .collect();

    // (2) Prune stale: hub entries whose user no longer exists in
    //     workspaces. Per-entry failures warn instead of aborting so a
    //     single orphaned permission denial doesn't block the rest.
    //     单条 prune 失败不阻塞——继续处理其余 entry。
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

    // (3) Add missing: link users present in workspaces but not yet in
    //     the hub. Existing entries are left alone to preserve any
    //     VuePress cache linkage. Users whose tasks/ directory is
    //     missing (broken skeleton) are also skipped.
    //     已存在的 entry 跳过——保留任何 vuepress 缓存关联。源 tasks/ 不
    //     存在的 user（workspace 骨架被破坏的边角场景）也跳过。
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

/// Remove all docs symlinks under `.hotpot-hub/docs/`.
///
/// 删除 .hotpot-hub/docs 下绑定的文档软链接。**手动创建的目录将会被一并删除**。
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
            anyhow::anyhow!(
                "failed to remove link, link: {}",
                entry.file_name().display()
            )
        })?;
    }

    Ok(())
}

// ============================================================================
// pnpm 检测
// pnpm detection
// ============================================================================

/// Locates a usable pnpm command name on `PATH`. On Windows pnpm is
/// commonly installed as `pnpm.cmd` or `pnpm.exe`, and Rust's
/// `Command::new("pnpm")` does not auto-suffix. We probe each
/// candidate with `--version` and return the first one whose process
/// exited successfully. Returns a stable `&'static str` so callers can
/// hand it to `Command::new` without lifetime juggling.
///
/// 在 PATH 中查找可用的 pnpm 命令名，按候选顺序逐个 `--version` 探测，第一个成功的就是可用名。
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

/// Bilingual warning comment block written above the `[vuepress]` table
/// on first config insertion. Tells users not to flip `enabled` by hand;
/// `{PORT}` is substituted with the actual port by
/// [`enable_in_config_toml`].
///
/// 首次写入 `[vuepress]` 表时附带的双语警告注释模板，提醒不要手动修改
/// `enabled`；`{PORT}` 占位符会替换为实际端口。
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

/// Writes `[vuepress] enabled = true / port = <port>` to
/// `.hotpot/config.toml`. If the table already exists only those two
/// keys are updated — sibling fields like `ttl_seconds` survive. On
/// first insertion the bilingual warning block from
/// [`VUEPRESS_TABLE_TEMPLATE`] is prepended.
///
/// 启用 VuePress 并写入配置；已有表只更新 `enabled` 与 `port`，首次写入时
/// 带上 [`VUEPRESS_TABLE_TEMPLATE`] 警告注释。
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
        // Existing table: update enabled / port, preserve siblings & comments.
        // 已存在 [vuepress] 表：只动 enabled / port，保留其它字段与原有注释。
        table["enabled"] = toml_edit::value(true);
        table["port"] = toml_edit::value(i64::from(port));
    } else {
        // No table yet: append the commented template and re-parse so the
        // resulting document is well-formed before serializing back.
        // 没有 [vuepress] 表：把带注释的模板拼到现有内容末尾后重新解析。
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

/// Flips `[vuepress] enabled = false` in `.hotpot/config.toml`, leaving
/// `port` / `ttl_seconds` etc. intact for a future re-install. If the
/// file or table is missing the function returns Ok silently — disable
/// means "make sure it's disabled," and missing-equals-disabled.
///
/// 关闭 VuePress 并保留 `port` / `ttl_seconds` 等字段；配置缺失时视为
/// already-disabled，静默成功。
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

/// One-shot VuePress install: deploy hub template → `pnpm install` →
/// `sync_tasks_links` → write `[vuepress] enabled = true` to
/// `.hotpot/config.toml`. Ordered so failure-prone checks (pnpm
/// availability) happen before any disk writes, and the most visible
/// state (config.toml) is the last thing flipped. Idempotent re-runs
/// only no-op the steps whose target is already at the desired state.
///
/// 一站式安装 VuePress 集成；先做 pnpm 检测，再部署资产，最后写
/// config.toml，以避免配置声明与磁盘状态不一致。
pub fn install_hub(root_dir: &str, port: u16, force: bool) -> Result<()> {
    // (1) pnpm must be available.
    // (1) pnpm 必须可用。
    let pnpm = find_pnpm()?;
    println!("Using pnpm: {pnpm}");

    let project_dir = PathBuf::from(root_dir);
    let hub_dir = hotpot_hub_dir(root_dir);

    // (2) Deploy the hub template to .hotpot-hub/.
    // (2) 部署 hub 模板文件到 .hotpot-hub/。
    let stats = assets::install_vuepress_hub(&project_dir, force, false, true)?;
    println!(
        "Hub assets: {} written, {} skipped.",
        stats.written.len(),
        stats.skipped.len()
    );

    // (3) `pnpm install` (long-running).
    // (3) pnpm install (长操作)。
    println!("Running `{pnpm} install` in {} ...", hub_dir.display());
    let status = std::process::Command::new(pnpm)
        .current_dir(&hub_dir)
        .arg("install")
        .status()
        .with_context(|| format!("failed to invoke `{pnpm} install`"))?;
    if !status.success() {
        bail!("`{pnpm} install` exited with status {status}");
    }

    // (4) Symlink user task dirs into the hub docs root. Best-effort:
    //     missing workspace (no tasks created yet) is fine and ignored;
    //     we'll re-link when the user actually creates tasks.
    //     用户还没创建任何任务时 workspace 不存在，此时不报错。
    match sync_tasks_links(root_dir) {
        Ok(()) => println!("Docs symlinks created."),
        Err(err) => {
            // The existing mklink also errors when the workspace dir
            // simply doesn't exist yet; demote that to an informational
            // line instead of a hard failure.
            // 现有 mklink 在 workspace 不存在时也返回 Err；这里软化为 info。
            println!("Skipping docs symlinks (no workspace tasks yet, or hub missing): {err}");
        }
    }

    // (5) Install the opt-in prompt assets BEFORE flipping the config
    //     switch — if prompt write fails, `enabled` stays false and the
    //     atomic state remains consistent.
    //     先于 config 翻转，避免 prompt 写盘失败后留下启用但缺资产的状态。
    let prompt_stats = assets::install_vuepress_prompts(&project_dir, force, false, true)?;
    println!(
        "Opt-in prompts: {} written, {} skipped.",
        prompt_stats.written.len(),
        prompt_stats.skipped.len()
    );

    // (6) Flip the config.toml switch LAST, so any failure above leaves
    //     enabled=false (i.e. we never lie about install completeness).
    //     最后翻转，避免出现 enabled=true 但 hub / 提示词残缺的状态。
    enable_in_config_toml(root_dir, port)?;
    println!("Set [vuepress] enabled = true / port = {port} in .hotpot/config.toml.");

    println!("VuePress installed. Start with `hotpot vuepress start`.");
    Ok(())
}

/// One-shot VuePress uninstall: stop service → flip
/// `[vuepress] enabled = false` (so hooks stop injecting VP env
/// immediately) → tear down docs symlinks → remove `.hotpot-hub/`.
/// Stops are best-effort: a failure to stop merely warns and we
/// continue with the rest of the teardown.
///
/// 一站式卸载 VuePress 集成；先关闭 config 开关，再清理资源，避免 hook
/// 读到 enabled=true 但磁盘已空的中间态。
pub fn uninstall_hub(root_dir: &str) -> Result<()> {
    // (1) Stop first (tolerates the lifecycle stubs we'll fill in later).
    // (1) 先停服务（容错：如果 service lifecycle 还没接入，本次只是 noop）。
    if let Err(err) = stop_if_running(root_dir) {
        eprintln!("Warning: vuepress stop failed: {err}");
    }

    // (2) Flip the switch first so any hook firing during the rest of
    //     this command sees `enabled=false` and stops injecting VP env.
    //     必须先于资源清理，让 hooks 在下一轮就停止注入 env。
    disable_in_config_toml(root_dir)?;
    println!("Set [vuepress] enabled = false in .hotpot/config.toml.");

    // (3) Remove opt-in prompt assets, catalog-driven to keep this in
    //     sync with installs. A missing file is fine — uninstall just
    //     wants the file gone.
    //     用 catalog 驱动，避免重复维护文件清单；单个文件缺失不算错。
    let project_dir = PathBuf::from(root_dir);
    for rel in assets::vuepress_prompt_paths() {
        let target = project_dir.join(rel);
        match fs::remove_file(&target) {
            Ok(()) => println!("Removed {}.", target.display()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => eprintln!("Warning: failed to remove {}: {err}", target.display()),
        }
    }

    // (4) Tear down docs symlinks.
    // (4) 拆 docs 软链。
    if let Err(err) = remove_vuepress_links(root_dir) {
        eprintln!("Warning: failed to remove docs symlinks: {err}");
    }

    // (5) Remove the .hotpot-hub/ tree. runtime.json lives inside it so
    //     it is cleaned up automatically.
    //     runtime.json 也住在里面，会自然连带消失。
    let hub_dir = hotpot_hub_dir(root_dir);
    if hub_dir.exists() {
        fs::remove_dir_all(&hub_dir)
            .with_context(|| format!("failed to remove {}", hub_dir.display()))?;
        println!("Removed {}.", hub_dir.display());
    }

    println!("VuePress uninstalled. Run `hotpot vuepress install` to re-enable.");
    Ok(())
}

/// Pre-start sanity check. Catches the "user flipped enabled by hand"
/// case by requiring all three pieces of the atomic state to agree:
/// (a) `[vuepress] enabled = true` in config.toml, (b) the hub
/// `package.json` exists on disk, and (c) the opt-in `vuepress.md`
/// prompt exists. Any failure bails with a repair hint.
///
/// 启动前的三者一致性校验，用于拦截手动修改 enabled 导致的原子状态错配。
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
// Service-management stubs (real logic lands in task #7)
// 服务管理 stub（Task #7 填充实际逻辑）
// ============================================================================

/// Idempotent shorthand for `stop(root_dir, true)`. Used by the
/// `/hotpot:execute` pre-flight directive and by every platform's
/// SessionEnd hook so they can call vuepress cleanup unconditionally
/// without first checking whether anything is running.
///
/// 幂等地停止当前可能在跑的 vuepress dev server；runtime.json 不存在或 pid
/// 已死时均返回成功。
pub fn stop_if_running(root_dir: &str) -> Result<()> {
    stop(root_dir, true)
}

// ============================================================================
// Service lifecycle: runtime.json + start / stop / status
// 服务管理：runtime.json + start / stop / status
// ============================================================================

/// Schema of `.hotpot-hub/vuepress.runtime.json`. The file is the
/// single source of truth for what we believe is running; liveness is
/// independently verified through [`is_pid_alive`] before trusting any
/// field.
///
/// `.hotpot-hub/vuepress.runtime.json` 的内容 schema；进程存活性通过
/// [`is_pid_alive`] 单独验证。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeState {
    pid: u32,
    port: u16,
    url: String,
    started_at: DateTime<Utc>,
    /// `None` means no expiry (user passed `--ttl 0`); `Some` is treated
    /// as stale once reached and triggers lazy cleanup.
    /// `None` 表示无过期（用户传 `--ttl 0`）；`Some` 表示到期后视为 stale。
    expires_at: Option<DateTime<Utc>>,
}

/// Runtime state file lives inside `.hotpot-hub/` so `uninstall_hub` /
/// 用户手动 rm -rf `.hotpot-hub` 都能顺带把它清理掉，避免悬挂状态。
///
/// 返回 runtime.json 的绝对路径。
fn runtime_json_path(root_dir: &str) -> PathBuf {
    hotpot_hub_dir(root_dir).join("vuepress.runtime.json")
}

/// Writes runtime.json, creating parent directories as needed.
///
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

/// Reads runtime.json; missing or corrupt files return `None`.
///
/// 读取 runtime.json；不存在或损坏均返回 `None`，由调用方按"未运行"语义处理。
fn read_runtime_state(root_dir: &str) -> Option<RuntimeState> {
    let path = runtime_json_path(root_dir);
    let raw = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Silently discards runtime.json; missing is not an error.
///
/// 静默清理 runtime.json；缺失不算错。stale 状态恢复时调用。
fn discard_runtime_state(root_dir: &str) {
    let path = runtime_json_path(root_dir);
    let _ = fs::remove_file(&path);
}

/// Unix: `kill -0 <pid>` exit code 0 = alive; non-zero = dead/no
/// permission. Windows: `tasklist /FI "PID eq <pid>" /NH` stdout
/// contains the pid string iff a process with that pid exists.
///
/// 跨平台进程活跃性检测。
/// 任意检测失败 → 视为已死，让 stale 清理流程接管。
fn is_pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
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
                // tasklist exits successfully even when nothing matches
                // ("INFO: No tasks..."), so we look for the pid literal
                // in stdout — present iff a row was emitted.
                // 无匹配时仍返回 success；命中时输出行包含 pid 数字。
                stdout.contains(&pid.to_string())
            }
            _ => false,
        }
    }
}

/// Process target selected by the VuePress cleanup planner.
///
/// VuePress 清理计划选中的进程目标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CleanupTarget {
    /// Terminate exactly one process id.
    /// 只终止单个进程 id。
    Pid(u32),
    /// Terminate the Unix process group whose id is the runtime pid.
    /// 终止 Unix 上以 runtime pid 为组号的进程组。
    #[cfg(unix)]
    ProcessGroup(u32),
    /// Terminate a Windows process tree with `taskkill /T`.
    /// 使用 Windows `taskkill /T` 终止进程树。
    #[cfg(windows)]
    ProcessTree(u32),
}

/// Returns the cleanup targets for a runtime pid.
///
/// 返回 runtime pid 对应的清理目标列表。
fn cleanup_targets_for_runtime_pid(pid: u32) -> Vec<CleanupTarget> {
    #[cfg(unix)]
    {
        vec![CleanupTarget::ProcessGroup(pid), CleanupTarget::Pid(pid)]
    }

    #[cfg(windows)]
    {
        vec![CleanupTarget::ProcessTree(pid)]
    }

    #[cfg(not(any(unix, windows)))]
    {
        vec![CleanupTarget::Pid(pid)]
    }
}

/// A process currently owning the runtime port.
///
/// 当前占用 runtime 端口的进程候选。
#[derive(Debug, Clone, PartialEq, Eq)]
struct PortOwner {
    pid: u32,
    command: String,
}

/// Filters port owners down to Hotpot VuePress-related processes.
///
/// 将端口占用进程收窄到 Hotpot VuePress 相关进程。
fn hotpot_vuepress_port_cleanup_targets(hub_dir: &Path, owners: &[PortOwner]) -> Vec<u32> {
    let hub = hub_dir.to_string_lossy();
    owners
        .iter()
        .filter(|owner| {
            let command = owner.command.to_ascii_lowercase();
            owner.command.contains(hub.as_ref())
                && (command.contains("vuepress")
                    || command.contains("vite")
                    || command.contains("pnpm")
                    || command.contains("docs:dev"))
        })
        .map(|owner| owner.pid)
        .collect()
}

/// Reads process ids listening on a TCP port.
///
/// 读取监听指定 TCP 端口的进程 id。
fn list_port_owners(port: u16) -> Vec<PortOwner> {
    #[cfg(unix)]
    {
        let output = std::process::Command::new("lsof")
            .args(["-nP", "-sTCP:LISTEN", "-Fp", &format!("-iTCP:{port}")])
            .output();
        let Ok(output) = output else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .filter_map(|line| line.strip_prefix('p'))
            .filter_map(|pid| pid.parse::<u32>().ok())
            .map(|pid| PortOwner {
                pid,
                command: process_command_line(pid).unwrap_or_default(),
            })
            .collect()
    }

    #[cfg(not(unix))]
    {
        let _ = port;
        Vec::new()
    }
}

/// Reads a process command line for conservative ownership checks.
///
/// 读取进程命令行，用于保守判断端口占用是否属于 Hotpot VuePress。
fn process_command_line(pid: u32) -> Option<String> {
    #[cfg(unix)]
    {
        let output = std::process::Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "command="])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        None
    }
}

/// Terminates one cleanup target with the requested force level.
///
/// 按指定强度终止一个清理目标。
fn terminate_cleanup_target(target: &CleanupTarget, force: bool) -> Result<()> {
    match target {
        CleanupTarget::Pid(pid) => terminate_pid(*pid, force),
        #[cfg(unix)]
        CleanupTarget::ProcessGroup(pid) => terminate_unix_process_group(*pid, force),
        #[cfg(windows)]
        CleanupTarget::ProcessTree(pid) => terminate_windows_process_tree(*pid, force),
    }
}

/// Terminates the Unix process group created by `setsid()`.
///
/// 终止由 `setsid()` 创建的 Unix 进程组。
#[cfg(unix)]
fn terminate_unix_process_group(pid: u32, force: bool) -> Result<()> {
    let signal = if force { "-KILL" } else { "-TERM" };
    let group = format!("-{pid}");
    std::process::Command::new("kill")
        .args([signal, &group])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .with_context(|| format!("failed to invoke `kill {signal} {group}`"))?;
    Ok(())
}

/// Returns the Unix `kill` arguments for a cleanup target.
///
/// 返回 Unix 清理目标对应的 `kill` 参数，便于测试不会污染 stderr 的调用形态。
#[cfg(all(unix, test))]
fn unix_kill_args_for_cleanup_target(target: CleanupTarget, force: bool) -> Vec<String> {
    let signal = if force { "-KILL" } else { "-TERM" }.to_string();
    let target = match target {
        CleanupTarget::Pid(pid) => pid.to_string(),
        CleanupTarget::ProcessGroup(pid) => format!("-{pid}"),
    };
    vec![signal, target]
}

/// Terminates a Windows process tree with `taskkill /T`.
///
/// 使用 `taskkill /T` 终止 Windows 进程树。
#[cfg(windows)]
fn terminate_windows_process_tree(pid: u32, force: bool) -> Result<()> {
    let mut cmd = std::process::Command::new("taskkill");
    cmd.args(["/PID", &pid.to_string(), "/T"]);
    if force {
        cmd.arg("/F");
    }
    cmd.status()
        .with_context(|| format!("failed to invoke `taskkill /PID {pid} /T`"))?;
    Ok(())
}

/// Runs the shared TERM -> wait -> KILL cleanup sequence.
///
/// 执行共享的 TERM -> 等待 -> KILL 清理流程。
fn cleanup_runtime_process(root_dir: &str, state: &RuntimeState, print_stopped: bool) {
    let targets = cleanup_targets_for_runtime_pid(state.pid);
    if is_pid_alive(state.pid) {
        for target in &targets {
            let _ = terminate_cleanup_target(target, false);
        }

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline && is_pid_alive(state.pid) {
            thread::sleep(Duration::from_millis(200));
        }

        if is_pid_alive(state.pid) {
            for target in &targets {
                let _ = terminate_cleanup_target(target, true);
            }
        }
    }

    cleanup_runtime_port(root_dir, state.port);
    discard_runtime_state(root_dir);

    if print_stopped {
        println!(
            "Stopped vuepress (pid {}, was running on port {}).",
            state.pid, state.port
        );
    }
}

/// Best-effort cleanup for VuePress-like processes still owning the runtime port.
///
/// 对仍占用 runtime 端口的 VuePress 类进程做尽力清理。
fn cleanup_runtime_port(root_dir: &str, port: u16) {
    let hub_dir = hotpot_hub_dir(root_dir);
    let owners = list_port_owners(port);
    for pid in hotpot_vuepress_port_cleanup_targets(&hub_dir, &owners) {
        let _ = terminate_pid(pid, false);
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline && is_pid_alive(pid) {
            thread::sleep(Duration::from_millis(100));
        }
        if is_pid_alive(pid) {
            let _ = terminate_pid(pid, true);
        }
    }
}

/// Terminates a process cross-platform.
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
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
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

/// Hard upper bound for `wait_for_port_ready`; the empirical ceiling
/// for a first pnpm + vuepress build of this hub. Defined above the
/// consumer per the module's "constants before usage" convention
/// (see `VUEPRESS_TABLE_TEMPLATE`).
///
/// `wait_for_port_ready` 的硬上限；超过就 bail 并附 vuepress.log tail。
const READINESS_TIMEOUT: Duration = Duration::from_secs(30);

/// Per-attempt connect timeout (250 ms): long enough for the kernel
/// to complete a local SYN handshake, short enough to keep the poll
/// loop responsive.
///
/// 单次 `TcpStream::connect_timeout` 调用的超时。
const READINESS_CONNECT_TIMEOUT: Duration = Duration::from_millis(250);

/// Poll interval between connect attempts: yields CPU and gives Vite
/// a moment to make progress. With a 30s hard cap that's up to ~120
/// probes, which is plenty of granularity.
///
/// 两次 connect 尝试之间的等待，让出 CPU 并给 vite 构建时间。
const READINESS_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Polls `127.0.0.1:<port>` until vuepress accepts a TCP connection.
/// `READINESS_TIMEOUT` (30s) is the empirical upper bound for a first
/// pnpm+vuepress build of this hub. On timeout returns `Err`; the
/// caller should attach the tail of `vuepress.log` as a diagnostic
/// hint so the user can see what pnpm logged before bailing.
///
/// 等 `127.0.0.1:<port>` 接受 TCP 连接，作为 vuepress 端口就绪的判据。
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

/// Best-effort tail of `vuepress.log`. Any IO error returns an empty
/// string; the readiness timeout error is the primary signal.
///
/// 读 `vuepress.log` 末尾 `max_lines` 行作为 readiness 超时诊断附录。
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
///
/// 启动 VuePress dev server；spawn 后写 runtime.json 并输出单行 JSON，
/// detach 与日志重定向必须同时保留，避免父 shell 等 EOF 的工具 hang 住。
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

    // Write runtime.json BEFORE the readiness probe: during the probe
    // window pnpm is already alive, and any concurrent reader (a
    // SessionEnd hook, a parallel `vuepress status`, or `vuepress
    // stop`) that finds no runtime.json wrongly concludes "not
    // running" — leaking the just-spawned process and making stop
    // bail. Hoisting the writeback closes that visibility gap.
    // readiness probe 之前落盘，关闭并发 status/stop 误判为未运行的可见性窗口。
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

    // Port-readiness probe (P-A): VuePress's Vite dev server forks
    // first and binds the port only after the build worker completes
    // an initial pass. Printing the URL before the bind would let the
    // user click into a connection-refused / spinning browser tab,
    // which they mis-read as "start hung". Block up to 30s here until
    // `127.0.0.1:<port>` accepts a TCP connect — this hub typically
    // becomes ready in 5–15s; 30s leaves margin for cold caches and
    // slow disks. On timeout we bail with the log tail attached so
    // the user has something concrete to debug.
    // 端口就绪探测阻塞最多 30s，避免 URL 打印过早导致 connection refused。
    if let Err(err) = wait_for_port_ready(port) {
        let tail = read_log_tail(&log_path, 20);
        // Reuse stop's TERM → 3 s poll → KILL escalation AND its
        // runtime.json cleanup so pnpm and its grandchildren
        // (vuepress.js / esbuild) are reaped together, not just the
        // group leader. This also keeps runtime.json consistent with
        // reality. Errors from stop are best-effort — the
        // readiness-timeout error is the primary signal.
        // 复用 stop 清理整棵进程树与 runtime.json；readiness-timeout 是主信号。
        let _ = stop(root_dir, true);
        let tail_msg = if tail.is_empty() {
            "vuepress.log tail: <empty>".to_string()
        } else {
            format!("vuepress.log tail:\n{tail}")
        };
        return Err(err.context(tail_msg));
    }

    // Single-line JSON: the brainstorming-closing prompt parses this
    // directly to extract the URL.
    // 单行 JSON：AI 用 Bash 拿到 stdout 后直接 JSON.parse。
    let summary = serde_json::json!({ "url": url, "pid": pid });
    println!("{summary}");

    Ok(())
}

/// Stops the VuePress dev server. `if_running=true` is the idempotent
/// mode used by hook callers — missing runtime.json or dead pid both
/// resolve to Ok. `if_running=false` (normal CLI) bails when nothing is
/// running. Kill sequence: TERM → 3 s polling wait → KILL if still
/// alive → remove runtime.json.
///
/// 停止 VuePress dev server；`if_running=true` 为幂等模式，普通 CLI 模式
/// 在未运行时返回错误。
pub fn stop(root_dir: &str, if_running: bool) -> Result<()> {
    let Some(state) = read_runtime_state(root_dir) else {
        if if_running {
            return Ok(());
        }
        bail!("vuepress is not running (no runtime.json found)");
    };

    cleanup_runtime_process(root_dir, &state, true);
    Ok(())
}

/// Reports the dev server's running state as one-line JSON. Lazily
/// cleans up stale runtime files (dead pid or expired ttl); for the
/// ttl-expired case we also kill the process so the OS doesn't carry
/// the leak indefinitely.
///
/// 查询当前 VuePress dev server 运行状态；输出单行 JSON，并懒清理 stale 状态。
pub fn status(root_dir: &str) -> Result<()> {
    let mut output = serde_json::Map::new();
    output.insert("running".into(), serde_json::Value::Bool(false));

    if let Some(state) = read_runtime_state(root_dir) {
        let alive = is_pid_alive(state.pid);
        let expired = match state.expires_at {
            Some(exp) => exp <= Utc::now(),
            None => false,
        };

        if !alive || expired {
            // Dead pid or expired ttl: reuse stop cleanup, including
            // process-group/tree and runtime-port fallback.
            // pid 已死或 ttl 过期：复用 stop 清理，包括进程组/进程树和端口兜底。
            cleanup_runtime_process(root_dir, &state, false);
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
    use tempfile::TempDir;

    #[test]
    fn vuepress_new_prompt_requires_prewrite_style_gate() -> anyhow::Result<()> {
        let prompt = read_repo_text("assets/prompts/hotpot-new.md")?;
        let section = prompt
            .split("## Optional: VuePress Integration")
            .nth(1)
            .context("missing Optional: VuePress Integration section")?;

        assert!(
            section.contains("BEFORE creating the task file"),
            "VuePress integration must require the gate before creating the task file"
        );
        assert!(
            section.contains("vuepress-style.md"),
            "VuePress integration must require reading vuepress-style.md"
        );
        assert!(
            section.contains("BEFORE Step 7"),
            "VuePress integration must place the style gate before the task-file write step"
        );

        Ok(())
    }

    #[test]
    fn codex_new_skill_forbids_two_phase_vuepress_task_write() -> anyhow::Result<()> {
        let skill = read_repo_text(".codex/skills/hotpot-new/SKILL.md")?;

        assert!(
            skill.contains("Do NOT create a plain Markdown task file first"),
            "Codex skill must forbid plain-Markdown-first task creation"
        );
        assert!(
            skill.contains("*** Add File"),
            "Codex skill must require apply_patch Add File for the final task file"
        );
        assert!(
            skill.contains("vuepress-style.md"),
            "Codex skill must mention the VuePress style prompt"
        );

        Ok(())
    }

    #[test]
    fn stop_uses_process_group_on_unix_when_runtime_pid_is_alive() {
        let targets = cleanup_targets_for_runtime_pid(4242);

        #[cfg(unix)]
        assert_eq!(
            targets,
            vec![CleanupTarget::ProcessGroup(4242), CleanupTarget::Pid(4242)],
            "Unix cleanup must target the setsid-created process group before the fallback pid"
        );

        #[cfg(windows)]
        assert_eq!(
            targets,
            vec![CleanupTarget::ProcessTree(4242)],
            "Windows cleanup must preserve taskkill process-tree semantics"
        );
    }

    #[test]
    #[cfg(unix)]
    fn unix_cleanup_target_kill_args_encode_process_group() {
        assert_eq!(
            unix_kill_args_for_cleanup_target(CleanupTarget::ProcessGroup(4242), false),
            vec!["-TERM".to_string(), "-4242".to_string()],
            "Unix process-group cleanup must use a negative pid target"
        );
        assert_eq!(
            unix_kill_args_for_cleanup_target(CleanupTarget::Pid(4242), true),
            vec!["-KILL".to_string(), "4242".to_string()],
            "Unix pid fallback must remain available after process-group cleanup"
        );
    }

    #[test]
    fn stop_if_running_cleans_runtime_when_pid_dead_but_port_has_vuepress_owner() {
        let hub_dir = Path::new("/repo/.hotpot-hub");
        let owners = vec![
            PortOwner {
                pid: 10,
                command: "python -m http.server 8080".to_string(),
            },
            PortOwner {
                pid: 11,
                command: "/repo/.hotpot-hub/node_modules/.bin/vuepress dev docs --port 8080"
                    .to_string(),
            },
            PortOwner {
                pid: 12,
                command: "pnpm docs:dev --port 8080".to_string(),
            },
        ];

        assert_eq!(
            hotpot_vuepress_port_cleanup_targets(hub_dir, &owners),
            vec![11],
            "port fallback must only include identifiable Hotpot VuePress owners under the hub"
        );
    }

    #[test]
    fn status_expired_ttl_reuses_stop_cleanup() {
        let targets = cleanup_targets_for_runtime_pid(31337);

        #[cfg(unix)]
        assert!(
            targets.contains(&CleanupTarget::ProcessGroup(31337)),
            "TTL cleanup must reuse process-group cleanup, not single-pid termination"
        );

        #[cfg(windows)]
        assert!(
            targets.contains(&CleanupTarget::ProcessTree(31337)),
            "TTL cleanup must reuse process-tree cleanup, not single-pid termination"
        );
    }

    #[test]
    fn arch_documents_vuepress_prewrite_gate_and_process_cleanup() -> anyhow::Result<()> {
        let english = read_repo_text("docs/ARCH.md")?;
        let chinese = read_repo_text("docs/ARCH.zh_CN.md")?;

        assert!(
            english.contains("before writing the task file"),
            "ARCH.md must describe the VuePress style gate before writing the task file"
        );
        assert!(
            english.contains("process group")
                && english.contains("process tree")
                && english.contains("runtime port fallback")
                && english.contains("TTL"),
            "ARCH.md must describe pid, process-group/tree, TTL, and port cleanup defenses"
        );
        assert!(
            chinese.contains("写任务文件前"),
            "ARCH.zh_CN.md must describe the VuePress style gate before writing the task file"
        );
        assert!(
            chinese.contains("进程组")
                && chinese.contains("进程树")
                && chinese.contains("runtime 端口兜底")
                && chinese.contains("TTL"),
            "ARCH.zh_CN.md must describe pid, process-group/tree, TTL, and port cleanup defenses"
        );

        Ok(())
    }

    #[test]
    fn test_sync_tasks_links() -> anyhow::Result<()> {
        let root = temp_vuepress_root("sync-links")?;
        fs::create_dir_all(root.path().join(".hotpot/workspaces/alice/tasks"))?;
        fs::create_dir_all(root.path().join(".hotpot-hub/docs"))?;

        sync_tasks_links(&root.path().display().to_string())?;

        assert!(
            root.path()
                .join(".hotpot-hub/docs/alice")
                .symlink_metadata()
                .is_ok()
        );

        Ok(())
    }

    #[test]
    fn test_get_vuepress_user_dir_entries() -> anyhow::Result<()> {
        let root = temp_vuepress_root("entries")?;
        fs::create_dir_all(root.path().join(".hotpot-hub/docs/alice"))?;

        let entries = get_vuepress_user_dir_entries(&root.path().display().to_string());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file_name(), "alice");
        Ok(())
    }

    #[test]
    fn test_remove_vuepress_links() -> anyhow::Result<()> {
        let root = temp_vuepress_root("remove-links")?;
        fs::create_dir_all(root.path().join(".hotpot/workspaces/alice/tasks"))?;
        fs::create_dir_all(root.path().join(".hotpot-hub/docs"))?;
        sync_tasks_links(&root.path().display().to_string())?;

        remove_vuepress_links(&root.path().display().to_string())?;

        assert!(
            root.path()
                .join(".hotpot-hub/docs/alice")
                .symlink_metadata()
                .is_err()
        );

        Ok(())
    }

    /// Creates an isolated temporary root for VuePress filesystem tests.
    ///
    /// 为 VuePress 文件系统测试创建隔离临时根目录，避免依赖开发者本机绝对路径。
    fn temp_vuepress_root(label: &str) -> anyhow::Result<TempDir> {
        let root = tempfile::Builder::new()
            .prefix(&format!("hotpot-vuepress-{label}-"))
            .tempdir()?;
        fs::create_dir_all(root.path())?;
        Ok(root)
    }

    /// Reads a repository file relative to `CARGO_MANIFEST_DIR`.
    ///
    /// 按 `CARGO_MANIFEST_DIR` 读取仓库内文件，供文本资产断言测试复用。
    fn read_repo_text(relative: &str) -> anyhow::Result<String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))
    }
}
