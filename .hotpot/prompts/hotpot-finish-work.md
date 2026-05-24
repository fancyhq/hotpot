<!--
Hotpot shared workflow body: finish the active task, promote review memory, optionally commit.

Platform-specific thin shells (Claude / OpenCode / Codex / Pi) reference this file via
`@.hotpot/prompts/hotpot-finish-work.md` or `$HOTPOT_FINISH_WORK_PROMPT`. Do not paste
platform-specific syntax (slash-command name, subagent name, frontmatter) into this body.
Codex/Pi thin shells provide the env-var substitution table for `@.hotpot/prompts/*` tokens
that appear below.
-->

You are finishing the currently active Hotpot task. This command is manually triggered and must orchestrate user confirmation, review-memory promotion, an optional git commit, and the task-lifecycle update.

## Goal

Close the Hotpot loop for the active task:

1. Confirm with the user that the task is complete and validated.
2. Resolve any ambiguity in `overview.jsonl` (multiple `active=true` rows).
3. Read temporary review-memory candidates, summarize them with `@.hotpot/prompts/summarize-issue-candidates.md`, ask the user to approve the `{promoted, discarded, merged}` proposal, then promote and clear.
4. If the project is a git repository, offer to create a commit for the task; on user approval, build a Conventional-Commits-style message, run `git commit`, and capture the resulting SHA.
5. Mark the task as `Done` and (when a commit was created) backfill the `commit` hash.
6. If finish-work automatically created the task commit, automatically commit the task-done ledger diff with subject `chore: record task Done`.
7. Report a single concise summary to the user.

Do not silently write to `.hotpot/issues.jsonl`. Do not stage or commit task files without explicit user confirmation. Do not mark the task `Done` until the previous steps succeed or are explicitly skipped. The task-done ledger commit is the only exception to the second confirmation rule: when the user already approved finish-work's automatic task commit, that approval also covers the follow-up ledger-record commit described below.

## Output Language

