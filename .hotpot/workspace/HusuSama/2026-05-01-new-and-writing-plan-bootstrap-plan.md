# `/new` Command and `writing-plan` Skill Bootstrap — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Author the four markdown files (two English templates + two Claude-Code working copies) that implement the `/new` slash command and the `writing-plan` skill described in the spec.

**Architecture:** Each component is authored once as an English template at `commands/new.md` or `skills/writing-plan.md` (flat, no subfolder for skills), then mirrored byte-for-byte into `.claude/commands/` and `.claude/skills/` so Claude Code picks them up immediately. No build step. All behavior — HARD-GATE during brainstorming, save-path conventions, self-review — is encoded in the markdown body. Frontmatter stays minimal (`name`, `description`) for cross-agent compatibility; no `allowed-tools` whitelist.

**Tech Stack:** Markdown, Bash (`mkdir`, `cp`, `diff`, `git`), Git.

---

## File Structure

| Path | Action | Responsibility |
|---|---|---|
| `commands/new.md` | Create | English template source for the `/new` slash command. Encodes the brainstorming flow + HARD-GATE + handoff to `writing-plan`. |
| `skills/writing-plan.md` | Create | English template source for the `writing-plan` skill. Encodes the procedure for writing `.hotpot/workspace/<username>/YYYY-MM-DD-<topic>.md`. |
| `.claude/commands/new.md` | Create | Byte-for-byte copy of `commands/new.md`. Discovered by Claude Code at session start so `/new` works immediately. |
| `.claude/skills/writing-plan.md` | Create | Byte-for-byte copy of `skills/writing-plan.md`. Read by AI when `/new` hands off after design approval. |

No source code is touched. No tests written (the work is content authoring; verification is read-back grep + `diff` between source and copy).

---

## Task 1: Author `commands/new.md` template

**Files:**
- Create: `commands/new.md`

- [ ] **Step 1: Write the file with full content**

Create `commands/new.md` with this exact content:

````markdown
---
description: Start a new task — guided brainstorming, then save the spec+plan to .hotpot/workspace/<your-name>/.
---

# `/new` — Start a New Task

You are starting a new task workflow. The user invoked `/new` to enter guided brainstorming. Your role here is to help the user turn a rough idea into a concrete, approved design — and only after that approval, hand off to the `writing-plan` skill which writes the document.

## HARD-GATE — Read this first

During the brainstorming phase (everything before user approval) you MUST NOT:

- Write or edit any file (no `Write`, `Edit`, `NotebookEdit`).
- Run any `Bash` command that mutates disk (no `mkdir`, `rm`, `touch`, `git commit`, `git add`, file redirects, etc.).
- Scaffold a project, create directories, or take any implementation action.
- Invoke any other skill (no `frontend-design`, no `mcp-builder`, no `writing-plans`).

You MAY use read-only tools to understand context: `Read`, `Glob`, `Grep`, and read-only `Bash` such as `git log`, `git status`, `git diff`, `ls`.

The single allowed terminal action is invoking the `writing-plan` skill — and only after the user has explicitly approved the design.

## What this command does

1. Acknowledge the new task. Briefly restate the HARD-GATE so expectations are clear.
2. Explore the project context (read-only).
3. Ask clarifying questions, one at a time, multiple-choice when the options are clear. Cover purpose, constraints, success criteria.
4. Propose 2–3 approaches with tradeoffs. Lead with your recommendation.
5. Present the design in sections (architecture, file structure, data/control flow, error handling, testing). Get the user's approval after each section before moving on.
6. Once the user has explicitly approved every section: invoke the `writing-plan` skill.

## Conversation language

Match the user's language. If they write in Chinese, reply in Chinese. If they write in English, reply in English. The output document produced by `writing-plan` will follow the same language.

## Topic seed (optional)

If the user typed `/new <a short phrase>`, treat the phrase as a topic seed. Use it to ground your first questions. If they typed only `/new`, ask "What's the task you want to start?" first.

## Question style

- One question per message.
- Multiple choice when options are clear (label them A / B / C); open-ended only when truly necessary.
- Don't pile up unrelated questions. If a topic needs more depth, follow up after the user answers.

## Approach exploration

When you understand the goal, propose 2–3 distinct approaches:

- For each: 1–2 sentences on what it does, then trade-offs.
- Lead with your recommendation and why.

## Design presentation

Present the design one section at a time, in this order:

1. Architecture — components, boundaries, who-talks-to-whom.
2. File structure — what files will be created or modified, each with a one-sentence responsibility.
3. Data / control flow.
4. Error handling and edge cases.
5. Testing strategy.

