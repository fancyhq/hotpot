# Hotpot Architecture

Quick orientation for agents that plan, implement, or extend Hotpot. Read this before touching commands, agents, prompts, or CLI subcommands.

## What Hotpot Is

A cross-platform task orchestrator for coding agents (Claude Code, OpenCode, Codex, Pi). One Hotpot lifecycle:

1. Capture an idea as a fully specified task file.
2. Execute the task through an execution subagent.
3. Audit the result through a read-only review subagent that consumes accumulated project memory.
4. Loop fix → review until the change is acceptable (max 2 rounds).
5. On user confirmation, promote useful lessons into long-lived shared memory.

Every command is **manually triggered** by the user. Hotpot never spawns automatic work — it orchestrates between user, file system, and subagents.

## Core Concepts

- **Task file** (`<workspace>/tasks/<YYYY-MM-DD>-<title>.md`): the self-contained handoff document. Must include `## Task`, `## Plan`, and `## Execution Instructions`. `## Plan > ### Mode` carries `- tdd: true|false` for TDD vs default flow.
- **Overview ledger** (`<workspace>/overview.jsonl`): per-user task ledger. Invariant: at most one row with `active=true && status=In Progress` per user.
- **Issue candidates** (`<workspace>/issue-candidates.jsonl`): transient, per-user review-memory drafts buffered during execute and promoted (or discarded) during finish-work.
- **Issues** (`.hotpot/issues.jsonl`): shared, long-lived review memory. Append-only via `hotpot issues promote`; never written without user approval.
- **Shared prompts** (`.hotpot/prompts/`): cross-platform LLM prompt assets installed by `hotpot init` / `hotpot update`. Not user-edited.

## Directory Layout

```
<project>/
├── .hotpot/
│   ├── config.toml                      # Project config (e.g. language)
│   ├── issues.jsonl                     # Shared, long-lived memory
│   ├── prompts/                         # Installed prompt assets (do not edit)
│   ├── brainstorm/<session>/            # Transient visual-companion artifacts
│   └── workspaces/<username>/
│       ├── overview.jsonl
│       ├── issue-candidates.jsonl
│       └── tasks/<YYYY-MM-DD>-<title>.md
├── .claude/  .opencode/  .codex/  .pi/   # Platform configuration directories
```

Path resolution: `src/paths.rs`. Username chain (`src/context.rs::resolve_username`): `HOTPOT_USERNAME` → `git config --local user.name` → `git config --global user.name` → `"default"`.

## Commands

Four manual user-facing commands. Each platform exposes them with its native mechanism.

| Command | Purpose |
|---|---|
| `hotpot init` | Install platform-specific assets and shared prompts. Idempotent. Use `--platform {claude\|opencode\|codex\|pi\|all}`. |
| `hotpot update` | Day-1 entry for collaborators. Detects installed platforms, refreshes assets, bootstraps the current user's workspace, merges the hotpot block into `.gitignore`, runs a health self-check. |
| `/hotpot:new` | Brainstorm → approve design → `hotpot task create [--switch\|--inactive]` → write the handoff task file. No code modifications during `new`. |
| `/hotpot:execute` | Resolve active task → run execution subagent → collect diff and relevant memory → run read-only review subagent → fix loop (≤ 2 rounds) → buffer issue candidates → ask user which to keep → write approved candidates via `hotpot issues candidate add`. |
| `/hotpot:finish-work` | Confirm completion → summarize candidates → user-approve promotion → `hotpot issues promote` → `hotpot issues candidate clear` → optional git commit → `hotpot task done [--commit <SHA>]` → optional switch-and-continue to another in-progress task. |

Platform-specific surfaces:

- **Claude Code**: `.claude/commands/hotpot/*.md` + `.claude/agents/hotpot-{execution,review}.md`.
- **OpenCode**: `.opencode/commands/hotpot/*.md` + `.opencode/agents/hotpot-{execution,review}.md` + TypeScript plugins under `.opencode/plugins/`.
- **Codex**: `.codex/skills/hotpot-*/SKILL.md` + `.codex/agents/hotpot-{execution,review}.toml` (no project-level Markdown commands).
- **Pi**: `.pi/prompts/hotpot-*.md` + `.pi/extensions/` (no native subagents — single-session phased execution; review phase stays read-only).

