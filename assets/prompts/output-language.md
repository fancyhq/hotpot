<!--
Hotpot shared output-language directive.

Referenced by every main workflow prompt (`hotpot-new.md`, `hotpot-execute.md`,
`hotpot-finish-work.md`, `tdd-protocol.md`). Keep this file as the single
source of truth for "what language does the agent speak to the user" so the
behavior cannot drift across workflows.

Claude / OpenCode pull this file via `@.hotpot/prompts/output-language.md`
(recursive `@path` expansion). Codex / Pi have no `@path` expansion — each
of their thin shells lists the substitution
`@.hotpot/prompts/output-language.md → $ROOT_DIR/.hotpot/prompts/output-language.md`,
so the agent there must `Read` the resolved path manually.
-->

# Output Language

Before producing any natural-language output for the user, obey the project language preference. The configured value is pushed into your context on every model turn by Hotpot's platform hooks — you do not need to read `config.toml` yourself.

## How The Language Reaches You

Hotpot resolves `<project>/.hotpot/config.toml::language` (or the `HOTPOT_LANGUAGE` env override) in Rust on every hook invocation and re-injects it into your context **each turn**:

- **Claude Code**: lightweight `PreToolUse` and `SubagentStart` hooks carry a `HOTPOT_LANGUAGE: <value>` line plus a one-line directive in `additionalContext`.
- **Codex**: `SessionStart` plus lightweight `PreToolUse` hooks carry the same value via `systemMessage` and `additionalContext`.
- **OpenCode**: the Hotpot plugin's `shell.env` exports `HOTPOT_LANGUAGE` to every Bash tool call; it does not inject the bootstrap context into model context. The orchestrator and sub-agent bodies restate the directive.
- **Pi**: `pi.on("context", …)` injects `HOTPOT_LANGUAGE` plus a one-line directive into every provider request.

Trust the **most recent injection** you have seen — it is always fresh. The value is a free-form string written verbatim by the user (e.g. `简体中文`, `english`, `日本語`, `Français`, `zh-CN`). When in doubt, default to English.

## What MUST Follow The Configured Language

Apply the detected language to **every** natural-language artifact you produce in this command, including:

- The full body of any task `.md` you write under `.hotpot/workspaces/<user>/tasks/` (titles, summaries, design, requirements, plan prose, validation hints, open questions, etc.).
- Brainstorming questions, design discussions, option lists, and clarification requests shown to the user.
- Phase announcements (e.g. "I am now in the EXECUTION phase" / "I am now in the READ-ONLY REVIEW phase"), execution reports, review reports, fix-loop status updates, and the final per-command summary.
- Issue-memory natural-language fields when you produce them inline: `title`, `scene`, `problem`, `solution`, `description`, `review_check`, `summary`, `fix`, `reason`, `promote_hint`.
- Any user-facing prompts emitted by sub-skills or sub-agents you spawn from this command — pass the detected language to them in the embedded prompt so they stay consistent.

## What MUST Remain In English (Regardless Of `language`)

These items are part of Hotpot's structural / machine-readable contract and translating them will break tooling:

- Structural JSON / TOML keys and field names (e.g. `kind`, `date`, `tags`, `paths`, `time`, `title`, `task_id`, `commit`, `status`, `active`).
- Code identifiers, file paths, CLI flag spellings, command names, env-var names.
- Machine-readable tokens emitted by Hotpot CLI — slash-command orchestrators pattern-match on these literals:
  - `ACTIVE_CONFLICT:` (error prefix from `hotpot task create`)
  - `Not found active task` (error string from `hotpot task active` / related queries)
- The Hotpot task file slug segment used in filenames (kebab-case ASCII; see `hotpot-new.md` "Task Creation").
- The literal value of `## Plan > ### Mode` (`tdd: true` / `tdd: false`) — the execute flow parses this line.
- Checkbox markers (`- [ ]`, `- [x]`) and step prefixes used by TDD mode (`R1`, `G1`, `F1`, etc.).
- Markdown section headings that the execute / review flow keys off (e.g. `## Task`, `## Plan`, `### Mode`, `### File Map`, `### Implementation Tasks`, `### Validation`, `### Risks and Watchouts`, `## Execution Instructions`, `## Open Questions`). Translate the **body** of these sections but keep the heading text in English.

When in doubt, prefer keeping a token in English. Translation is for prose the user reads; it is not for keys, paths, or anchors that other code or other prompts grep for.

## Recovery When `language` Is Unreadable

Hotpot's Rust resolver already handles all parse / IO failures silently and falls back to English (`HOTPOT_LANGUAGE=English`). In normal operation you should never see a missing or corrupt value — the hook always delivers a usable string. If you somehow still observe an empty / blank `HOTPOT_LANGUAGE` in an injected payload, treat it as English and continue; never abort the workflow over a language ambiguity.

If you suspect the user's `config.toml::language` is being ignored (e.g. you keep replying in English when the user clearly wants Chinese), run `hotpot update --json` and check the `language` / `language_source` fields — that report shows which link of the resolution chain produced the current value.
