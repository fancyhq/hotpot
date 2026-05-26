<!--
Hotpot shared workflow body: create a new task through brainstorming and planning.

Platform-specific thin shells (Claude / OpenCode / Codex / Pi) reference this file via
`@.hotpot/prompts/hotpot-new.md` or `$HOTPOT_NEW_PROMPT`. Do not paste platform-specific
syntax (slash-command name, subagent name, frontmatter) into this body — keep all such
overrides in the thin shells.
-->

You are creating a new Hotpot task. This command is manually triggered and must run a brainstorming and planning flow before creating the task record and task file.

## Goal

Turn the user's initial idea into a clear executable task file for a future execution agent. Do not create a separate plan file. The task file itself is the handoff document and must include both the task definition and the implementation plan: context, requirements, constraints, approved design, file map, step-by-step execution tasks, validation, and sub-agent execution instructions.

## Output Language

Apply the project language preference to every natural-language output produced by this command — brainstorming questions, design summaries, the entire body of the task `.md` you write, and the final per-command report. Structural anchors and machine-readable tokens (CLI flags, JSON keys, `ACTIVE_CONFLICT:`, markdown section headings like `## Task` / `## Plan` / `### Mode` / `### Execution Strategy`, the `tdd: true|false` and `git-worktree: true|false` literals, kebab-case filename slugs) MUST stay in English. The full rule lives in:

@.hotpot/prompts/output-language.md

Codex / Pi (no `@path` expansion): the thin shell's substitution table maps this to `$ROOT_DIR/.hotpot/prompts/output-language.md`. `Read` that path before proceeding.

## Command Usage

- The user-facing invocation pattern is supplied by the platform thin shell (e.g. `/hotpot:new`, `/hotpot-new`, or the Codex skill).
- If the user provided text after the command, treat it as the initial task idea.
- If no task idea was provided, ask one concise question to get the initial task idea.
- In normal usage, run `hotpot ...` commands.

## Required Flow

1. Classify the workspace (live vs stale actives) and, only if a live active exists, ask the user whether to switch or record-without-switching.
2. Run brainstorming to understand and shape the task.
3. Decide the execution strategy before planning, including at minimum whether this task must use an isolated `git-worktree`.
4. Convert the approved design and execution strategy into an implementation plan.
5. Create the Hotpot task record with a title, passing `--switch` / `--inactive` / no-flag based on step 1.
6. Resolve the task file path (`hotpot task active --path` for the default/switch cases; derive from `TaskInfo` JSON for `--inactive` cases).
7. **Create** the task file at the resolved path and write the finalized handoff content. The `.md` file does **not** exist after `hotpot task create` — only the new row in `overview.jsonl` exists. Use your platform's create-file tool (Claude `Write`, OpenCode `write`, Codex `apply_patch` with `*** Add File`, Pi `write`). **Do NOT `Read`, `Edit`, `ls`, or otherwise probe the path before writing.** A "File not found" from `Read` here is not actionable signal — it just means you haven't created the file yet. See "Writing The Task File" below for the full rule.
8. Report the created task title, task file path, implementation task count, TDD mode, `git-worktree` strategy, and whether the new task is the current active.

## Active Task Handling

Hotpot enforces "at most one `active=true && status=In Progress` task per user". Before brainstorming, classify the workspace and decide whether to ask the user about switching execution focus.

1. **List In-Progress tasks as JSONL** (each line is a full `TaskInfo`):

   ```bash
   hotpot task list --status "In Progress" --json
   ```

   Run:

   ```bash
   hotpot task list --status "In Progress" --json
   ```

2. **Inspect only `active=true` rows from this filtered output**. Because the CLI already filtered to `status="In Progress"`, every matching active row is a live active. Hotpot enforces ≤ 1; the orchestrator must prompt the user before clobbering. Do **not** separately inspect or classify `Done` / `Cancelled` rows here; `hotpot task create` silently clears stale active rows in every mode.

3. **If there are no active rows in the filtered output**: skip the user prompt. Continue to brainstorming and use the plain create command later (Step 4 of Required Flow):

   ```bash
   hotpot task create --title "<TITLE>"
   ```

