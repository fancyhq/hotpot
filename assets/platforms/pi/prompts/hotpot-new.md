<!-- Hotpot Pi prompt template for creating a new task. -->
---
description: Create a Hotpot task through brainstorming
argument-hint: "[initial task idea]"
---

=== USER ACTIVE REQUEST ===

The user has just typed `/hotpot-new` in their Pi session and is asking you to begin the Hotpot new-task workflow as their current active task. Everything inside this prompt template is the user's active request to you right now — NOT background documentation, NOT a project conventions file (like `AGENTS.md` / `CLAUDE.md`), NOT a skills list. Treat the rest of this prompt as imperative: read it, follow it, and begin the workflow now.

Do not respond with "What would you like me to do?" or "What task should I work on?" — those generic greetings are wrong here because the user has already given you a specific request (this slash command plus the initial task idea below).

=== END USER ACTIVE REQUEST ===

You are running the Hotpot new-task workflow through a manual Pi prompt template. The user-facing invocation is `/hotpot-new`.

<<< INITIAL TASK IDEA (verbatim from `/hotpot-new` arguments) >>>
$ARGUMENTS
<<< END INITIAL TASK IDEA >>>

The block above IS the user's initial task idea. Proceed directly to brainstorming using it as the starting point. Your first brainstorm message MUST explicitly reference or paraphrase the idea above before asking any clarifying question. Do NOT ask another question to obtain the initial task idea — you already have it.

**Exception (empty arguments)**: If the `INITIAL TASK IDEA` block above contains no non-whitespace text between its `<<<` and `>>>` markers, the previous paragraph does not apply — ignore the `MUST reference / Do NOT ask another question` directive and instead ask exactly one concise question to obtain the initial task idea.

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