Apply the project language preference to every natural-language output produced by this command — completion-confirmation questions, candidate summaries (`{promoted, discarded, merged}` proposal), commit-message preview prose shown to the user (the final commit message body itself follows the user's approval), and the final per-command report. Pass the same language directive into the prompts you embed for sub-skills like `summarize-issue-candidates`. Structural anchors and machine-readable tokens (CLI flags, JSON keys, `ACTIVE_CONFLICT:`, markdown section headings, kebab-case slugs, git SHAs, Conventional Commits type prefixes) MUST stay in English. Full rule:

@.hotpot/prompts/output-language.md

Codex / Pi (no `@path` expansion): the thin shell's substitution table maps this to `$ROOT_DIR/.hotpot/prompts/output-language.md`. `Read` that path before proceeding.

## Precondition: Task File Must Exist

The active task `.md` MUST already exist on disk before this command runs. If `Read` on the resolved active path returns "File not found", `/hotpot:new` for this title did not complete its write step. Stop and tell the user to re-run `/hotpot:new` for the task — do NOT silently recreate the file ad-hoc from `overview.jsonl`, the brainstorming and approved design would be lost.

## Command Usage

- The user-facing invocation pattern is supplied by the platform thin shell.
- In normal usage, run `hotpot ...` commands.

## Required Flow

1. Resolve the active task file path.
2. Detect multiple `active=true` rows and resolve with the user before any state change.
3. Read the task file. Confirm task completion and validation with the user.
4. Probe whether a worktree is attached (`hotpot worktree path`); remember the path. All git operations below are prefixed with `cd <worktree-path> && …` when attached.
5. Read temporary issue candidates via `hotpot issues candidate list`.
6. If candidates exist, summarize via `@.hotpot/prompts/summarize-issue-candidates.md`, present `{promoted, discarded, merged}`, and ask the user to approve.
7. On approval, promote via `hotpot issues promote` (stdin JSONL), then `hotpot issues candidate clear`.
8. Detect git availability. If available, offer to create a commit (inside the worktree when attached). On approval, stage task-related files, build the message, run `git commit`, and capture the resulting SHA.
9. If a worktree is attached, ask the user how to dispose of it: merge into the base branch / keep the branch / discard. Execute the choice (using `hotpot worktree remove`).
10. Mark the task `Done` via `hotpot task done [--commit <SHA>]`. The SHA captured in step 8 is always the worktree-branch SHA — keep it honest regardless of the disposal choice.
11. If step 8 produced a commit through finish-work's automatic commit path, automatically commit any task-done ledger diff created by step 10. If the user skipped the commit or chose `I already committed manually — please use my HEAD as the task commit`, skip this ledger commit.
12. Offer to switch to a remaining `In Progress` task and continue executing it inside the same session.
13. Report the final state.

## Probe Attached Worktree

Before touching git or the issue-candidate pipeline, learn whether the active task has an attached worktree:

```bash
hotpot worktree path
```

Run:

```bash
hotpot worktree path
```

- Non-empty stdout → record the value as `<worktree-path>` and the working branch as `hotpot/<task-id>`. Every git invocation in the rest of this command MUST be prefixed with `cd <worktree-path> && …` (concrete examples appear in the sections below).
- Empty stdout → no worktree attached. Behavior below is identical to the previous version of this prompt; skip the **Worktree Disposal** section entirely.

## Resolve the Active Task

Run:

```bash
hotpot task active --path
```

Run:

```bash
hotpot task active --path
```

If the command fails or returns no path, stop and tell the user there is no active task to finish.

## Multiple Active Tasks

Normally Hotpot enforces a single `active=true` row, but `hotpot task create --inactive` legitimately leaves the previous In-Progress active intact (the new row starts `active=false`), so `overview.jsonl` can hold more than one `active=true` row when a user has explicitly recorded parallel tasks. (Stale `active=true && status=Done|Cancelled` rows are silently cleaned by every `task create` call and should not appear here.) Detect any remaining multi-active state before doing anything destructive.

Run:

```bash
hotpot task list
```

Run:

```bash
hotpot task list
```

The output is tab-separated: `<task_id>\t<status>\t<title>\t<date>`. Filter mentally for `In Progress` rows. Cross-check with:

```bash
hotpot task active --count
```

If more than one task is currently active, do not silently pick one. List them to the user and ask which one to finish. Show the candidate `task_id` and `title` for each row, and offer these choices:

- `Finish task A and leave the others active` — only when the user genuinely is finishing one of several parallel tasks.
- `Finish task A and stop the others` — runs `hotpot task stop --all` first, then proceeds.
- `Abort finish-work` — stops the command so the user can clean up state manually.

Remember the chosen `task_id` for the `Mark the Task Done` step.

## Confirm Completion

Read the entire active task file. Show the user the task title and the validation commands or manual validation steps from the `## Plan` section. Ask explicitly whether the task has been implemented and validated, and tell the user the upcoming steps will:

1. Promote temporary review-memory candidates into long-term `issues.jsonl` if any.
2. Optionally create a git commit for the changes.
3. Mark the task as `Done` in `overview.jsonl`.

If the user says no, stop. Do not make any further changes.

## Review-Memory Candidate Flow

Read the temporary issue candidates:

```bash
hotpot issues candidate list
```

Run:

```bash
hotpot issues candidate list
```

The output is JSONL (one `IssueCandidate` per line). If the output is empty, skip directly to the git-commit step and tell the user there are no review-memory candidates to promote.

If candidates exist, apply the rules in `@.hotpot/prompts/summarize-issue-candidates.md` to the candidates together with:

- The final changed files (from `git diff --name-only` if git is available, otherwise from the candidates' own `changed_files`).
- Keywords extracted from the task title, the `## Task` section, and the changed files.
- The final task summary (one paragraph derived from the task file).
- The validation commands or checks from the `## Plan` section.
- Existing issues from `.hotpot/issues.jsonl` if any (`hotpot issues list` returns markdown; consult it to avoid promoting duplicates).

Produce a single JSON object with `promoted`, `discarded`, and `merged` arrays following `@.hotpot/prompts/get-issue.md`. Show the proposal to the user before writing anything:

- For each promoted entry: `<title> — <short rationale>`.
- For each merged entry: `<candidates> → <promoted_title>`.
- For each discarded entry: `<candidate> — <reason>`.

Ask whether to write the promoted issues to `.hotpot/issues.jsonl` and clear the candidates. Wait for explicit user approval before continuing. Never auto-promote.

After approval:

1. Serialize the `promoted` array into newline-delimited JSON (one `Issue` per line, no markdown fences).
2. Pipe that into `hotpot issues promote`:

   ```bash
   printf '<line1>\n<line2>\n' | hotpot issues promote
   ```

   Run:

   ```bash
   printf '<line1>\n<line2>\n' | hotpot issues promote
   ```

   The command prints `{"promoted":N}` on success. If it errors, stop and report the failure — do NOT proceed to clear, since the candidates are still the only record.

3. Only after promotion succeeds, run:

   ```bash
   hotpot issues candidate clear
   ```

   Run:

   ```bash
   hotpot issues candidate clear
   ```

   The command prints `{"cleared":N}`. If candidates failed to clear after a successful promote, still continue with the rest of finish-work — the task lifecycle is independent — but call this out clearly in the final report so the user can clean up manually.

## Optional Git Commit

Check whether the project is inside a git work tree:

```bash
git rev-parse --is-inside-work-tree
```

If a worktree is attached, run the check (and every subsequent git command in this section) prefixed with `cd <worktree-path> && …`, e.g.:

```bash
cd <worktree-path> && git rev-parse --is-inside-work-tree
cd <worktree-path> && git status --porcelain
cd <worktree-path> && git add -- <path1> <path2>
cd <worktree-path> && git commit -m "..."
cd <worktree-path> && git rev-parse HEAD
```

The commit will land on the `hotpot/<task-id>` branch (the working branch inside the worktree). Whether to merge that branch back into the base branch is decided **after** the commit, in the **Worktree Disposal** section.

If the project is not a git repository, skip this step and tell the user the commit step was skipped because git is unavailable.

If it is a git repository, ask the user whether to create a commit now. Offer these choices:

- `Yes, commit task-related files now` — recommended.
- `No, skip the commit` — proceed without recording a commit hash.
- `I already committed manually — please use my HEAD as the task commit` — capture `git rev-parse HEAD` (using the `cd <worktree-path> && …` prefix when attached) and pass it to the `Mark the Task Done` step.

### Build the Staging File List

If the user approves an automatic commit:

1. Read the `## Plan > File Map` from the task file to learn the intended changed files.
2. Run `git status --porcelain` to learn the actual changed files (modified, added, deleted, renamed, untracked).
3. Intersect the two: prefer files that appear in both. List files that appear in only one of them and ask the user how to handle each ambiguity (include in commit, leave out, or stop to sort it out manually).
4. Never run `git add -A`. Always stage files by explicit path:

   ```bash
   git add -- <path1> <path2> ...
   ```

5. After staging, run `git diff --cached --name-only` and show the final list to the user. If the staged set is empty (e.g. all changes were untracked and the user said "leave them out"), tell the user there is nothing to commit and skip ahead without a commit hash.

### Build the Commit Message

Pick the Conventional Commits kind by inspecting the task title and `## Task` content:

- `feat:` — new functionality (new commands, new UI, new feature surface).
- `fix:` — defect repair, regression fix.
- `refactor:` — internal restructuring without behavior change.
- `docs:` — documentation changes only.
- `test:` — test-only changes.
- `chore:` — tooling, dependency, or configuration changes.
- `perf:` — performance-only improvement.

Build the subject:

- Format: `<kind>: <short summary>`.
- The **entire subject line, including the prefix**, must be **≤ 50 characters**.
- Use imperative mood (`add`, `fix`, `support`), not past tense.
- Do not end with a period.

Build the body:

- One short paragraph or 2-3 short bullet points.
- Describe *what* changed and *why* in user-visible terms; do not paste diffs.
- Keep each line ≤ 72 characters.
- Optional trailing line referencing the task file path for traceability.

Show the proposed message to the user and ask them to confirm, edit, or cancel. Wait for explicit user approval. If the user wants edits, regenerate and re-confirm.

### Run the Commit

On user approval, commit using a HEREDOC to preserve newlines and avoid quoting issues:

```bash
git commit -m "$(cat <<'EOF'
<subject>

<body>
EOF
)"
```

If a pre-commit hook fails, surface the stderr verbatim to the user and let them decide whether to fix the hook issue and retry or skip the commit. Do not use `--no-verify`.

On success, capture the new HEAD SHA:

```bash
git rev-parse HEAD
```

Keep the full SHA. It will be passed to `hotpot task done` next.

## Worktree Disposal

Only run this section when **Probe Attached Worktree** captured a `<worktree-path>`. If nothing was attached, skip directly to **Mark the Task Done**.

The captured SHA (from the **Optional Git Commit** step) is always the SHA on `hotpot/<task-id>`. It will be passed to `hotpot task done --commit <SHA>` regardless of the disposal choice below — keep that SHA available.

Ask the user to choose one of three disposal options:

1. **Merge `hotpot/<task-id>` into `<base-branch>` and remove the worktree.** Use this when the work is ready to ship on the base branch right now.
2. **Keep the branch but remove the worktree.** Use this when the user wants to open a PR or push the branch elsewhere and merge later.
3. **Discard the branch and the worktree.** Use this when the work should not be preserved at all (the task is being closed without integrating its changes).

`<base-branch>` is the branch the worktree was forked from. Resolve it from the JSON returned by `hotpot worktree create` (its `base_branch` field), or by inspecting `hotpot worktree list --json` for the same task id.

Execute the chosen disposal. Show the user every command before running it; on any failure surface the stderr verbatim and stop.

### Option 1 — Merge And Remove

```bash
# Switch to the base branch in the main repository (NOT the worktree).
git checkout <base-branch>

# Merge the worktree branch with a merge commit to preserve history.
git merge --no-ff hotpot/<task-id>

# Remove the worktree directory and delete the branch.
hotpot worktree remove
```

If `git merge` reports conflicts, stop and let the user resolve them; do not run `hotpot worktree remove` until the merge is committed.

### Option 2 — Keep Branch, Remove Worktree

```bash
hotpot worktree remove --keep-branch
```

The `hotpot/<task-id>` branch remains in the main repository; the user can push or PR it later.

### Option 3 — Discard Both

```bash
hotpot worktree remove --force
```

`--force` makes the removal succeed even if the worktree has uncommitted changes; pair it with the branch deletion (the default behavior of `worktree remove` without `--keep-branch`) so nothing of the discarded work is left behind.

### After Disposal

After `hotpot worktree remove` returns, the task row no longer carries `worktree_path`. Subsequent operations (including `hotpot task done` below) proceed in the main repository as usual. Continue with **Mark the Task Done**.

## Mark the Task Done

Run `hotpot task done`. Pass `--task-id` only when the Multiple-Active step detected ambiguity; pass `--commit` only when a commit was created or the user supplied one in the git step.

Common forms:

```bash
hotpot task done                                 # single active, no commit
hotpot task done --commit <SHA>                  # single active, with commit
hotpot task done --task-id <ID>                  # ambiguous active, no commit
hotpot task done --task-id <ID> --commit <SHA>   # ambiguous active, with commit
```

The command prints the updated `TaskInfo` row as JSON. Verify the JSON shows `"status":"Done"` and `"active":false`; if `--commit` was supplied, verify the commit hash matches what `git rev-parse HEAD` reported.

If `hotpot task done` errors (e.g. cancelled task, missing task id, commit mismatch), surface the error to the user and stop. Do not retry blindly.

## Auto-Commit Task-Done Ledger Diff

Run this section only when all of these are true:

1. The user chose `Yes, commit task-related files now` in **Optional Git Commit**.
2. The automatic task commit succeeded and produced the `<SHA>` passed to `hotpot task done --commit <SHA>`.
3. `hotpot task done` succeeded and verified `"status":"Done"` and `"active":false`.

Skip this section when the user skipped the task commit, chose `I already committed manually — please use my HEAD as the task commit`, or when no commit SHA was passed to `hotpot task done`.

The ledger-record commit runs in the main repository after **Worktree Disposal** has completed. If a worktree was attached, never run these git commands inside `<worktree-path>` after `hotpot worktree remove`; that directory may no longer exist, and the Hotpot workspace ledger belongs to the main repository.

Derive the workspace ledger path from the active task path returned earlier (`.hotpot/workspaces/<username>/tasks/<file>.md` → `.hotpot/workspaces/<username>/overview.jsonl`) or from the `TaskInfo` JSON emitted by `hotpot task done` if it exposes enough path context. Treat the derived path as `<overview-jsonl-path>`.

Check whether task-done wrote an uncommitted ledger diff:

```bash
git status --porcelain -- <overview-jsonl-path>
```

If stdout is empty, skip cleanly: the task-done ledger change may already be included in an earlier user-approved commit, or no tracked diff remains.

If stdout is non-empty, stage only that ledger file and commit it with the fixed subject:

```bash
git add -- <overview-jsonl-path>
git commit -m "chore: record task Done"
git rev-parse HEAD
```

Never stage any other file in this section. Never use `git add -A`, `git commit -a`, or a broad path such as `.hotpot/`. Capture the resulting ledger-record commit SHA for the final response.

If this commit fails, do not roll back `hotpot task done` or the task commit. Preserve the stderr for the user, continue to the final response when possible, and list the ledger-record commit failure as a leftover blocker.

## Offer to Resume Next Task

After `hotpot task done` (or `hotpot task cancel`) succeeds and before the final response, check whether the workspace has any tasks that are still open, and offer to continue with one of them inside this session.

1. List remaining tasks via `hotpot task list`. Output shape is one row per task: `<task_id>\t<status>\t<title>\t<date>`.
2. Filter rows where the status column equals `In Progress`. The row you just finished now has status `Done` or `Cancelled`, so the filter will exclude it naturally; verify by id to be safe.
3. If the filtered list is empty, skip this entire step and go straight to the final response.
4. Otherwise, present the list to the user as a short numbered choice. For each candidate show its `task_id`, `title`, and `date`. End with an explicit "or type `n` to skip" option.
5. Wait for the user's selection. **Never silently pick the first row.**
6. If the user picks `n` (or any "skip"), proceed to the final response without resuming.
7. If the user picks a row, run:

   ```bash
   hotpot task resume --task-id <ID>
   ```

   Run:

   ```bash
   hotpot task resume --task-id <ID>
   ```

   The command prints the updated `TaskInfo` row as JSON. Verify the JSON shows `"active":true` and `"status":"In Progress"`. If the command errors (e.g. `Done`/`Cancelled` rejection, missing id), surface the error verbatim to the user and stop without invoking the execution agent.
8. After resume succeeds, fetch the new active task's file path:

   ```bash
   hotpot task active --path
   ```

   Read the entire task file content.
9. Invoke the Hotpot execution agent with the full task file content inlined, reusing the same prompt shape as the execute flow's "Execution Agent" section. The thin shell maps this invocation to the platform's registered execution agent; on platforms with no registered execution agent, fall through to a same-session execution phase using the embedded execution prompt from `@.hotpot/prompts/hotpot-execute.md`.

   **Do not run the review phase, the fix loop, or the candidate-recording step here.** Those still belong to the execute flow; the user can trigger them next.
10. After the execution agent returns, fold its report into the final response: resumed task title, changed files reported by the execution agent, validation result, and any blockers. Recommend the user run the execute flow next if they want the review / fix loop on the resumed task.

## Final Response

Respond with one concise summary block:

- Task title and final status (`Done`).
- Task id, task commit hash if any, and ledger-record commit hash if any.
- Promoted count, discarded count, merged count from the candidate flow.
- Whether candidates were cleared.
- Whether a task git commit was created, and the subject line.
- Whether a ledger-record commit was created or skipped, including `chore: record task Done` and its SHA when present.
- Resumed task (if any) plus its execution outcome.
- Any leftover blockers (e.g. pre-commit hook failure, candidates failed to clear).

Keep the response factual. Do not restate the entire `## Task` section.

## Constraints

- Never write to `.hotpot/issues.jsonl` without showing the proposal and getting explicit user approval.
- Never call `hotpot issues candidate clear` until promote succeeds, or the user explicitly chooses to discard all candidates.
- Never run `git add -A` or `git commit -a`.
- Never use `git commit --no-verify` to bypass pre-commit hooks.
- Never call `hotpot task done` until the user has confirmed task completion.
- Never silently pick one row when multiple `active=true` rows exist.
- The commit subject line must be ≤ 50 characters total, including the Conventional Commits prefix.
- Do not include full diffs in promoted issues; rely on `source.summary` instead.
- Do not modify the task file content during finish-work; checkbox updates belong to the execute flow.
- Never silently resume a task without showing the candidate list and getting an explicit user choice.
- Never run the review phase, the fix loop, or the candidate-recording step inside the resume bridge; those remain the execute flow's job.
- Never silently pick a worktree disposal option. The user must choose merge / keep-branch / discard explicitly before `hotpot worktree remove` runs.
- The SHA passed to `hotpot task done --commit <SHA>` is always the commit made on `hotpot/<task-id>`, even when the user chose "merge to base". Recording the worktree-branch SHA keeps the task ledger honest about what work was actually done.
- When finish-work automatically creates the task commit, that confirmation also authorizes the follow-up task-done ledger commit; do not ask for a second confirmation before committing `overview.jsonl` with `chore: record task Done`.
- The automatic ledger-record commit may stage only the derived Hotpot workspace `overview.jsonl` path. It must not stage issue promotion, candidate cleanup, task work files, or unrelated user changes.
- The ledger-record commit SHA is separate from the task commit SHA and must never replace the SHA passed to `hotpot task done --commit <SHA>`.
