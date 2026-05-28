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
    #[command(
        about = "Bootstrap the temporary Hotpot runtime context for the current hook",
        long_about = None
    )]
    Bootstrap(BootstrapArgs),
    /// Handle Claude Code platform hook events.
    #[command(
        about = "Handle Claude Code platform hook events",
        long_about = None
    )]
    Claude {
        /// Claude Code hook event to handle.
        #[command(subcommand)]
        command: ClaudeHookCommand,
    },
    /// Handle Codex platform hook events.
    #[command(
        about = "Handle Codex platform hook events",
        long_about = None
    )]
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
    #[command(
        about = "Prepare Hotpot context before Claude Code runs a shell tool",
        long_about = None
    )]
    PreToolUse,
    /// Prepare Hotpot review-memory context when a Hotpot subagent starts.
    #[command(
        about = "Prepare Hotpot review-memory context when a Hotpot subagent starts",
        long_about = None
    )]
    SubagentStart,
    /// Reassert Hotpot output language at the start of every user turn.
    ///
    /// Restates `HOTPOT_LANGUAGE` to fight the "instruction once, drift
    /// forever" pattern. Fires on Claude Code's `UserPromptSubmit` event.
    ///
    /// 每条用户消息前重申输出语言；对抗"一次指令长会话漂移"。
    #[command(
        about = "Reassert Hotpot output language at the start of every user turn",
        long_about = None
    )]
    UserPromptSubmit,
    /// Release any running VuePress dev server when a Claude Code session ends.
    ///
    /// Fires on Claude Code's `SessionEnd` event. Invokes the idempotent
    /// `vuepress::stop_if_running` helper using the project root derived
    /// from the hook payload's `cwd` (so it works regardless of which
    /// directory the user invoked Claude Code from). Defense-in-depth
    /// second layer behind `/hotpot:execute` pre-flight stop — catches
    /// the case where the user closes the session without running
    /// `/hotpot:execute`.
    ///
    /// session 结束时释放可能在跑的 VuePress dev server。第二层防护，
    /// 兜底 `/hotpot:execute` 入口 stop 没被触发的场景。
    #[command(
        about = "Release VuePress dev server when a Claude Code session ends",
        long_about = None
    )]
    SessionEnd,
}

/// Codex hook events supported by `hotpot hook codex`.
#[derive(Subcommand, Debug)]
pub enum CodexHookCommand {
    /// Prepare Hotpot context before Codex runs a shell tool.
    #[command(
        about = "Prepare Hotpot context before Codex runs a shell tool",
        long_about = None
    )]
    PreToolUse,
    /// Prepare Hotpot review-memory context when a Codex session starts.
    #[command(
        about = "Prepare Hotpot review-memory context when a Codex session starts",
        long_about = None
    )]
    SessionStart,
    /// Reassert Hotpot output language at the start of every user turn.
    ///
    /// 同 Claude `UserPromptSubmit`：Codex 每轮用户消息前重申输出语言。
    #[command(
        about = "Reassert Hotpot output language at the start of every user turn",
        long_about = None
    )]
    UserPromptSubmit,
}

