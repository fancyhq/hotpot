use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{context, issues};

#[derive(Subcommand, Debug)]
pub enum IssuesCommand {
    List,
    Relevant(RelevantArgs),
}

#[derive(Args, Debug)]
pub struct RelevantArgs {
    #[arg(long = "changed-file", value_name = "PATH")]
    changed_files: Vec<String>,

    #[arg(long = "keyword", value_name = "WORD")]
    keywords: Vec<String>,

    #[arg(long, default_value_t = 5)]
    limit: usize,
}

pub fn list_issues() -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    print!("{}", issues::render_issues_to_markdown(&root_dir)?);
    Ok(())
}

pub fn relevant_issues(args: RelevantArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let context = issues::ChangeContext {
        changed_files: args.changed_files,
        keywords: args.keywords,
    };
    print!(
        "{}",
        issues::render_relevant_issues_to_markdown(&root_dir, &context, args.limit)?
    );
    Ok(())
}