After each section ask "Does this look right?" — wait for confirmation before continuing.

Scale section length to complexity: a few sentences for simple things, up to 200–300 words when nuanced.

## Approval and handoff

When the user has approved every section, do not write any file yourself. Instead say:

> "Design approved. Handing off to the `writing-plan` skill to produce the document."

Then invoke the `writing-plan` skill. On Claude Code, this is done by reading and following `.claude/skills/writing-plan.md`. On other agents, the skill lives at the same filename inside the agent's skills directory.

## Anti-patterns

- "This is too simple to need a design." — wrong. Even a one-file utility goes through this. The design can be brief, but it must be presented and approved.
- Implementing during the dialog. — forbidden by the HARD-GATE.
- Asking five questions in one message. — one at a time.
- Writing the document yourself. — that is the `writing-plan` skill's job.
````

- [ ] **Step 2: Verify the file content has the required markers**

Run:

```bash
grep -c "HARD-GATE" commands/new.md
grep -c "writing-plan" commands/new.md
grep -c "One question per message" commands/new.md
test -f commands/new.md && echo OK
```

Expected output: each `grep -c` returns at least `1`; final line prints `OK`.

- [ ] **Step 3: No commit yet** — Task 2 will mirror this file and commit both together to keep source and copy in lockstep.

---

## Task 2: Mirror `/new` command into `.claude/commands/`

**Files:**
- Create: `.claude/commands/new.md`

- [ ] **Step 1: Ensure target directory exists**

Run:

```bash
mkdir -p .claude/commands
```

- [ ] **Step 2: Copy the source to the Claude Code working location**

Run:

```bash
cp commands/new.md .claude/commands/new.md
```

- [ ] **Step 3: Verify byte-equality**

Run:

```bash
diff commands/new.md .claude/commands/new.md && echo IDENTICAL
```

Expected output: `IDENTICAL` (and no diff output above it).

- [ ] **Step 4: Commit both files together**

Run:

```bash
git add commands/new.md .claude/commands/new.md
git commit -m "$(cat <<'EOF'
feat(commands): add /new command for guided brainstorming

Source template at commands/new.md plus a byte-for-byte working copy at
.claude/commands/new.md so Claude Code picks it up at session start.

Behavior is encoded in the document body (HARD-GATE: brainstorming-only,
no file writes; terminal handoff to writing-plan skill). No allowed-tools
frontmatter — kept minimal for cross-agent compatibility.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected output: a new commit listed by `git log -1 --oneline` containing the two new files.

---

## Task 3: Author `skills/writing-plan.md` template

**Files:**
- Create: `skills/writing-plan.md`

- [ ] **Step 1: Write the file with full content**

Create `skills/writing-plan.md` with this exact content:

````markdown
---
name: writing-plan
description: Internal handoff target invoked by /new after design approval. Writes the combined design+plan markdown to .hotpot/workspace/<git user.name>/. Do not invoke directly.
---

# `writing-plan` — Persist an Approved Design

You were invoked by the `/new` command after the user approved a design through brainstorming. Your job: write a single markdown file to the developer's hotpot workspace, combining the design and the implementation plan.

You will write exactly one file. You will not modify other parts of the repository as part of this skill (commits and other follow-ups are the user's call).

## Output file

Path:

```
.hotpot/workspace/<username>/<YYYY-MM-DD>-<topic>.md
```

- `<username>` — value of `git config user.name`. Local config takes precedence over global.
- `<YYYY-MM-DD>` — today's date.
- `<topic>` — kebab-case slug derived from the topic agreed during brainstorming.

## Output language

Write the file body in the language the user is currently conversing in. The skill template itself is English; the produced document follows the user — Chinese, English, etc. Section headings, fixed labels, code blocks, and identifiers (e.g. `Goal`, `Constraints`) stay in their natural language; explanatory body text follows the user.

## Output structure

The file must contain these four sections in this order:

```markdown
# <Title>

> Generated by hotpot writing-plan skill on <YYYY-MM-DD> by <username>

## 1. Design

- Goal — one sentence.
- Constraints — bullet list.
- Success Criteria — bullet list of testable conditions.
- Architecture & Components — short prose; include a small diagram only if it adds clarity.
- Data flow / Error handling / Testing strategy — bullet lists.

## 2. File Structure

A table or list: which files this plan will Create / Modify / Delete, with a one-sentence responsibility for each.

## 3. Implementation Steps

Bite-sized tasks. Each task names its files and consists of `- [ ]` checkbox steps. Each step is 2–5 minutes of work and contains the actual code or command an engineer would execute. Forbidden: "TBD", "TODO", "implement later", "similar to Task N", or steps that describe what to do without showing how.