4. **If there is ≥ 1 active row in the filtered output**: ask the user **exactly once** with clear options:

   > There is already an active In-Progress task: `<task_id> — <title>`.
   > Should the new task **switch** execution to itself (clears the existing active), or just be **recorded** without switching (new task starts inactive)?

   Offer these choices when the UI supports choices:

   - `Switch active to the new task` — recommended when the new task is the one you want to execute next. The CLI will clear the existing active atomically.
   - `Record without switching` — keep the existing active task as the execution focus; the new task is created with `active=false` and will NOT be picked up by `/hotpot:execute` until you `hotpot task resume --task-id <NEW_ID>` it later.
   - `Abort` — stop the new flow so the user can clean up state manually.

5. Remember the user's choice for **Step 4 of Required Flow** (which flag to pass to `hotpot task create`). Do NOT run `hotpot task stop --all` here — preemption is atomic inside `hotpot task create --switch`.

6. **Fallback** — if you forget to pass the right flag and the workspace had a live active, the CLI will bail with a message starting `ACTIVE_CONFLICT:` on stderr. Surface that message verbatim, ask the user again, and retry with the chosen flag.

## Brainstorming Flow

- Explore project context before proposing the task shape.
- Ask clarifying questions one at a time.
- Prefer multiple-choice questions when useful, but use open-ended questions when the user's intent is unclear.
- Understand purpose, constraints, success criteria, relevant files, impacted users, and expected validation.
- For overly broad requests, help decompose the work into a smaller first task before creating it.
- Propose 2-3 approaches with trade-offs when there are meaningful implementation options.
- Recommend one approach and explain why.
- Present the final task design and get user approval before creating the task.

Do not write code, scaffold features, or modify application files during this command. The only file this command should write is the new Hotpot task file, after the user approves the task design.

## TDD Mode Assessment

After the user approves the final design and **before** entering the Planning Flow, decide whether to run this task under Hotpot TDD mode. TDD mode forces the execution agent to follow Red → Green → Refactor for every Implementation Task, and the review agent will audit that the cycle was actually observed.

1. Self-assess the task against these adaptability signals:
   - Is the change mainly pure functions, algorithms, parsers, state machines, API protocols, data transforms, business rules, or other deterministic logic?
   - Can the new behavior be asserted by automated input → output tests?
   - Does the project already have a runnable test framework (`cargo test`, `pytest`, `vitest`, `jest`, etc.)?
   - Is the change NOT primarily UI polish, visual styling, copy/documentation edits, config tweaks, dependency bumps, or migration scripts?
2. Count the signals that are clearly true. 2 or more → recommend TDD mode; fewer → recommend skipping.
3. Ask the user **exactly once**, with the recommended option listed first. Do NOT default-pick.

   When recommending TDD:

   > This task looks suitable for TDD (logic / deterministic-test-friendly). Should the execution flow enforce Red → Green → Refactor for every Implementation Task?
   >
   > - `Enable TDD mode for this task` (recommended)
   > - `Skip TDD mode`
   > - `Abort and rethink the task`

   When recommending non-TDD:

   > This task looks better suited for the default review-driven flow (UI / docs / config / one-off work). Should the execution flow use the default mode?
   >
   > - `Skip TDD mode` (recommended)
   > - `Enable TDD mode for this task`
   > - `Abort and rethink the task`

4. Remember the user's choice for the Planning Flow and the Task File Content step:
   - `Enable TDD mode` → write `tdd: true` in `## Plan > ### Mode` AND build each `#### Task N` with Red / Green / Refactor sub-blocks (template below).
   - `Skip TDD mode` → write `tdd: false` in `## Plan > ### Mode` AND use the default flat checkbox layout.
5. If the user picks Abort, stop the command without calling `hotpot task create`. The user can rerun the new-task flow later.

## Execution Strategy Assessment

