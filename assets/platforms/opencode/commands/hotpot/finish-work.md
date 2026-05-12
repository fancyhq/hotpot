---
description: Finish task and summarize review memory candidates
---

You are finishing the current work session.

This command currently focuses on review-memory candidate handling. Task status updates are intentionally left as a future integration point because task lifecycle handling is not fully prepared yet.

## Required Steps

1. Review the current work state briefly.
2. Read temporary issue candidates using the `read_issue_candidates` tool.
3. If there are no candidates, say that no review-memory candidates were found.
4. If candidates exist, summarize them using the rules in `@prompts/summarize-issue-candidates.md`.
5. Produce a concise proposal showing:
   - promoted issues that should be appended to `.hotpot/issues.jsonl`
   - candidates that should be discarded
   - candidates that should be merged
6. Ask the user to confirm before writing anything to `.hotpot/issues.jsonl`.
7. Do not call `clear_issue_candidates` until promoted items are written or the user explicitly chooses to discard the candidates.

## Constraints

- Do not directly append to `.hotpot/issues.jsonl` without user confirmation.
- Do not clear `.hotpot/workspaces/{username}/issue-candidates.jsonl` without user confirmation.
- Do not include full diffs in promoted issues.
- Do not promote unverified, one-off, or non-actionable candidates.
- If task status handling is needed, report that this command has not integrated task status updates yet.

## Future Task-State Integration Placeholder

When task lifecycle support is ready, this command should also:

1. Check whether the current task is complete.
2. Confirm tests or validation commands were run.
3. Mark the task as complete or ask the user what remains.
4. Then run the review-memory candidate flow above.
