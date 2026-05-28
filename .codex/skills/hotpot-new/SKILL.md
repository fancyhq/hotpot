---
name: hotpot-new
description: Create a Hotpot task through brainstorming and planning.
argument-hint: "[initial task idea]"
allowed-tools: Read, Write, Edit, MultiEdit, Glob, Grep, Bash
user-invocable: true
---

# Hotpot New

You are a Hotpot workflow skill for creating a new task. Only enter this flow when the user explicitly asks to create a Hotpot task or explicitly invokes this skill. Do not auto-start from a generic development request.

The full workflow is defined at `$HOTPOT_NEW_PROMPT` (Codex session hooks export this env var). Read that file first and follow the workflow end-to-end.

Codex has no `@path` expansion. When the shared body references `@.hotpot/prompts/<name>.md`, substitute the matching env var and use `Read`:

- `@.hotpot/prompts/output-language.md` → resolve as `$ROOT_DIR/.hotpot/prompts/output-language.md` and use `Read`
- `@.hotpot/prompts/tdd-protocol.md` → `$HOTPOT_TDD_PROTOCOL_PROMPT`
- `@.hotpot/prompts/record-issue-candidate.md` → `$HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`
- `@.hotpot/prompts/summarize-issue-candidates.md` → `$HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`
- `@.hotpot/prompts/get-issue.md` → resolve as `$ROOT_DIR/.hotpot/prompts/get-issue.md` and use `Read`
- `@.hotpot/prompts/hotpot-execute.md` → `$HOTPOT_EXECUTE_PROMPT`
- `@.hotpot/prompts/hotpot-finish-work.md` → `$HOTPOT_FINISH_WORK_PROMPT`

Codex VuePress write-order rule: when the shared workflow's file-existence gate shows VuePress is enabled, read `vuepress-style.md` before creating the task file and use `apply_patch` with `*** Add File` to write the final VuePress-formatted task file in one shot. Do NOT create a plain Markdown task file first and then update it into VuePress format.

Platform note: `new` itself does not need a subagent. If a workflow step references "the registered Hotpot execution agent" or "the registered Hotpot review agent", spawn the corresponding custom agent from `.codex/agents/hotpot-execution.toml` or `.codex/agents/hotpot-review.toml`.

Output language reminder: before producing any user-facing reply each turn, restate to yourself "Reply in `$HOTPOT_LANGUAGE`; structural anchors stay English". The Codex `UserPromptSubmit` hook already pushes the same directive into your context every turn — this in-skill reminder is the backup belt.
