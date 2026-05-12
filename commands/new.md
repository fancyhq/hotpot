---
description: Create a Hotpot task through brainstorming
---

You are creating a new Hotpot task. This command is manually triggered with `/hotpot:new` and must run a brainstorming and planning flow before creating the task record and task file.

## Goal

Turn the user's initial idea into a clear executable task file for a future execution agent. Do not create a separate plan file. The task file itself is the handoff document and must include both the task definition and the implementation plan: context, requirements, constraints, approved design, file map, step-by-step execution tasks, validation, and sub-agent execution instructions.

## Command Usage

- Use `/hotpot:new` to start creating a task.
- If the user provided text after the command, treat it as the initial task idea.
- If no task idea was provided, ask one concise question to get the initial task idea.
- In normal usage, run `hotpot ...` commands.
- When testing or running inside this repository without an installed `hotpot` binary, use `cargo run -- ...` instead of `hotpot ...`.

## Required Flow

1. Check active task state.
2. Run brainstorming to understand and shape the task.
3. Convert the approved design into an implementation plan.
4. Create the Hotpot task record with a title.
5. Resolve the active task file path.
6. Write the finalized task handoff content into the active task file.
7. Report the created task title, task file path, and implementation task count.

## Active Task Handling

Only one task may be actively executing at a time. The `active` execution marker indicates whether a task is currently being executed; stopping active tasks only clears execution state and does not change the task's lifecycle status.

Before brainstorming, run:

```bash
hotpot task active --count
```

If testing in this repository, run:

```bash
cargo run -- task active --count
```

If the active count is greater than `0`, ask the user whether they want to switch execution to this new task.

Ask exactly one question with a clear default recommendation:

> There is already an active task. Should I stop all currently active execution markers so the new task can become the active task?

Offer these choices if the UI supports choices:

- `Stop existing active tasks` - Recommended when the new task should be executed now.
- `Keep current active task` - Use when the new task should be recorded but not become the current execution focus.

If the user chooses to stop existing active tasks, run:

```bash
hotpot task stop --all
```

If testing in this repository, run:

```bash
cargo run -- task stop --all
```

If the user chooses to keep the current active task, continue brainstorming and task creation, but make it clear that active-path resolution may still point at the already active task. If that would prevent writing the new task file correctly, stop and explain the blocker instead of overwriting the wrong file.

## Brainstorming Flow

Use the brainstorming behavior from the `brainstorming` skill, adapted for manual task creation:

- Explore project context before proposing the task shape.
- Ask clarifying questions one at a time.
- Prefer multiple-choice questions when useful, but use open-ended questions when the user's intent is unclear.
- Understand purpose, constraints, success criteria, relevant files, impacted users, and expected validation.
- For overly broad requests, help decompose the work into a smaller first task before creating it.
- Propose 2-3 approaches with trade-offs when there are meaningful implementation options.
- Recommend one approach and explain why.
- Present the final task design and get user approval before creating the task.

Do not write code, scaffold features, or modify application files during this command. The only file this command should write is the new Hotpot task file, after the user approves the task design.

## Planning Flow

After the user approves the final design, convert the approved design into an implementation plan inside the Hotpot task file. Do not create a separate plan file.

Use the planning quality bar from the `writing-plans` skill, adapted for Hotpot:

- Map the files that will likely be created, modified, or tested before defining implementation tasks.
- Break the work into bite-sized implementation tasks that can be executed and reviewed independently.
- Each implementation task must include exact file paths when they are known from project exploration.
- Each implementation task must include checkbox steps using `- [ ]` syntax.
- Include validation commands with expected results.
- Include code snippets only when they are known and useful; do not invent APIs or exact code that has not been verified.
- If exact code cannot be safely determined during task creation, specify the exact files, functions, or patterns the execution agent must inspect before editing.
- Do not write placeholders such as `TBD`, `TODO`, `implement later`, `add tests`, or `handle edge cases` without concrete instructions.
- Keep the plan focused enough for one implementation session when possible. If the request is too broad, decompose it and create the first executable task only.

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

If testing in this repository, run:

```bash
cargo run -- server start --project-dir "<project-root>" --daemon
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

If testing in this repository, run:

```bash
cargo run -- server stop --session-dir "<session-dir>"
```

Use the app's own dev server separately when inspecting a real frontend. The companion server is for brainstorming mockups and visual choices; the app dev server is for viewing the actual product UI.

## Task Creation

After the user approves the final task design, derive a concise title. Then run:

```bash
hotpot task create --title "<task title>"
```

If testing in this repository, run:

```bash
cargo run -- task create --title "<task title>"
```

This command only creates task metadata in `overview.jsonl`; it does not write the task handoff content.

Then resolve the task file path by running:

```bash
hotpot task active --path
```

If testing in this repository, run:

```bash
cargo run -- task active --path
```

Use the returned path as the task file to write. Do not guess the task file path.

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

### File Map

- Modify: `<exact/path>` - <why this file likely changes.>
- Create: `<exact/path>` - <responsibility of the new file.>
- Test: `<exact/path>` - <what behavior this test validates.>

### Implementation Tasks

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
- Preserve user constraints and acceptance criteria exactly.
- Keep the final task focused enough to execute in one implementation session when possible.
- The task file must include both `## Task` and `## Plan` sections.
- The `## Plan` section must include checkbox implementation steps and validation commands.
- Do not create a vague task file that only describes the desired outcome without execution steps.

## Final Response

After writing the task file, respond with:

- Created task title.
- Task file path.
- Number of implementation tasks captured in the `Plan` section.
- A short summary of what was captured.
- Any remaining open questions, if present.
