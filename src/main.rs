mod commands;
mod issues;
mod paths;
mod server;
mod task;
mod utils;

use clap::Parser;

use crate::commands::HotpotCLI;

fn main() -> anyhow::Result<()> {
    HotpotCLI::parse().run()
}
