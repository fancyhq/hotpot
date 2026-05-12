# Hotpot Issue Memory Handoff

Date: 2026-05-11

This document records the current state of the Hotpot issue-memory work so a future session can continue without relying on chat history.

## Goal

Hotpot needs a lightweight review-memory system that helps AI avoid repeating historical mistakes.

The intended flow is:

1. Long-term reusable memories live in `.hotpot/issues.jsonl`.
2. Temporary repair candidates live in `.hotpot/workspaces/{username}/issue-candidates.jsonl`.
3. During normal work, AI may record validated reusable repair candidates.
4. At the end of work, `/finish-work` summarizes candidates, asks the user to confirm, then promotes selected records to `.hotpot/issues.jsonl`.
5. Future review or edit sessions filter long-term issues by changed paths and generated keywords, then render only relevant memories to markdown for AI review.

## Completed Work

### Issue Schema And Markdown

`src/issues.rs` now defines long-term issue memory with these concepts:

- `IssueKind`: `bug` or `optimization`.
- `IssueSource`: factual source metadata with `changed_files` and a short `summary`.
- `Issue`: long-term review memory with `date`, `title`, `kind`, `tags`, `paths`, `scene`, `description`, `review_check`, `solution`, and `source`.
- `ChangeContext`: filtering input with `changed_files` and generated `keywords`.

The markdown rendering was adjusted from a generic issue dump into review-memory language:

- Header: `Hotpot Review Memory` style guidance.
- Each issue renders title, kind, date, scene, problem, review check, and solution.
- `indoc!` is used inside `format!`; this works because `indoc!` expands to a string literal.

### Filtering

Filtering no longer depends on full diffs.

Current filtering design:

- Match issue `paths` against changed files.
- Match issue `tags` against generated keywords.
- Rank by simple explainable score.
- Render only top relevant issues to markdown.

Important reason: full diffs can be too large, noisy, privacy-sensitive, and can pollute model context. Use changed files plus compact keywords or summaries instead.

### Candidate Storage

`src/paths.rs` now includes:

- `issue_candidates_file_path(root_dir, username)`

It returns:

```text
.hotpot/workspaces/{username}/issue-candidates.jsonl
```

`src/issues.rs` now also defines `IssueCandidate` with:

- `created_at`
- `reason`
- `changed_files`
- `keywords`
- `problem`
- `fix`
- `validation`
- `promote_hint`

Candidate helpers added:

- `get_issue_candidates_list(root_dir, username)`
- `append_issue_candidate(root_dir, username, candidate)`
- `clear_issue_candidates(root_dir, username)`

Candidates are temporary. They should not be treated as confirmed long-term memory until `/finish-work` summarizes them and the user confirms promotion.

### Prompts

The following prompt files exist:

- `prompts/get-issue.md`
- `prompts/record-issue-candidate.md`
- `prompts/summarize-issue-candidates.md`

`prompts/get-issue.md` defines the official long-term `.hotpot/issues.jsonl` record format.

`prompts/record-issue-candidate.md` defines when AI should or should not record a temporary repair candidate.

Record candidates only when the fix is reusable, validated, and can become a future review check. Do not record ordinary implementation steps, one-off requirements, unverified fixes, intermediate attempts, or full diffs.

`prompts/summarize-issue-candidates.md` defines finish-stage merging and promotion rules. It outputs one JSON object with:

- `promoted`
- `discarded`
- `merged`

### OpenCode Plugin Tools

Review-memory plugin files are now installed from platform templates:

- `assets/platforms/opencode/plugins/review-memory.ts`
- installed target: `.opencode/plugins/review-memory.ts`

The plugin exposes three tools:

- `record_issue_candidate`
- `read_issue_candidates`
- `clear_issue_candidates`

The plugin also exposes environment variables:

- `HOTPOT_ISSUE_CANDIDATES_FILE`
- `HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`
- `HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`

Important limitation: the plugin does not automatically inject or read prompt contents. The AI or command flow must decide to read/use the prompt files. The tools only persist, read, and clear candidate data.

### OpenCode Finish Command

The first version of `/hotpot:finish-work` is installed from platform templates:

- `assets/platforms/opencode/commands/hotpot/finish-work.md`
- installed target: `.opencode/commands/hotpot/finish-work.md`

Current command behavior:

- Review current work state briefly.
- Call `read_issue_candidates`.
- If no candidates exist, report that none were found.
- If candidates exist, summarize them with `@prompts/summarize-issue-candidates.md`.
- Propose promoted, discarded, and merged candidates.
- Ask user confirmation before appending anything to `.hotpot/issues.jsonl`.
- Do not call `clear_issue_candidates` until promoted records are written or user explicitly discards candidates.

