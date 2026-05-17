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

- **Task file** (`<workspace>/tasks/<YYYY-MM-DD>-<title>.md`): the self-contained handoff document. Must include `## Task`, `## Plan`, and `## Execution Instructions`. `## Plan > ### Mode` carries `- tdd: true|false` for TDD vs default flow. `<title>` is the kebab-case slug passed to `task create --title`; the CLI collapses any residual whitespace runs in `title` into a single `-` when building the filename (defensive only — `/hotpot:new` produces kebab-case up front and no further slugify happens).
- **Overview ledger** (`<workspace>/overview.jsonl`): per-user task ledger. Invariant: at most one row with `active=true && status=In Progress` per user.
- **Issue candidates** (`<workspace>/issue-candidates.jsonl`): transient, per-user review-memory drafts buffered during execute and promoted (or discarded) during finish-work.
- **Issues** (`.hotpot/issues.jsonl`): shared, long-lived review memory. Append-only via `hotpot issues promote`; never written without user approval.
- **Shared prompts** (`.hotpot/prompts/`): cross-platform LLM prompt assets installed by `hotpot init` / `hotpot update`. Not user-edited.
- **VuePress hub** (`.hotpot-hub/`, opt-in): the VuePress project Hotpot deploys when the user runs `hotpot vuepress install` (or `hotpot init --enable-vuepress`). Hosts `pnpm docs:dev` so task files render in a browser. Tied atomically to `.hotpot/config.toml::[vuepress] enabled` and to two opt-in prompt assets (`.hotpot/prompts/vuepress.md` / `vuepress-style.md`) — managed only via `hotpot vuepress install` / `uninstall`. Manual flipping of `enabled` desyncs the trio.

## Directory Layout

```
<project>/
├── .hotpot/
│   ├── config.toml                      # Project config (language, [vuepress])
│   ├── issues.jsonl                     # Shared, long-lived memory
│   ├── prompts/                         # Installed prompt assets (do not edit)
│   ├── brainstorm/<session>/            # Transient visual-companion artifacts
│   └── workspaces/<username>/
│       ├── overview.jsonl
│       ├── issue-candidates.jsonl
│       └── tasks/<YYYY-MM-DD>-<title>.md
├── .hotpot-hub/                          # VuePress hub (opt-in; gitignored)
│   ├── package.json                     # vuepress + theme deps
│   ├── docs/                            # task files surfaced via user symlinks
│   └── vuepress.runtime.json            # dev-server pid/port/url/ttl state
├── .claude/  .opencode/  .codex/  .pi/   # Platform configuration directories
```

Path resolution: `src/paths.rs`. Username chain (`src/context.rs::resolve_username`): `HOTPOT_USERNAME` → `git config --local user.name` → `git config --global user.name` → `"default"`.

## Commands

User-facing commands. Slash commands are AI workflows; CLI subcommands are state and resource management.

| Command | Purpose |
|---|---|
| `hotpot init` | Install platform-specific assets and shared prompts. Idempotent. Use `--platform {claude\|opencode\|codex\|pi\|all}`. `--enable-vuepress` (or interactive yes) additionally runs `hotpot vuepress install`. |
| `hotpot update` | Day-1 entry for collaborators. Detects installed platforms, refreshes assets, bootstraps the current user's workspace, merges the hotpot block into `.gitignore`, runs a health self-check. |
| `hotpot vuepress {install,uninstall,start,stop,status}` | Manage the opt-in VuePress integration. `install` deploys `.hotpot-hub/` + `pnpm install` + opt-in prompts + flips `[vuepress] enabled = true`. `uninstall` reverses everything. `start` / `stop` / `status` manage the `pnpm docs:dev` process via `.hotpot-hub/vuepress.runtime.json`. See **VuePress Integration** below. |
| `/hotpot:new` | Brainstorm → approve design → `hotpot task create [--switch\|--inactive]` → write the handoff task file. When VuePress is enabled, the closing flow additionally prompts the user to open the task in a browser and runs `hotpot vuepress start`. No code modifications during `new`. |
| `/hotpot:execute` | Pre-flight `hotpot vuepress stop --if-running` (releases any dev server started by `/hotpot:new`) → resolve active task → run execution subagent → collect diff and relevant memory → run read-only review subagent → fix loop (≤ 2 rounds) → buffer issue candidates → ask user which to keep → write approved candidates via `hotpot issues candidate add`. |
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

