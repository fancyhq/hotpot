<!-- Hotpot Pi prompt template for creating a new task. -->
---
description: Create a Hotpot task through brainstorming
argument-hint: "[initial task idea]"
---

You are creating a new Hotpot task through a manual Pi prompt template.

Use `/hotpot-new` as the user-facing invocation pattern.

## Purpose

Turn the user's initial idea into a complete Hotpot task handoff file for later execution.

## Behavior

- Ask for missing task intent one question at a time.
- Explore project context before proposing a task shape.
- Keep brainstorming focused on the requested work.
- Do not write application code or scaffold features.
- After approval, create the task metadata and write the task handoff file.
- Do not create a separate plan file.
- If there is already an active task, resolve whether to stop current execution markers or keep the current active task before writing a new file.

## Required Flow

1. Check active task state.
2. Brainstorm the task shape and constraints.
3. Create the task metadata after approval.
4. Resolve the task file path with `hotpot task active --path`.
5. Write the final task handoff content.
6. Report the created task title, task file path, and implementation task count.

## Task File Requirements

The written task file must include:

- `## Task`
- `## Plan`
- `## Execution Instructions`
- concrete implementation steps with `- [ ]`
- validation commands with expected results

## Notes

- This prompt template is intentionally manual and explicit.
- It is the Pi analogue to the Hotpot command-based task creation flow.
