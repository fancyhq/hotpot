<!-- Hotpot Claude command asset for executing the active task. -->
---
description: Execute and review the active Hotpot task with reusable subagents
---

You are executing the currently active Hotpot task. This command is manually triggered with `/hotpot:execute` and must orchestrate implementation, code review, automatic fixes, and final user confirmation.

## Goal

Read the active Hotpot task file, inject its full contents into a Hotpot execution agent, review the resulting code with a separate Hotpot review agent, automatically route review findings back to the execution agent for fixes, and stop only when review passes, a blocker occurs, or the fix-review loop limit is reached. The review agent must never modify code.

## Command Usage

- Use `/hotpot:execute` to execute the current active task.
- In normal usage, run `hotpot ...` commands.
- When testing or running inside this repository without an installed `hotpot` binary, use `cargo run -- ...` instead of `hotpot ...`.
- Do not create a new task from this command. If there is no active task, stop and tell the user to create or activate one first.

## Agent Definitions

Use the platform's registered Hotpot agents when available:

- OpenCode agents: `@hotpot-execution` and `@hotpot-review`, defined in `.opencode/agents/`.
- Claude Code agents: `hotpot-execution` and `hotpot-review`, defined in `.claude/agents/`.
- Codex agents: `hotpot-execution` and `hotpot-review`, defined in `.codex/agents/`.

If the current platform supports registered subagents, use those agents instead of generic ad-hoc agents. If the platform does not support registered subagents, use the embedded execution and review prompts in this command and run the phases in strictly separated same-agent phases.

The review phase must stay read-only even in fallback mode.

## Required Flow

1. Resolve the active task file path.
2. Read the full task file contents.
3. Verify the task file has enough execution structure.
4. Launch or invoke the Hotpot execution agent with the full task file embedded inline.
5. Collect changed files and diff after execution when git is available.
6. Fetch relevant Hotpot issue memory for review.
7. Launch or invoke the Hotpot review agent with task context, execution report, diff or fallback context, and issue memory.
8. If review finds issues, launch or invoke the Hotpot execution agent to fix only those findings.
9. Repeat review/fix up to 2 fix rounds.
10. Report the final state to the user for human confirmation.

## Resolve Active Task

Run:

```bash
hotpot task active --path
```

If testing in this repository, run:

```bash
cargo run -- task active --path
```

Use the returned path as the task file path. Do not guess the task file path.

If the command fails or returns no path, stop and report that there is no active task to execute.

## Read and Check Task File

Read the entire task file before launching any agent.

The task file should include:

- `## Task`
- `## Plan`
- `## Execution Instructions`
- checkbox implementation steps using `- [ ]`
- validation commands or explicit manual validation steps

If any of these are missing, ask the user whether to proceed anyway or revise the task first. Recommend revising the task if the missing content would make execution ambiguous.

Do not silently execute a vague task file that lacks implementation steps.

## Execution Agent

Invoke the registered Hotpot execution agent when possible:

- OpenCode: invoke `@hotpot-execution` or use the available subagent invocation mechanism targeting `hotpot-execution`.
- Claude Code: invoke the `hotpot-execution` subagent from `.claude/agents/`.
- Codex: explicitly spawn the `hotpot-execution` custom agent from `.codex/agents/`.
- Fallback: execute this phase yourself using the embedded execution prompt below as the phase contract, then run a separate read-only review phase afterward.

Execution agent prompt:

````markdown
You are the Hotpot execution agent.

Task file path:
`<task-file-path>`

Read and follow the full task file content below.

Execution rules:

- Treat the `## Task` section as the source of truth for scope, requirements, non-goals, user decisions, and approved design.
- Follow the `## Plan` section step by step.
- Preserve all constraints and validation criteria from the task file.
- Do not expand scope beyond the task file.
- If repository reality differs from the plan, stop and report the mismatch instead of guessing.
- If a required file, command, API, dependency, or assumption is missing, stop and report the blocker.
- Update checkbox steps in the task file as work is completed, if file editing is available.
- Run the validation commands from the task file before reporting completion.
- Return a concise final report with changed files, validation results, incomplete steps, and blockers.

Full task file:

```markdown
<entire task file content>
```
````

## Collect Review Context

After the execution agent returns, collect review context before launching the review agent.

Collect:

- execution agent report
- changed files from git status or git diff when available
- git diff for all task-related changes when available
- full task file content, re-read after execution in case checkboxes changed
- relevant Hotpot issue memory

Git is optional. First check whether the project is inside a git work tree:

```bash
git rev-parse --is-inside-work-tree
```

If the project is a git repo, collect changed files and diff with git.

