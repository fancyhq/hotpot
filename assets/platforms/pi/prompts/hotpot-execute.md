<!-- Hotpot Pi prompt template for executing the active task. -->
---
description: Execute and review the active Hotpot task
argument-hint: "[execution notes]"
---

You are running the Hotpot execute workflow through a manual Pi prompt template. The user-facing invocation is `/hotpot-execute`.

The full workflow is defined at `$HOTPOT_EXECUTE_PROMPT` (the Hotpot Pi extension exports this env var via `pi.on("context", ...)` and prepends an `export` line to every Bash tool call). Read that file first and follow the workflow end-to-end.

Pi has no `@path` expansion. When the shared body references `@.hotpot/prompts/<name>.md`, substitute the matching env var and use `Read`:

- `@.hotpot/prompts/output-language.md` → resolve as `$ROOT_DIR/.hotpot/prompts/output-language.md` and use `Read`
- `@.hotpot/prompts/tdd-protocol.md` → `$HOTPOT_TDD_PROTOCOL_PROMPT`
- `@.hotpot/prompts/record-issue-candidate.md` → `$HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`
- `@.hotpot/prompts/summarize-issue-candidates.md` → `$HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`
- `@.hotpot/prompts/get-issue.md` → resolve as `$ROOT_DIR/.hotpot/prompts/get-issue.md` and use `Read`

Platform note: Pi has no dedicated subagents. When the shared body refers to "the registered Hotpot execution agent" or "the registered Hotpot review agent", run the execution and review phases in the same session, strictly separated. Announce the current phase at its start (`I am now in the EXECUTION phase` / `I am now in the READ-ONLY REVIEW phase`) so context does not bleed across phases. The review phase must never use write/edit tools, even in this fallback. This is unchanged in TDD mode.

## Pi Same-Session Review Memory Injection

The registered review subagent on Claude / OpenCode / Codex receives issue memory implicitly through the orchestrator's "Collect Review Context" step. Pi has no subagent boundary, so the same injection MUST be done inline by you before crossing into the read-only review phase. Skipping it makes Pi reviews silently weaker than the other three platforms.

Between the end of the EXECUTION phase and the start of the READ-ONLY REVIEW phase, perform these three steps in order:

1. **Collect the changed file list.** Take it from the execution phase's final report when available. If the project is a git repo, also run `git status --porcelain` (prefixed with `cd <worktree-path> && …` when a worktree is attached) to confirm or augment the list. If git is unavailable, use the execution report verbatim and tell the review phase that git diff is unavailable.

2. **Fetch relevant issue memory.** Build keywords from the task title, changed modules, commands, components, or APIs touched by the change, and run:

   ```bash
   hotpot issues relevant \
     --changed-file <path> \
     --keyword <keyword> \
     --limit 5
   ```

   When testing inside this repository, use `cargo run -- issues relevant …` instead. Capture the rendered markdown verbatim — this is what the registered review subagent would receive in its prompt on the other platforms.

3. **Apply each memory item during review.** Inside the read-only review phase, for every memory entry returned in step 2, decide whether its `Scene` matches the current change; if it matches, perform its `Review Check` and surface findings exactly the way the canonical `hotpot-review` agent does. Do not skip this step just because there is no subagent handoff.

The review phase stays read-only — even when memory injection happens inline in the same session, never use write/edit tools. This is unchanged in TDD mode (the TDD Conformance Check is layered on top of these steps, not in place of them).
