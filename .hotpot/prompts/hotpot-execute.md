<!--
Hotpot shared workflow body: execute, review, fix the active task.

Platform-specific thin shells (Claude / OpenCode / Codex / Pi) reference this file via
`@.hotpot/prompts/hotpot-execute.md` or `$HOTPOT_EXECUTE_PROMPT`. Do not paste
platform-specific syntax (slash-command name, subagent name, frontmatter) into this
body — keep all such overrides in the thin shells. Likewise, do not duplicate
`@.hotpot/prompts/<other>.md` token resolution rules — Codex/Pi thin shells provide
the substitution tables.
-->

You are executing the currently active Hotpot task. This command is manually triggered and must orchestrate implementation, code review, automatic fixes, and final user confirmation.

## Goal

Read the active Hotpot task file, inject its full contents into a Hotpot execution agent, review the resulting code with a separate Hotpot review agent, automatically route review findings back to the execution agent for fixes, and stop only when review passes, a blocker occurs, or the fix-review loop limit is reached. The review agent must never modify code.

## Output Language

Apply the project language preference to every natural-language output produced by this command — phase announcements, execution / review reports, fix-loop status lines, candidate summaries surfaced to the user, and the final per-command report. Pass the same language directive into the prompts you embed for the execution and review agents so their output is consistent. Structural anchors and machine-readable tokens (CLI flags, JSON keys, `ACTIVE_CONFLICT:`, markdown section headings, `tdd: true|false`, kebab-case slugs) MUST stay in English. Full rule:

@.hotpot/prompts/output-language.md

Codex / Pi (no `@path` expansion): the thin shell's substitution table maps this to `$ROOT_DIR/.hotpot/prompts/output-language.md`. `Read` that path before proceeding.

## Pre-flight: Release VuePress server (if any)

