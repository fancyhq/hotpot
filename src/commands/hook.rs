//! Hotpot hook command implementations.

use std::{io::Read, path::PathBuf};

use anyhow::{Context as _, Result};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};

use crate::context::Context;

/// Hook commands supported by `hotpot hook`.
#[derive(Subcommand, Debug)]
pub enum HookCommand {
    /// Bootstrap the temporary Hotpot runtime context for the current hook.
    Bootstrap(BootstrapArgs),
    /// Handle Claude Code platform hook events.
    Claude {
        /// Claude Code hook event to handle.
        #[command(subcommand)]
        command: ClaudeHookCommand,
    },
    /// Handle Codex platform hook events.
    Codex {
        /// Codex hook event to handle.
        #[command(subcommand)]
        command: CodexHookCommand,
    },
}

/// Claude Code hook events supported by `hotpot hook claude`.
#[derive(Subcommand, Debug)]
pub enum ClaudeHookCommand {
    /// Prepare Hotpot context before Claude Code runs a shell tool.
    PreToolUse,
    /// Prepare Hotpot review-memory context when a Hotpot subagent starts.
    SubagentStart,
    /// Reassert Hotpot output language at the start of every user turn.
    ///
    /// Restates `HOTPOT_LANGUAGE` to fight the "instruction once, drift
    /// forever" pattern. Fires on Claude Code's `UserPromptSubmit` event.
    ///
    /// 每条用户消息前重申输出语言；对抗"一次指令长会话漂移"。
    UserPromptSubmit,
}

/// Codex hook events supported by `hotpot hook codex`.
#[derive(Subcommand, Debug)]
pub enum CodexHookCommand {
    /// Prepare Hotpot context before Codex runs a shell tool.
    PreToolUse,
    /// Prepare Hotpot review-memory context when a Codex session starts.
    SessionStart,
    /// Reassert Hotpot output language at the start of every user turn.
    ///
    /// 同 Claude `UserPromptSubmit`：Codex 每轮用户消息前重申输出语言。
    UserPromptSubmit,
}

/// Bootstrap the temporary Hotpot runtime context for the current hook.
///
/// `hotpot hook bootstrap` 的 CLI 参数：为当前 hook 准备 Hotpot 运行时上下文。
#[derive(Args, Debug)]
pub struct BootstrapArgs {
    /// Output format for the bootstrap context.
    ///
    /// bootstrap 上下文的输出格式。
    #[arg(long, value_enum, default_value = "shell")]
    format: BootstrapFormat,

    /// Explicit project root override.
    ///
    /// 显式指定项目根目录，覆盖自动解析。
    #[arg(long)]
    root_dir: Option<PathBuf>,
}

/// Output formats supported by hook bootstrap.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum BootstrapFormat {
    /// Emit shell exports for direct command consumption.
    Shell,
    /// Emit a JSON object for structured consumers.
    Json,
}

/// Executes the hook bootstrap command.
///
/// 走 env-first 链解析根目录，并显式创建 issue 候选 JSONL，让 OpenCode /
/// Pi 等平台插件在首次注入 env 时拿到已经存在的文件。
pub fn bootstrap(args: BootstrapArgs) -> Result<()> {
    let context = Context::resolve(args.root_dir)?;
    context.ensure_issue_candidates_file()?;

    match args.format {
        BootstrapFormat::Shell => print_shell_exports(&context),
        BootstrapFormat::Json => print_json(&context)?,
    }

    Ok(())
}

/// Executes a Claude Code hook event command.
///
/// 走 `Context::from_payload` 入口，hook 输出反映平台传入的 cwd，不被
/// ambient `ROOT_DIR` 污染。
pub fn claude(command: ClaudeHookCommand) -> Result<()> {
    let payload = read_hook_payload()?;
    let context = Context::from_payload(&payload)?;
    let event_name = hook_event_name(
        &payload,
        match command {
            ClaudeHookCommand::PreToolUse => "PreToolUse",
            ClaudeHookCommand::SubagentStart => "SubagentStart",
            ClaudeHookCommand::UserPromptSubmit => "UserPromptSubmit",
        },
    );

    let additional_context = match command {
        ClaudeHookCommand::PreToolUse => shell_context_message(
            &context,
            "Hotpot shell context was resolved from the Claude Code hook payload cwd.",
        ),
        ClaudeHookCommand::SubagentStart => review_memory_message(&context),
        ClaudeHookCommand::UserPromptSubmit => language_directive_message(&context.language),
    };

    print_value(&json!({
        "continue": true,
        "suppressOutput": false,
        "hookSpecificOutput": {
            "hookEventName": event_name,
            "additionalContext": additional_context,
        },
    }))
}