After the user approves the final design and before the Planning Flow writes the task file, decide the task's execution strategy. This decision must be complete before calling `hotpot task create`; `/hotpot:execute` will not ask the user to choose an execution strategy later.

At minimum, decide whether the task should use an isolated git worktree:

1. Self-assess the task against these signals:
   - Does the task involve risky edits, broad refactors, dependency changes, generated files, or experiments where isolating changes from the main checkout would reduce risk?
   - Does the user need to continue other work in the main checkout while this task is in progress?
   - Is the repository state clean enough and compatible with `hotpot worktree create`?
   - Is the task small, prompt-only, documentation-only, or otherwise safe to run directly in the current checkout?
2. Ask or confirm the `git-worktree` decision exactly once unless the user already stated it explicitly. List the recommended option first:

   When recommending a worktree:

   > This task looks safer in an isolated git worktree. Should the execution flow use `git-worktree: true`?
   >
   > - `Use git-worktree: true` (recommended)
   > - `Use git-worktree: false`
   > - `Abort and rethink the task`

   When recommending the current checkout:

   > This task looks safe to run in the current checkout. Should the execution flow use `git-worktree: false`?
   >
   > - `Use git-worktree: false` (recommended)
   > - `Use git-worktree: true`
   > - `Abort and rethink the task`

3. Remember the decision for the Planning Flow and Task File Content step:
   - `Use git-worktree: true` → write `git-worktree: true` in `## Plan > ### Execution Strategy`.
   - `Use git-worktree: false` → write `git-worktree: false` in `## Plan > ### Execution Strategy`.
4. If the user picks Abort, stop the command without calling `hotpot task create`.
5. Structural field names and values stay English: `### Execution Strategy`, `git-worktree`, `true`, and `false` must not be translated.

## Planning Flow

After the user approves the final design and after TDD / execution-strategy decisions are resolved, convert the approved design into an implementation plan inside the Hotpot task file. Do not create a separate plan file.

- Map the files that will likely be created, modified, or tested before defining implementation tasks.
- Break the work into bite-sized implementation tasks that can be executed and reviewed independently.
- Each implementation task must include exact file paths when they are known from project exploration.
- Each implementation task must include checkbox steps using `- [ ]` syntax.
- Include validation commands with expected results.
- Include `### Execution Strategy` directly under `## Plan`, after `### Mode`, with a machine-readable `- git-worktree: true|false` line plus any useful rationale.
- Include code snippets only when they are known and useful; do not invent APIs or exact code that has not been verified.
- If exact code cannot be safely determined during task creation, specify the exact files, functions, or patterns the execution agent must inspect before editing.
- Do not write placeholders such as `TBD`, `TODO`, `implement later`, `add tests`, or `handle edge cases` without concrete instructions.
- Keep the plan focused enough for one implementation session when possible. If the request is too broad, decompose it and create the first executable task only.
- **TDD mode (when the user chose `Enable TDD mode`)**: structure every `#### Task N` as three explicit sub-blocks — `##### Red` (R-prefixed checkbox steps that write a failing test and run it expecting failure), `##### Green` (G-prefixed checkbox steps that make the minimal change to pass and re-run the validation), `##### Refactor` (F-prefixed checkbox steps that either describe a concrete cleanup or write `no refactor needed`). The exact test command, exact implementation file, and exact verification command must be named in the steps; no placeholders. See the template in "Task File Content" below.

## Project Context Exploration

Before asking detailed questions, inspect enough context to avoid guessing:

- Read relevant documentation or existing task conventions if they exist.
- Search the codebase for relevant modules, commands, UI routes, tests, or configuration.
- Check recent structure or patterns that affect the task.
- Keep exploration focused on the proposed task; do not perform unrelated refactoring analysis.

For frontend or visual work, include stronger brainstorming support:

- Identify how to run the app, style guide, design system, component library, and test commands.
- If visual choices matter, offer to use a browser or local preview to inspect the UI.
- If the user agrees and the project supports it, start the development server or preview server needed to inspect styling.
- Use browser inspection or screenshots when a visual comparison is more useful than text.
- Capture relevant visual constraints and target states in the task file.

