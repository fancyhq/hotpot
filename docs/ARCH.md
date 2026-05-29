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

- **Task file** (`<workspace>/tasks/<YYYY-MM-DD>-<title>.md`): the self-contained handoff document. Must include `## Task`, `## Plan`, and `## Execution Instructions`. `## Plan > ### Mode` carries `- tdd: true|false` for TDD vs default flow. `## Plan > ### Execution Strategy` carries `- git-worktree: true|false`, which is the authoritative execution-time worktree decision. `<title>` is the kebab-case slug passed to `task create --title`; the CLI collapses any residual whitespace runs in `title` into a single `-` when building the filename (defensive only — `/hotpot:new` produces kebab-case up front and no further slugify happens).
- **Overview ledger** (`<workspace>/overview.jsonl`): per-user task ledger. Invariant: at most one row with `active=true && status=In Progress` per user.
- **Issue candidates** (`.hotpot/issue-candidates.jsonl`): project-shared temporary review-memory candidates buffered during execute and promoted (or discarded) during finish-work. Legacy per-user candidate files under `.hotpot/workspaces/<username>/issue-candidates.jsonl` are migrated into this global file once and then truncated.
- **Issues** (`.hotpot/issues.jsonl`): shared, long-lived review memory. Append-only via `hotpot issues promote`; never written without user approval.
- **Shared prompts** (`.hotpot/prompts/`): cross-platform LLM prompt assets installed by `hotpot init` / `hotpot update`. Not user-edited.
- **VuePress hub** (`.hotpot-hub/`, opt-in): the VuePress project Hotpot deploys when the user runs `hotpot vuepress install` (or `hotpot init --enable-vuepress`). Hosts `pnpm docs:dev` so task files render in a browser. Tied atomically to `.hotpot/config.toml::[vuepress] enabled` and to two opt-in prompt assets (`.hotpot/prompts/vuepress.md` / `vuepress-style.md`) — managed only via `hotpot vuepress install` / `uninstall`. Manual flipping of `enabled` desyncs the trio.

## Directory Layout

```
<project>/
├── .hotpot/
│   ├── config.toml                      # Project config (language, [vuepress])
│   ├── issues.jsonl                     # Shared, long-lived memory
│   ├── issue-candidates.jsonl           # Shared, temporary review-memory candidates
│   ├── prompts/                         # Installed prompt assets (do not edit)
│   ├── brainstorm/<session>/            # Transient visual-companion artifacts; stop prunes brainstorm session dirs
│   └── workspaces/<username>/
│       ├── overview.jsonl
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
| `hotpot update` | Day-1 entry for collaborators. Detects installed platforms, refreshes assets, bootstraps the current user's workspace, merges the hotpot block into `.gitignore`, runs a health self-check. `--force` overwrites differing Hotpot-private owned templates while preserving merge/config/user-owned asset strategies. |
| `hotpot vuepress {install,uninstall,start,stop,status}` | Manage the opt-in VuePress integration. `install` deploys `.hotpot-hub/` + `pnpm install` + opt-in prompts + flips `[vuepress] enabled = true`. `uninstall` reverses everything. `start` / `stop` / `status` manage the `pnpm docs:dev` process via `.hotpot-hub/vuepress.runtime.json`. See **VuePress Integration** below. |
| `/hotpot:new` | Brainstorm → approve design → decide execution strategy (`## Plan > ### Execution Strategy`, including `git-worktree: true|false`) → run the VuePress file-existence gate before writing the task file → when enabled, read `vuepress-style.md` → `hotpot task create [--switch\|--inactive]` → write the final handoff task file once. When VuePress is enabled, the closing flow additionally prompts the user to open the task in a browser and runs `hotpot vuepress start`. No code modifications during `new`. |
| `/hotpot:execute` | Pre-flight `hotpot vuepress stop --if-running` (releases any dev server started by `/hotpot:new`) → resolve and read active task → parse `## Plan > ### Execution Strategy` → create/reuse/forbid worktree according to `git-worktree: true|false` → run execution subagent → collect diff and relevant memory → run read-only review subagent → fix loop (≤ 2 rounds) → buffer issue candidates → ask user which to keep → write approved candidates via `hotpot issues candidate add`. |
| `/hotpot:finish-work` | Confirm completion → summarize candidates → user-approve promotion → `hotpot issues promote` → `hotpot issues candidate clear` → optional git commit → `hotpot task done [--commit <SHA>]` → when finish-work created the task commit, auto-commit the task-done ledger diff as `chore: record task Done` → optional switch-and-continue to another in-progress task. |

