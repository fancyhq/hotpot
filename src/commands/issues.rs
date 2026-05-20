use std::io::{self, BufRead};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};

use crate::{context, issues};

/// Subcommands of `hotpot issues`.
///
/// `hotpot issues` 的子命令集合。
#[derive(Subcommand, Debug)]
pub enum IssuesCommand {
    /// Print all promoted issues as Markdown on stdout.
    ///
    /// 把全部已晋升 issue 以 Markdown 形式打印到 stdout。
    List,
    /// Print issues relevant to the given changed files or keywords.
    ///
    /// 按给定的变更文件 / 关键字筛选并打印相关 issue。
    Relevant(RelevantArgs),
    /// Append promoted issues from stdin JSONL into `.hotpot/issues.jsonl`.
    ///
    /// 从 stdin 读取每行一个 Issue 的 JSONL，批量追加到 `.hotpot/issues.jsonl`。
    Promote,
    /// Manage project-shared temporary issue candidates.
    ///
    /// 管理项目级共享的临时 issue 候选 (`.hotpot/issue-candidates.jsonl`)。
    Candidate {
        #[command(subcommand)]
        command: CandidateCommand,
    },
}

/// Subcommands operating on the temporary `issue-candidates.jsonl` file.
///
/// 操作临时候选 JSONL 文件的子命令集合。
#[derive(Subcommand, Debug)]
pub enum CandidateCommand {
    /// Print all project-shared temporary candidates as JSONL on stdout.
    ///
    /// 把项目级共享的全部临时候选以 JSONL 形式打印到 stdout。
    List,
    /// Append user-confirmed candidates from stdin JSONL into the project-shared
    /// `.hotpot/issue-candidates.jsonl`.
    ///
    /// 从 stdin 按 JSONL 读取已被用户确认的候选，追加到项目级共享的
    /// `.hotpot/issue-candidates.jsonl`。
    Add,
    /// Truncate the candidates file after promotion or deliberate discard.
    ///
    /// 在晋升或主动丢弃完成后清空候选文件。
    Clear,
}

/// CLI arguments for `hotpot issues relevant`.
///
/// `hotpot issues relevant` 的 CLI 参数。
#[derive(Args, Debug)]
pub struct RelevantArgs {
    /// Path of a changed file used to rank relevant issues; may be repeated.
    ///
    /// 用于排序相关 issue 的变更文件路径，可重复传入。
    #[arg(long = "changed-file", value_name = "PATH")]
    changed_files: Vec<String>,

    /// Free-form keyword to match against issue content; may be repeated.
    ///
    /// 用于匹配 issue 内容的自由关键字，可重复传入。
    #[arg(long = "keyword", value_name = "WORD")]
    keywords: Vec<String>,

    /// Maximum number of relevant issues to return.
    ///
    /// 返回的相关 issue 条数上限。
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

/// Reads promoted issues as JSONL from stdin and appends them to
/// `.hotpot/issues.jsonl`.
///
/// The slash command for `/hotpot:finish-work` builds the promoted list from
/// the LLM-produced `.hotpot/prompts/summarize-issue-candidates.md` output, asks the
/// user to confirm, and then pipes the approved promoted issues through this
/// command. Empty stdin is rejected explicitly so an accidental no-op doesn't
/// look like a silent success. On exit, `{"promoted": N}` is printed to
/// stdout so the slash command can confirm what landed.
///
/// 从 stdin 按 JSONL 读取已晋升的 issue，逐行追加到 `.hotpot/issues.jsonl`。
/// `/hotpot:finish-work` 在拿到 `summarize-issue-candidates.md` 产出的
/// `promoted` 列表、用户确认之后，把这些 issue 通过管道传进来。空输入会
/// 显式报错，避免"什么都没做却看起来成功"。结束时输出
/// `{"promoted": N}` 方便 slash command 核对。
pub fn promote_issues() -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let stdin = io::stdin();
    let mut promoted_count = 0usize;
    let mut saw_any_line = false;

