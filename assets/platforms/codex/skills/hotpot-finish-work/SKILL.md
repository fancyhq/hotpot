---
name: hotpot-finish-work
description: Finish the active Hotpot task, promote review memory candidates, optionally create a git commit, and mark the task done.
argument-hint: "[finish notes]"
allowed-tools: Read, Bash
user-invocable: true
---

# Hotpot Finish Work

You are a Hotpot workflow skill for finishing the active task. Only enter this flow when the user explicitly asks to finish a Hotpot task or explicitly invokes this skill. Do not auto-start from a generic development request.

The full workflow is defined at `$ROOT_DIR/.hotpot/prompts/hotpot-finish-work.md`. Read that file first and follow the workflow end-to-end.

Codex has no `@path` expansion. When the shared body references `@.hotpot/prompts/<name>.md`, resolve the matching file via `$ROOT_DIR/.hotpot/prompts/<name>.md` and use `Read`:

- `@.hotpot/prompts/output-language.md` → resolve as `$ROOT_DIR/.hotpot/prompts/output-language.md` and use `Read`
- `@.hotpot/prompts/summarize-issue-candidates.md` → resolve as `$ROOT_DIR/.hotpot/prompts/summarize-issue-candidates.md` and use `Read`
- `@.hotpot/prompts/get-issue.md` → resolve as `$ROOT_DIR/.hotpot/prompts/get-issue.md` and use `Read`
- `@.hotpot/prompts/hotpot-execute.md` → resolve as `$ROOT_DIR/.hotpot/prompts/hotpot-execute.md` and use `Read`

Platform note: when the shared body's "Offer to Resume Next Task" step needs to invoke the Hotpot execution agent, spawn the custom agent from `.codex/agents/hotpot-execution.toml`.

Output language reminder: before producing any user-facing reply each turn, restate to yourself "Reply in `$HOTPOT_LANGUAGE`; structural anchors stay English". The Codex `PreToolUse` hook pushes the same directive into your context on every tool-use turn — this in-skill reminder is the backup belt.
