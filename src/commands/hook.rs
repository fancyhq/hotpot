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
            ClaudeHookCommand::SessionEnd => unreachable!("handled in early return above"),
        },
    );

    let additional_context = match command {
        ClaudeHookCommand::PreToolUse => prompt_context_message(
            &context,
            "Hotpot shell context was resolved from the Claude Code hook payload cwd.",
        ),
        ClaudeHookCommand::SubagentStart => claude_review_memory_message(&context),
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
        CodexHookCommand::PreToolUse => (
            prompt_context_message(context, "Hotpot shell context was resolved from the Codex hook payload cwd."),
            "PreToolUse",
        ),
        CodexHookCommand::SessionStart => (codex_review_memory_message(context), "SessionStart"),
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
///
/// Retained for the full-context contract used by review-memory hooks
/// and backward compatibility. PreToolUse now uses the lightweight
/// [`prompt_context_message`] instead.
#[allow(dead_code)]
fn shell_context_message(context: &Context, intro: &str) -> String {
    let mut lines = vec![
        intro.to_string(),
        "Use these values for Hotpot-related Bash commands:".to_string(),
    ];
    lines.extend(context_lines(context));
    lines.join("\n")
}

/// Builds the Codex pre-tool context message including an export hint.
///
/// Retained for backward compatibility. PreToolUse now uses
/// [`prompt_context_message`] instead.
#[allow(dead_code)]
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

/// Builds a lightweight model-visible context message for pre-tool-use hooks.
///
/// Only includes `ROOT_DIR` and `HOTPOT_LANGUAGE` plus the short language
/// directive. Other prompt paths (`HOTPOT_NEW_PROMPT`, `HOTPOT_EXECUTE_PROMPT`,
/// etc.) are NOT listed here — the model should resolve prompt files via
/// `$ROOT_DIR/.hotpot/prompts/<name>.md`.
///
/// Used by both Claude and Codex `PreToolUse` hooks. Replaces the older
/// `shell_context_message` / `codex_shell_context_message` which carry the
/// full `context_lines` (still used by review-memory hooks).
///
/// 轻量模型可见上下文消息，用于 PreToolUse hook。
/// 只包含 `ROOT_DIR`、`HOTPOT_LANGUAGE` 和简短语言指令。
/// 其它 prompt 路径不再逐项列出——模型应通过
/// `$ROOT_DIR/.hotpot/prompts/<name>.md` 拼接定位 prompt 文件。
fn prompt_context_message(context: &Context, intro: &str) -> String {
    let mut lines = vec![
        intro.to_string(),
        "Use these values for Hotpot-related Bash commands:".to_string(),
        format!("- ROOT_DIR: {}", context.root_dir),
        format!("- HOTPOT_LANGUAGE: {}", context.language),
    ];
    // Append the short language directive as reinforcement.
    // 附加简短语言指令作为强化。
    lines.push(String::new());
    lines.push(language_directive_message(&context.language));
    lines.join("\n")
}

/// Builds a short language directive that re-primes the model's output
/// language on every turn.
///
/// Two lines, < 200 chars: short enough to inline without bloating
/// `additionalContext`. Used by `prompt_context_message` (PreToolUse
/// lightweight context) and review-memory messages. The structural-anchor
/// whitelist here is a representative sample (the long list lives in
/// `assets/prompts/output-language.md`); the goal is to re-prime the
/// model against the most common drift trigger every turn.
///
/// 简短的"每轮重申"语言指令（两行 < 200 字符），由 PreToolUse 的轻量
/// 上下文和 review-memory 消息使用。完整锚点清单在 `output-language.md`。
fn language_directive_message(language: &str) -> String {
    format!(
        "Hotpot output language for this turn: `{language}`. \
         Reply in that language for all user-facing prose. \
         Structural anchors stay English: `## Task`, `## Plan`, `### Mode`, `tdd: true|false`, `ACTIVE_CONFLICT:`, kebab-case slugs."
    )
}

/// Returns all Hotpot context values formatted for human-readable hook output.
///
/// Retained for the full-env contract used by review-memory hooks and
/// `shell_export_assignments` / `print_shell_exports`. PreToolUse now
/// uses [`prompt_context_message`] which only lists `ROOT_DIR` and
/// `HOTPOT_LANGUAGE`.
#[allow(dead_code)]
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
///
/// Used by `codex_shell_context_message` (retained for backward compat).
/// This is part of the public full-env bootstrap contract and must be kept.
#[allow(dead_code)]
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

    const REMOVED_PROMPT_HOOK: &str = concat!("UserPrompt", "Submit");

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
    fn claude_pre_tool_use_uses_lightweight_prompt_context() {
        let payload = test_payload();
        let context = Context::from_payload(&payload).unwrap();
        let message = prompt_context_message(
            &context,
            "Hotpot shell context was resolved from the Claude Code hook payload cwd.",
        );

        // Must contain ROOT_DIR
        assert!(
            message.contains(&context.root_dir),
            "PreToolUse (Claude): expected ROOT_DIR in context, got:\n{message}",
        );
        // Must contain HOTPOT_LANGUAGE
        assert!(
            message.contains(&context.language),
            "PreToolUse (Claude): expected HOTPOT_LANGUAGE in context, got:\n{message}",
        );
        // Must NOT contain HOTPOT_NEW_PROMPT
        assert!(
            !message.contains(&context.new_prompt),
            "PreToolUse (Claude): unexpected HOTPOT_NEW_PROMPT in context, got:\n{message}",
        );
        // Must NOT contain HOTPOT_EXECUTE_PROMPT
        assert!(
            !message.contains(&context.execute_prompt),
            "PreToolUse (Claude): unexpected HOTPOT_EXECUTE_PROMPT in context, got:\n{message}",
        );
        // Must NOT contain HOTPOT_FINISH_WORK_PROMPT
        assert!(
            !message.contains(&context.finish_work_prompt),
            "PreToolUse (Claude): unexpected HOTPOT_FINISH_WORK_PROMPT in context, got:\n{message}",
        );
        // Must NOT contain HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT
        assert!(
            !message.contains(&context.record_issue_candidate_prompt),
            "PreToolUse (Claude): unexpected HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT in context, got:\n{message}",
        );
    }

    #[test]
    fn codex_pre_tool_use_uses_lightweight_prompt_context() {
        let payload = test_payload();
        let context = Context::from_payload(&payload).unwrap();
        let value = build_codex_response(&CodexHookCommand::PreToolUse, &context);

        let system_message = value
            .get("systemMessage")
            .and_then(|v| v.as_str())
            .expect("PreToolUse (Codex): expected top-level systemMessage");
        let hso = value
            .get("hookSpecificOutput")
            .and_then(|v| v.as_object())
            .expect("PreToolUse (Codex): expected hookSpecificOutput");
        let additional_context = hso
            .get("additionalContext")
            .and_then(|v| v.as_str())
            .expect("PreToolUse (Codex): expected additionalContext");

        // Both systemMessage and additionalContext must be lightweight
        for (label, message) in [("systemMessage", system_message), ("additionalContext", additional_context)] {
            assert!(
                message.contains(&context.root_dir),
                "PreToolUse (Codex) {label}: expected ROOT_DIR in lightweight context, got:\n{message}",
            );
            assert!(
                message.contains(&context.language),
                "PreToolUse (Codex) {label}: expected HOTPOT_LANGUAGE in lightweight context, got:\n{message}",
            );
            assert!(
                !message.contains(&context.new_prompt),
                "PreToolUse (Codex) {label}: unexpected HOTPOT_NEW_PROMPT, got:\n{message}",
            );
            assert!(
                !message.contains(&context.execute_prompt),
                "PreToolUse (Codex) {label}: unexpected HOTPOT_EXECUTE_PROMPT, got:\n{message}",
            );
            assert!(
                !message.contains(&context.finish_work_prompt),
                "PreToolUse (Codex) {label}: unexpected HOTPOT_FINISH_WORK_PROMPT, got:\n{message}",
            );
            assert!(
                !message.contains(&context.record_issue_candidate_prompt),
                "PreToolUse (Codex) {label}: unexpected HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT, got:\n{message}",
            );
        }
    }

    // ── Task 2: Adjust Platform Hook Trigger Timing ───────────────────

    #[test]
    fn platform_hook_assets_do_not_register_per_user_message_prompt_hook() {
        let claude_settings = include_str!("../../assets/platforms/claude/settings.json");
        let codex_config = include_str!("../../assets/platforms/codex/config.toml");

        // Claude settings must not have the removed per-user-message event hook entry.
        // Claude settings 不能保留已移除的按用户消息触发的 hook。
        assert!(
            !claude_settings.contains(REMOVED_PROMPT_HOOK),
            "Claude settings should not have the removed prompt event, got:\n{claude_settings}",
        );
        // Codex config must not mention the removed per-user-message event.
        // Codex 配置不能提及已移除的按用户消息触发的事件。
        assert!(
            !codex_config.contains(REMOVED_PROMPT_HOOK),
            "Codex config should not mention the removed prompt event, got:\n{codex_config}",
        );
    }

    #[test]
    fn platform_pre_tool_use_assets_cover_edit_write_and_hotpot_bash() {
        let claude_settings = include_str!("../../assets/platforms/claude/settings.json");
        let codex_config = include_str!("../../assets/platforms/codex/config.toml");

        // Claude PreToolUse must cover Bash and Edit|Write (tool-name matchers).
        // Since Claude matchers are regex over tool names, we use Bash|Edit|Write
        // to cover both hotpot-bash and Edit|Write scenarios.
        assert!(
            claude_settings.contains("Bash"),
            "Claude PreToolUse should cover Bash, got:\n{claude_settings}",
        );
        assert!(
            claude_settings.contains("Edit") || claude_settings.contains("Write"),
            "Claude PreToolUse should cover Edit|Write, got:\n{claude_settings}",
        );

        // Codex PreToolUse must cover Bash and Edit|Write
        assert!(
            codex_config.contains("Bash"),
            "Codex PreToolUse should cover Bash, got:\n{codex_config}",
        );
        assert!(
            codex_config.contains("Edit|Write") ||
            (codex_config.contains("Edit") && codex_config.contains("Write")),
            "Codex PreToolUse should cover Edit|Write, got:\n{codex_config}",
        );
    }

    // ── Task 3: Move Prompt Path Guidance To ROOT_DIR Composition ─────

    #[test]
    fn codex_skills_reference_prompt_paths_via_root_dir() {
        let new_skill = include_str!("../../assets/platforms/codex/skills/hotpot-new/SKILL.md");
        let execute_skill = include_str!("../../assets/platforms/codex/skills/hotpot-execute/SKILL.md");
        let finish_skill = include_str!("../../assets/platforms/codex/skills/hotpot-finish-work/SKILL.md");

        // Each skill must reference its main workflow via $ROOT_DIR/.hotpot/prompts/
        assert!(
            new_skill.contains("$ROOT_DIR/.hotpot/prompts/hotpot-new.md"),
            "hotpot-new SKILL.md should reference main workflow via $ROOT_DIR, got:\n{new_skill}",
        );
        assert!(
            execute_skill.contains("$ROOT_DIR/.hotpot/prompts/hotpot-execute.md"),
            "hotpot-execute SKILL.md should reference main workflow via $ROOT_DIR, got:\n{execute_skill}",
        );
        assert!(
            finish_skill.contains("$ROOT_DIR/.hotpot/prompts/hotpot-finish-work.md"),
            "hotpot-finish-work SKILL.md should reference main workflow via $ROOT_DIR, got:\n{finish_skill}",
        );

        // Must NOT use $HOTPOT_*_PROMPT as the primary workflow reference
        assert!(
            !new_skill.contains("$HOTPOT_NEW_PROMPT"),
            "hotpot-new SKILL.md should not use $HOTPOT_NEW_PROMPT as primary path",
        );
        assert!(
            !execute_skill.contains("$HOTPOT_EXECUTE_PROMPT"),
            "hotpot-execute SKILL.md should not use $HOTPOT_EXECUTE_PROMPT as primary path",
        );
        assert!(
            !finish_skill.contains("$HOTPOT_FINISH_WORK_PROMPT"),
            "hotpot-finish-work SKILL.md should not use $HOTPOT_FINISH_WORK_PROMPT as primary path",
        );
    }

    #[test]
    fn pi_context_message_is_lightweight() {
        let pi_extension = include_str!("../../assets/platforms/pi/extensions/hotpot/index.ts");

        // Find the actual `pi.on("context", async (_event, ctx) => {` handler
        // (skip doc-comment mentions of the pattern).
        let marker = "pi.on(\"context\", async (_event, ctx) => {";
        let handler_pos = pi_extension
            .find(marker)
            .expect("Could not find pi.on(\"context\", async (_event, ctx) => { handler");
        let handler_block = &pi_extension[handler_pos..];

        // The handler body ends at the closing `});` of the callback.
        let body_end = handler_block.find("});").map(|p| handler_pos + p + 3).unwrap_or(pi_extension.len());
        let body = &pi_extension[handler_pos..body_end];

        // The context handler's system message must reference ROOT_DIR and HOTPOT_LANGUAGE
        // explicitly, NOT via Object.entries(hotpot) expansion.
        // (injectHotpotEnv and slash command builders still use full HotpotContext.)
        assert!(
            body.contains("ROOT_DIR"),
            "Pi context handler must reference ROOT_DIR explicitly, got:\n{body}",
        );
        assert!(
            body.contains("HOTPOT_LANGUAGE"),
            "Pi context handler must reference HOTPOT_LANGUAGE explicitly, got:\n{body}",
        );
        assert!(
            !body.contains("Object.entries(hotpot)"),
            "Pi context handler must NOT use Object.entries(hotpot) expansion, got:\n{body}",
        );
    }

    // ── Task 4: Update Architecture And Platform Documentation ────────

    #[test]
    fn architecture_docs_describe_lightweight_hook_prompt_contract() {
        let arch_en = include_str!("../../docs/ARCH.md");
        let arch_zh = include_str!("../../docs/ARCH.zh_CN.md");

        // ARCH.md must describe the new hook prompt contract:
        // model-visible context only contains ROOT_DIR and HOTPOT_LANGUAGE;
        // prompt paths are composed via ROOT_DIR/.hotpot/prompts/;
        // full bootstrap/env contract is preserved for shell/plugin use.
        assert!(
            arch_en.contains("lightweight model-visible context") || arch_en.contains("model-visible hook prompt"),
            "ARCH.md should describe the lightweight hook prompt contract",
        );
        assert!(
            arch_en.contains("ROOT_DIR") && arch_en.contains("HOTPOT_LANGUAGE"),
            "ARCH.md should mention ROOT_DIR and HOTPOT_LANGUAGE as model-visible fields",
        );
        assert!(
            arch_en.contains("bootstrap") && arch_en.contains("contract"),
            "ARCH.md should mention the bootstrap/env contract is preserved",
        );

        // ARCH.zh_CN.md must describe the same (in Chinese)
        assert!(
            arch_zh.contains("轻量 hook prompt 契约") || arch_zh.contains("模型可见上下文"),
            "ARCH.zh_CN.md should describe the lightweight hook prompt contract",
        );
        assert!(
            arch_zh.contains("ROOT_DIR") && arch_zh.contains("HOTPOT_LANGUAGE"),
            "ARCH.zh_CN.md should mention ROOT_DIR and HOTPOT_LANGUAGE",
        );
    }

    #[test]
    fn platform_docs_do_not_mention_removed_per_user_message_prompt_hook() {
        let claude_doc = include_str!("../../docs/platforms/claude-code.md");
        let codex_doc = include_str!("../../docs/platforms/codex.md");
        let pi_doc = include_str!("../../docs/platforms/pi.md");
        let arch_en = include_str!("../../docs/ARCH.md");
        let arch_zh = include_str!("../../docs/ARCH.zh_CN.md");

        // Claude doc should mention PreToolUse as the primary delivery mechanism.
        // Claude 文档应说明 PreToolUse 是主要投递机制。
        assert!(
            claude_doc.contains("PreToolUse"),
            "Claude doc should mention PreToolUse as the hook trigger, got excerpt:\n{}",
            &claude_doc[..claude_doc.len().min(200)],
        );

        // Codex doc should mention the new trigger behavior (PreToolUse with Bash|Edit|Write)
        assert!(
            codex_doc.contains("PreToolUse"),
            "Codex doc should mention PreToolUse as the hook trigger",
        );

        // Pi doc should describe the lightweight context approach
        assert!(
            pi_doc.contains("lightweight") || pi_doc.contains("轻量"),
            "Pi doc should describe lightweight context approach",
        );
        assert!(
            pi_doc.contains("ROOT_DIR") && pi_doc.contains("HOTPOT_LANGUAGE"),
            "Pi doc should mention ROOT_DIR and HOTPOT_LANGUAGE as the model-visible fields",
        );

        for (name, content) in [
            ("docs/ARCH.md", arch_en),
            ("docs/ARCH.zh_CN.md", arch_zh),
            ("docs/platforms/claude-code.md", claude_doc),
            ("docs/platforms/codex.md", codex_doc),
        ] {
            assert!(
                !content.contains(REMOVED_PROMPT_HOOK),
                "{name} should not mention removed Hotpot prompt hooks",
            );
        }

        // Also check asset agent files (installed templates) use current wording:
        // language/context arrives through PreToolUse (+ SubagentStart / SessionStart),
        // and removed hooks should not be documented.
        // 同时检查资产 agent 文件使用当前措辞：语言/上下文通过 PreToolUse 注入，
        // 已移除的 hook 不应再出现在文档中。
        let claude_exec_agent = include_str!("../../assets/platforms/claude/agents/hotpot-execution.md");
        let claude_review_agent = include_str!("../../assets/platforms/claude/agents/hotpot-review.md");
        let codex_exec_agent = include_str!("../../assets/platforms/codex/agents/hotpot-execution.toml");
        let codex_review_agent = include_str!("../../assets/platforms/codex/agents/hotpot-review.toml");

        for (name, content) in [
            ("claude/agents/hotpot-execution.md", claude_exec_agent),
            ("claude/agents/hotpot-review.md", claude_review_agent),
        ] {
            assert!(
                content.contains("PreToolUse"),
                "{name} should mention PreToolUse as the context delivery mechanism",
            );
            assert!(
                !content.contains(REMOVED_PROMPT_HOOK),
                "{name} must not mention removed prompt hooks",
            );
        }

        for (name, content) in [
            ("codex/agents/hotpot-execution.toml", codex_exec_agent),
            ("codex/agents/hotpot-review.toml", codex_review_agent),
        ] {
            assert!(
                content.contains("PreToolUse"),
                "{name} should mention PreToolUse as the context delivery mechanism",
            );
            assert!(
                !content.contains(REMOVED_PROMPT_HOOK),
                "{name} must not mention removed prompt hooks",
            );
        }

        // Also check output-language.md (the shared prompt referenced by all workflows).
        // This file is the single source-of-truth for language directives; it must not
        // mention removed per-user-message prompt hooks.
        // 同时检查 output-language.md（所有工作流引用的共享提示词）。
        // 该文件是语言指令的唯一来源，不应再提及已移除的按用户消息触发的 hook。
        let output_lang_asset = include_str!("../../assets/prompts/output-language.md");
        let output_lang_installed = include_str!("../../.hotpot/prompts/output-language.md");

        for (name, content) in [
            ("assets/prompts/output-language.md", output_lang_asset),
            (".hotpot/prompts/output-language.md", output_lang_installed),
        ] {
            assert!(
                content.contains("PreToolUse"),
                "{name} should mention PreToolUse as the context delivery mechanism",
            );
            assert!(
                !content.contains(REMOVED_PROMPT_HOOK),
                "{name} must not mention removed prompt hooks",
            );
        }
    }

    #[test]
    fn opencode_does_not_inject_bulk_context_into_model_context() {
        let bash_before = include_str!("../../assets/platforms/opencode/plugins/hotpot-bash-before.ts");
        let review_memory = include_str!("../../assets/platforms/opencode/plugins/hotpot-review-memory.ts");
        let opencode_doc = include_str!("../../docs/platforms/opencode.md");

        for (name, content) in [
            ("hotpot-bash-before.ts", bash_before),
            ("hotpot-review-memory.ts", review_memory),
        ] {
            assert!(
                content.contains("\"shell.env\""),
                "{name} should inject full context only into shell.env",
            );
            assert!(
                !content.contains("additionalContext") && !content.contains("systemMessage"),
                "{name} must not inject Hotpot bootstrap data into model context",
            );
            assert!(
                !content.contains("Object.entries(hotpot)"),
                "{name} must not expand the full Hotpot context into model-visible text",
            );
        }

        assert!(
            opencode_doc.contains("does not inject") && opencode_doc.contains("model context"),
            "OpenCode docs should state bulk bootstrap data is not injected into model context",
        );
    }
}
