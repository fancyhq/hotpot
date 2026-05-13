//! Hotpot hook command implementations.

use std::{env, fs, io::Read, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};

use crate::paths;

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
}

/// Codex hook events supported by `hotpot hook codex`.
#[derive(Subcommand, Debug)]
pub enum CodexHookCommand {
    /// Prepare Hotpot context before Codex runs a shell tool.
    PreToolUse,
    /// Prepare Hotpot review-memory context when a Codex session starts.
    SessionStart,
}

/// Arguments for the `hotpot hook bootstrap` command.
#[derive(Args, Debug)]
pub struct BootstrapArgs {
    /// Output format for the bootstrap context.
    #[arg(long, value_enum, default_value = "shell")]
    format: BootstrapFormat,

    /// Explicit project root override.
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

/// Resolved Hotpot hook context.
#[derive(Debug, serde::Serialize)]
struct HookContext {
    /// Absolute project root.
    #[serde(rename = "ROOT_DIR")]
    root_dir: String,

    /// Session username used by Hotpot workspace files.
    #[serde(rename = "HOTPOT_USERNAME")]
    username: String,

    /// JSONL file that stores temporary issue candidates.
    #[serde(rename = "HOTPOT_ISSUE_CANDIDATES_FILE")]
    issue_candidates_file: String,

    /// Prompt path used when recording a candidate.
    #[serde(rename = "HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT")]
    record_issue_candidate_prompt: String,

