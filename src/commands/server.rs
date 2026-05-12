use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{server, utils};

#[derive(Subcommand, Debug)]
pub enum ServerCommand {
    Start(StartArgs),
    #[command(hide = true)]
    Serve(ServeArgs),
    Stop(StopArgs),
}

#[derive(Args, Debug)]
pub struct StartArgs {
    #[arg(long, value_name = "DIR")]
    project_dir: Option<PathBuf>,

    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long)]
    url_host: Option<String>,

    #[arg(long, default_value_t = 0)]
    port: u16,

    #[arg(long)]
    daemon: bool,
}

#[derive(Args, Debug)]
pub struct ServeArgs {
    #[arg(long, value_name = "DIR")]
    pub session_dir: PathBuf,

    #[arg(long)]
    pub host: String,

    #[arg(long)]
    pub url_host: String,

    #[arg(long)]
    pub port: u16,
}

#[derive(Args, Debug)]
pub struct StopArgs {
    #[arg(long, value_name = "DIR")]
    session_dir: Option<PathBuf>,

    #[arg(long)]
    all: bool,
}

pub fn start_server(args: StartArgs) -> Result<()> {
    let project_dir = match args.project_dir {
        Some(path) => path,
        None => PathBuf::from(utils::get_root_dir()?),
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
