<!-- Hotpot Pi prompt template for creating a new task. -->
---
description: Create a Hotpot task through brainstorming
argument-hint: "[initial task idea]"
---

You are running the Hotpot new-task workflow through a manual Pi prompt template. The user-facing invocation is `/hotpot-new`.

Initial task idea from command arguments: $ARGUMENTS

If the command arguments value above is non-empty, treat it as the user's initial task idea for the shared workflow. If it is empty, follow the shared workflow and ask one concise question for the initial task idea.

The full workflow is defined at `$HOTPOT_NEW_PROMPT` (the Hotpot Pi extension exports this env var via `pi.on("context", ...)` and prepends an `export` line to every Bash tool call). Read that file first and follow the workflow end-to-end.

Pi has no `@path` expansion. When the shared body references `@.hotpot/prompts/<name>.md`, substitute the matching env var and use `Read`:

- `@.hotpot/prompts/output-language.md` → resolve as `$ROOT_DIR/.hotpot/prompts/output-language.md` and use `Read`
- `@.hotpot/prompts/tdd-protocol.md` → `$HOTPOT_TDD_PROTOCOL_PROMPT`
- `@.hotpot/prompts/record-issue-candidate.md` → `$HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`
- `@.hotpot/prompts/summarize-issue-candidates.md` → `$HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`
- `@.hotpot/prompts/get-issue.md` → resolve as `$ROOT_DIR/.hotpot/prompts/get-issue.md` and use `Read`
- `@.hotpot/prompts/hotpot-execute.md` → `$HOTPOT_EXECUTE_PROMPT`
- `@.hotpot/prompts/hotpot-finish-work.md` → `$HOTPOT_FINISH_WORK_PROMPT`

Platform note: Pi has no dedicated subagents. `new` itself does not need one. If a workflow step ever references "the registered Hotpot execution agent" or "the registered Hotpot review agent", run the corresponding phase in the same session, strictly separated, and announce each phase at its start (`I am now in the EXECUTION phase` / `I am now in the READ-ONLY REVIEW phase`). The review phase must never use write/edit tools.

## Output Language

The Hotpot Pi extension's `pi.on("context", …)` handler pushes `HOTPOT_LANGUAGE` and a one-line directive into every provider request, so the value is already in your system context for every turn. Reply in that language for the brainstorming session, the task `.md` body, and the final summary. Structural anchors stay English regardless: `## Task`, `## Plan`, `### Mode`, `tdd: true|false`, the kebab-case `<title>` slug passed to `hotpot task create --title`, `ACTIVE_CONFLICT:`. See `$ROOT_DIR/.hotpot/prompts/output-language.md` for the full anchor whitelist.