/// Bootstrap the temporary Hotpot runtime context for the current hook.
///
/// `hotpot hook bootstrap` 的 CLI 参数：为当前 hook 准备 Hotpot 运行时上下文。
#[derive(Args, Debug)]
#[command(
    about = "Bootstrap the temporary Hotpot runtime context for the current hook",
    long_about = None
)]
pub struct BootstrapArgs {
    /// Output format for the bootstrap context.
    ///
    /// bootstrap 上下文的输出格式。
    #[arg(
        long,
        value_enum,
        default_value = "shell",
        help = "Output format for the bootstrap context",
        long_help = None
    )]
    format: BootstrapFormat,

    /// Explicit project root override.
    ///
    /// 显式指定项目根目录，覆盖自动解析。
    #[arg(
        long,
        help = "Explicit project root override",
        long_help = None
    )]
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

    // SessionEnd 是清理类 hook：不注入 additionalContext，只调
    // vuepress::stop_if_running 释放 dev server，然后返回 minimal JSON。
    // stop_if_running 失败仅 stderr 警告，不让 hook 失败——session 已经
    // 在关闭，能清理就清理，清不掉也别 panic。
    //
    // SessionEnd is cleanup-only: no additionalContext to inject, just
    // call the idempotent stop helper and return minimal JSON. Failures
    // only stderr-warn so a flaky kill doesn't bubble up as a Claude
    // hook error during session teardown.
    if matches!(command, ClaudeHookCommand::SessionEnd) {
        if let Err(err) = crate::vuepress::stop_if_running(&context.root_dir) {
            eprintln!("vuepress stop_if_running failed: {err}");
        }
        return print_value(&json!({
            "continue": true,
            "suppressOutput": true,
        }));
    }

    let event_name = hook_event_name(
        &payload,
        match command {
            ClaudeHookCommand::PreToolUse => "PreToolUse",
            ClaudeHookCommand::SubagentStart => "SubagentStart",
            ClaudeHookCommand::UserPromptSubmit => "UserPromptSubmit",
            ClaudeHookCommand::SessionEnd => unreachable!("handled in early return above"),
        },
    );

    let additional_context = match command {
        ClaudeHookCommand::PreToolUse => shell_context_message(
            &context,
            "Hotpot shell context was resolved from the Claude Code hook payload cwd.",
        ),
        ClaudeHookCommand::SubagentStart => claude_review_memory_message(&context),
        ClaudeHookCommand::UserPromptSubmit => language_directive_message(&context.language),
        ClaudeHookCommand::SessionEnd => unreachable!("handled in early return above"),
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

/// Builds the review-memory bootstrap message for subagent/session hooks.
///
/// Subagent/session boundaries are where the orchestrator's once-only
/// language detection most often gets dropped, so the bootstrap message
/// carries both the literal `HOTPOT_LANGUAGE` value (for grep / shell
/// reuse) and a one-line directive (for direct steering).
fn claude_review_memory_message(context: &Context) -> String {
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

/// Executes a Codex hook event command.
///
/// 同 `claude`，走 payload-first 入口。
pub fn codex(command: CodexHookCommand) -> Result<()> {
    let payload = read_hook_payload()?;
    let context = Context::from_payload(&payload)?;
    print_value(&build_codex_response(&command, &context))
}

fn codex_review_memory_message(context: &Context) -> String {
    [
        "Hotpot review-memory context is ready.".to_string(),
        format!("- ROOT_DIR: {}", context.root_dir),
        format!("- HOTPOT_USERNAME: {}", context.username),
        format!("- HOTPOT_LANGUAGE: {}", context.language),
        format!(
            "- HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT: {}",
            context.record_issue_candidate_prompt
        ),
        format!(
            "- HOTPOT_ISSUE_CANDIDATES_FILE: {}",
            context.issue_candidates_file
        ),
        format!(
            "- HOTPOT_TDD_PROTOCOL_PROMPT: {}",
            context.tdd_protocol_prompt
        ),
        format!("- HOTPOT_NEW_PROMPT: {}", context.new_prompt),
        "Record only validated, reusable repair memories in this JSONL file.".to_string(),
        language_directive_message(&context.language),
    ]
    .join("\n")
}

/// Builds the Codex hook response JSON value for the given command and context.
///
/// Returns a `serde_json::Value` matching the Codex hook output schema.
/// Used by both the public `codex()` function and tests.
///
/// 为指定 Codex hook 命令与上下文构建响应 JSON。
/// 返回符合 Codex hook 输出 schema 的 `serde_json::Value`。
fn build_codex_response(command: &CodexHookCommand, context: &Context) -> Value {
    let (message, event_name) = match command {
        CodexHookCommand::PreToolUse => (codex_shell_context_message(context), "PreToolUse"),
        // CodexHookCommand::SessionStart => (review_memory_message(context), "SessionStart"),
        CodexHookCommand::SessionStart => (codex_review_memory_message(context), "SessionStart"),
        CodexHookCommand::UserPromptSubmit => (
            language_directive_message(&context.language),
            "UserPromptSubmit",
        ),
    };
    // Unified schema: systemMessage at top level, additionalContext inside
    // hookSpecificOutput. Codex rejects top-level additionalContext and only
    // accepts permissionDecision when denying (absence = allow). Each event
    // must carry its matching hookEventName.
    //
    // 统一结构：systemMessage 在顶层，additionalContext 在 hookSpecificOutput
    // 内部。Codex 拒绝顶层 additionalContext，且仅拒绝时才设 permissionDecision。
    // 每个事件须携带对应的 hookEventName。
    json!({
        "systemMessage": message,
        "hookSpecificOutput": {
            "hookEventName": event_name,
            "additionalContext": message,
        },
    })
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
    // VuePress trio: `HOTPOT_VUEPRESS_ENABLED` is always exported as the
    // public hook / bootstrap contract field (the AI's branch decision is
    // driven by the prompt file-existence gate in `hotpot-new.md`, not by
    // this env-var); port/url are exported only when enabled to avoid
    // leaking stale state from disabled projects.
    // VuePress 三件套：enabled 始终 export 作为 hook / bootstrap 公共契约
    // 字段（AI 分支由 `hotpot-new.md` 的 prompt file-existence gate 决定，
    // 不再依赖该 env-var）；port/url 仅启用时 export，避免禁用项目残留状态
    // 泄漏。
    println!(
        "export HOTPOT_VUEPRESS_ENABLED='{}'",
        shell_quote(&context.vuepress_enabled)
    );
    if !context.vuepress_port.is_empty() {
        println!(
            "export HOTPOT_VUEPRESS_PORT='{}'",
            shell_quote(&context.vuepress_port)
        );
    }
    if !context.vuepress_url.is_empty() {
        println!(
            "export HOTPOT_VUEPRESS_URL='{}'",
            shell_quote(&context.vuepress_url)
        );
    }
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

/// Returns all Hotpot context values formatted for human-readable hook output.
fn context_lines(context: &Context) -> Vec<String> {
    let mut lines = vec![
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
        // enabled 始终列出，作为 AI 走分支的确定信号；port/url 仅启用时附上。
        // enabled is always listed (the deterministic branch signal for AI);
        // port/url appear only when enabled.
        format!("- HOTPOT_VUEPRESS_ENABLED: {}", context.vuepress_enabled),
    ];
    if !context.vuepress_port.is_empty() {
        lines.push(format!("- HOTPOT_VUEPRESS_PORT: {}", context.vuepress_port));
    }
    if !context.vuepress_url.is_empty() {
        lines.push(format!("- HOTPOT_VUEPRESS_URL: {}", context.vuepress_url));
    }
    lines
}

/// Returns shell assignment snippets for all Hotpot context values.
fn shell_export_assignments(context: &Context) -> String {
    let mut pairs: Vec<(&'static str, &String)> = vec![
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
        ("HOTPOT_TDD_PROTOCOL_PROMPT", &context.tdd_protocol_prompt),
        ("HOTPOT_NEW_PROMPT", &context.new_prompt),
        ("HOTPOT_EXECUTE_PROMPT", &context.execute_prompt),
        ("HOTPOT_FINISH_WORK_PROMPT", &context.finish_work_prompt),
        ("HOTPOT_VUEPRESS_ENABLED", &context.vuepress_enabled),
    ];
    if !context.vuepress_port.is_empty() {
        pairs.push(("HOTPOT_VUEPRESS_PORT", &context.vuepress_port));
    }
    if !context.vuepress_url.is_empty() {
        pairs.push(("HOTPOT_VUEPRESS_URL", &context.vuepress_url));
    }
    pairs
        .into_iter()
        .map(|(key, value)| format!("{key}={}", serde_json::to_string(value).unwrap_or_default()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Escapes a string for safe single-quoted shell output.
fn shell_quote(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Builds a minimal hook payload that can be consumed by
    /// `Context::from_payload`, pointing at the real project root so
    /// `build_context` can resolve language, prompts, etc.
    ///
    /// 构造一个能被 `Context::from_payload` 消费的最小 hook payload，
    /// 指向当前项目根目录以便 `build_context` 正常解析。
    fn test_payload() -> Value {
        let cwd = env!("CARGO_MANIFEST_DIR");
        json!({"cwd": cwd})
    }

    /// Helper: creates a test Context, calls `build_codex_response` for the
    /// given command, and returns the JSON value.
    ///
    /// 创建测试 Context，为指定命令调用 `build_codex_response`，返回 JSON。
    fn build_response(command: CodexHookCommand) -> Value {
        let payload = test_payload();
        let context = Context::from_payload(&payload).unwrap();
        build_codex_response(&command, &context)
    }

    #[test]
    fn codex_pre_tool_use_outputs_valid_hook_json() {
        let value = build_response(CodexHookCommand::PreToolUse);
        // Must have systemMessage at top level
        assert!(
            value
                .get("systemMessage")
                .and_then(|v| v.as_str())
                .is_some(),
            "PreToolUse: expected top-level systemMessage, got {:?}",
            value,
        );
        // Must have hookSpecificOutput.additionalContext
        let hso = value
            .get("hookSpecificOutput")
            .and_then(|v| v.as_object())
            .unwrap_or_else(|| {
                panic!("PreToolUse: expected hookSpecificOutput object, got {value:?}")
            });
        assert!(
            hso.get("additionalContext")
                .and_then(|v| v.as_str())
                .is_some(),
            "PreToolUse: expected hookSpecificOutput.additionalContext, got {:?}",
            hso,
        );
        // Must NOT have permissionDecision (Codex only accepts "deny", absence = "allow")
        assert!(
            !hso.contains_key("permissionDecision"),
            "PreToolUse: permissionDecision should not be present when allowing, got {:?}",
            hso,
        );
        // Must have hookSpecificOutput.hookEventName matching the event
        assert_eq!(
            hso.get("hookEventName").and_then(|v| v.as_str()),
            Some("PreToolUse"),
            "PreToolUse: expected hookSpecificOutput.hookEventName = \"PreToolUse\", got {:?}",
            hso.get("hookEventName"),
        );
    }

    #[test]
    fn codex_session_start_outputs_valid_hook_json() {
        let value = build_response(CodexHookCommand::SessionStart);
        // Must have systemMessage at top level
        assert!(
            value
                .get("systemMessage")
                .and_then(|v| v.as_str())
                .is_some(),
            "SessionStart: expected top-level systemMessage, got {:?}",
            value,
        );
        // Must NOT have additionalContext at top level
        assert!(
            value.get("additionalContext").is_none(),
            "SessionStart: additionalContext must NOT be at top level, got {:?}",
            value,
        );
        // Must have hookSpecificOutput.additionalContext
        let hso = value
            .get("hookSpecificOutput")
            .and_then(|v| v.as_object())
            .unwrap_or_else(|| {
                panic!("SessionStart: expected hookSpecificOutput object, got {value:?}")
            });
        assert!(
            hso.get("additionalContext")
                .and_then(|v| v.as_str())
                .is_some(),
            "SessionStart: expected hookSpecificOutput.additionalContext, got {:?}",
            hso,
        );
        // Must have hookSpecificOutput.hookEventName matching the event
        assert_eq!(
            hso.get("hookEventName").and_then(|v| v.as_str()),
            Some("SessionStart"),
            "SessionStart: expected hookSpecificOutput.hookEventName = \"SessionStart\", got {:?}",
            hso.get("hookEventName"),
        );
    }

    #[test]
    fn codex_user_prompt_submit_outputs_valid_hook_json() {
        let value = build_response(CodexHookCommand::UserPromptSubmit);
        // Must have systemMessage at top level
        assert!(
            value
                .get("systemMessage")
                .and_then(|v| v.as_str())
                .is_some(),
            "UserPromptSubmit: expected top-level systemMessage, got {:?}",
            value,
        );
        // Must NOT have additionalContext at top level
        assert!(
            value.get("additionalContext").is_none(),
            "UserPromptSubmit: additionalContext must NOT be at top level, got {:?}",
            value,
        );
        // Must have hookSpecificOutput.additionalContext
        let hso = value
            .get("hookSpecificOutput")
            .and_then(|v| v.as_object())
            .unwrap_or_else(|| {
                panic!("UserPromptSubmit: expected hookSpecificOutput object, got {value:?}")
            });
        assert!(
            hso.get("additionalContext")
                .and_then(|v| v.as_str())
                .is_some(),
            "UserPromptSubmit: expected hookSpecificOutput.additionalContext, got {:?}",
            hso,
        );
        // Must have hookSpecificOutput.hookEventName matching the event
        assert_eq!(
            hso.get("hookEventName").and_then(|v| v.as_str()),
            Some("UserPromptSubmit"),
            "UserPromptSubmit: expected hookSpecificOutput.hookEventName = \"UserPromptSubmit\", got {:?}",
            hso.get("hookEventName"),
        );
    }
}