### Visual Companion Startup

The visual companion is built into the Hotpot binary. Start it only after the user agrees to use browser-based visual help.

Start the companion as a Hotpot daemon:

```bash
hotpot server start --project-dir "<project-root>" --daemon
```

Run:

```bash
hotpot server start --project-dir "<project-root>" --daemon
```

The command returns JSON like:

```json
{"type":"server-started","port":52341,"url":"http://localhost:52341","screen_dir":"<project-root>/.hotpot/brainstorm/<session>/content","state_dir":"<project-root>/.hotpot/brainstorm/<session>/state","session_dir":"<project-root>/.hotpot/brainstorm/<session>"}
```

After startup:

- Save `url`, `screen_dir`, `state_dir`, and `session_dir` in conversation context.
- Tell the user to open `url`.
- Write visual mockups, diagrams, or option screens as new `.html` files in `screen_dir`.
- Prefer HTML fragments; the server wraps them with the companion frame and interaction script.
- Never reuse filenames. Use semantic names like `layout.html`, `visual-style.html`, `architecture.html`, then `layout-v2.html` for iterations.
- On the next turn after user feedback, read `state_dir/events` if it exists and merge browser clicks with the user's terminal response.
- When returning to text-only discussion, write a fresh waiting screen so stale visual choices are not left on screen.

Directory meanings:

- `session_dir` is the root directory for this visual companion brainstorming session. It is used to stop the server later.
- `screen_dir` is where the agent writes temporary `.html` screens for the browser preview.
- `state_dir` is where Hotpot stores runtime state such as `server.pid`, `server-info`, and browser interaction events in `events`.
- These files are temporary brainstorming artifacts under `.hotpot/brainstorm/<session>` and are separate from the final task file.

Minimal visual option fragment:

```html
<h2>Which layout works better?</h2>
<p class="subtitle">Consider readability and visual hierarchy.</p>

<div class="options">
  <div class="option" data-choice="a" onclick="toggleSelect(this)">
    <div class="letter">A</div>
    <div class="content">
      <h3>Single Column</h3>
      <p>Focused reading flow with fewer navigation distractions.</p>
    </div>
  </div>
  <div class="option" data-choice="b" onclick="toggleSelect(this)">
    <div class="letter">B</div>
    <div class="content">
      <h3>Two Column</h3>
      <p>Sidebar navigation with faster access to related sections.</p>
    </div>
  </div>
</div>
```

If the visual companion URL is unreachable in a remote/containerized environment, restart it with an explicit host:

```bash
hotpot server start \
  --project-dir "<project-root>" \
  --host 0.0.0.0 \
  --url-host localhost \
  --daemon
```

If the companion is no longer needed, stop it with:

```bash
hotpot server stop --session-dir "<session-dir>"
```

Run:

```bash
hotpot server stop --session-dir "<session-dir>"
```

Use the app's own dev server separately when inspecting a real frontend. The companion server is for brainstorming mockups and visual choices; the app dev server is for viewing the actual product UI.

## Task Creation

After the user approves the final task design, derive a concise **kebab-case** title suitable for direct use as part of a filename. The title MUST satisfy:

- All lowercase ASCII letters, digits, and `-` only.
- Words separated by a single `-`; no spaces, no underscores, no punctuation, no leading/trailing `-`.
- 3-8 words is typical (e.g. `add-login-retry`, `fix-overview-jsonl-parse`, `refactor-task-storage-locking`).
- If the user's idea is in Chinese or another non-ASCII language, translate the gist into a short English kebab-case slug — do NOT romanize or pinyin. When the slug is non-obvious, confirm with the user before calling `task create`.

Examples:

- Good: `add-login-retry`, `improve-overview-jsonl-locking`
- Bad: `add login retry` (contains spaces), `Add-Login-Retry` (contains uppercase), `添加登录重试` (non-ASCII), `add_login_retry` (contains underscores)

Then run `hotpot task create` with the **flag chosen in "Active Task Handling" step 4**, passing the kebab-case title verbatim:

