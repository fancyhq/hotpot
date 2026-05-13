<!-- Hotpot Codex skill asset for executing the active task. -->
---
name: hotpot-execute
description: Execute and review the active Hotpot task with review-fix loops.
argument-hint: "[execution notes]"
allowed-tools: Read, Write, Edit, MultiEdit, Glob, Grep, Bash
user-invocable: true
---

# Hotpot Execute

You are a Hotpot workflow skill for executing the currently active task.

Only enter the Hotpot execution flow when the user explicitly asks to execute a Hotpot task or explicitly invokes this skill. Do not auto-start from a generic development request.

## Purpose

Read the active task file, execute the implementation plan, run review, and route actionable findings back into fix rounds until review passes or the loop limit is reached.

## Behavior

- Resolve the active task file with `hotpot task active --path`.
- Read the full task file before making changes.
- Verify the task has `## Task`, `## Plan`, `## Execution Instructions`, checkbox steps, and validation commands.
- Use the registered `hotpot-execution` and `hotpot-review` agents when available.
- Keep the review phase read-only.
- If the platform cannot spawn subagents, fall back to a strictly separated execution phase and read-only review phase in the same session.
- Do not create new tasks from this skill.

## Required Flow

1. Resolve the active task file path.
2. Read the full task file.
3. Run the execution phase.
4. Collect diff and changed files.
5. Run the review phase.
6. Fix actionable findings only, up to two rounds.
7. Report final status for human confirmation.

## Constraints

- Review must remain read-only.
- Do not expand scope beyond the task file.
- Preserve non-goals and validation requirements.
- Stop and report blockers if repository reality differs from the task handoff.
