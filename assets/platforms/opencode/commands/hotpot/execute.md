---
description: Execute and review the active Hotpot task with reusable subagents
---

You are running the Hotpot execute workflow. The user-facing invocation is `/hotpot:execute`.

The full workflow is defined at:

@.hotpot/prompts/hotpot-execute.md

Follow that workflow end-to-end. OpenCode expands `@path` tokens recursively, so the nested `@.hotpot/prompts/tdd-protocol.md` and `@.hotpot/prompts/record-issue-candidate.md` references inside the shared body will resolve at read time.

Platform note: when the shared body refers to "the registered Hotpot execution agent" or "the registered Hotpot review agent", invoke the registered subagents `@hotpot-execution` and `@hotpot-review` from `.opencode/agents/`.

OpenCode also ships an `append_issue_candidate` plugin tool that writes to `issue-candidates.jsonl`. Prefer `hotpot issues candidate add` for cross-platform consistency; the plugin tool is acceptable only as a fallback when the CLI is unavailable. The shared-body user-approval gate applies to both write paths — never write candidates before user approval, regardless of which path you use.