## VuePress Integration

VuePress is **opt-in**: disabled projects never see VuePress prompts, env vars, or hub files in their context. Enabling and disabling are atomic CLI operations — flipping the config flag by hand is unsupported.

### Atomic state — three pieces that must agree

1. `.hotpot/config.toml::[vuepress] enabled = true`
2. `.hotpot-hub/` exists with `package.json` + `pnpm install` already run
3. `.hotpot/prompts/vuepress.md` + `.hotpot/prompts/vuepress-style.md` exist

`hotpot vuepress install` puts all three in place atomically; `hotpot vuepress uninstall` reverses them in the opposite order (prompts → docs symlinks → `.hotpot-hub/` → config flag). `hotpot vuepress start` calls `verify_install_consistency` before spawning the dev server, so a half-broken state (e.g. user manually flipped `enabled = true` without running `install`) fails fast with a repair hint instead of producing confusing pnpm errors.

The `[vuepress]` table is written with a bilingual warning comment block by `enable_in_config_toml` (`src/vuepress.rs`); never bypass this writer.

### Asset tiers

| Tier | Catalog | Install time | Purpose |
|---|---|---|---|
| Shared | `SHARED_ASSETS` (`src/assets/shared.rs`) | every `hotpot init` | Cross-platform prompts (output-language, tdd-protocol, hotpot-new/execute/finish-work, etc.). |
| VuePress opt-in prompts | `VUEPRESS_OPT_IN_ASSETS` (`src/assets/vuepress_opt_in.rs`) | `hotpot vuepress install` only | `vuepress.md` (brainstorming closing flow) + `vuepress-style.md` (markdown writing conventions). The env-gate in `hotpot-new.md` only Reads these when `$HOTPOT_VUEPRESS_ENABLED == "true"`, so disabled projects keeping no copy on disk is what guarantees a clean AI context. |
| VuePress hub project | `VUEPRESS_HUB_ASSETS` (`src/assets/vuepress_hub.rs`) | `hotpot vuepress install` only | `.hotpot-hub/` containing `package.json`, `pnpm-lock.yaml`, `docs/README.md` and the four `.vuepress/` files (`config.js`, `client.js`, `sidebar.js`, `components/TaskIndex.vue`). The actual `pnpm install` + `sync_tasks_links` (idempotent: prunes stale per-user links, adds links for new users, leaves existing ones alone) are orchestrated by `vuepress::install_hub`, not by the asset engine. |

### Service lifecycle — three layers of defense

The `pnpm docs:dev` process spawned by `hotpot vuepress start` must not leak past the user's session. Three independent layers cooperate:

1. **`/hotpot:execute` pre-flight stop** (primary). The prompt's first step is `hotpot vuepress stop --if-running`. This is idempotent and covers the normal path where the user proceeds from `/hotpot:new` to `/hotpot:execute`.
2. **`SessionEnd` / `session_shutdown` hook** (defense layer 2). Claude Code, OpenCode, and Pi all fire a session-close event when the user closes the agent without proceeding. The hook invokes the same idempotent stop. **Codex has no SessionEnd event** in its documented hook clinic — Codex users rely on layer 3 alone.
3. **`--ttl` lazy expiry** (final fallback). `start` writes an `expires_at` timestamp (default 30 min) into `vuepress.runtime.json`. The next `status` or `start` call checks this timestamp and kills the process if expired. This is the only safety net for Codex.

`runtime.json` lives at `.hotpot-hub/vuepress.runtime.json` (inside the hub, so `uninstall` cleans it up naturally). `stale` states (dead pid or expired ttl) are lazy-cleaned on read; nothing polls in the background.

