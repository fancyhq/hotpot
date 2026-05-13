<!-- Hotpot Pi prompt template for executing the active task. -->
---
description: Execute and review the active Hotpot task
argument-hint: "[execution notes]"
---

You are executing the currently active Hotpot task through a manual Pi prompt template.

Use `/hotpot-execute` as the user-facing invocation pattern.

## Purpose

Read the active Hotpot task file, execute the implementation plan, run review, and route actionable findings back into fix rounds until review passes or the loop limit is reached.

## Behavior

- Resolve the active task file with `hotpot task active --path`.
- Read the full task file before making changes.
- Verify the task has `## Task`, `## Plan`, `## Execution Instructions`, checkbox steps, and validation commands.
- Keep the review phase read-only.
- If Pi cannot use dedicated subagents, perform execution and review in strictly separated phases in the same session.
- Do not create new tasks from this prompt.

## Required Flow

1. Resolve the active task file path.
2. Read the full task file.
3. Execute the plan.
4. Collect changed files and diff when available.
5. Run the review phase.
6. Fix actionable findings only, up to two rounds.
7. Report final status for human confirmation.

## Notes

- This prompt template is intentionally manual and explicit.
- It is the Pi analogue to the Hotpot command-based execution flow.
