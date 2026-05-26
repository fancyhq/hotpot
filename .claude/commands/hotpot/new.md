---
description: Create a Hotpot task through brainstorming
---

<!-- Hotpot Claude command asset for creating a new task. -->

You are running the Hotpot new-task workflow. The user-facing invocation is `/hotpot:new`.

The full workflow is defined at:

@.hotpot/prompts/hotpot-new.md

Follow that workflow end-to-end. Claude expands `@path` tokens recursively, so any nested `@.hotpot/prompts/...` references inside the shared body will resolve to their files at read time.

Platform note: this platform has no dedicated subagents for `new`. If the workflow needs an execution agent (for example via a `--switch` flow that immediately hands off), the registered subagents `hotpot-execution` and `hotpot-review` live in `.claude/agents/`.
