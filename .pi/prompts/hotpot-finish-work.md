<!-- Hotpot Pi prompt template for finishing the active task. -->
---
description: Finish the active Hotpot task, promote review memory, and optionally commit
argument-hint: "[finish notes]"
---

You are running the Hotpot finish-work workflow through a manual Pi prompt template. The user-facing invocation is `/hotpot-finish-work`.

The full workflow is defined at `$HOTPOT_FINISH_WORK_PROMPT` (the Hotpot Pi extension exports this env var via `pi.on("context", ...)` and prepends an `export` line to every Bash tool call). Read that file first and follow the workflow end-to-end.

Pi has no `@path` expansion. When the shared body references `@.hotpot/prompts/<name>.md`, substitute the matching env var and use `Read`:

- `@.hotpot/prompts/output-language.md` → resolve as `$ROOT_DIR/.hotpot/prompts/output-language.md` and use `Read`
- `@.hotpot/prompts/summarize-issue-candidates.md` → `$HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`
- `@.hotpot/prompts/get-issue.md` → resolve as `$ROOT_DIR/.hotpot/prompts/get-issue.md` and use `Read`
- `@.hotpot/prompts/hotpot-execute.md` → `$HOTPOT_EXECUTE_PROMPT`

Platform note: Pi has no dedicated execution subagent. When the shared body's "Offer to Resume Next Task" step needs to invoke the Hotpot execution agent, continue the work in this same session by following the execution-phase rules in `$HOTPOT_EXECUTE_PROMPT`, omitting the review phase, fix loop, and candidate-recording step. Tell the user explicitly that if they want the review and fix loop on the resumed task, they need to run `/hotpot-execute` manually next.

## Output Language

The Hotpot Pi extension's `pi.on("context", …)` handler pushes `HOTPOT_LANGUAGE` and a one-line directive into every provider request, so the value is already in your system context for every turn. Reply in that language for the candidate summary, promotion prompts, commit confirmation prose, and the final report. Structural anchors stay English regardless: `## Task`, `## Plan`, `### Mode`, `tdd: true|false`, kebab-case slugs, `ACTIVE_CONFLICT:`. See `$ROOT_DIR/.hotpot/prompts/output-language.md` for the full anchor whitelist.
