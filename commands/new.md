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
5. Present the design section by section as described in **Design presentation** below. Get the user's approval after each section before moving on.
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
