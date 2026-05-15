---
description: Create a Hotpot task through brainstorming
---

You are running the Hotpot new-task workflow. The user-facing invocation is `/hotpot:new`.

The full workflow is defined at:

@.hotpot/prompts/hotpot-new.md

Follow that workflow end-to-end. OpenCode expands `@path` tokens recursively, so any nested `@.hotpot/prompts/...` references inside the shared body will resolve at read time.

Platform note: this command does not need a subagent for `new`. If the workflow ever needs the execution agent, the registered subagents `@hotpot-execution` and `@hotpot-review` live in `.opencode/agents/`.
