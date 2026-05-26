---
description: Execute and review the active Hotpot task with reusable subagents
---

<!-- Hotpot Claude command asset for executing the active task. -->

You are running the Hotpot execute workflow. The user-facing invocation is `/hotpot:execute`.

The full workflow is defined at:

@.hotpot/prompts/hotpot-execute.md

Follow that workflow end-to-end. Claude expands `@path` tokens recursively, so the nested `@.hotpot/prompts/tdd-protocol.md` and `@.hotpot/prompts/record-issue-candidate.md` references inside the shared body will resolve to their files at read time.

Platform note: when the shared body refers to "the registered Hotpot execution agent" or "the registered Hotpot review agent", invoke the `hotpot-execution` and `hotpot-review` subagents from `.claude/agents/`.
