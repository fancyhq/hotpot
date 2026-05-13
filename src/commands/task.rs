use anyhow::{Result, bail};
use clap::{Args, Subcommand};

use crate::{context, task};

#[derive(Subcommand, Debug)]
pub enum TaskCommand {
    List,
    Create(CreateArgs),
    Active(ActiveArgs),
    Stop(StopArgs),
}

#[derive(Debug, Args)]
pub struct StopArgs {
    #[arg(long)]
    all: bool,
}

#[derive(Debug, Args)]
pub struct ActiveArgs {
    #[arg(long)]
    count: bool,

    #[arg(long)]
    path: bool,
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    #[arg(value_name = "TITLE")]
    title_arg: Option<String>,

    #[arg(value_name = "COMMIT")]
    commit_arg: Option<String>,

    #[arg(long, value_name = "TITLE")]
    title: Option<String>,

    #[arg(long, value_name = "COMMIT")]
    commit: Option<String>,
}

pub fn list_tasks() -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;
    let tasks = task::get_task_list(&root_dir, &username)?;
    if tasks.is_empty() {
        println!("No tasks found.");
        return Ok(());
    }
    for task in tasks {
        println!(
            "{}\t{}\t{}\t{}",
            task.task_id,
            task.status.as_str(),
            task.title,
            task.time
        )
    }
    Ok(())
}

pub fn create_task(args: CreateArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;
    let has_positional_args = args.title_arg.is_some() || args.commit_arg.is_some();
    let has_named_args = args.title.is_some() || args.commit.is_some();

    if has_positional_args && has_named_args {
        bail!("Do not mix positional arguments with --title/--commit.");
    }

    let (title, commit) = if has_named_args {
        let title = args
            .title
            .ok_or_else(|| anyhow::anyhow!("--title is required."))?;
        (title, args.commit)
    } else {
        let title = args
            .title_arg
            .ok_or_else(|| anyhow::anyhow!("TITLE is required."))?;
        (title, args.commit_arg)
    };

    task::create_task(&root_dir, &username, &title, commit.as_deref())?;
    Ok(())
}

pub fn active_task(args: ActiveArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;

    if args.count {
        let count = task::get_active_task_count(&root_dir, &username)?;
        println!("{count}");
    } else if args.path {
        let path = task::get_active_task_filepath(&root_dir, &username)?;
        print!("{}", path.display())
    }

    Ok(())
}

pub fn stop_task(args: StopArgs) -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;

    if args.all {
        task::stop_all_active_tasks(&root_dir, &username)?;
    } else {
        todo!()
    }

    Ok(())
}
