mod commands;
mod context;
mod issues;
mod lock;
mod paths;
mod server;
mod task;
mod worktree;

use clap::Parser;

use crate::commands::HotpotCLI;

fn main() -> anyhow::Result<()> {
    HotpotCLI::parse().run()
}
