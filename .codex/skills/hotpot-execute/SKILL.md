---
name: hotpot-execute
description: Execute and review the active Hotpot task with review-fix loops.
argument-hint: "[execution notes]"
allowed-tools: Read, Write, Edit, MultiEdit, Glob, Grep, Bash
user-invocable: true
---

# Hotpot Execute

You are a Hotpot workflow skill for executing the currently active task. Only enter this flow when the user explicitly asks to execute a Hotpot task or explicitly invokes this skill. Do not auto-start from a generic development request.

The full workflow is defined at `$ROOT_DIR/.hotpot/prompts/hotpot-execute.md`. Read that file first and follow the workflow end-to-end.

Codex has no `@path` expansion. When the shared body references `@.hotpot/prompts/<name>.md`, resolve the matching file via `$ROOT_DIR/.hotpot/prompts/<name>.md` and use `Read`:

- `@.hotpot/prompts/output-language.md` → resolve as `$ROOT_DIR/.hotpot/prompts/output-language.md` and use `Read`
- `@.hotpot/prompts/tdd-protocol.md` → resolve as `$ROOT_DIR/.hotpot/prompts/tdd-protocol.md` and use `Read`
- `@.hotpot/prompts/record-issue-candidate.md` → resolve as `$ROOT_DIR/.hotpot/prompts/record-issue-candidate.md` and use `Read`
- `@.hotpot/prompts/summarize-issue-candidates.md` → resolve as `$ROOT_DIR/.hotpot/prompts/summarize-issue-candidates.md` and use `Read`
- `@.hotpot/prompts/get-issue.md` → resolve as `$ROOT_DIR/.hotpot/prompts/get-issue.md` and use `Read`

Platform note: when the shared body refers to "the registered Hotpot execution agent" or "the registered Hotpot review agent", spawn the corresponding custom agent from `.codex/agents/hotpot-execution.toml` or `.codex/agents/hotpot-review.toml`. Codex supports subagents natively, so the review phase runs in a separate read-only context.

Output language reminder: before producing any user-facing reply each turn, restate to yourself "Reply in `$HOTPOT_LANGUAGE`; structural anchors stay English". The Codex `PreToolUse` hook pushes the same directive into your context on every tool-use turn — this in-skill reminder is the backup belt.
