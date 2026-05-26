//! CLI shell for the `hotpot vuepress` family.
//!
//! Splits into asset lifecycle (install / uninstall) and service
//! lifecycle (start / stop / status); real work lives in
//! [`crate::vuepress`]. Port resolution mirrors
//! [`crate::context::resolve_vuepress_port`] so the value the hook
//! layer injects matches what `start` actually binds.
//!
//! `hotpot vuepress` 子命令族的 CLI 入口。本文件是个薄壳：解析 clap
//! 参数后转交 [`crate::vuepress`] 模块完成实际工作。子命令分两类——
//!
//! - 资产生命周期：[`VuepressCommand::Install`] / [`VuepressCommand::Uninstall`]
//!   维护 `.hotpot-hub/` 目录、`.hotpot/config.toml::[vuepress] enabled`
//!   开关、opt-in prompt 资产之间的原子状态。
//! - 服务生命周期：[`VuepressCommand::Start`] / [`VuepressCommand::Stop`] /
//!   [`VuepressCommand::Status`] 管理 `pnpm docs:dev` 子进程与
//!   `.hotpot-hub/vuepress.runtime.json` 状态文件。
//!
//! 端口解析链：CLI `--port` flag → `.hotpot/config.toml::[vuepress] port`
//! → 字面量 `8080`。这与 [`crate::context::resolve_vuepress_port`] 保持
//! 一致，让 hook 注入的 `HOTPOT_VUEPRESS_PORT` 与 CLI 实际启动端口对齐。

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{context, vuepress};

/// All subcommands of `hotpot vuepress`.
///
/// `hotpot vuepress` 子命令集合。
#[derive(Subcommand, Debug)]
pub enum VuepressCommand {
    /// Install the VuePress hub project under `.hotpot-hub/` and enable
    /// the integration.
    ///
    /// 部署 VuePress hub 项目并启用集成。
    #[command(
        about = "Install the VuePress hub project under .hotpot-hub/ and enable the integration",
        long_about = None
    )]
    Install(InstallArgs),
    /// Uninstall the VuePress hub project and disable the integration.
    ///
    /// 卸载 VuePress hub 项目并关闭集成。
    #[command(
        about = "Uninstall the VuePress hub project and disable the integration",
        long_about = None
    )]
    Uninstall,
    /// Start the VuePress dev server in the background.
    ///
    /// 后台启动 VuePress dev server。
    #[command(
        about = "Start the VuePress dev server in the background",
        long_about = None
    )]
    Start(StartArgs),
    /// Stop the VuePress dev server (if running).
    ///
    /// 停止运行中的 VuePress dev server。
    #[command(
        about = "Stop the VuePress dev server (if running)",
        long_about = None
    )]
    Stop(StopArgs),
    /// Print VuePress dev server runtime state as JSON.
    ///
    /// 以 JSON 形式输出 VuePress dev server 运行时状态。
    #[command(
        about = "Print VuePress dev server runtime state as JSON",
        long_about = None
    )]
    Status,
}

/// CLI arguments for `hotpot vuepress install`.
///
/// `hotpot vuepress install` 的 CLI 参数。
#[derive(Args, Debug)]
#[command(
    about = "Install VuePress hub project under .hotpot-hub/",
    long_about = None
)]
pub struct InstallArgs {
    /// Port persisted to `.hotpot/config.toml::[vuepress] port`. Defaults
    /// to the value already in config (or `8080` on first install).
    ///
    /// 写入 `.hotpot/config.toml` 的端口；不传则取现有配置（或首次
    /// 安装的字面量 `8080`）。
    #[arg(
        long,
        help = "Port persisted to .hotpot/config.toml; defaults to existing config or 8080",
        long_help = None
    )]
    port: Option<u16>,

    /// Overwrite differing files in `.hotpot-hub/`. Has no effect on
    /// `.hotpot/config.toml` (which is always merged) or on user-edited
    /// files that already match the bundled template.
    ///
    /// 覆盖 `.hotpot-hub/` 中内容不一致的文件。`config.toml` 始终合并；
    /// 已匹配模板的文件不受影响。
    #[arg(
        long,
        help = "Overwrite differing files in .hotpot-hub/; config.toml always merges",
        long_help = None
    )]
    force: bool,
}

/// CLI arguments for `hotpot vuepress start`.
///
/// `hotpot vuepress start` 的 CLI 参数。
#[derive(Args, Debug)]
#[command(
    about = "Start the VuePress dev server in the background",
    long_about = None
)]
pub struct StartArgs {
    /// Port to bind. Defaults to the resolved
    /// `.hotpot/config.toml::[vuepress] port`.
    ///
    /// 绑定的端口；不传则取 `.hotpot/config.toml::[vuepress] port` 的
    /// 解析值。
    #[arg(
        long,
        help = "Port to bind; defaults to .hotpot/config.toml [vuepress] port",
        long_help = None
    )]
    port: Option<u16>,

    /// Hard expiry in seconds before the server is auto-killed on next
    /// `status` poll. `0` disables expiry. Default `1800` (30 minutes).
    /// Acts as the fallback when neither `/hotpot:execute` pre-flight
    /// nor SessionEnd hook fires (e.g. on Codex where SessionEnd is
    /// unsupported).
    ///
    /// 服务硬过期秒数（兜底）：到期后下次 `status` 调用会自动 kill。
    /// `0` 表示不过期，默认 1800 秒。补 Codex 平台缺少 SessionEnd 留下
    /// 的缺口。
    #[arg(
        long,
        default_value_t = 1800,
        help = "Hard expiry in seconds before auto-kill; 0 disables expiry",
        long_help = None
    )]
    ttl: u64,
}

/// CLI arguments for `hotpot vuepress stop`.
///
/// `hotpot vuepress stop` 的 CLI 参数。
#[derive(Args, Debug)]
#[command(
    about = "Stop the VuePress dev server (if running)",
    long_about = None
)]
pub struct StopArgs {
    /// Treat "nothing running" as success. Used by hook integrations
    /// and the `/hotpot:execute` pre-flight where the call must be
    /// idempotent.
    ///
    /// 把"没在跑"视作成功。供 hook 与 `/hotpot:execute` 入口幂等调用。
    #[arg(
        long = "if-running",
        help = "Treat nothing running as success; enables idempotent calls",
        long_help = None
    )]
    if_running: bool,
}

/// 入口：`hotpot vuepress install`。
pub fn install(args: InstallArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    // 端口解析：CLI flag 优先；缺省则取 config.toml 解析链返回值（含 8080
    // 默认）。这样首次 install 无 --port 参数 = 端口 8080。
    // Port resolution: CLI flag wins; fall back to the config-driven
    // resolver (which itself defaults to 8080 on a fresh project).
    let port = args
        .port
        .unwrap_or_else(|| context::resolve_vuepress_port(&root_dir));
    vuepress::install_hub(&root_dir, port, args.force)
}

/// 入口：`hotpot vuepress uninstall`。
pub fn uninstall() -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    vuepress::uninstall_hub(&root_dir)
}

/// 入口：`hotpot vuepress start`。
pub fn start(args: StartArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let port = args
        .port
        .unwrap_or_else(|| context::resolve_vuepress_port(&root_dir));
    vuepress::start(&root_dir, port, args.ttl)
}

/// 入口：`hotpot vuepress stop`。
pub fn stop(args: StopArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    vuepress::stop(&root_dir, args.if_running)
}

/// 入口：`hotpot vuepress status`。
pub fn status() -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    vuepress::status(&root_dir)
}