## 4. Acceptance

A bullet list: what conditions, expressed as commands or manual checks, indicate the plan is fully implemented.
```

## Procedure

Do these steps in order. Stop and surface to the user if any step fails.

### Step 1 — Resolve username

Run:

```bash
git config user.name
```

If the output is empty, tell the user:

> "Cannot determine your name from git. Please run `git config --global user.name "<your-name>"` then re-run `/new`. No file written."

Stop. Do not write anything.

### Step 2 — Resolve date

Use today's date in `YYYY-MM-DD` format. Read it from the conversation context if a `currentDate` is provided; otherwise run:

```bash
date +%Y-%m-%d
```

On Windows shells where `date` differs, use:

```bash
git log -1 --format=%cd --date=format:%Y-%m-%d
```

as a portable fallback.

### Step 3 — Build the topic slug

From the topic agreed in brainstorming:

1. Lowercase.
2. Replace any whitespace runs with `-`.
3. Strip every character that is not `[a-z0-9-]`.
4. Collapse repeated `-` into one. Trim leading and trailing `-`.

Example: `"My New Feature!"` → `my-new-feature`.

### Step 4 — Ensure the workspace directory exists

Run:

```bash
mkdir -p .hotpot/workspace/<username>/
```

### Step 5 — Conflict check

Compute the candidate filename `<YYYY-MM-DD>-<topic>.md`. Check whether `.hotpot/workspace/<username>/<filename>` already exists.

If it does, ask the user:

> "A file already exists at `<full-path>`. Choose:
> A. Overwrite it.
> B. Change the topic and re-resolve.
> C. Append a numeric suffix (`-2`, `-3`, …) automatically."

Wait for the user's choice. For C, find the smallest integer N ≥ 2 such that `<YYYY-MM-DD>-<topic>-<N>.md` does not exist. Never silently overwrite.

### Step 6 — Write the file

Use the structure from "Output structure" above. Fill every section with concrete content from the brainstorming transcript:

- **Goal** — paraphrase the agreed goal in one sentence.
- **Constraints / Success Criteria** — extract from the dialog.
- **Architecture** — from the design sections the user approved.
- **File Structure** — from the design's file plan.
- **Implementation Steps** — decompose into tasks of 2–5 minutes per step. Embed actual code/commands in code blocks.
- **Acceptance** — list verifiable conditions.

### Step 7 — Self-review

After writing the file, re-read it and check:

1. **Placeholders.** Search for `TBD`, `TODO`, `fill in later`, `similar to Task`. Fix every hit.
2. **Internal consistency.** Names introduced in Task 1 (functions, types, files) must match exactly when referenced in later tasks.
3. **File coverage.** Every file listed in section 2 must be touched by at least one task in section 3.
4. **Scope.** If the plan covers multiple independent subsystems, tell the user it should be split into separate plans, and offer to do that.

Fix issues inline. No need to re-review after fixes.

### Step 8 — Report and stop

Tell the user:

> "Plan written to `.hotpot/workspace/<username>/<filename>`. Ready when you want to start implementing."

Do not invoke any other skill. Do not start implementing. Do not commit. Those are the user's call.

## Tools you may use

- `Bash` — read-only (`git config`, `date`, `ls`) plus `mkdir -p` for the workspace directory.
- `Read` — to verify the file after writing.
- `Write` — for the output file.

Do not use `Edit`, `NotebookEdit`, or any other writing tool against files outside the single output file.

## What this skill does NOT do

- Does not auto-trigger. Only invoked from `/new`.
- Does not commit. Git commit is the user's choice.
- Does not modify any file other than the single output file.
- Does not invoke other skills.
- Does not start implementation.
````

- [ ] **Step 2: Verify the file content has the required markers**

Run:

```bash
grep -c "name: writing-plan" skills/writing-plan.md
grep -c ".hotpot/workspace" skills/writing-plan.md
grep -c "Step 1 — Resolve username" skills/writing-plan.md
grep -c "Step 8 — Report and stop" skills/writing-plan.md
test -f skills/writing-plan.md && echo OK
```

Expected output: each `grep -c` returns `1` (or more); final line prints `OK`.

- [ ] **Step 3: No commit yet** — Task 4 mirrors this file and commits both together.

---

## Task 4: Mirror `writing-plan` skill into `.claude/skills/`

**Files:**
- Create: `.claude/skills/writing-plan.md`

- [ ] **Step 1: Ensure target directory exists**

Run:

```bash
mkdir -p .claude/skills
```

- [ ] **Step 2: Copy the source to the Claude Code working location**

Run:

```bash
cp skills/writing-plan.md .claude/skills/writing-plan.md
```

- [ ] **Step 3: Verify byte-equality**

Run:

```bash
diff skills/writing-plan.md .claude/skills/writing-plan.md && echo IDENTICAL
```

Expected output: `IDENTICAL`.

- [ ] **Step 4: Commit both files together**

Run:

```bash
git add skills/writing-plan.md .claude/skills/writing-plan.md
git commit -m "$(cat <<'EOF'
feat(skills): add writing-plan skill for /new workflow