Current command constraints:

- No direct append to `.hotpot/issues.jsonl` without confirmation.
- No clearing temp candidates without confirmation.
- No full diffs in promoted issues.
- Do not promote unverified, one-off, or non-actionable candidates.
- If task status handling is needed, report that it has not been integrated yet.

## Validation Performed

The targeted issues tests pass:

```text
cargo test issues::tests -- --nocapture
```

Result:

```text
3 passed; 0 failed
```

Known warnings from the last run:

- `render_issue_refs_to_markdown` is currently unused.
- `render_relevant_issues_to_markdown` is currently unused.
- `has_multi_active_task` is currently unused.

Earlier `cargo check` passed with dead-code warnings. `cargo fmt --check` previously failed because of an existing formatting diff around `src/command.rs`; later `src/command.rs` was observed to be empty, but `cargo fmt --check` was not rerun after that observation.

## Current Important Files

- `src/issues.rs`: issue schema, candidate schema, JSONL helpers, markdown rendering, filtering, tests.
- `src/paths.rs`: issue and issue-candidate file path helpers.
- `.hotpot/issues.jsonl`: upgraded long-term issue records used by tests.
- `prompts/get-issue.md`: official long-term issue JSON prompt.
- `prompts/record-issue-candidate.md`: temporary candidate recording prompt.
- `prompts/summarize-issue-candidates.md`: finish-stage promotion prompt.
- `assets/platforms/opencode/plugins/review-memory.ts`: OpenCode review-memory plugin template.
- `.opencode/plugins/review-memory.ts`: installed OpenCode tools.
- `assets/platforms/opencode/commands/hotpot/finish-work.md`: OpenCode finish command template.
- `.opencode/commands/hotpot/finish-work.md`: installed finish command.

## Design Decisions

### JSONL Is Source Of Truth

Use `.hotpot/issues.jsonl` as structured long-term storage. Markdown should be generated only when needed for AI review.

### Temporary Candidates Before Promotion

AI should not write directly to `.hotpot/issues.jsonl` during every fix. It should write temporary candidates only when the repair reveals a reusable future review check. `/finish-work` handles deduplication, merging, and user confirmation.

### Manual Or Command-Orchestrated Promotion

Do not rely only on natural-language detection like “修改”, “优化”, or “修复”. Explicit command flow is more reliable. Automatic detection can remind or record candidates, but long-term promotion should be confirmed.

### No Full Diff Storage

Do not save full diffs in candidates or long-term issues. Store changed files, keywords, and short summaries instead.

## Next Work

### 1. Integrate Task Status Into `/finish-work`

`/finish-work` currently has only a placeholder for task lifecycle.

Next implementation should decide how Hotpot represents task state and then update the command to:

1. Check whether the current task is complete.
2. Confirm relevant validation commands were run.
3. Mark the task complete or ask what remains.
4. Run review-memory candidate flow after task status is handled.

Useful existing clue: `src/task.rs` has an unused `has_multi_active_task(root_dir, username)` function. Inspect `src/task.rs` before designing task-state integration.

### 2. Wire Relevant Issue Rendering Into Review/Edit Flow

Filtering and rendering helpers exist but are not used yet.

Need to decide where AI review context is assembled, then call something equivalent to:

```rust
render_relevant_issues_to_markdown(root_dir, context, limit)
```

The caller should provide `ChangeContext` with changed files and generated keywords, not a large diff.

### 3. Decide Whether Candidate Recording Should Be Hook-Assisted

Current plugin exposes `record_issue_candidate`, but there is no automatic event reminder or prompt injection.

Potential next options:

1. Keep tool manual and rely on AI instruction.
2. Add a lightweight command for recording a candidate.
3. Add event-based reminders after edits or test fixes, without forcing writes.

Avoid fully automatic long-term promotion.

### 4. Add Append-To-Issues Promotion Helper

There is candidate append and clear support, but promotion from summarized JSON into `.hotpot/issues.jsonl` still needs a safe path.

Potential helper:

```rust
append_issue(root_dir, issue)
```

It should schema-validate and write one compact JSON object per line.

### 5. Revisit Formatting And Dead Code

Run:

```text
cargo fmt --check
cargo check
cargo test issues::tests -- --nocapture
```

If `cargo fmt --check` still fails, inspect whether the diff is unrelated before changing anything. Do not revert unrelated user changes.

## Cautions For Future Sessions

- The repository may have unrelated dirty worktree changes. Do not revert them unless explicitly asked.
- Keep issue-memory records concise and reusable.
- Do not promote candidates without validation and user confirmation.
- Do not add backward-compatibility code unless there is a concrete persisted-data or external-consumer need.
- Prefer minimal changes that wire the existing helpers into the next flow.
