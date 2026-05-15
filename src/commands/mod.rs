pub mod hook;
pub mod init;
pub mod issues;
pub mod server;
pub mod task;
pub mod update;
pub mod worktree;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// Hotpot — cross-platform task orchestrator for coding agents.
///
/// Hotpot 跨平台任务编排器，面向 Claude Code / OpenCode / Codex / Pi。
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct HotpotCLI {
    #[command(subcommand)]
    command: Command,
}

/// Top-level Hotpot subcommands.
///
/// Hotpot 的顶层子命令集合。
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Install platform assets and shared prompts into the project.
    ///
    /// 把平台资产与共享 prompt 安装到当前项目（幂等）。
    Init(init::InitArgs),
    /// Refresh installed platforms, bootstrap the workspace, and run health checks.
    ///
    /// 协作者 day-1 入口：刷新已安装平台、初始化 workspace、运行自检。
    Update(update::UpdateArgs),
    /// Platform hook entrypoints invoked by Claude Code / Codex hooks.
    ///
    /// 由 Claude Code / Codex 平台 hook 调用的入口。
    Hook {
        #[command(subcommand)]
        command: hook::HookCommand,
    },
    /// Manage promoted issues and per-user issue candidates.
    ///
    /// 管理已晋升 issue 与当前用户的临时 issue 候选。
    Issues {
        #[command(subcommand)]
        command: issues::IssuesCommand,
    },
    /// Manage the local Hotpot HTTP server.
    ///
    /// 管理本地 Hotpot HTTP 服务进程。
    Server {
        #[command(subcommand)]
        command: server::ServerCommand,
    },
    /// Manage Hotpot tasks in the active user workspace.
    ///
    /// 管理当前用户 workspace 下的 Hotpot 任务。
    Task {
        #[command(subcommand)]
        command: task::TaskCommand,
    },
    /// Manage git worktrees attached to Hotpot tasks.
    ///
    /// 管理与 Hotpot 任务绑定的 git worktree。
    Worktree {
        #[command(subcommand)]
        command: worktree::WorktreeCommand,
    },
}

impl HotpotCLI {
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::Init(args) => init::init(args),
            Command::Update(args) => update::update(args),
            Command::Hook { command } => match command {
                hook::HookCommand::Bootstrap(args) => hook::bootstrap(args),
                hook::HookCommand::Claude { command } => hook::claude(command),
                hook::HookCommand::Codex { command } => hook::codex(command),
            },
            Command::Issues { command } => match command {
                issues::IssuesCommand::List => issues::list_issues(),
                issues::IssuesCommand::Relevant(args) => issues::relevant_issues(args),
                issues::IssuesCommand::Promote => issues::promote_issues(),
                issues::IssuesCommand::Candidate { command } => match command {
                    issues::CandidateCommand::List => issues::list_candidates(),
                    issues::CandidateCommand::Add => issues::add_candidates(),
                    issues::CandidateCommand::Clear => issues::clear_candidates(),
                },
            },
            Command::Server { command } => match command {
                server::ServerCommand::Start(args) => server::start_server(args),
                server::ServerCommand::Serve(args) => server::serve_server(args),
                server::ServerCommand::Stop(args) => server::stop_server(args),
            },
            Command::Task { command } => match command {
                task::TaskCommand::List(args) => task::list_tasks(args),
                task::TaskCommand::Create(args) => task::create_task(args),
                task::TaskCommand::Active(args) => task::active_task(args),
                task::TaskCommand::Stop(args) => task::stop_task(args),
                task::TaskCommand::Done(args) => task::done_task(args),
                task::TaskCommand::Cancel(args) => task::cancel_task(args),
                task::TaskCommand::Resume(args) => task::resume_task(args),
            },
            Command::Worktree { command } => match command {
                worktree::WorktreeCommand::Create(args) => worktree::create(args),
                worktree::WorktreeCommand::Remove(args) => worktree::remove(args),
                worktree::WorktreeCommand::Path(args) => worktree::path(args),
                worktree::WorktreeCommand::List(args) => worktree::list(args),
            },
        }
    }
}