## Execution Flow (one task)

```
new → task file written → execute → execution subagent
                                  → review subagent (with relevant memory)
                                  → fix loop (≤ 2 rounds)
                                  → propose candidates (user approves subset)
                                  → write approved candidates
finish-work → summarize candidates → user approves promotion
            → promote to issues.jsonl → clear candidates
            → optional git commit → mark task Done [+ SHA]
            → optional switch to next In-Progress task → run execution only
```

CLI surface for state transitions (always go through CLI, not ad-hoc file edits):

- `hotpot task create [--switch|--inactive] --title <t>` — enforces single-active invariant; bails with `ACTIVE_CONFLICT:` prefix on conflict (treat as machine-readable token, do not localize).
- `hotpot task list --json`, `hotpot task active [--path|--count]`
- `hotpot task done [--task-id <id>] [--commit <sha>]`, `hotpot task cancel`, `hotpot task resume`
- `hotpot issues relevant --changed-file <p> --keyword <k> --limit 5`
- `hotpot issues promote` (stdin JSONL → `{"promoted":N}`)
- `hotpot issues candidate {list,add,clear}` (`add` reads stdin JSONL → `{"added":N}`)

## Design Principles

- **The task file is the contract.** New abstractions should map onto sections already in the task file rather than creating sidecar files.
- **Orchestration belongs to the slash command; intelligence belongs to subagents.** The command file collects context (paths, diffs, memory) and calls subagents with explicit prompts. Subagents do not call other subagents.
- **Memory pipeline is two-stage on purpose.** Candidates are cheap and per-user; promoted issues are expensive and shared. Never bypass the candidate stage.
- **Review is always read-only.** Even in Pi's same-session fallback.
- **State changes go through CLI subcommands**, not through ad-hoc file edits.
- **Idempotency.** `hotpot init` / `hotpot update` must be safe to re-run.
- **Cross-platform first.** Any new behavior must be designed for all four platforms (or explicitly scoped with a documented reason). Single-platform additions are compatibility regressions.

## Notes For Future Agents

- Adding any new asset under `assets/platforms/<platform>/` requires registering it in `src/commands/init/<platform>.rs::ASSETS`, or `hotpot init` will skip it. Pick `Asset::owned(...)` for Hotpot-private files; `Asset::merge_json(...)` / `Asset::merge_toml(...)` for platform main-config files (anchors live in `src/commands/init/merge.rs`).
- Cross-platform LLM prompts go under `assets/prompts/`, registered once in `src/commands/init/mod.rs::SHARED_ASSETS`, and are installed into every project's `.hotpot/prompts/`. Runtime resolution: `src/context.rs::prompt_path`.
- Public env-var contract used by hooks/bootstrap: `ROOT_DIR`, `HOTPOT_USERNAME`, `HOTPOT_ISSUE_CANDIDATES_FILE`, `HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`, `HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`, `HOTPOT_TDD_PROTOCOL_PROMPT`. Adding a new env var requires extending `hotpot hook bootstrap`'s output so all platforms see it.
- TDD-mode changes must land synchronously on all four platforms' `new` and `execute` assets plus the `hotpot-execution` / `hotpot-review` subagents. Reference the shared protocol via `@.hotpot/prompts/tdd-protocol.md` (Claude/OpenCode) or `$HOTPOT_TDD_PROTOCOL_PROMPT` (Codex/Pi); never inline its content.
- Critical sections that rewrite `overview.jsonl` / `issues.jsonl` / `issue-candidates.jsonl` go through `src/lock.rs::with_file_lock`. Hard rule: **do not spawn subprocesses while holding the lock** — platform hooks may re-enter `hotpot` and deadlock.
- Outputs in code stay English (per `AGENTS.md`); natural-language responses follow `.hotpot/config.toml::language`.
