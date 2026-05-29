---
name: hotpot-new
description: Create a Hotpot task through brainstorming and planning.
argument-hint: "[initial task idea]"
allowed-tools: Read, Write, Edit, MultiEdit, Glob, Grep, Bash
user-invocable: true
---

# Hotpot New

You are a Hotpot workflow skill for creating a new task. Only enter this flow when the user explicitly asks to create a Hotpot task or explicitly invokes this skill. Do not auto-start from a generic development request.

The full workflow is defined at `$ROOT_DIR/.hotpot/prompts/hotpot-new.md`. Read that file first and follow the workflow end-to-end.

When VuePress is enabled, Do NOT create a plain Markdown task file first and rewrite it later. Read `vuepress-style.md` before the task-file write gate, then create the final task file once with Codex `apply_patch` using an `*** Add File` hunk.

Codex has no `@path` expansion. When the shared body references `@.hotpot/prompts/<name>.md`, resolve the matching file via `$ROOT_DIR/.hotpot/prompts/<name>.md` and use `Read`:

- `@.hotpot/prompts/output-language.md` → resolve as `$ROOT_DIR/.hotpot/prompts/output-language.md` and use `Read`
- `@.hotpot/prompts/tdd-protocol.md` → resolve as `$ROOT_DIR/.hotpot/prompts/tdd-protocol.md` and use `Read`
- `@.hotpot/prompts/record-issue-candidate.md` → resolve as `$ROOT_DIR/.hotpot/prompts/record-issue-candidate.md` and use `Read`
- `@.hotpot/prompts/summarize-issue-candidates.md` → resolve as `$ROOT_DIR/.hotpot/prompts/summarize-issue-candidates.md` and use `Read`
- `@.hotpot/prompts/get-issue.md` → resolve as `$ROOT_DIR/.hotpot/prompts/get-issue.md` and use `Read`
- `@.hotpot/prompts/hotpot-execute.md` → resolve as `$ROOT_DIR/.hotpot/prompts/hotpot-execute.md` and use `Read`
- `@.hotpot/prompts/hotpot-finish-work.md` → resolve as `$ROOT_DIR/.hotpot/prompts/hotpot-finish-work.md` and use `Read`

Platform note: `new` itself does not need a subagent. If a workflow step references "the registered Hotpot execution agent" or "the registered Hotpot review agent", spawn the corresponding custom agent from `.codex/agents/hotpot-execution.toml` or `.codex/agents/hotpot-review.toml`.

Output language reminder: before producing any user-facing reply each turn, restate to yourself "Reply in `$HOTPOT_LANGUAGE`; structural anchors stay English". The Codex `PreToolUse` hook pushes the same directive into your context on every tool-use turn — this in-skill reminder is the backup belt.