Platform-specific surfaces:

- **Claude Code**: `.claude/commands/hotpot/*.md` + `.claude/agents/hotpot-{execution,review}.md`.
- **OpenCode**: `.opencode/commands/hotpot/*.md` + `.opencode/agents/hotpot-{execution,review}.md` + TypeScript plugins under `.opencode/plugins/`.
- **Codex**: `.codex/skills/hotpot-*/SKILL.md` + `.codex/agents/hotpot-{execution,review}.toml` (no project-level Markdown commands).
- **Pi**: `.pi/extensions/` — `pi.registerCommand` registers `/hotpot-new` / `/hotpot-execute` / `/hotpot-finish-work` slash commands that send user-voice messages via `pi.sendUserMessage` (the method is on `ExtensionAPI`, not on the handler's `ctx: ExtensionCommandContext`); no prompt-template thin shells; no native subagents — single-session phased execution; review phase stays read-only. Each handler also arms `pendingFirstToolGuard` before `pi.sendUserMessage`; the single `tool_call` hook must block every first tool call except `read` of the exact workflow prompt path until that read succeeds, then disarm so normal workflow exploration can continue.

## Execution Flow (one task)

```
new → task file written (includes Execution Strategy)
    → execute consumes strategy → execution subagent
                                → review subagent (with relevant memory)
                                → fix loop (≤ 2 rounds)
                                → propose candidates (user approves subset)
                                → write approved candidates
finish-work → summarize candidates → user approves promotion
            → promote to issues.jsonl → clear candidates
            → optional git commit → mark task Done [+ SHA]
            → auto commit task-done ledger diff when task commit was automatic
            → optional switch to next In-Progress task → run execution only
```

CLI surface for state transitions (always go through CLI, not ad-hoc file edits):

- `hotpot task create [--switch|--inactive] --title <t>` — enforces single-active invariant; bails with `ACTIVE_CONFLICT:` prefix on conflict (treat as machine-readable token, do not localize).
- `hotpot task list --json`, `hotpot task active [--path|--count]`
- `hotpot task done [--task-id <id>] [--commit <sha>]` — after the ledger is
  updated, the CLI also syncs the task markdown file's VuePress Overview
  `Status` cell to `Done` when VuePress is enabled. I/O errors produce
  an `eprintln!` warning but do not fail the command. Skipped outcomes
  (VuePress disabled, missing file, no Overview table) are silent.
  The sync lives in `src/task/markdown.rs::sync_task_file_status`.
- `hotpot task cancel`, `hotpot task resume`
- `hotpot issues relevant --changed-file <p> --keyword <k> --limit 5`
- `hotpot issues promote` (stdin JSONL → `{"promoted":N}`)
- `hotpot issues candidate {list,add,clear}` (`add` reads stdin JSONL → `{"added":N}`)

Worktree execution contract:

- `/hotpot:new` must resolve the execution strategy before it creates the task record and writes the task file. The strategy lives under `## Plan > ### Execution Strategy` and must include `- git-worktree: true` or `- git-worktree: false`.
- `/hotpot:execute` only consumes the task-file strategy; it does not ask the user whether to use a worktree. Missing or invalid `git-worktree` values stop execution and require re-running `/hotpot:new` or revising the task file.
- With `git-worktree: true`, execute reuses an already attached worktree from `hotpot worktree path`, or runs `hotpot worktree create` when none is attached. Create failures are blockers, not prompts to downgrade.
- With `git-worktree: false`, execute runs in the current checkout. If `hotpot worktree path` reports an attached worktree, execute stops because the task-file strategy and ledger state conflict.

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
| VuePress opt-in prompts | `VUEPRESS_OPT_IN_ASSETS` (`src/assets/vuepress_opt_in.rs`) | `hotpot vuepress install` only | `vuepress.md` (brainstorming closing flow) + `vuepress-style.md` (markdown writing conventions). The file-existence gate in `hotpot-new.md` only Reads these when both files are present on disk — which is exactly when VuePress is installed (atomic invariant maintained by `hotpot vuepress install` / `uninstall`). Same observable on every platform, so OpenCode (which never surfaces the `HOTPOT_VUEPRESS_ENABLED` env-var into AI conversation context) behaves identically to Claude / Codex / Pi. |
| VuePress hub project | `VUEPRESS_HUB_ASSETS` (`src/assets/vuepress_hub.rs`) | `hotpot vuepress install` only | `.hotpot-hub/` containing `package.json`, `pnpm-lock.yaml`, `docs/README.md` and the **five `.vuepress/` files** (`config.js`, `client.js`, `sidebar.js`, `styles/index.scss`, `components/TaskIndex.vue`). The first four runtime files are tightly coupled (`config.js` ↔ `client.js` ↔ `sidebar.js` ↔ `TaskIndex.vue` via the `__HOTPOT_TASK_INDEX__` compile-time inject); `styles/index.scss` is an **independent decorative layer** loaded automatically by `@vuepress/theme-default`'s `styles/index.scss` convention — safe to edit or delete without breaking the home-page TaskIndex injection chain. The actual `pnpm install` + `sync_tasks_links` (idempotent: prunes stale per-user links, adds links for new users, leaves existing ones alone) are orchestrated by `vuepress::install_hub`, not by the asset engine. |

### Service lifecycle — three layers of defense

The `pnpm docs:dev` process spawned by `hotpot vuepress start` must not leak past the user's session. Three independent layers cooperate:

1. **`/hotpot:execute` pre-flight stop** (primary). The prompt's first step is `hotpot vuepress stop --if-running`. This is idempotent and covers the normal path where the user proceeds from `/hotpot:new` to `/hotpot:execute`.
2. **`SessionEnd` / `session_shutdown` hook** (defense layer 2). Claude Code, OpenCode, and Pi all fire a session-close event when the user closes the agent without proceeding. The hook invokes the same idempotent stop. **Codex has no SessionEnd event** in its documented hook clinic — Codex users rely on layer 3 alone.
3. **`--ttl` lazy expiry** (final fallback). `start` writes an `expires_at` timestamp (default 30 min) into `vuepress.runtime.json`. The next `status` or `start` call checks this timestamp and kills the process if expired. This is the only safety net for Codex.

`runtime.json` lives at `.hotpot-hub/vuepress.runtime.json` (inside the hub, so `uninstall` cleans it up naturally). `stale` states (dead pid or expired ttl) are lazy-cleaned on read; nothing polls in the background.

The CLI stop path is deliberately stronger than a single runtime pid kill. On Unix, `start` calls `setsid()`, so `stop` first targets the runtime pid's process group and then falls back to the pid; on Windows it preserves `taskkill /T` process tree semantics. Dead runtime pids and TTL expiry reuse the same cleanup path. If the parent pid is gone but the runtime port is still held by a command line that is identifiable as this hub's VuePress/Vite/pnpm process, the runtime port fallback terminates only that Hotpot-owned process and then deletes `runtime.json`.

Public env-var contract for VuePress: `HOTPOT_VUEPRESS_ENABLED` is always serialized (`"true"` or `"false"`); `HOTPOT_VUEPRESS_PORT` + `HOTPOT_VUEPRESS_URL` are emitted only when enabled.

## npm Distribution

The project ships a lightweight npm wrapper package at `npm/` (published as `@fancyhq/hotpot`) for global installation via `npm install -g @fancyhq/hotpot`. The installed CLI command remains `hotpot`.

### Architecture

- `npm/package.json` — defines the package metadata (published as `@fancyhq/hotpot`), a `bin.hotpot` entry, and a `postinstall` script.
- `npm/bin/hotpot.js` — the CLI entry; forwards all arguments and stdio to the native Rust binary located in the same `bin/` directory.
- `npm/scripts/install.js` — the postinstall script; detects `process.platform` / `process.arch`, maps to the release asset label, downloads the correct archive from GitHub Releases (`https://github.com/fancyhq/hotpot/releases/download/<tag>/hotpot-<tag>-<label>.<ext>`), extracts the binary into `bin/`, and sets executable permissions.

Supported platforms: Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64. Unsupported platforms produce an English error message and exit with code 1.

### Version Synchronization

The npm package version is kept in sync with the Rust crate version via `release-please`. The `release-please-config.json` lists `npm/package.json` in the `extra-files` array, so Release PRs automatically update both `Cargo.toml` and `npm/package.json` from the conventional commits.

### Release Workflow

The `.github/workflows/release-please.yml` workflow includes a `publish-npm` job that runs after `build-release-assets` when a release is created. It checks out the release tag, runs `npm pack --dry-run` to validate, then publishes with `npm publish ./npm --access public` using `NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}`. The `--access public` flag is required because scoped packages (`@fancyhq/hotpot`) default to private on npm.

The `.github/workflows/rebuild-release-assets.yml` manual workflow does NOT publish npm — it only rebuilds and uploads binary assets for existing tags.

### Wrapper Executable Bit Contract

The npm CLI entry point `npm/bin/hotpot.js` must remain executable on Unix so that shells like fish (which check the symlink target's executable bit) can run the installed `hotpot` command. This is enforced by:

- **Source file mode**: `npm/bin/hotpot.js` is committed with executable permission (`chmod +x`, mode `0o755`). Git tracks the mode change (`100644 → 100755`).
- **Tarball validation**: `npm pack --dry-run --json ./npm` captures the file mode of each tarball entry. Regression tests in `npm/scripts/install.test.js` assert that the `bin/hotpot.js` entry has executable bits set (`mode & 0o111 !== 0`).
- **Deterministic (no-network) tests**: `node --test npm/scripts/install.test.js` runs the full test suite including executable bit, bin mapping, tarball file inclusion, and `setExecutable` helper validation, without performing any network I/O.

The `npm/scripts/install.js` `setExecutable` helper (used for the downloaded native binary) is also exported and tested independently via `node:test`. Any change that removes the npm wrapper's executable permission or the native binary's chmod call will be caught by CI.

### Prerequisites

- The `NPM_TOKEN` repository secret must be configured with an npm automation token that has publish permissions for the `@fancyhq/hotpot` package.
- npm installation requires network access to GitHub Releases. Offline environments or networks where GitHub is blocked will fail.
- If your npm is configured with a custom or internal registry mirror, the `@fancyhq/hotpot` package may not be available there. Use `--registry https://registry.npmjs.org` for one-off installs, or `npm config set @fancyhq:registry https://registry.npmjs.org/` for a persistent scope-level override.

## Multi-Channel Distribution

Hotpot is distributed through multiple package channels. This section covers the release workflow for each channel, version synchronization, and required prerequisites.

### Channel Overview

| Channel | Type | Automatic Publish | Secret Required | Status |
|---------|------|-------------------|-----------------|--------|
| GitHub Release (binary assets) | Direct download | Yes (assets built + uploaded) | `GITHUB_TOKEN` (built-in) | ✅ Production |
| npm (`@fancyhq/hotpot`) | npm registry | Yes (workflow `publish-npm` job) | `NPM_TOKEN` | ✅ Production |
| crates.io (`hotpot-ai`) | Rust crate registry | Yes (workflow `publish-crates-io` job) | `CARGO_REGISTRY_TOKEN` | ✅ New |

### Asset Naming Convention

All binary archives follow the naming pattern:

```
hotpot-${TAG}-${ASSET_LABEL}${EXT}
```

Where:
- `${TAG}` is the full GitHub Release tag (e.g., `hotpot-v0.3.2`)
- `${ASSET_LABEL}` identifies the platform (e.g., `windows-x86_64`, `macos-aarch64`, `linux-x86_64`)
- `${EXT}` is `.tar.gz` for Linux/macOS or `.zip` for Windows

When `TAG=hotpot-v0.3.2`, an example filename is `hotpot-hotpot-v0.3.2-windows-x86_64.zip`.

### Package Naming and Release Contracts

The crates.io package name `hotpot-ai` is separate from the release tag, asset naming, and the installed CLI command name. This separation is maintained by the following contracts:

- **`release-please-config.json`** explicitly sets `component: "hotpot"` for the root package. This ensures release-please generates release tags in the format `hotpot-v<version>` rather than deriving `hotpot-ai-v<version>` from the Cargo package name.
- **`Cargo.toml`** keeps an explicit `[[bin]] name = "hotpot"` target, so `cargo install hotpot-ai` installs the `hotpot` CLI command (not `hotpot-ai`).
- **`npm/package.json`** exposes `"bin": { "hotpot": "bin/hotpot.js" }`, so `npm install -g @fancyhq/hotpot` makes the `hotpot` CLI command available on PATH.
- **Release asset filenames** follow the pattern `hotpot-${TAG}-${ASSET_LABEL}${EXT}` (e.g., `hotpot-hotpot-v0.3.2-windows-x86_64.zip`). The tag in the asset name is the full GitHub Release tag (`hotpot-v<version>`), not the Cargo package name.
- **Archive internal structure** always contains a single binary named `hotpot` (or `hotpot.exe` on Windows), never `hotpot-ai`.

These contracts prevent the crates.io package name change from leaking into GitHub Release tags, download URLs, or the installed user-facing command. Any future changes to the distribution channels must preserve this separation.

### Release Workflow Job Graph

```
release-please
  ├── build-release-assets (matrix: 5 platforms)
  │     └── publish-npm (after build)
  └── publish-crates-io (independent, only needs tag)
```

All jobs are gated by `needs.release-please.outputs.release_created == 'true'`, so they only run when a new release is created (not on ordinary `main` pushes).

### crates.io Channel

The `publish-crates-io` job in `.github/workflows/release-please.yml`:
1. Checks out the release tag.
2. Validates crate packaging via `cargo package --locked --no-verify`.
3. Publishes to crates.io via `cargo publish --locked --token "$CARGO_REGISTRY_TOKEN"`.

**Prerequisites:**
- `CARGO_REGISTRY_TOKEN` repository secret configured with a crates.io API token.
- Cargo metadata (`description`, `readme`, `keywords`, `categories`) must be present in `Cargo.toml`.
- The package is published under the name `hotpot-ai`; `Cargo.toml` keeps an explicit `[[bin]]` target named `hotpot`, so `cargo install hotpot-ai` still installs the `hotpot` CLI command.

### Version Synchronization

Versions across all channels are synchronized through `release-please`:
- The `release-please-config.json` `extra-files` array lists files that `release-please` updates during Release PR generation:
  - `Cargo.lock` (Rust lockfile)
  - `npm/package.json` (npm package version)

### Deferred Channels: Homebrew, Scoop, winget

The current implementation does NOT maintain or publish Homebrew, Scoop, or winget manifests. These channels are deferred because repository-local manifests do not provide the direct install experience users expect from commands such as `brew install hotpot`, `scoop install hotpot`, or `winget install fancyhq.hotpot`.

Future work should add one of the real distribution paths for each channel, such as a Homebrew tap or Homebrew Core PR, a Scoop bucket entry, and a `microsoft/winget-pkgs` submission workflow.

### Manual Rebuild Workflow

The `.github/workflows/rebuild-release-assets.yml` workflow only rebuilds and uploads binary assets for an existing tag. It does NOT:
- Publish to npm or crates.io.

For package publishing, use the full `release-please.yml` workflow.

## Design Principles

- **The task file is the contract.** New abstractions should map onto sections already in the task file rather than creating sidecar files.
- **Orchestration belongs to the slash command; intelligence belongs to subagents.** The command file collects context (paths, diffs, memory) and calls subagents with explicit prompts. Subagents do not call other subagents.
- **Memory pipeline is two-stage on purpose.** Candidates are cheap and project-shared temporary records; promoted issues are expensive and shared long-term memory. Never bypass the candidate stage.
- **Review is always read-only.** Even in Pi's same-session fallback.
- **State changes go through CLI subcommands**, not through ad-hoc file edits.
- **Idempotency.** `hotpot init` / `hotpot update` must be safe to re-run. `hotpot update --force` is the explicit escape hatch for replacing differing Hotpot-private owned templates; it must not turn `Merge*` assets or `CreateIfMissing` seeds into whole-file overwrites.
- **Cross-platform first.** Any new behavior must be designed for all four platforms (or explicitly scoped with a documented reason). Single-platform additions are compatibility regressions.

## Notes For Future Agents

- Adding any new asset under `assets/platforms/<platform>/` requires registering it in `src/commands/init/<platform>.rs::ASSETS`, or `hotpot init` will skip it. Pick `Asset::owned(...)` for Hotpot-private files; `Asset::merge_json(...)` / `Asset::merge_toml(...)` for platform main-config files (anchors live in `src/commands/init/merge.rs`).
- Cross-platform LLM prompts go under `assets/prompts/`, registered once in `src/commands/init/mod.rs::SHARED_ASSETS`, and are installed into every project's `.hotpot/prompts/`. Runtime resolution: `src/context.rs::prompt_path`.
- **Hook prompt contract (lightweight model-visible context)**: model-visible hook output from `PreToolUse` now carries only `ROOT_DIR` and `HOTPOT_LANGUAGE` plus a short language directive, composed by the `prompt_context_message` helper in `src/commands/hook.rs`. Other prompt paths (`HOTPOT_NEW_PROMPT`, `HOTPOT_EXECUTE_PROMPT`, etc.) are NOT listed — the model resolves them via `$ROOT_DIR/.hotpot/prompts/<name>.md` instead. This lightweight context is bundled into both Claude and Codex `PreToolUse` hooks (firing before `Edit|Write` and every `Bash` call, because platform matchers filter by tool name rather than Bash command content; the lightweight payload makes this broader trigger acceptable for Hotpot Bash commands). Per-user-message prompt hooks are not configured or handled. Review-memory hooks (`SubagentStart` / `SessionStart`) still use the full context via `context_lines` for the review agent, which needs the complete env-var map. `hotpot hook bootstrap` (both `--format json` and `--format shell`) continues to emit every field — the full env contract for shell/plugin use is preserved unchanged.
- Public env-var contract used by hooks/bootstrap: `ROOT_DIR`, `HOTPOT_USERNAME`, `HOTPOT_LANGUAGE`, `HOTPOT_ISSUE_CANDIDATES_FILE`, `HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`, `HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`, `HOTPOT_TDD_PROTOCOL_PROMPT`, `HOTPOT_NEW_PROMPT`, `HOTPOT_EXECUTE_PROMPT`, `HOTPOT_FINISH_WORK_PROMPT`, plus the VuePress trio (`HOTPOT_VUEPRESS_ENABLED` always present; `HOTPOT_VUEPRESS_PORT` / `HOTPOT_VUEPRESS_URL` only when enabled — see **VuePress Integration**). Adding a new env var requires extending `hotpot hook bootstrap`'s output and `src/context.rs::Context` so all platforms see it.
- **Path-string invariant for agent-facing env-vars.** Every path field on `Context` (`ROOT_DIR`, `HOTPOT_ISSUE_CANDIDATES_FILE`, every `HOTPOT_*_PROMPT`) is emitted in POSIX form on **all** platforms — `dunce::canonicalize` strips Windows's `\\?\` verbatim prefix, and `path_to_agent_string` (`src/context.rs`) further replaces every `\` with `/`. Windows fs APIs accept forward slashes natively, so the normalization is lossless; its purpose is to keep backslashes out of downstream markdown rendering (where `\h`, `\R` etc. get eaten as escapes), URI assembly (where `\\?\` URI-encodes to `%3F` and produces `file://%3F\D:\...`), and JSON-in-prompt expansion (where backslashes need double-escaping that downstream consumers routinely get wrong). Any new path-typed field on `Context` must go through `path_to_agent_string`; never call `.display().to_string()` directly when populating a `Context` field, and never call `std::fs::canonicalize` in code paths that feed `Context` (use `dunce::canonicalize` instead).
- TDD-mode changes must land synchronously on all four platforms' `new` and `execute` assets plus the `hotpot-execution` / `hotpot-review` subagents. Reference the shared protocol via `@.hotpot/prompts/tdd-protocol.md` (Claude/OpenCode) or `$HOTPOT_TDD_PROTOCOL_PROMPT` (Codex/Pi); never inline its content.
- Critical sections that rewrite `overview.jsonl` / `issues.jsonl` / `.hotpot/issue-candidates.jsonl` go through `src/lock.rs::with_file_lock`. The global candidates lock also protects one-time legacy migration from `.hotpot/workspaces/<username>/issue-candidates.jsonl`. Hard rule: **do not spawn subprocesses while holding the lock** — platform hooks may re-enter `hotpot` and deadlock.
- Bilingual comments follow English-first convention (per `AGENTS.md`): the English paragraph or sentence comes first, followed by a short Chinese supplement. Detailed technical explanations stay in English. This applies to all Rust (`//!`, `///`, `//`), TypeScript (`//`, `/* */`), and config file (`#`) comments.
- Outputs in code stay English (per `AGENTS.md`); natural-language responses follow `.hotpot/config.toml::language`. Resolution lives in Rust (`src/context.rs::resolve_language[_with_source]`, chain: env `HOTPOT_LANGUAGE` → `<root>/.hotpot/config.toml` top-level `language` → `"English"`). Every platform hook re-injects the value to defeat the "instruction once, drift forever" pattern: Claude `PreToolUse` + `SubagentStart`; Codex `PreToolUse` + `SessionStart`; OpenCode plugin `shell.env` plus orchestrator prompt text; Pi `pi.on("context", …)`. The shared `language_directive_message` helper (`src/commands/hook.rs`) is the single source of the human-readable one-liner, now bundled into the lightweight `prompt_context_message` helper for `PreToolUse` hooks; the long-form spec lives in `assets/prompts/output-language.md` and is referenced by every main workflow prompt via `@.hotpot/prompts/output-language.md` (Claude/OpenCode) or `$ROOT_DIR/.hotpot/prompts/output-language.md` (Codex/Pi). Structural anchors (CLI flags, JSON keys, `ACTIVE_CONFLICT:`, markdown section headings, `tdd: true|false`, kebab-case slugs) MUST stay English regardless of the configured language. OpenCode's full bootstrap JSON is assigned only to `shell.env` for Bash/tool processes; it is not expanded into model-visible context.
- `hotpot task create` only appends to `overview.jsonl`; it does **not** materialize the task `<time>-<title>.md` file. The `/hotpot:new` slash command is responsible for creating that file via the platform's create-file tool (Claude `Write`, OpenCode `write`, Codex `apply_patch *** Add File`, Pi `write`). Slash-command prompts must explicitly forbid a `Read`-before-`Write` probe of the task path (a missing file there is the normal post-create state, not an error). The CLI ensures `<workspace>/tasks/` exists as a non-fatal side effect of `task create`, but slash commands must not depend on that as a contract.
- When VuePress is enabled, `/hotpot:new` must decide that through the opt-in prompt file-existence gate and read `vuepress-style.md` before writing the task file. The first create-file operation must already be the final VuePress-formatted task; do not create ordinary Markdown first and then rewrite it for VuePress.
- Pi no longer uses prompt-template thin shells to back its slash commands. When adding a new Hotpot Pi slash command, register it via `pi.registerCommand` in `assets/platforms/pi/extensions/hotpot/index.ts` and assemble the user-voice message through the shared `buildPiCommandMessage` helper (delimiter block + framing + workflow-prompt absolute path + `@.hotpot/prompts/*` substitution table + Platform note + empty-arguments Exception). The handler's `@path` substitution table must be kept in sync with the shared workflow body it points at; whenever the shared `assets/prompts/hotpot-*.md` adds or removes a `@.hotpot/prompts/<name>.md` reference, mirror the change in the Pi handler's `atPathRefs`. Any newly deprecated Pi asset path must be added to `src/assets/platforms/pi.rs::cleanup_deprecated_pi_prompts` so `hotpot init` / `hotpot update` can erase it idempotently. `buildPiCommandMessage` body must put the user-input block (`<<< userInputLabel >>>`) FIRST, before the first-tool-call directive; this user-input-first ordering is a mitigation against the third documented Pi failure mode (attention loss on user-message body — see `docs/platforms/pi.md`). Each handler MUST also assign the closure-scoped `pendingWorkflow` field (in `assets/platforms/pi/extensions/hotpot/index.ts`) immediately before `pi.sendUserMessage`, so the next `pi.on("context", ...)` event injects a per-turn system reinforcement naming the workflow path, the user-input block markers, and the FORBIDDEN behaviors. Each handler MUST also arm `pendingFirstToolGuard` with the same command, workflow prompt path, and user-input label before `pi.sendUserMessage`; the existing single `pi.on("tool_call", ...)` hook must evaluate that runtime first-tool guard before bash env injection, return `{ block: true, reason }` for every non-workflow first call, keep the guard armed after blocked calls, and clear it only after the exact workflow `read`. This fourth Pi failure mode showed prompt-only defenses can fail, so the runtime guard is load-bearing. Adding a new Pi slash command requires extending all four: the handler registration, the `buildPiCommandMessage` invocation with the right `ideaBlockLabel` / `atPathRefs`, the `pendingWorkflow` assignment, AND the `pendingFirstToolGuard` assignment.
