<!-- Hotpot Codex skill asset for executing the active task. -->
---
name: hotpot-execute
description: Execute and review the active Hotpot task with review-fix loops.
argument-hint: "[execution notes]"
allowed-tools: Read, Write, Edit, MultiEdit, Glob, Grep, Bash
user-invocable: true
---

# Hotpot Execute

You are a Hotpot workflow skill for executing the currently active task. Only enter this flow when the user explicitly asks to execute a Hotpot task or explicitly invokes this skill. Do not auto-start from a generic development request.

The full workflow is defined at `$HOTPOT_EXECUTE_PROMPT` (Codex session hooks export this env var). Read that file first and follow the workflow end-to-end.

Codex has no `@path` expansion. When the shared body references `@.hotpot/prompts/<name>.md`, substitute the matching env var and use `Read`:

- `@.hotpot/prompts/output-language.md` → resolve as `$ROOT_DIR/.hotpot/prompts/output-language.md` and use `Read`
- `@.hotpot/prompts/tdd-protocol.md` → `$HOTPOT_TDD_PROTOCOL_PROMPT`
- `@.hotpot/prompts/record-issue-candidate.md` → `$HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`
- `@.hotpot/prompts/summarize-issue-candidates.md` → `$HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`
- `@.hotpot/prompts/get-issue.md` → resolve as `$ROOT_DIR/.hotpot/prompts/get-issue.md` and use `Read`

Platform note: when the shared body refers to "the registered Hotpot execution agent" or "the registered Hotpot review agent", spawn the corresponding custom agent from `.codex/agents/hotpot-execution.toml` or `.codex/agents/hotpot-review.toml`. Codex supports subagents natively, so the review phase runs in a separate read-only context.