Source template at skills/writing-plan.md plus a byte-for-byte working
copy at .claude/skills/writing-plan.md. Flat layout (no subfolder) per
project convention.

Skill is intentionally non-auto-triggering — description marks it as an
internal handoff target invoked only by /new after design approval.
Procedure encoded in the document: resolve git user.name, kebab-case
the topic, save to .hotpot/workspace/<username>/YYYY-MM-DD-<topic>.md,
self-review, report and stop. Never silently overwrites.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

Expected output: a new commit listed by `git log -1 --oneline` containing the two new files.

---

## Task 5: Acceptance verification

**Files:** none (read-only verification).

- [ ] **Step 1: All four files exist**

Run:

```bash
ls -la commands/new.md skills/writing-plan.md .claude/commands/new.md .claude/skills/writing-plan.md
```

Expected output: four lines, each showing a non-empty file. No `No such file` errors.

- [ ] **Step 2: Source and copy pairs are identical**

Run:

```bash
diff commands/new.md .claude/commands/new.md && echo PAIR-1-OK
diff skills/writing-plan.md .claude/skills/writing-plan.md && echo PAIR-2-OK
```

Expected output: `PAIR-1-OK` and `PAIR-2-OK` with no `diff` lines between them.

- [ ] **Step 3: Walk the spec acceptance checklist**

Open `.hotpot/workspace/HusuSama/2026-05-01-new-and-writing-plan-bootstrap-design.md` and confirm each item in section 7:

- 4 files exist with identical pairs — verified by Steps 1 & 2 above.
- Source files in English — confirm by reading `commands/new.md` and `skills/writing-plan.md`.
- `/new` command document carries the HARD-GATE language — `grep "HARD-GATE" commands/new.md` returns 1+.
- `/new` document hands off to `writing-plan` skill — `grep "writing-plan" commands/new.md` returns 1+.
- `writing-plan` skill writes to `.hotpot/workspace/<username>/YYYY-MM-DD-<topic>.md` — `grep ".hotpot/workspace" skills/writing-plan.md` returns 1+.
- Conflict handling exists — `grep "Never silently overwrite" skills/writing-plan.md` returns 1.
- 4-section output structure — `grep -E "^## (1\\. Design|2\\. File Structure|3\\. Implementation Steps|4\\. Acceptance)" skills/writing-plan.md` returns 4 lines.
- Output language follows user — `grep "language the user is currently conversing in" skills/writing-plan.md` returns 1.
- Empty `git config user.name` is rejected — `grep "Cannot determine your name from git" skills/writing-plan.md` returns 1.

If any check fails, fix the corresponding template, mirror to `.claude/`, verify equality with `diff`, then redo this step.

- [ ] **Step 4: Manual smoke test (record only, do not run from this plan)**

Note these manual smoke-test steps for the user to run later in a fresh Claude Code session in this project:

```
/new build a tiny status banner CLI
```

Expected behavior:
- Claude restates the HARD-GATE briefly.
- Claude asks one clarifying question (multiple-choice preferred), e.g. about the output target or styling.
- Claude does NOT write any file during the dialog.
- After "approve" on the design, Claude reads `.claude/skills/writing-plan.md` and produces a file at `.hotpot/workspace/HusuSama/<today>-status-banner-cli.md`.
- The file contains the four sections (`Design`, `File Structure`, `Implementation Steps`, `Acceptance`) in the user's conversation language.
- If a same-named file already exists, Claude prompts the user to choose A / B / C instead of overwriting.

This is recorded as the post-implementation acceptance test; running it is the user's call.

- [ ] **Step 5: No commit** — verification step only.

---

## Acceptance

The plan is fully implemented when:

- `commands/new.md`, `skills/writing-plan.md`, `.claude/commands/new.md`, `.claude/skills/writing-plan.md` all exist.
- Source ↔ `.claude/` copy diffs are empty for both pairs.
- All `grep` checks in Task 5 Step 3 succeed.
- Two commits are landed on `main` (one per pair) with messages following the templates above.
- Manual smoke test (Task 5 Step 4) is documented for the user to run on demand.