    for (index, line_result) in stdin.lock().lines().enumerate() {
        let line = line_result.with_context(|| format!("读取 stdin 第 {} 行失败", index + 1))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        saw_any_line = true;

        let issue: issues::Issue = serde_json::from_str(trimmed).with_context(|| {
            format!(
                "解析 stdin 第 {} 行为 Issue 失败，原始内容：{}",
                index + 1,
                trimmed
            )
        })?;
        issues::append_issue(&root_dir, &issue)?;
        promoted_count += 1;
    }

    if !saw_any_line {
        bail!("stdin 没有 issue 内容。期望按 JSONL 每行传入一个 Issue。");
    }

    println!("{{\"promoted\":{promoted_count}}}");
    Ok(())
}

/// Reads user-confirmed issue candidates as JSONL from stdin and appends them
/// to the project-shared `.hotpot/issue-candidates.jsonl`.
///
/// `/hotpot:execute` decides which repairs are worth recording by applying the
/// `.hotpot/prompts/record-issue-candidate.md` rules, shows the proposed candidates to
/// the user, and then pipes the user-approved candidates through this command.
/// The "value gate" lives in the prompt and in the user confirmation step — by
/// design this command does not score or dedupe candidates; only schema
/// validation is performed via serde. Empty stdin is rejected explicitly so an
/// accidental no-op doesn't look like a silent success. On exit,
/// `{"added": N}` is printed to stdout so the slash command can confirm what
/// landed.
///
/// The resolved username is passed through the existing Rust API for backward
/// compatibility only; it no longer selects the candidates file path.
///
/// 从 stdin 按 JSONL 读取已被用户确认的 issue 候选，逐行追加到项目级共享的
/// `.hotpot/issue-candidates.jsonl`。`/hotpot:execute` 按 `.hotpot/prompts/record-issue-candidate.md`
/// 的 When/When-Not 规则筛选候选，展示给用户确认后再把通过的部分通过管道
/// 传进来。"价值闸"由 prompt 与用户确认承担——这里刻意不做评分或去重，
/// 只有 serde 层的 schema 校验。空输入显式报错，避免"什么都没做却看起来
/// 成功"。结束时输出 `{"added": N}` 方便 slash command 核对。解析出的
/// username 仅为兼容既有 Rust API 继续透传，不再决定 candidates 文件路径。
pub fn add_candidates() -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;
    let stdin = io::stdin();
    let mut added_count = 0usize;
    let mut saw_any_line = false;

    for (index, line_result) in stdin.lock().lines().enumerate() {
        let line = line_result.with_context(|| format!("读取 stdin 第 {} 行失败", index + 1))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        saw_any_line = true;

        let candidate: issues::IssueCandidate =
            serde_json::from_str(trimmed).with_context(|| {
                format!(
                    "解析 stdin 第 {} 行为 IssueCandidate 失败，原始内容：{}",
                    index + 1,
                    trimmed
                )
            })?;
        issues::append_issue_candidate(&root_dir, &username, &candidate)?;
        added_count += 1;
    }

    if !saw_any_line {
        bail!("stdin 没有 issue 候选内容。期望按 JSONL 每行传入一个 IssueCandidate。");
    }

    println!("{{\"added\":{added_count}}}");
    Ok(())
}

/// Lists project-shared temporary issue candidates as JSONL on stdout.
///
/// 把项目级共享的临时 issue 候选以 JSONL 形式输出到 stdout。
pub fn list_candidates() -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;
    let candidates = issues::get_issue_candidates_list(&root_dir, &username)?;

    for candidate in &candidates {
        let line = serde_json::to_string(candidate).context("序列化issue候选失败")?;
        println!("{line}");
    }
    Ok(())
}

/// Truncates the project-shared temporary issue candidates file.
///
/// Prints `{"cleared": N}` showing how many candidates were dropped so the
/// slash command can include the number in its final report.
///
/// 清空项目级共享的临时 issue 候选文件。输出 `{"cleared": N}` 给 slash
/// command，告诉它本次实际清掉了多少条。
pub fn clear_candidates() -> Result<()> {
    let root_dir = context::resolve_root_dir(None)?;
    let username = context::resolve_username(&root_dir)?;
    let dropped = issues::get_issue_candidates_list(&root_dir, &username)?.len();
    issues::clear_issue_candidates(&root_dir, &username)?;
    println!("{{\"cleared\":{dropped}}}");
    Ok(())
}