- **No live active was found** (or only stale actives existed):

  ```bash
  hotpot task create --title "<kebab-case-title>"
  ```

- **User chose "Switch active to the new task"**:

  ```bash
  hotpot task create --title "<kebab-case-title>" --switch
  ```

- **User chose "Record without switching"**:

  ```bash
  hotpot task create --title "<kebab-case-title>" --inactive
  ```

Concrete example:

```bash
hotpot task create --title "add-login-retry"
```

The CLI prints the new `TaskInfo` row as JSON on stdout. Verify `"status":"In Progress"` and that `"active"` matches what you asked for. If the command bails with a message starting `ACTIVE_CONFLICT:`, you passed no flag while a live active existed — go back to "Active Task Handling", surface the error verbatim to the user, ask which option, then retry with `--switch` or `--inactive`.

This command only creates task metadata in `overview.jsonl`; it does not write the task handoff content.

### Task File Naming Convention

Every Hotpot task file lives at `<workspace>/tasks/<YYYY-MM-DD>-<title>.md`. Both segments come directly from the JSON returned by `hotpot task create`:

- `<YYYY-MM-DD>` is the `"time"` field, for example `"time":"2026-05-15"`.
- `<title>` is the `"title"` field. Because you produced a kebab-case title in **Task Creation** above, the filename segment is already shell-safe. As a defensive safety net, the CLI also collapses any residual whitespace runs (single spaces, tabs, leading/trailing spaces) in `title` into a single `-` when building the filename — but you should never rely on that fallback. Always produce a clean kebab-case title up front.

Example: `{"time":"2026-05-15","title":"add-login-retry"}` → `2026-05-15-add-login-retry.md`.

Capture both fields from the `task create` output before resolving the path.

### Resolve The Task File Path

Choose ONE of these two paths depending on the flag passed to `task create`:

- **No flag or `--switch`** — the new task is now the live active. Resolve its file via:

  ```bash
  hotpot task active --path
  ```

  Run:

  ```bash
  hotpot task active --path
  ```

- **`--inactive`** — the new task is `active=false`, so `hotpot task active --path` still resolves to the **existing** live active task's path, NOT the new one. Construct the new task's path yourself from the captured JSON using the convention above (`<workspace>/tasks/<time>-<title>.md`), and tell the user that `/hotpot:execute` will keep targeting the previous active until they `hotpot task resume --task-id <NEW_ID>` later.

Use the resolved path as the task file to write. Do not guess the task file path.

### Writing The Task File

After resolving the path, **create** the file with your platform's create-file tool and write the full handoff content in one shot:

- Claude Code: `Write` tool. Do NOT `Read` the path first — the file does not yet exist, and a `Read` failure is not actionable signal here.
- OpenCode: `write` tool.
- Codex: `apply_patch` with an `*** Add File: <resolved-path>` header (not `*** Update File`).
- Pi: `write` tool.

Common pitfalls to avoid:

- **Do not `Read` the resolved path before writing.** `hotpot task create` only appends a row to `overview.jsonl`; it does **not** stub the `.md`. The parent directory `.hotpot/workspaces/<user>/tasks/` may also be missing for a brand-new user workspace — the create-file tool will materialize parents as needed (and the CLI itself ensures the tasks directory exists at create time, but you must not rely on `ls` to verify).
- **Do not second-guess the resolved path when `Read` reports "File not found".** That error simply means "you haven't created it yet". Proceed to write at the path you already resolved from `hotpot task active --path` (or built from the `TaskInfo` JSON for `--inactive`).
- **Do not retry with a different path** if a real OS-level write error occurs (permission denied, disk full, ENOENT for the project root). Surface the error verbatim and stop; silently rerouting the write would lose the task file or land it in the wrong workspace.

## Task File Content

Write a complete handoff document to the active task file. The whole file should be safe to pass directly to a sub-agent for execution. Use this structure and scale detail to task complexity:

