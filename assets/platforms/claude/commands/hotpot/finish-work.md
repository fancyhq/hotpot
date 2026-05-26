---
description: Finish the active Hotpot task, promote review memory, and optionally commit
---

<!-- Hotpot Claude command asset for finishing the active task. -->

You are running the Hotpot finish-work workflow. The user-facing invocation is `/hotpot:finish-work`.

The full workflow is defined at:

@.hotpot/prompts/hotpot-finish-work.md

Follow that workflow end-to-end. Claude expands `@path` tokens recursively, so the nested `@.hotpot/prompts/summarize-issue-candidates.md`, `@.hotpot/prompts/get-issue.md`, and `@.hotpot/prompts/hotpot-execute.md` references inside the shared body will resolve to their files at read time.

Platform note: when the shared body's "Offer to Resume Next Task" step needs to invoke the Hotpot execution agent, use the `hotpot-execution` subagent from `.claude/agents/`.
