pub mod hook;
pub mod init;
pub mod issues;
pub mod server;
pub mod task;
pub mod update;
pub mod vuepress;
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

#[cfg(test)]
mod tests {
    //! Command help text regression tests.
    //!
    //! 命令帮助文案回归测试，防止旧的 per-user candidates 语义重新出现。

    use clap::CommandFactory;

    use super::*;

    #[test]
    fn issues_help_mentions_project_shared_candidates() {
        let help = HotpotCLI::command().render_help().to_string();

        assert!(
            help.contains("promoted issues and project-shared temporary candidates"),
            "Issues help should describe global candidates, got:\n{help}"
        );
        assert!(
            !help.contains("per-user issue candidates"),
            "Issues help must not mention per-user candidates:\n{help}"
        );
        assert!(
            !help.contains("当前用户的临时 issue 候选"),
            "Issues help must not mention current-user candidates:\n{help}"
        );
    }
}

/// Top-level Hotpot subcommands.
///
/// Hotpot 的顶层子命令集合。
#[derive(Subcommand, Debug)]
#[command(about = "Hotpot — cross-platform task orchestrator for coding agents")]
pub enum Command {
    /// Install platform assets and shared prompts into the project.
    ///
    /// 把平台资产与共享 prompt 安装到当前项目（幂等）。
    #[command(
        about = "Install platform assets and shared prompts into the project",
        long_about = None
    )]
    Init(init::InitArgs),
    /// Refresh installed platforms, bootstrap the workspace, and run health checks.
    ///
    /// 协作者 day-1 入口：刷新已安装平台、初始化 workspace、运行自检。
    #[command(
        about = "Refresh installed platforms, bootstrap the workspace, and run health checks",
        long_about = None
    )]
    Update(update::UpdateArgs),
    /// Platform hook entrypoints invoked by Claude Code / Codex hooks.
    ///
    /// 由 Claude Code / Codex 平台 hook 调用的入口。
    #[command(
        about = "Platform hook entrypoints invoked by Claude Code / Codex hooks",
        long_about = None
    )]
    Hook {
        #[command(subcommand)]
        command: hook::HookCommand,
    },
    /// Manage promoted issues and project-shared temporary candidates.
    ///
    /// 管理已晋升 issue 与项目级共享的临时 issue 候选。
    #[command(
        about = "Manage promoted issues and project-shared temporary candidates",
        long_about = None
    )]
    Issues {
        #[command(subcommand)]
        command: issues::IssuesCommand,
    },
    /// Manage the local Hotpot HTTP server.
    ///
    /// 管理本地 Hotpot HTTP 服务进程。
    #[command(
        about = "Manage the local Hotpot HTTP server",
        long_about = None
    )]
    Server {
        #[command(subcommand)]
        command: server::ServerCommand,
    },
    /// Manage Hotpot tasks in the active user workspace.
    ///
    /// 管理当前用户 workspace 下的 Hotpot 任务。
    #[command(
        about = "Manage Hotpot tasks in the active user workspace",
        long_about = None
    )]
    Task {
        #[command(subcommand)]
        command: task::TaskCommand,
    },
    /// Manage VuePress integration (hub deployment + dev server).
    ///
    /// 管理 VuePress 集成（hub 部署 + dev server 生命周期）。
    #[command(
        about = "Manage VuePress integration (hub deployment + dev server)",
        long_about = None
    )]
    Vuepress {
        #[command(subcommand)]
        command: vuepress::VuepressCommand,
    },
    /// Manage git worktrees attached to Hotpot tasks.
    ///
    /// 管理与 Hotpot 任务绑定的 git worktree。
    #[command(
        about = "Manage git worktrees attached to Hotpot tasks",
        long_about = None
    )]
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
            Command::Vuepress { command } => match command {
                vuepress::VuepressCommand::Install(args) => vuepress::install(args),
                vuepress::VuepressCommand::Uninstall => vuepress::uninstall(),
                vuepress::VuepressCommand::Start(args) => vuepress::start(args),
                vuepress::VuepressCommand::Stop(args) => vuepress::stop(args),
                vuepress::VuepressCommand::Status => vuepress::status(),
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