/// Executes a Codex hook event command.
///
/// 同 `claude`，走 payload-first 入口。
pub fn codex(command: CodexHookCommand) -> Result<()> {
    let payload = read_hook_payload()?;
    let context = Context::from_payload(&payload)?;

    match command {
        CodexHookCommand::PreToolUse => {
            let message = codex_shell_context_message(&context);
            print_value(&json!({
                "systemMessage": message,
                "hookSpecificOutput": {
                    "permissionDecision": "allow",
                    "additionalContext": message,
                },
            }))
        }
        CodexHookCommand::SessionStart => {
            let message = review_memory_message(&context);
            print_value(&json!({
                "systemMessage": message,
                "additionalContext": message,
            }))
        }
        CodexHookCommand::UserPromptSubmit => {
            let message = language_directive_message(&context.language);
            print_value(&json!({
                "systemMessage": message,
                "additionalContext": message,
            }))
        }
    }
}

/// Reads a platform hook payload from stdin.
fn read_hook_payload() -> Result<Value> {
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .context("failed to read hook payload from stdin")?;

    if raw.trim().is_empty() {
        return Ok(json!({}));
    }

    serde_json::from_str(raw.trim()).context("failed to parse hook payload JSON")
}

/// Returns the hook event name from a payload, falling back to a stable event name.
fn hook_event_name(payload: &Value, fallback: &str) -> String {
    payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

/// Prints shell exports that can bootstrap a hook runtime.
fn print_shell_exports(context: &Context) {
    println!("export ROOT_DIR='{}'", shell_quote(&context.root_dir));
    println!(
        "export HOTPOT_USERNAME='{}'",
        shell_quote(&context.username)
    );
    println!(
        "export HOTPOT_LANGUAGE='{}'",
        shell_quote(&context.language)
    );
    println!(
        "export HOTPOT_ISSUE_CANDIDATES_FILE='{}'",
        shell_quote(&context.issue_candidates_file)
    );
    println!(
        "export HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT='{}'",
        shell_quote(&context.record_issue_candidate_prompt)
    );
    println!(
        "export HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT='{}'",
        shell_quote(&context.summarize_issue_candidates_prompt)
    );
    println!(
        "export HOTPOT_TDD_PROTOCOL_PROMPT='{}'",
        shell_quote(&context.tdd_protocol_prompt)
    );
    println!(
        "export HOTPOT_NEW_PROMPT='{}'",
        shell_quote(&context.new_prompt)
    );
    println!(
        "export HOTPOT_EXECUTE_PROMPT='{}'",
        shell_quote(&context.execute_prompt)
    );
    println!(
        "export HOTPOT_FINISH_WORK_PROMPT='{}'",
        shell_quote(&context.finish_work_prompt)
    );
}

/// Prints the hook context as JSON.
fn print_json(context: &Context) -> Result<()> {
    println!("{}", serde_json::to_string(context)?);
    Ok(())
}

/// Prints a JSON value as one hook response line.
fn print_value(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

/// Builds the shared shell context message for platform hooks.
fn shell_context_message(context: &Context, intro: &str) -> String {
    let mut lines = vec![
        intro.to_string(),
        "Use these values for Hotpot-related Bash commands:".to_string(),
    ];
    lines.extend(context_lines(context));
    lines.join("\n")
}

/// Builds the Codex pre-tool context message including an export hint.
fn codex_shell_context_message(context: &Context) -> String {
    let mut lines = vec![
        "Hotpot shell context was resolved from the Codex hook payload cwd.".to_string(),
        "Use these values for Hotpot-related Bash commands:".to_string(),
    ];
    lines.extend(context_lines(context));
    lines.push(format!(
        "For shell commands, prefix with: export {};",
        shell_export_assignments(context)
    ));
    lines.join("\n")
}

/// Builds the per-turn language directive for `UserPromptSubmit` hooks.
///
/// Two lines, < 200 chars: short enough to inline into every model turn
/// without bloating `additionalContext`. The structural-anchor whitelist
/// here is a representative sample (the long list lives in
/// `assets/prompts/output-language.md`); the goal is to re-prime the
/// model against the most common drift trigger every turn.
///
/// 简短的"每轮重申"指令（两行 < 200 字符），用于 Claude/Codex 的
/// `UserPromptSubmit` 钩子。完整锚点清单仍在 `output-language.md`，
/// 这里只列最常诱发漂移的几个，避免 additionalContext 膨胀。
fn language_directive_message(language: &str) -> String {
    format!(
        "Hotpot output language for this turn: `{language}`. \
         Reply in that language for all user-facing prose. \
         Structural anchors stay English: `## Task`, `## Plan`, `### Mode`, `tdd: true|false`, `ACTIVE_CONFLICT:`, kebab-case slugs."
    )
}

/// Builds the review-memory bootstrap message for subagent/session hooks.
///
/// Subagent/session boundaries are where the orchestrator's once-only
/// language detection most often gets dropped, so the bootstrap message
/// carries both the literal `HOTPOT_LANGUAGE` value (for grep / shell
/// reuse) and a one-line directive (for direct steering).
fn review_memory_message(context: &Context) -> String {
    [
        "Hotpot review-memory context is ready.".to_string(),
        format!("- ROOT_DIR: {}", context.root_dir),
        format!("- HOTPOT_USERNAME: {}", context.username),
        format!("- HOTPOT_LANGUAGE: {}", context.language),
        format!(
            "- HOTPOT_ISSUE_CANDIDATES_FILE: {}",
            context.issue_candidates_file
        ),
        "Record only validated, reusable repair memories in this JSONL file.".to_string(),
        language_directive_message(&context.language),
    ]
    .join("\n")
}

/// Returns all Hotpot context values formatted for human-readable hook output.
fn context_lines(context: &Context) -> Vec<String> {
    vec![
        format!("- ROOT_DIR: {}", context.root_dir),
        format!("- HOTPOT_USERNAME: {}", context.username),
        format!("- HOTPOT_LANGUAGE: {}", context.language),
        format!(
            "- HOTPOT_ISSUE_CANDIDATES_FILE: {}",
            context.issue_candidates_file
        ),
        format!(
            "- HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT: {}",
            context.record_issue_candidate_prompt
        ),
        format!(
            "- HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT: {}",
            context.summarize_issue_candidates_prompt
        ),
        format!(
            "- HOTPOT_TDD_PROTOCOL_PROMPT: {}",
            context.tdd_protocol_prompt
        ),
        format!("- HOTPOT_NEW_PROMPT: {}", context.new_prompt),
        format!("- HOTPOT_EXECUTE_PROMPT: {}", context.execute_prompt),
        format!(
            "- HOTPOT_FINISH_WORK_PROMPT: {}",
            context.finish_work_prompt
        ),
    ]
}

/// Returns shell assignment snippets for all Hotpot context values.
fn shell_export_assignments(context: &Context) -> String {
    [
        ("ROOT_DIR", &context.root_dir),
        ("HOTPOT_USERNAME", &context.username),
        ("HOTPOT_LANGUAGE", &context.language),
        (
            "HOTPOT_ISSUE_CANDIDATES_FILE",
            &context.issue_candidates_file,
        ),
        (
            "HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT",
            &context.record_issue_candidate_prompt,
        ),
        (
            "HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT",
            &context.summarize_issue_candidates_prompt,
        ),
        (
            "HOTPOT_TDD_PROTOCOL_PROMPT",
            &context.tdd_protocol_prompt,
        ),
        ("HOTPOT_NEW_PROMPT", &context.new_prompt),
        ("HOTPOT_EXECUTE_PROMPT", &context.execute_prompt),
        ("HOTPOT_FINISH_WORK_PROMPT", &context.finish_work_prompt),
    ]
    .into_iter()
    .map(|(key, value)| format!("{key}={}", serde_json::to_string(value).unwrap_or_default()))
    .collect::<Vec<_>>()
    .join(" ")
}

/// Escapes a string for safe single-quoted shell output.
fn shell_quote(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}
