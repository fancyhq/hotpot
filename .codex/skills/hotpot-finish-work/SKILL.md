<!-- Hotpot Codex skill asset for finishing the active task. -->
---
name: hotpot-finish-work
description: Finish the active Hotpot task, promote review memory candidates, optionally create a git commit, and mark the task done.
argument-hint: "[finish notes]"
allowed-tools: Read, Bash
user-invocable: true
---

# Hotpot Finish Work

You are a Hotpot workflow skill for finishing the active task. Only enter this flow when the user explicitly asks to finish a Hotpot task or explicitly invokes this skill. Do not auto-start from a generic development request.

The full workflow is defined at `$HOTPOT_FINISH_WORK_PROMPT` (Codex session hooks export this env var). Read that file first and follow the workflow end-to-end.

Codex has no `@path` expansion. When the shared body references `@.hotpot/prompts/<name>.md`, substitute the matching env var and use `Read`:

- `@.hotpot/prompts/output-language.md` → resolve as `$ROOT_DIR/.hotpot/prompts/output-language.md` and use `Read`
- `@.hotpot/prompts/summarize-issue-candidates.md` → `$HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`
- `@.hotpot/prompts/get-issue.md` → resolve as `$ROOT_DIR/.hotpot/prompts/get-issue.md` and use `Read`
- `@.hotpot/prompts/hotpot-execute.md` → `$HOTPOT_EXECUTE_PROMPT`

Platform note: when the shared body's "Offer to Resume Next Task" step needs to invoke the Hotpot execution agent, spawn the custom agent from `.codex/agents/hotpot-execution.toml`.

Output language reminder: before producing any user-facing reply each turn, restate to yourself "Reply in `$HOTPOT_LANGUAGE`; structural anchors stay English". The Codex `UserPromptSubmit` hook already pushes the same directive into your context every turn — this in-skill reminder is the backup belt.