Run `hotpot vuepress stop --if-running` once at the start of this command (run via your platform's bash tool). The command is idempotent: it succeeds whether or not a VuePress dev server is currently running, and whether or not VuePress is even installed in this project. Do not block on its output and do not branch on whether it printed anything — the goal is just to release any dev-server process started by `/hotpot:new` so it does not outlive the brainstorming session into execution.

Rationale: when VuePress is enabled, `/hotpot:new` may have spawned a `pnpm docs:dev` process to give the user a browser preview. Calling stop here is the primary mechanism that releases that process; the per-platform SessionEnd hook is a secondary safety net for cases where the user closes the session instead of running this command.

## Pre-flight: Verify Subagent Registration

Run this check **before** the Worktree Decision (Step 0). Its purpose is to catch the OpenCode-style `Unknown agent type: hotpot-execution` failure mode (and its Claude / Codex equivalents) at a single, recoverable point — not deep inside the execute / review loop where the symptom is opaque.

### What To Check

Probe whichever platform configuration directories exist under `$ROOT_DIR`. If a project has multiple platforms installed (e.g. both `.claude/` and `.opencode/`), check each one in turn:

- Claude Code present → require `$ROOT_DIR/.claude/agents/hotpot-execution.md` and `$ROOT_DIR/.claude/agents/hotpot-review.md`
- OpenCode present → require `$ROOT_DIR/.opencode/agents/hotpot-execution.md` and `$ROOT_DIR/.opencode/agents/hotpot-review.md`
- Codex present → require `$ROOT_DIR/.codex/agents/hotpot-execution.toml` and `$ROOT_DIR/.codex/agents/hotpot-review.toml`
- Pi present → **skip this check**. Pi has no native subagent registry; `/hotpot:execute` runs as same-session phased execution, so there is no `Unknown agent type` failure surface.

Use a `$ROOT_DIR`-relative bash probe (do NOT hard-code absolute paths from another user's machine):

```bash
for plat in claude opencode; do
  if [ -d "$ROOT_DIR/.$plat" ]; then
    for role in execution review; do
      f="$ROOT_DIR/.$plat/agents/hotpot-$role.md"
      [ -f "$f" ] || echo "MISSING: $f"
    done
  fi
done
if [ -d "$ROOT_DIR/.codex" ]; then
  for role in execution review; do
    f="$ROOT_DIR/.codex/agents/hotpot-$role.toml"
    [ -f "$f" ] || echo "MISSING: $f"
  done
fi
```

### How To React

- All required files present and a subsequent subagent call succeeds → continue silently.
- At least one required file is missing → STOP and reply to the user with:

  > Hotpot subagent file missing at `<path>`. The `<platform>` registry cannot resolve `hotpot-execution` / `hotpot-review`. Run `hotpot update --platform <platform>`, then restart the agent session before re-running `/hotpot:execute`.

- All required files are present on disk but a later subagent invocation still fails with `Unknown agent type: hotpot-execution` (or `hotpot-review`) → treat this as a platform subagent-registry cache miss. Apply the **same** recovery path: ask the user to run `hotpot update --platform <platform>` and to **restart the agent session** (the session must be torn down so the registry is rebuilt), then STOP. Do not retry the same subagent in the current session — the cache will keep returning the same `Unknown agent type` error.

This pre-flight runs **before** the Worktree Decision (Step 0) so a doomed run does not create a worktree that will immediately be orphaned.

## Precondition: Task File Must Exist

The active task `.md` MUST already exist on disk before this command runs. If `Read` on the resolved active path returns "File not found", `/hotpot:new` for this title did not complete its write step. Stop and tell the user to re-run `/hotpot:new` for the task — do NOT silently recreate the file ad-hoc from `overview.jsonl`, the brainstorming and approved design would be lost.

## Command Usage

- The user-facing invocation pattern is supplied by the platform thin shell.
- In normal usage, run `hotpot ...` commands.
- When testing or running inside this repository without an installed `hotpot` binary, use `cargo run -- ...` instead of `hotpot ...`.
- Do not create a new task from this command. If there is no active task, stop and tell the user to create or activate one first.

## Agent Definitions

Use the platform's registered Hotpot agents `hotpot-execution` and `hotpot-review` when available. The thin shell maps the agent names to the platform's invocation mechanism (subagent registry, `@mention`, custom-agent spawn, or same-session phased execution).

If the platform has no dedicated subagent system, run the execution and review phases as strictly separated phases in the same session. In that fallback, announce the current phase explicitly at its start (for example `I am now in the EXECUTION phase` or `I am now in the READ-ONLY REVIEW phase`) so context does not bleed across phases.

The review phase must stay read-only even in fallback mode.

## Required Flow

0. **Worktree decision (opt-in)**: probe whether a worktree is already attached to the task; if not, ask the user whether to create one and (on yes) run `hotpot worktree create`. Remember the resulting worktree path — every subsequent step in this run inherits it.
1. Resolve the active task file path.
2. Read the full task file contents.
3. Verify the task file has enough execution structure.
4. Detect TDD mode from `## Plan > ### Mode`.
5. Launch or invoke the Hotpot execution agent with the full task file embedded inline, plus the worktree directive if one is attached.
6. Collect changed files and diff after execution when git is available, scoped to the worktree if attached.
7. Fetch relevant Hotpot issue memory for review.
8. Launch or invoke the Hotpot review agent with task context, execution report, diff or fallback context, issue memory, and the worktree directive if one is attached.
9. If review finds issues, launch or invoke the Hotpot execution agent to fix only those findings (worktree directive still applies).
10. Repeat review/fix up to 2 fix rounds.
11. Decide which repairs are worth recording as reusable issue candidates by applying `@.hotpot/prompts/record-issue-candidate.md`; buffer them in memory only.
12. Show the buffered candidate summary to the user and write only the user-approved ones via `hotpot issues candidate add`.
13. Report the final state to the user for human confirmation.

## Worktree Decision (Step 0)

Before resolving the active task path, decide whether this run will operate inside an isolated per-task git worktree. The decision is fully opt-in: the default is "no worktree, behavior unchanged from previous Hotpot versions".

### Probe Existing Attachment

First check whether the active task already has a worktree attached (e.g. a previous interrupted run left one behind):

```bash
hotpot worktree path
```

If testing in this repository, run:

```bash
cargo run -- worktree path
```

- Non-empty stdout → a worktree is already attached. Announce the path to the user (`Worktree already attached: <path>`) and skip the question. Reuse this path for the rest of the run.
- Empty stdout → no worktree attached. Proceed to **Ask The User** below.
- Non-zero exit (e.g. no active task) → stop here and report; the existing "no active task" handling in Step 1 covers it.

### Ask The User

Ask the user **exactly once** at the start of the run:

> Use an isolated git worktree for this task? It creates `<base_dir>/<task-id>/` on a new `hotpot/<task-id>` branch, and `/hotpot:finish-work` will offer merge / keep-branch / discard options when you're done. (default: no)

Wait for an explicit yes / no. On any answer other than a clear yes, treat it as **no** and continue with the previous behavior (no worktree attached).

### Create On Yes

If the user says yes, run:

```bash
hotpot worktree create
```

If testing in this repository, run:

```bash
cargo run -- worktree create
```

Expect stdout to be a single JSON line containing at least `{"path": "...", "branch": "hotpot/<task-id>", "base_branch": "..."}`. Capture the `path` value — call it `<worktree-path>` below. Surface the JSON to the user so they can see the new worktree.

If the command fails (for example because the branch `hotpot/<task-id>` already exists, or `HEAD` is detached), report the error to the user and ask whether to retry, switch to a different task, or proceed without a worktree. Do not silently degrade.

### Apply For The Rest Of The Run

When a worktree path was captured (either from the probe or from create):

- All subsequent `git` invocations in this run (status, diff, blame, etc.) MUST be prefixed with `cd <worktree-path> && …`.
- The execution and review subagents MUST receive an explicit worktree directive (see the agent prompts below). The directive tells them to operate entirely inside `<worktree-path>` and to prefix every Bash command with `cd <worktree-path> && …`.
- The fix-loop reuses the same `<worktree-path>` — do not re-prompt the user inside the loop.
- `hotpot …` invocations stay untouched: those commands resolve `.hotpot/` against the main repo regardless of cwd.

When no worktree was captured, all behavior below is identical to the previous version of this prompt.

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

## Detect TDD Mode

After validating the task structure, scan the `## Plan > ### Mode` block for a line matching `- tdd: true` or `- tdd: false`:

- `tdd: true` → enable **TDD Execution Mode** for both the execution and review phases below.
- `tdd: false` → use **Default Execution Mode**.
- Missing `### Mode` block entirely → treat as `tdd: false` (backwards compatible with task files created before TDD mode existed).
- Any other value (`tdd:` followed by `1`, `yes`, `on`, an empty string, etc.) → STOP and ask the user to clarify which mode they want. Do NOT silently degrade.

Remember this choice; it controls which prompt variant you inject into the execution and review subagents and how the Final Response is formatted. The choice is loaded once at the start of the command and applies to every fix round in this run.

## Subagent Invocation Error Handling

This section governs **every** subagent invocation in this command — the execution agent, the review agent, and every fix-round execution agent. It exists because provider-side transient failures (concurrency limits, rate limits, 5xx, gateway timeouts) routinely kill an orchestrator turn mid-flight and require the user to manually nudge the session back to life. Apply these rules uniformly; do not invent ad-hoc retries elsewhere in the prompt.

### Transient Error Signature (case-insensitive substring match)

Treat a subagent invocation as **transient-failed** when its surfaced error contains any of:

- `Concurrency limit exceeded`
- `rate limit` / `Rate limit exceeded` / HTTP `429`
- `API Error 524` / `502` / `503` / `service unavailable` / `gateway timeout`
- `internal error` / `internal server error` (HTTP 5xx)
- `connection reset` / `read timeout` / `socket hang up` / `ECONNRESET` / `ETIMEDOUT`

Match is case-insensitive substring on the error text the platform surfaces (status line, error body, or thrown message).

### Retry Behavior

- Retry the **same** subagent with the **exact same prompt bytes** — do not regenerate the prompt, do not trim context, do not change the agent name. The prompt must be byte-identical to the failed attempt.
- Wait 5–10 seconds between attempts (orchestrator AI may pick any value in that window).
- Maximum 2 retries → up to 3 total attempts per subagent invocation.
- The orchestrator MUST keep an explicit attempt counter in its reasoning trace, e.g. `attempt 1/3`, `attempt 2/3`, `attempt 3/3`. Never enter a retry without naming the current attempt number; this is the only thing standing between transient-error recovery and an infinite retry loop.

### After All 3 Attempts Fail

- Surface the **raw provider error verbatim** to the user, no paraphrasing, no localization.
- Ask the user to re-run `/hotpot:execute` (the `## Resume After Transient Failure` section then drives the resume path on the next turn).
- Do **not** write any issue candidate to disk.
- Do **not** silently fall back to inline same-session execution — that bypasses the read-only review invariant and pollutes the orchestrator context with execution-agent-only material.

### Non-Transient Failure Modes (do NOT retry)

- `Unknown agent type: hotpot-execution` / `Unknown agent type: hotpot-review` → this is a platform subagent-registry problem, not a provider transient. Stop and follow `## Pre-flight: Verify Subagent Registration` → "How To React" (ask the user to run `hotpot update --platform <platform>` and restart the session). Retrying in the current session will keep hitting the cached miss.
- Subagent returns a final report containing a **blocker, mismatch, or refusal** (e.g. "stop and report the mismatch instead of guessing", "required file missing") → this is the normal blocker path, not a transient error. Surface the blocker to the user; do not retry.
- Any error not matching the transient signature list above → treat as a hard failure. Surface it to the user; do not retry.

The retry envelope only protects against provider-side transient failures. Logic / blocker / registry failures must surface immediately so the user can intervene.

## Execution Agent

Invoke the registered Hotpot execution agent according to the platform's mapping (see the thin shell's platform note). If the platform has no registered execution agent, fall through to a same-session execution phase using the embedded prompt below as the phase contract, then run a separate read-only review phase afterward.

### Default Execution Mode (when `tdd: false`)

Execution agent prompt:

````markdown
You are the Hotpot execution agent.

Task file path:
`<task-file-path>`

<worktree-directive-or-empty>

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

When a worktree is attached, replace `<worktree-directive-or-empty>` with the **Worktree Directive** below (verbatim, with `<worktree-path>` substituted). When no worktree is attached, drop the placeholder entirely.

#### Worktree Directive

```text
Worktree mode: this run operates inside an isolated git worktree.

- All filesystem reads, writes, and Bash commands MUST happen inside
  `<worktree-path>`. Prefix every Bash command with `cd <worktree-path> && …`.
- Do not edit files outside `<worktree-path>`.
- `hotpot …` invocations stay as-is — they resolve `.hotpot/` against the
  main repository automatically.
- The branch checked out in the worktree is `hotpot/<task-id>`; treat it
  as the working branch for all commits.
```

### TDD Execution Mode (when `tdd: true`)

Inject the TDD protocol at the top of the execution prompt. The execution agent MUST follow Red → Green → Refactor for every Implementation Task and capture the structured per-task evidence block defined in the protocol.

Execution agent prompt:

````markdown
You are the Hotpot execution agent running in TDD mode.

@.hotpot/prompts/tdd-protocol.md

Task file path:
`<task-file-path>`

<worktree-directive-or-empty>

Read and follow the full task file content below.

Execution rules (TDD mode):

- Treat the `## Task` section as the source of truth for scope, requirements, non-goals, user decisions, and approved design.
- For each `#### Task N` in `## Plan > ### Implementation Tasks`, complete `##### Red` → `##### Green` → `##### Refactor` in order. Do NOT tick any checkbox before its segment's evidence has been captured.
- Apply the rules in the TDD Protocol above verbatim: a valid Red must fail with an assertion/behavioral error tied to the new test (compile/dependency/environment failures do NOT count); a valid Green is the smallest change to pass; Refactor is either a concrete cleanup with re-run results or the literal phrase `no refactor needed`.
- Preserve all constraints and validation criteria from the task file. Do not expand scope.
- If repository reality differs from the plan, stop and report the mismatch instead of guessing.
- If a required test framework, test command, or implementation file is missing, stop and report the blocker. Do not silently degrade to non-TDD execution.
- Return a concise final report. The report MUST include, for each Implementation Task, the structured `Task <N>:` evidence block defined in the TDD Protocol. Without that block, the review agent will treat the task as non-conformant.

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

If a worktree was attached in Step 0, prefix every git invocation in this section with `cd <worktree-path> && …` so the diff reflects changes on the `hotpot/<task-id>` branch instead of the main checkout. Example:

```bash
cd <worktree-path> && git rev-parse --is-inside-work-tree
cd <worktree-path> && git status --porcelain
cd <worktree-path> && git diff
```

If the project is a git repo, collect changed files and diff with git.

### Diff Size Cap (review-prompt payload control)

Large diffs are a primary cause of provider transient errors (single-request body too large → 524 / gateway timeout). Bound the diff embedded in the review prompt with the following policy. The threshold is a deliberately explicit constant; treat it as the single knob.

- **`DIFF_CAP_BYTES = 40960`** (40 KB). This threshold balances "review agent can see the full hunk in one shot" against the provider single-request size window where 524s start appearing in practice. Do NOT introduce a separate per-mode threshold.

Procedure:

1. Run `git diff --stat` and keep the full output — it is always small (file-level summary) and always embedded verbatim.
2. Run `git diff` and measure its byte length (e.g. `git diff | wc -c`).
3. If `bytes ≤ DIFF_CAP_BYTES` → embed the full `git diff` output unchanged in the review prompt.
4. If `bytes > DIFF_CAP_BYTES` → switch to **per-file truncation**:
   - Embed the full `git diff --stat` output.
   - For each changed file, embed at most the first 8192 bytes of `git diff -- <file>`. Use **line-oriented truncation** (e.g. `git diff -- <file> | awk 'BEGIN{n=0} { if (n+length+1 > 8192) exit; print; n += length+1 }'`) rather than `head -c 8192` to avoid splitting a multi-byte UTF-8 character (Chinese paths, Chinese comments, emoji in commit messages, etc.) at a non-character boundary, which can produce invalid UTF-8 sequences that some provider request paths reject.
   - Append the literal marker `[... truncated; review agent should run `git diff -- <file>` for the full hunk ...]` immediately after any file whose hunk was cut.
   - The review prompt's diff section header MUST note that per-file truncation is active so the review agent knows to fetch full hunks via `git diff -- <file>` when needed.

Worktree caveat: when a worktree is attached, prefix every `git diff` / `git diff -- <file>` invocation in this procedure with `cd <worktree-path> && …`. The byte measurement and per-file truncation run inside the worktree.

The same cap applies to the diff embedded in the `## Automatic Fix Loop` fix prompt — fix-round diffs grow with each round, so they need the same protection.

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

### Issue Memory Cap (review-prompt payload control)

`hotpot issues relevant` already caps at `--limit 5`, but each row carries the full `Scene`, `Review Check`, and `Notes` fields, which can run to thousands of bytes per issue. Combined with a large diff, this is one of the primary contributors to the oversized review-prompt payload that triggers provider 524 / gateway-timeout errors.

Before embedding the `hotpot issues relevant` output into the review prompt, **strip every field except**:

- `issue_id`
- `Scene`
- `Review Check`

Drop `Notes`, timestamps, source provenance, and any other field the JSON output may carry. The reduction is purely on the orchestrator side at prompt-build time — do not modify `.hotpot/issues.jsonl` or the `hotpot issues relevant` CLI output schema.

If the review agent decides during review that a finding needs the full issue record (e.g. the stripped `Scene` is too terse, or `Notes` matters for the judgement), it can retrieve the full row with:

```bash
cat $ROOT_DIR/.hotpot/issues.jsonl | jq 'select(.issue_id == "<id>")'
```

The review prompt's "Relevant Hotpot issue memory" header MUST state explicitly that the memory has been reduced to `issue_id` + `Scene` + `Review Check`, so the review agent knows to use the `jq` command above when the reduced view is insufficient.

## Review Agent

Invoke the registered Hotpot review agent after execution. If the platform has no registered review agent, fall through to a same-session read-only review phase using the embedded prompt below as the phase contract.

The review agent must not modify files. It only reviews and reports findings.

### Default Review Mode (when `tdd: false`)

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

<worktree-directive-or-empty>

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

Git diff or fallback change context (may be per-file truncated when total diff exceeds 40 KB; see "Diff Size Cap" in the orchestrator's Collect Review Context section):

```diff
<git diff, possibly per-file truncated when the total diff size exceeds 40 KB; when truncated, each cut file ends with the literal marker `[... truncated; review agent should run `git diff -- <file>` for the full hunk ...]`; or an explicit message explaining that git diff is unavailable because the project is not a git repository or git failed>
```

If git diff is unavailable, inspect relevant files directly based on the task file, execution report, and reported changed files.

Relevant Hotpot issue memory (stripped to `issue_id`, `Scene`, `Review Check` only — fetch the full record on demand with `cat $ROOT_DIR/.hotpot/issues.jsonl | jq 'select(.issue_id == "<id>")'`):

```markdown
<output from hotpot issues relevant, reduced to issue_id + Scene + Review Check fields>
```

Review requirements:

- Check the diff against the full task file, especially `## Task`, `## Plan`, and `## Execution Instructions`.
- Check whether validation from the task file was actually run.
- Check whether the execution stayed inside scope and non-goals.
- For every relevant Hotpot issue memory, decide whether its `Scene` matches the current change. If it matches, perform its `Review Check`. If the stripped Scene/Review Check is too terse to judge, fetch the full record via the `jq` command above before deciding.
- If the diff section indicates per-file truncation, run `git diff -- <file>` for any file whose review-relevance demands the full hunk (e.g. semantic change spans more than 8 KB, or the cut marker hides the function body you need to read).
- Report only actionable findings. Do not nitpick unrelated style.
- Do not modify code or files.
````

### TDD Review Mode (when `tdd: true`)

Inject the TDD protocol at the top of the review prompt and pass the execution agent's structured `Task <N>:` blocks verbatim. The review agent performs the TDD Conformance Check in addition to the default review checks.

Review agent prompt:

````markdown
You are the Hotpot review agent running in TDD mode. Review only; do not modify files.

@.hotpot/prompts/tdd-protocol.md

Review mindset:

- Prioritize correctness bugs, behavioral regressions, missing validation, scope drift, and violations of the task requirements.
- Findings must be the primary output, ordered by severity.
- Include file and line references whenever possible.
- If there are no findings AND TDD conformance is verified, output `No findings.` followed by `TDD conformance: passed.`.
- Do not provide a large summary before findings.

Task file path:
`<task-file-path>`

<worktree-directive-or-empty>

Full task file:

```markdown
<entire task file content after execution>
```

Execution agent report (must include per-task `Task <N>:` evidence blocks):

```text
<execution report>
```

Changed files:

```text
<changed files>
```

Git diff or fallback change context (may be per-file truncated when total diff exceeds 40 KB; see "Diff Size Cap" in the orchestrator's Collect Review Context section):

```diff
<git diff, possibly per-file truncated when the total diff size exceeds 40 KB; when truncated, each cut file ends with the literal marker `[... truncated; review agent should run `git diff -- <file>` for the full hunk ...]`; or an explicit message explaining that git diff is unavailable because the project is not a git repository or git failed>
```

If git diff is unavailable, inspect relevant files directly based on the task file, execution report, and reported changed files.

Relevant Hotpot issue memory (stripped to `issue_id`, `Scene`, `Review Check` only — fetch the full record on demand with `cat $ROOT_DIR/.hotpot/issues.jsonl | jq 'select(.issue_id == "<id>")'`):

```markdown
<output from hotpot issues relevant, reduced to issue_id + Scene + Review Check fields>
```

Review requirements (TDD mode):

- All default review checks (scope, validation, issue-memory `Scene`/`Review Check` matching) still apply.
- If the diff section indicates per-file truncation, run `git diff -- <file>` for any file whose review-relevance demands the full hunk (TDD review especially needs to read the full test-vs-implementation diff for each Red/Green segment, so do not skip this fetch).
- If a Hotpot issue's stripped Scene/Review Check is too terse to judge, fetch the full record via `cat $ROOT_DIR/.hotpot/issues.jsonl | jq 'select(.issue_id == "<id>")'` before deciding.
- **TDD Conformance Check**: for every `#### Task N` listed in the task file's `## Plan > ### Implementation Tasks`, verify the execution report contains a structured `Task <N>:` evidence block with Red / Green / Refactor sub-fields.
  - Missing block, missing sub-field, or out-of-order segments → `High`-severity finding "TDD evidence missing for Task <N>".
  - Red's `failure` looks like a compile error, dependency error, command-not-found, or unrelated environment failure → `High`-severity finding "Invalid Red for Task <N> — failure is not an assertion-level signal".
  - Green's diff contains code unrelated to the failing test (new features, broad refactors, removed unrelated code) → `Medium`-severity finding "Green scope bloat for Task <N>".
  - Refactor block has neither a concrete action nor the literal phrase `no refactor needed` → `Medium`-severity finding "Refactor decision missing for Task <N>".
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

<worktree-directive-or-empty>

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

Current git diff or fallback change context (may be per-file truncated when total diff exceeds 40 KB; see "Diff Size Cap" in the orchestrator's Collect Review Context section):

```diff
<git diff, possibly per-file truncated when the total diff size exceeds 40 KB; when truncated, each cut file ends with the literal marker `[... truncated; review agent should run `git diff -- <file>` for the full hunk ...]`; or an explicit message explaining that git diff is unavailable because the project is not a git repository or git failed>
```

Fix rules:

- Modify code only to address the listed findings.
- Preserve all original task constraints and non-goals.
- Update task checkboxes only if they accurately reflect completed work.
- Run relevant validation after fixing.
- If the diff section indicates per-file truncation, run `git diff -- <file>` for any file whose fix demands seeing the full hunk before editing.
- Return changed files, validation results, remaining blockers, and any findings you could not fix.
````

After each fix agent returns, collect changed files, diff, relevant issue memory, and run the review agent again.

Maximum automatic fix-review loop: 2 fix rounds.

If findings remain after 2 fix rounds, stop and report them to the user as unresolved before human confirmation.

### TDD-aware Fix Behavior

When `tdd: true`, the fix prompt must also inject `@.hotpot/prompts/tdd-protocol.md` at the top and instruct the execution agent to address each finding through the same Red → Green → Refactor cycle:

- Fixing a "test not strict enough" finding → write a tightened test (Red, expect failure on the current implementation), then adjust the implementation (Green), then Refactor.
- Fixing an implementation bug → start with a regression test that reproduces the bug (Red), apply the minimal fix (Green), then Refactor.
- The fix execution report MUST include fresh `Task <N>:` blocks for every task it touched; review will re-verify TDD conformance on the updated diff.
- The 2-round hard limit still applies. Do not silently exceed it to "complete" the TDD cycle.

## Record Reusable Issue Candidates

After the review/fix loop converges (review reports `No findings`, or the 2-round limit is reached), explicitly decide which repairs are worth recording as reusable review memory candidates. **Do this even if review found no findings** — the original execution may have surfaced a project convention worth remembering.

Apply the rules in `@.hotpot/prompts/record-issue-candidate.md` verbatim. The prompt's "When To Record" and "When Not To Record" lists are the value gate; do not invent new criteria.

Skip recording (do not buffer a candidate) when the repair is:

- ordinary feature implementation,
- a one-off business requirement,
- an intermediate failed attempt,
- an unverified fix,
- a problem that cannot become an actionable future review check,
- simple formatting, renaming, or copy changes that don't reflect a project rule.

Buffer a candidate only when it matches a "When To Record" rule, for example:

- the user pointed out an AI mistake, omission, or incorrect assumption,
- the repair reveals a reusable problem pattern that may recur,
- the problem involves project conventions, data formats, architecture constraints, UI rules, testing rules, or review rules,
- the repair can become a concrete future `review_check`.

For each surviving repair, build one JSON object matching the `IssueCandidate` schema defined in `@.hotpot/prompts/record-issue-candidate.md`. **Hold the buffered candidates in memory only — do not write to `.hotpot/workspaces/<user>/issue-candidates.jsonl` yet.**

If no repair clears the gate, the buffer is empty; proceed to the next section and report `0 candidates proposed` to the user.

## Confirm Candidates With User

Before the final response, surface the buffered candidates to the user for a lightweight yes/no decision.

For each buffered candidate, show:

- one-line `reason`
- one-line `problem`
- one-line `fix`
- the `changed_files` list

Then ask the user explicitly:

- whether to record all, none, or a selected subset, and
- whether to edit any candidate's `reason`/`problem`/`fix` text before writing.

If the user approves none, drop the buffer and continue to the final response.

If the user approves some, pipe the approved JSONL into:

```bash
hotpot issues candidate add
```

If testing in this repository, run:

```bash
cargo run -- issues candidate add
```

Expect stdout `{"added":N}` where N is the number of candidates written. If the count is unexpected, surface the discrepancy in the final response.

**Constraint:** never write to `.hotpot/workspaces/<user>/issue-candidates.jsonl` (directly or via `hotpot issues candidate add`) before showing candidates to the user and receiving explicit approval. The value gate lives in the prompt and in this user confirmation step — bypassing it pollutes the per-user memory log.

## Resume After Transient Failure

This section runs **only when** the very first user message in the current turn is a short wake-up cue rather than a fresh `/hotpot:execute` invocation. Treat the following as wake-up cues (case-insensitive substring match, length ≤ 16 characters of meaningful text, leading/trailing whitespace ignored):

- `continue`, `resume`, `retry`, `proceed`, `go on`
- `继续`, `继续执行`, `重新执行`, `继续吧`, `接着`, `重试`
- A single repeat of the previous slash command (e.g. just `/hotpot:execute` with no new arguments) **only when** the immediately previous assistant turn ended with a provider transient error matching the `## Subagent Invocation Error Handling` signature list.

Additional gate: even when one of the cues above matches, you MUST confirm from the current session's visible context that the immediately previous assistant turn actually ended mid-flight with a transient-error signature. If you cannot confirm that from your own context (e.g. fresh shell, no recoverable transcript), treat the message as a fresh invocation and run from Step 0. Never enter the resume path on cue alone — false positives skip worktree decision and task-file structure validation.

If none of those apply, do **not** enter this resume path — run the command fresh from Step 0.

### Why This Section Exists

A provider transient error (concurrency limit, 429, 524, gateway timeout) can kill the orchestrator turn mid-flight, leaving partial state on disk: a worktree exists, the execution agent already ran (or partially ran), the review agent may have run, candidates may or may not be buffered. Restarting from Phase 0 would re-spawn subagents that already succeeded, double-write task-file checkboxes, double-fetch issue memory, and waste provider quota. This section locates the interruption point from on-disk evidence and resumes from the next unfinished action.

### Self-Probe Sequence

Run these probes **in order**, in the orchestrator session (not in a subagent), before deciding where to resume:

1. **Active task path** — `hotpot task active --path` (in this repo: `cargo run -q -- task active --path`). If empty, the previous run never reached `task create` → run the full command fresh.
2. **Task file checkbox state** — `Read` the task file from step 1 and scan `## Plan > ### Implementation Tasks` for `- [x]` vs `- [ ]` checkboxes. Ticked steps are work the execution agent already finished and should not be redone.
3. **Working-tree state** — `git status --porcelain` and `git diff --stat` (prefix with `cd <worktree-path> &&` when a worktree is attached). Empty status with no checkboxes ticked → execution agent never ran or was rolled back. Non-empty status → execution agent landed at least one edit.
4. **Candidate buffer state** — `hotpot issues candidate list` (in this repo: `cargo run -q -- issues candidate list`). Non-empty output means execution + review + candidate proposal all completed; only the user-confirmation step is pending.
5. **Last assistant transcript line** — re-read the immediately previous assistant turn (or the platform's transcript file when available) to see which phase its final line announced.

### Interruption Point → Next Action Map

Cross-reference the signals from the self-probe sequence and resume from the **next** unfinished action. Never replay a phase whose evidence is already on disk.

| Signals | Interruption Point | Resume From |
|---|---|---|
| no `task active --path`, or `--path` returns non-existent file | before Phase 0 | re-run the full command (the user message is treated as a fresh `/hotpot:execute`) |
| `task active` ok, `git status` empty, no checkboxes ticked | between Phase 0 and the execution agent's first edit | re-invoke the execution agent with the same prompt (do NOT recreate the worktree if one is already attached) |
| `git status` non-empty, some checkboxes ticked, candidate buffer empty, last announced phase was execution | execution agent died after partial edits | re-invoke the execution agent with the same prompt; it will read the existing checkboxes and continue |
| `git status` non-empty, last announced phase was "collect review context" | between execution-completion and review-agent invocation | jump to `## Collect Review Context` and then `## Review Agent` |
| `git status` non-empty, last announced phase was review or fix loop | during review or a fix round | resume the fix loop at the next unfinished round (preserve the current round counter; the 2-round hard cap is shared, not reset) |
| candidate buffer non-empty (`hotpot issues candidate list` returns rows) | between candidate proposal and user confirmation | jump to `## Confirm Candidates With User` and re-surface the buffered candidates to the user |

### Hard Constraints

- Do **not** restart from Phase 0 when on-disk evidence shows later phases completed.
- Do **not** re-spawn a subagent whose final report you can recover from the previous transcript. If the transcript is unavailable, prefer re-invoking the execution agent (idempotent for code already on disk) over re-invoking the review agent (which would produce a fresh review pass against the same diff and waste provider quota).
- Do **not** reset the fix-round counter. Fix rounds 1 and 2 are the same hard cap across resumes; an interrupted round 2 resumes as round 2, not round 1.
- Do **not** reset the worktree decision. If a worktree was attached before the interruption, every resumed step still operates inside `<worktree-path>`.

### Scope Note

True "no user message at all" auto-resume requires platform hook support (Claude `Stop`, OpenCode `session.*`, Codex has no reliable SessionEnd hook, Pi `session_shutdown`) and a separate orchestration layer outside this prompt. That work is explicitly **out of scope** for this section — this section only minimizes the cost of the one-word user nudge that today's platforms require.

## Final Response

Do not claim that the task is complete until both execution and review have run.

Respond with:

- Task file path.
- TDD mode: `enabled` or `disabled` (matches the task file's `### Mode` block). When `enabled`, append `, conformance: passed` or `, conformance: <N findings>` based on the final review result.
- Final status: ready for user confirmation, partially completed, blocked, failed, or unresolved review findings.
- Execution rounds, review rounds, and fix rounds.
- Changed files.
- Validation commands run and results.
- Review result: no findings, fixed findings, or unresolved findings.
- Issue candidates: number proposed, number written to `issue-candidates.jsonl`, number dropped per user decision.
- Remaining unchecked steps, blockers, risks, or follow-up work.

Keep the response concise and factual. End by asking the user to manually confirm the result when status is ready for user confirmation.
