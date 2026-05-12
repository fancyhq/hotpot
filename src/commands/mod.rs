pub mod init;
pub mod issues;
pub mod server;
pub mod task;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version)]
pub struct HotpotCLI {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Init(init::InitArgs),
    Issues {
        #[command(subcommand)]
        command: issues::IssuesCommand,
    },
    Server {
        #[command(subcommand)]
        command: server::ServerCommand,
    },
    Task {
        #[command(subcommand)]
        command: task::TaskCommand,
    },
}

impl HotpotCLI {
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::Init(args) => init::init(args),
            Command::Issues { command } => match command {
                issues::IssuesCommand::List => issues::list_issues(),
                issues::IssuesCommand::Relevant(args) => issues::relevant_issues(args),
            },
            Command::Server { command } => match command {
                server::ServerCommand::Start(args) => server::start_server(args),
                server::ServerCommand::Serve(args) => server::serve_server(args),
                server::ServerCommand::Stop(args) => server::stop_server(args),
            },
            Command::Task { command } => match command {
                task::TaskCommand::List => task::list_tasks(),
                task::TaskCommand::Create(args) => task::create_task(args),
                task::TaskCommand::Active(args) => task::active_task(args),
                task::TaskCommand::Stop(args) => task::stop_task(args),
            },
        }
    }
}
