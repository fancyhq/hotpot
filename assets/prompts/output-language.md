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

Before producing any natural-language output for the user, check the project language preference and obey it for the entire command.

## How To Detect The Language

1. Read `$ROOT_DIR/.hotpot/config.toml` if it exists.
2. If a top-level `language` field is present and non-empty, treat its value as a direct instruction for the output language. The value is free-form, written verbatim by the user. Examples: `简体中文`, `english`, `日本語`, `Français`, `zh-CN`.
3. If the file is missing, the field is missing, or the value is empty, default to English.

Do this detection **once at the start of the command** and remember the result for the rest of the session — do not re-read `config.toml` between every message.

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

If `config.toml` exists but parsing fails (corrupt TOML, unreadable file), do **not** abort the command. Fall back to English for natural-language output, surface a single short warning to the user mentioning the file path, and continue. The user can fix the file at their leisure; failing the whole workflow over a config typo is worse than degrading gracefully.
