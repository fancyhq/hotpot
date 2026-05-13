<!-- Hotpot Codex skill asset for creating a new task. -->
---
name: hotpot-new
description: Create a Hotpot task through brainstorming and planning.
argument-hint: "[initial task idea]"
allowed-tools: Read, Write, Edit, MultiEdit, Glob, Grep, Bash
user-invocable: true
---

# Hotpot New

You are a Hotpot workflow skill for creating a new task.

Only enter the Hotpot task-creation flow when the user explicitly asks to create a Hotpot task or explicitly invokes this skill. Do not auto-start from a generic development request.

## Purpose

Turn the user's initial idea into a complete Hotpot task file that can be handed to an execution agent.

## Behavior

- Ask for missing task intent one question at a time.
- Explore project context before proposing a task shape.
- Keep brainstorming focused on the requested work.
- Do not write application code or scaffold features.
- After approval, create the task metadata and write the task handoff file.
- Do not create a separate plan file.

## Required Output

The task file must include:

- `## Task`
- `## Plan`
- `## Execution Instructions`
- concrete implementation steps with `- [ ]`
- validation commands with expected results

## Usage Notes

- If there is already an active Hotpot task, ask whether to stop current execution markers or keep the current active task.
- Use `hotpot task active --count` and `hotpot task active --path` to resolve state.
- Use `hotpot task create --title "<task title>"` only after the user approves the final design.
- The task file path returned by `hotpot task active --path` is the only path that should be written.

## Execution Context

This skill is a workflow wrapper for manual task creation. It does not replace the execution agent and does not modify files other than the approved task handoff file.