    /// Prompt path used when summarizing candidates.
    #[serde(rename = "HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT")]
    summarize_issue_candidates_prompt: String,
}

/// Executes the hook bootstrap command.
pub fn bootstrap(args: BootstrapArgs) -> Result<()> {
    let root_dir = resolve_root_dir(args.root_dir)?;
    let context = build_context(&root_dir)?;

    match args.format {
        BootstrapFormat::Shell => print_shell_exports(&context),
        BootstrapFormat::Json => print_json(&context)?,
    }

    Ok(())
}

/// Executes a Claude Code hook event command.
pub fn claude(command: ClaudeHookCommand) -> Result<()> {
    let payload = read_hook_payload()?;
    let context = context_from_payload(&payload)?;
    let event_name = hook_event_name(
        &payload,
        match command {
            ClaudeHookCommand::PreToolUse => "PreToolUse",
            ClaudeHookCommand::SubagentStart => "SubagentStart",
        },
    );

    let additional_context = match command {
        ClaudeHookCommand::PreToolUse => shell_context_message(
            &context,
            "Hotpot shell context was resolved from the Claude Code hook payload cwd.",
        ),
        ClaudeHookCommand::SubagentStart => review_memory_message(&context),
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
pub fn codex(command: CodexHookCommand) -> Result<()> {
    let payload = read_hook_payload()?;
    let context = context_from_payload(&payload)?;

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
    }
}

/// Resolves the effective project root for hook bootstrap.
fn resolve_root_dir(root_dir: Option<PathBuf>) -> Result<PathBuf> {
    let path =
        root_dir.unwrap_or(env::current_dir().context("failed to resolve current directory")?);
    Ok(path.canonicalize().unwrap_or(path))
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

/// Builds a Hotpot context from a platform hook payload.
fn context_from_payload(payload: &Value) -> Result<HookContext> {
    let root_dir = payload
        .get("cwd")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let root_dir = resolve_root_dir(root_dir)?;

    build_context(&root_dir)
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

/// Builds the runtime context from the resolved project root.
fn build_context(root_dir: &PathBuf) -> Result<HookContext> {
    let root_dir = root_dir.display().to_string();
    let username = resolve_username(&root_dir)?;
    let issue_candidates_file = paths::issue_candidates_file_path(&root_dir, &username)
        .display()
        .to_string();
    let record_issue_candidate_prompt = prompt_path(&root_dir, "record-issue-candidate.md")
        .display()
        .to_string();
    let summarize_issue_candidates_prompt = prompt_path(&root_dir, "summarize-issue-candidates.md")
        .display()
        .to_string();
    ensure_issue_candidates_file(&issue_candidates_file)?;

    Ok(HookContext {
        root_dir,
        username,
        issue_candidates_file,
        record_issue_candidate_prompt,
        summarize_issue_candidates_prompt,
    })
}

/// Returns the project prompt path for a named Hotpot prompt.
fn prompt_path(root_dir: &str, name: &str) -> PathBuf {
    PathBuf::from(root_dir).join("prompts").join(name)
}

/// Resolves the Hotpot username from env or git configuration.
fn resolve_username(root_dir: &str) -> Result<String> {
    if let Some(username) = normalize_username(env::var("HOTPOT_USERNAME").ok()) {
        return Ok(username);
    }

    if let Some(username) = git_username(root_dir, &["config", "--local", "user.name"])? {
        return Ok(username);
    }

    if let Some(username) = git_username(root_dir, &["config", "--global", "user.name"])? {
        return Ok(username);
    }

    Ok("default".to_string())
}

/// Normalizes a string value to `Some` only when it is non-empty.
fn normalize_username(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

/// Runs a git command and returns trimmed stdout when successful.
fn git_username(root_dir: &str, args: &[&str]) -> Result<Option<String>> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root_dir)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(normalize_username(Some(stdout)))
        }
        Ok(_) | Err(_) => Ok(None),
    }
}

/// Ensures the issue candidates JSONL file exists.
fn ensure_issue_candidates_file(file_path: &str) -> Result<()> {
    let file_path = PathBuf::from(file_path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    if !file_path.exists() {
        fs::File::create(&file_path)
            .with_context(|| format!("failed to create {}", file_path.display()))?;
    }

    Ok(())
}

/// Prints shell exports that can bootstrap a hook runtime.
fn print_shell_exports(context: &HookContext) {
    println!("export ROOT_DIR='{}'", shell_quote(&context.root_dir));
    println!(
        "export HOTPOT_USERNAME='{}'",
        shell_quote(&context.username)
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
}

/// Prints the hook context as JSON.
fn print_json(context: &HookContext) -> Result<()> {
    println!("{}", serde_json::to_string(context)?);
    Ok(())
}

/// Prints a JSON value as one hook response line.
fn print_value(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

/// Builds the shared shell context message for platform hooks.
fn shell_context_message(context: &HookContext, intro: &str) -> String {
    let mut lines = vec![
        intro.to_string(),
        "Use these values for Hotpot-related Bash commands:".to_string(),
    ];
    lines.extend(context_lines(context));
    lines.join("\n")
}

/// Builds the Codex pre-tool context message including an export hint.
fn codex_shell_context_message(context: &HookContext) -> String {
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

/// Builds the review-memory bootstrap message for subagent/session hooks.
fn review_memory_message(context: &HookContext) -> String {
    [
        "Hotpot review-memory context is ready.".to_string(),
        format!("- ROOT_DIR: {}", context.root_dir),
        format!("- HOTPOT_USERNAME: {}", context.username),
        format!(
            "- HOTPOT_ISSUE_CANDIDATES_FILE: {}",
            context.issue_candidates_file
        ),
        "Record only validated, reusable repair memories in this JSONL file.".to_string(),
    ]
    .join("\n")
}

/// Returns all Hotpot context values formatted for human-readable hook output.
fn context_lines(context: &HookContext) -> Vec<String> {
    vec![
        format!("- ROOT_DIR: {}", context.root_dir),
        format!("- HOTPOT_USERNAME: {}", context.username),
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
    ]
}

/// Returns shell assignment snippets for all Hotpot context values.
fn shell_export_assignments(context: &HookContext) -> String {
    [
        ("ROOT_DIR", &context.root_dir),
        ("HOTPOT_USERNAME", &context.username),
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
