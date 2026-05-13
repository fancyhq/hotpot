mod commands;
mod context;
mod issues;
mod paths;
mod server;
mod task;

use clap::Parser;

use crate::commands::HotpotCLI;

fn main() -> anyhow::Result<()> {
    HotpotCLI::parse().run()
}