Public env-var contract for VuePress: `HOTPOT_VUEPRESS_ENABLED` is always serialized (`"true"` or `"false"`); `HOTPOT_VUEPRESS_PORT` + `HOTPOT_VUEPRESS_URL` are emitted only when enabled.

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
- Public env-var contract used by hooks/bootstrap: `ROOT_DIR`, `HOTPOT_USERNAME`, `HOTPOT_LANGUAGE`, `HOTPOT_ISSUE_CANDIDATES_FILE`, `HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`, `HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`, `HOTPOT_TDD_PROTOCOL_PROMPT`, `HOTPOT_NEW_PROMPT`, `HOTPOT_EXECUTE_PROMPT`, `HOTPOT_FINISH_WORK_PROMPT`, plus the VuePress trio (`HOTPOT_VUEPRESS_ENABLED` always present; `HOTPOT_VUEPRESS_PORT` / `HOTPOT_VUEPRESS_URL` only when enabled — see **VuePress Integration**). Adding a new env var requires extending `hotpot hook bootstrap`'s output and `src/context.rs::Context` so all platforms see it.
- **Path-string invariant for agent-facing env-vars.** Every path field on `Context` (`ROOT_DIR`, `HOTPOT_ISSUE_CANDIDATES_FILE`, every `HOTPOT_*_PROMPT`) is emitted in POSIX form on **all** platforms — `dunce::canonicalize` strips Windows's `\\?\` verbatim prefix, and `path_to_agent_string` (`src/context.rs`) further replaces every `\` with `/`. Windows fs APIs accept forward slashes natively, so the normalization is lossless; its purpose is to keep backslashes out of downstream markdown rendering (where `\h`, `\R` etc. get eaten as escapes), URI assembly (where `\\?\` URI-encodes to `%3F` and produces `file://%3F\D:\...`), and JSON-in-prompt expansion (where backslashes need double-escaping that downstream consumers routinely get wrong). Any new path-typed field on `Context` must go through `path_to_agent_string`; never call `.display().to_string()` directly when populating a `Context` field, and never call `std::fs::canonicalize` in code paths that feed `Context` (use `dunce::canonicalize` instead).
- TDD-mode changes must land synchronously on all four platforms' `new` and `execute` assets plus the `hotpot-execution` / `hotpot-review` subagents. Reference the shared protocol via `@.hotpot/prompts/tdd-protocol.md` (Claude/OpenCode) or `$HOTPOT_TDD_PROTOCOL_PROMPT` (Codex/Pi); never inline its content.
- Critical sections that rewrite `overview.jsonl` / `issues.jsonl` / `issue-candidates.jsonl` go through `src/lock.rs::with_file_lock`. Hard rule: **do not spawn subprocesses while holding the lock** — platform hooks may re-enter `hotpot` and deadlock.
- Outputs in code stay English (per `AGENTS.md`); natural-language responses follow `.hotpot/config.toml::language`. Resolution lives in Rust (`src/context.rs::resolve_language[_with_source]`, chain: env `HOTPOT_LANGUAGE` → `<root>/.hotpot/config.toml` top-level `language` → `"English"`). Every platform hook re-injects the value **per turn** to defeat the "instruction once, drift forever" pattern: Claude `PreToolUse` + `SubagentStart` + `UserPromptSubmit`; Codex `PreToolUse` + `SessionStart` + `UserPromptSubmit`; OpenCode plugin `shell.env`; Pi `pi.on("context", …)`. The shared `language_directive_message` helper (`src/commands/hook.rs`) is the single source of the human-readable one-liner; the long-form spec lives in `assets/prompts/output-language.md` and is referenced by every main workflow prompt via `@.hotpot/prompts/output-language.md` (Claude/OpenCode) or `$ROOT_DIR/.hotpot/prompts/output-language.md` (Codex/Pi). Structural anchors (CLI flags, JSON keys, `ACTIVE_CONFLICT:`, markdown section headings, `tdd: true|false`, kebab-case slugs) MUST stay English regardless of the configured language.
- `hotpot task create` only appends to `overview.jsonl`; it does **not** materialize the task `<time>-<title>.md` file. The `/hotpot:new` slash command is responsible for creating that file via the platform's create-file tool (Claude `Write`, OpenCode `write`, Codex `apply_patch *** Add File`, Pi `write`). Slash-command prompts must explicitly forbid a `Read`-before-`Write` probe of the task path (a missing file there is the normal post-create state, not an error). The CLI ensures `<workspace>/tasks/` exists as a non-fatal side effect of `task create`, but slash commands must not depend on that as a contract.