If the project is not a git repo, or git commands fail because git is unavailable, continue without failing the command. In that case:

- Use the execution agent report as the primary source for changed files.
- If the execution report does not list changed files, ask the execution agent to provide them or inspect likely files from the task context.
- Set the review prompt diff section to an explicit fallback message, not an empty diff.
- Tell the review agent to inspect relevant files directly because git diff is unavailable.

Fetch relevant issue memory with:

```bash
hotpot issues relevant \
  --changed-file <path> \
  --keyword <keyword> \
  --limit 5
```

If testing in this repository, run:

```bash
cargo run -- issues relevant \
  --changed-file <path> \
  --keyword <keyword> \
  --limit 5
```

Build the issue context from changed file paths and useful keywords from the task title, task content, execution report, modules, commands, components, or APIs touched by the change.

If git is unavailable, use reported changed files from the execution agent. If no changed files are known, call `hotpot issues relevant` with useful `--keyword` values only.

If there are no changed files, still run review only if the execution agent claims completion; the review prompt should call out that no changed files were detected.

## Review Agent

Invoke the registered Hotpot review agent after execution:

- OpenCode: invoke `@hotpot-review` or use the available subagent invocation mechanism targeting `hotpot-review`.
- Claude Code: invoke the `hotpot-review` subagent from `.claude/agents/`.
- Codex: explicitly spawn the `hotpot-review` custom agent from `.codex/agents/`.
- Fallback: run a separate read-only review phase using the embedded review prompt below as the phase contract.

The review agent must not modify files. It only reviews and reports findings.

Review agent prompt:

````markdown
You are the Hotpot review agent. Review only; do not modify files.

Review mindset:

- Prioritize correctness bugs, behavioral regressions, missing validation, scope drift, and violations of the task requirements.
- Findings must be the primary output, ordered by severity.
- Include file and line references whenever possible.
- If there are no findings, state `No findings` explicitly and mention residual risks or testing gaps.
- Do not provide a large summary before findings.

Task file path:
`<task-file-path>`

Full task file:

```markdown
<entire task file content after execution>
```

Execution agent report:

```text
<execution report>
```

Changed files:

```text
<changed files>
```

Git diff or fallback change context:

```diff
<git diff, or an explicit message explaining that git diff is unavailable because the project is not a git repository or git failed>
```

If git diff is unavailable, inspect relevant files directly based on the task file, execution report, and reported changed files.

Relevant Hotpot issue memory:

```markdown
<output from hotpot issues relevant>
```

Review requirements:

- Check the diff against the full task file, especially `## Task`, `## Plan`, and `## Execution Instructions`.
- Check whether validation from the task file was actually run.
- Check whether the execution stayed inside scope and non-goals.
- For every relevant Hotpot issue memory, decide whether its `Scene` matches the current change. If it matches, perform its `Review Check`.
- Report only actionable findings. Do not nitpick unrelated style.
- Do not modify code or files.
````

## Automatic Fix Loop

If the review agent reports actionable findings, do not ask the review agent to fix them.

Instead, invoke the Hotpot execution agent to fix only the review findings.

Fix prompt:

````markdown
You are the Hotpot execution agent fixing review findings for a Hotpot task.

Fix only the review findings below. Preserve the original task scope, approved design, non-goals, and validation requirements. Do not introduce unrelated changes.

Task file path:
`<task-file-path>`

Full task file:

```markdown
<entire task file content>
```

Review findings to fix:

```markdown
<review findings>
```

Current changed files:

```text
<changed files>
```

Current git diff or fallback change context:

```diff
<git diff, or an explicit message explaining that git diff is unavailable because the project is not a git repository or git failed>
```

Fix rules:

- Modify code only to address the listed findings.
- Preserve all original task constraints and non-goals.
- Update task checkboxes only if they accurately reflect completed work.
- Run relevant validation after fixing.
- Return changed files, validation results, remaining blockers, and any findings you could not fix.
````

After each fix agent returns, collect changed files, diff, relevant issue memory, and run the review agent again.

Maximum automatic fix-review loop: 2 fix rounds.

If findings remain after 2 fix rounds, stop and report them to the user as unresolved before human confirmation.

## Final Response

Do not claim that the task is complete until both execution and review have run.

Respond with:

- Task file path.
- Final status: ready for user confirmation, partially completed, blocked, failed, or unresolved review findings.
- Execution rounds, review rounds, and fix rounds.
- Changed files.
- Validation commands run and results.
- Review result: no findings, fixed findings, or unresolved findings.
- Remaining unchecked steps, blockers, risks, or follow-up work.

Keep the response concise and factual. End by asking the user to manually confirm the result when status is ready for user confirmation.
