---
description: Finish the active Hotpot task, promote review memory, and optionally commit
agent: build
---

You are running the Hotpot finish-work workflow. The user-facing invocation is `/hotpot:finish-work`.

The full workflow is defined at:

@.hotpot/prompts/hotpot-finish-work.md

Follow that workflow end-to-end. OpenCode expands `@path` tokens recursively, so the nested `@.hotpot/prompts/summarize-issue-candidates.md`, `@.hotpot/prompts/get-issue.md`, and `@.hotpot/prompts/hotpot-execute.md` references inside the shared body will resolve at read time.

Platform note: when the shared body's "Offer to Resume Next Task" step needs to invoke the Hotpot execution agent, invoke the registered subagent `@hotpot-execution` from `.opencode/agents/`.