```markdown
# <Task Title>

## Task

### Summary

<One concise paragraph describing the task and why it matters.>

### User Request

<Preserve the user's original request and any important follow-up decisions.>

### Approved Design

<The final design the user approved after brainstorming. Include architecture, components, data flow, error handling, and testing strategy when relevant.>

### Alternatives Considered

- <Approach A, trade-off, and why it was not chosen.>
- <Approach B, trade-off, and why it was not chosen.>
- <Recommended approach and why it was approved.>

### Requirements

- <Concrete requirement 1>
- <Concrete requirement 2>
- <Concrete requirement 3>

### Non-Goals

- <Explicitly excluded work, if any.>

### Project Context

<Relevant project structure, files, commands, current behavior, UI observations, constraints, and assumptions discovered during brainstorming.>

## Plan

### Mode

- tdd: <true or false>   <!-- machine-readable; the execute flow parses this line. -->

### Execution Strategy

- git-worktree: <true or false>   <!-- machine-readable; the execute flow parses this line. -->
- rationale: <Why this task should or should not run in an isolated worktree.>

### File Map

- Modify: `<exact/path>` - <why this file likely changes.>
- Create: `<exact/path>` - <responsibility of the new file.>
- Test: `<exact/path>` - <what behavior this test validates.>

### Implementation Tasks

> Choose ONE of the two layouts below based on the `### Mode` value above. Do not mix them.

#### Layout when `tdd: false` (default review-driven flow)

```markdown
#### Task 1: <Small Executable Unit>

**Files:**

- Modify: `<exact/path>`
- Test: `<exact/path>`

- [ ] Step 1: <Exact action or inspection to perform.>
- [ ] Step 2: Run `<exact command>` and expect <specific result>.
- [ ] Step 3: <Exact implementation action.>
- [ ] Step 4: Run `<exact command>` and expect <specific result>.

#### Task 2: <Small Executable Unit>

**Files:**

- Modify: `<exact/path>`
- Test: `<exact/path>`

- [ ] Step 1: <Exact action or inspection to perform.>
- [ ] Step 2: <Exact implementation action.>
- [ ] Step 3: Run `<exact command>` and expect <specific result>.
```

#### Layout when `tdd: true` (Red → Green → Refactor)

```markdown
#### Task 1: <Small Executable Unit>

**Files:**

- Test: `<exact/test/path>`
- Modify: `<exact/src/path>`

##### Red

- [ ] R1: In `<exact/test/path>`, add or modify test `<exact_test_name>` covering <specific behavior>.
- [ ] R2: Run `<exact test command, e.g. cargo test <exact_test_name>>`; **expect failure** with an assertion/behavioral error tied to the new test. Capture the failing test name and assertion line.

##### Green

- [ ] G1: In `<exact/src/path>`, make the smallest change to make `<exact_test_name>` pass.
- [ ] G2: Run `<exact test command>`; **expect pass**. Capture the pass summary.
- [ ] G3: Run `<full validation command, e.g. cargo test>`; **expect no other regressions**. Capture the validation summary.

##### Refactor

- [ ] F1: Inspect for naming/duplication/abstraction issues; either describe the concrete cleanup action OR write `no refactor needed`.
- [ ] F2: If a refactor happened, re-run `<exact test command>` and `<full validation command>`; **expect pass**. Otherwise mark `skipped (no refactor)`.
```

### Validation

- `<exact command>` - <expected result>
- <Manual QA or visual validation, if relevant.>

### Risks and Watchouts

- <Concrete risk, ambiguity, or compatibility concern.>
- <Constraint the execution agent must preserve.>

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task.
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff.
- Run the validation commands before reporting completion.

## Open Questions

