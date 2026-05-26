use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{context, server};

/// Subcommands of `hotpot server`.
///
/// `hotpot server` 的子命令集合。
#[derive(Subcommand, Debug)]
pub enum ServerCommand {
    /// Start the Hotpot local HTTP server.
    ///
    /// 启动 Hotpot 本地 HTTP 服务。
    #[command(
        about = "Start the Hotpot local HTTP server",
        long_about = None
    )]
    Start(StartArgs),
    /// Internal worker entry that runs the bound HTTP server in-process.
    ///
    /// 内部 worker 入口：在当前进程内运行已绑定的 HTTP 服务。
    #[command(hide = true)]
    Serve(ServeArgs),
    /// Stop a running Hotpot local HTTP server.
    ///
    /// 停止运行中的 Hotpot 本地 HTTP 服务。
    #[command(
        about = "Stop a running Hotpot local HTTP server",
        long_about = None
    )]
    Stop(StopArgs),
}

/// CLI arguments for `hotpot server start`.
///
/// `hotpot server start` 的 CLI 参数。
#[derive(Args, Debug)]
#[command(
    about = "Start the Hotpot local HTTP server",
    long_about = None
)]
pub struct StartArgs {
    /// Project directory to serve; defaults to the resolved Hotpot root.
    ///
    /// 服务对应的项目目录；默认取 Hotpot 解析出的项目根。
    #[arg(
        long,
        value_name = "DIR",
        help = "Project directory to serve; defaults to the resolved Hotpot root",
        long_help = None
    )]
    project_dir: Option<PathBuf>,

    /// Bind address for the HTTP server.
    ///
    /// HTTP 服务绑定地址。
    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "Bind address for the HTTP server",
        long_help = None
    )]
    host: String,

    /// Public host name used when printing the URL; defaults to `host`.
    ///
    /// 打印 URL 时使用的公开 host 名；默认与 `host` 一致。
    #[arg(
        long,
        help = "Public host name used when printing the URL; defaults to `host`",
        long_help = None
    )]
    url_host: Option<String>,

    /// TCP port to bind; `0` picks a random free port.
    ///
    /// 绑定的 TCP 端口；`0` 表示由系统挑选空闲端口。
    #[arg(
        long,
        default_value_t = 0,
        help = "TCP port to bind; 0 picks a random free port",
        long_help = None
    )]
    port: u16,

    /// Detach and run the server as a background daemon.
    ///
    /// 以后台 daemon 形式运行服务。
    #[arg(
        long,
        help = "Detach and run the server as a background daemon",
        long_help = None
    )]
    daemon: bool,
}

/// CLI arguments for the internal `hotpot server serve` worker.
///
/// 内部 `hotpot server serve` worker 的 CLI 参数。
#[derive(Args, Debug)]
pub struct ServeArgs {
    /// Session directory the worker reads / writes runtime state under.
    ///
    /// worker 用来读写运行时状态的 session 目录。
    #[arg(
        long,
        value_name = "DIR",
        help = "Session directory for runtime state",
        long_help = None
    )]
    pub session_dir: PathBuf,

    /// Bind address for the HTTP server.
    ///
    /// HTTP 服务绑定地址。
    #[arg(
        long,
        help = "Bind address for the HTTP server",
        long_help = None
    )]
    pub host: String,

    /// Public host name used in printed URLs.
    ///
    /// 打印 URL 时使用的公开 host 名。
    #[arg(
        long,
        help = "Public host name used in printed URLs",
        long_help = None
    )]
    pub url_host: String,

    /// TCP port to bind.
    ///
    /// 绑定的 TCP 端口。
    #[arg(
        long,
        help = "TCP port to bind",
        long_help = None
    )]
    pub port: u16,
}

/// CLI arguments for `hotpot server stop`.
///
/// `hotpot server stop` 的 CLI 参数。
#[derive(Args, Debug)]
#[command(
    about = "Stop a running Hotpot local HTTP server",
    long_about = None
)]
pub struct StopArgs {
    /// Session directory of the target server; defaults to the active one.
    ///
    /// 目标服务的 session 目录；不传则取当前 active 服务。
    #[arg(
        long,
        value_name = "DIR",
        help = "Session directory of the target server; defaults to the active one",
        long_help = None
    )]
    session_dir: Option<PathBuf>,

    /// Stop every running Hotpot server for the current user.
    ///
    /// 停止当前用户所有运行中的 Hotpot 服务。
    #[arg(
        long,
        help = "Stop every running Hotpot server for the current user",
        long_help = None
    )]
    all: bool,
}

pub fn start_server(args: StartArgs) -> Result<()> {
    let project_dir = match args.project_dir {
        Some(path) => path,
        None => PathBuf::from(context::resolve_root_dir(None)?),
    };
    let url_host = args.url_host.unwrap_or_else(|| {
        if args.host == "127.0.0.1" {
            "localhost".to_string()
        } else {
            args.host.clone()
        }
    });

    server::start(server::StartOptions {
        project_dir,
        host: args.host,
        url_host,
        port: args.port,
        daemon: args.daemon,
    })
}

pub fn serve_server(args: ServeArgs) -> Result<()> {
    server::serve(server::ServeOptions {
        session_dir: args.session_dir,
        host: args.host,
        url_host: args.url_host,
        port: args.port,
        print_info: false,
    })
}

pub fn stop_server(args: StopArgs) -> Result<()> {
    server::stop(server::StopOptions {
        session_dir: args.session_dir,
        all: args.all,
    })
}