- <Only include unresolved questions that remain after approval. Omit this section if none.>
```

The task file must be useful on its own for an execution sub-agent. Include enough context that the next agent does not need to repeat the whole brainstorming conversation. It must explain both what to build and how to execute the work.

## Constraints

- Do not create a separate plan file.
- Do not create the Hotpot task before the user approves the final task design.
- Do not overwrite a task file unless its path came from `hotpot task active --path` after creating or activating the intended task.
- Do not proceed if active task ambiguity could cause writing to the wrong task file.
- Do not use `hotpot task create` without a `--title` value.
- The `--title` value passed to `hotpot task create` MUST be kebab-case (lowercase ASCII letters/digits + `-`, no spaces, no underscores, no punctuation). See **Task Creation** for the full rule and examples.
- Preserve user constraints and acceptance criteria exactly.
- Keep the final task focused enough to execute in one implementation session when possible.
- The task file must include both `## Task` and `## Plan` sections.
- The `## Plan` section must include checkbox implementation steps and validation commands.
- Do not create a vague task file that only describes the desired outcome without execution steps.
- Do NOT omit the `## Plan > ### Mode` block. Write `tdd: true` or `tdd: false` explicitly so the execute flow can detect the mode unambiguously. A missing block is treated as `tdd: false`, but writing it explicitly avoids surprise.
- Do NOT omit the `## Plan > ### Execution Strategy` block. Write `git-worktree: true` or `git-worktree: false` explicitly so the execute flow can decide worktree behavior without asking the user. A missing block, missing `git-worktree` line, or any value other than `true|false` causes `/hotpot:execute` to stop and ask the user to re-run `/hotpot:new` or revise the task file.
- Resolve all worktree-related questions before calling `hotpot task create`; do not leave worktree decisions for `/hotpot:execute`.
- When `tdd: true`, every `#### Task N` MUST follow the Red / Green / Refactor template with concrete test commands, exact failing-test names, exact implementation files, and exact verification commands. Do not write placeholders.
- Do NOT `Read`, `Edit`, `ls`, or otherwise probe the task `.md` path before writing it. The `.md` is created by this command's write step, not by `hotpot task create`. A pre-write `Read` will return "File not found", waste a turn, and tempt you to second-guess the path.

## Optional: VuePress Integration

Do NOT rely on any VuePress-related env-var to decide this branch — such variables are not propagated into AI conversation context on every platform (OpenCode's plugin only injects them into shell subprocesses via `shell.env`, so the AI never sees them). Instead, use a **file-existence gate** that every platform can observe identically through its Bash tool: probe whether the two VuePress opt-in prompt assets are on disk. They are kept atomically in sync with `[vuepress] enabled = true` by `hotpot vuepress install` / `uninstall`, so "both files exist" is the ground truth for "VuePress is enabled".

Run this probe (via your Bash tool) before deciding whether to follow the VuePress closing flow:

```bash
[ -f "$ROOT_DIR/.hotpot/prompts/vuepress.md" ] && \
  [ -f "$ROOT_DIR/.hotpot/prompts/vuepress-style.md" ] && \
  echo enabled || echo disabled
```

If the probe prints `enabled`:

1. **BEFORE** you write the task `.md`, use your Read tool to load `.hotpot/prompts/vuepress-style.md` and apply its markdown conventions while writing. The task file must still satisfy every Hotpot structural requirement above (`## Task` / `## Plan` / `## Execution Instructions` / kebab-case slug / etc.) — VuePress conventions are layered ON TOP, not in place of, those requirements.
2. **AFTER** you have finished writing the task `.md`, use your Read tool to load `.hotpot/prompts/vuepress.md` and follow its closing-flow instructions. Those instructions OVERRIDE the default closing message in the `## Final Response` section below: instead of just emitting the file path, they walk you through a yes/no prompt → `hotpot vuepress start` → URL emission. Treat `vuepress.md` as the authoritative closing protocol whenever VuePress is enabled.

If the probe prints `disabled`, ignore this entire section. Do NOT Read `vuepress.md` or `vuepress-style.md` — they are not on disk in disabled projects, and a blind Read would return "File not found" and pollute your context. Use the default closing message from `## Final Response` below.

## Final Response

After writing the task file, respond with:

- Created task title.
- Task file path.
- Number of implementation tasks captured in the `Plan` section.
- TDD mode: `enabled` or `disabled` (matches the `### Mode` block).
- Execution strategy: `git-worktree: true` or `git-worktree: false`.
- A short summary of what was captured.
- Any remaining open questions, if present.
