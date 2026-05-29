<!-- Documentation reference for Pi platform integration formats. -->

# Pi Platform Reference

This document summarizes the Pi Coding Agent formats that matter when adapting Hotpot commands, agents, hooks/plugins, and tools.

Sources checked: `https://pi.dev/docs/latest`, especially Using Pi, Extensions, Skills, Prompt Templates, Pi Packages, and Settings.

## Commands

Pi has three command-like mechanisms:

- Built-in slash commands.
- Prompt templates, which expand Markdown prompts from `/name`.
- Extension commands, which are registered from TypeScript with `pi.registerCommand()`.

Built-in slash commands include:

- `/login`, `/logout`
- `/model`
- `/settings`
- `/resume`
- `/new`
- `/name`
- `/session`
- `/tree`
- `/fork`
- `/clone`
- `/compact`
- `/copy`
- `/export`
- `/share`
- `/reload`
- `/hotkeys`
- `/changelog`
- `/quit`

Prompt template locations:

- Global templates live in `~/.pi/agent/prompts/*.md`.
- Project templates live in `.pi/prompts/*.md`.
- Packages can provide `prompts/` directories or `pi.prompts` entries in `package.json`.
- Settings can list prompt paths in `prompts`.
- CLI can load prompt templates with `--prompt-template <path>`.

Prompt template format:

```markdown
---
description: Execute and review the active Hotpot task
argument-hint: "[instructions]"
---

Resolve the active Hotpot task and run the execution/review loop.

Arguments: $ARGUMENTS
First arg: $1
Remaining from second arg: ${@:2}
```

Template rules:

- The filename becomes the command name; `execute.md` becomes `/execute`.
- `description` is optional; the first non-empty line is used if missing.
- `argument-hint` appears in autocomplete.
- Templates support `$1`, `$2`, `$@`, `$ARGUMENTS`, `${@:N}`, and `${@:N:L}`.
- Pi does not implicitly append slash-command arguments to the prompt body. Template authors must place `$ARGUMENTS`, `$@`, or positional variables in the Markdown body when the model needs to see command text.
- **Hotpot legacy lesson (deprecated)**: Hotpot historically routed its slash commands through Pi prompt-template thin shells (`.pi/prompts/hotpot-*.md`) that injected `$ARGUMENTS` via a three-part pattern (delimiter block + unconditional directive + Exception override) plus an `=== USER ACTIVE REQUEST ===` framing header. In Pi projects that load competing system context (`AGENTS.md`, `CLAUDE.md`, global skills lists), even that hardened template was silently absorbed as background documentation, causing the model to greet with "What would you like me to do?" instead of starting the workflow (see the `migrate-pi-commands-to-extension` task). Hotpot now delivers all three slash commands through Pi extension commands (`pi.registerCommand` + `pi.sendUserMessage`); see **Hotpot Pi extension commands** below. (`sendUserMessage` lives on `ExtensionAPI` — the factory's `pi` parameter — **not** on `ExtensionCommandContext`; calling `ctx.sendUserMessage` throws `is not a function` at runtime.)
- Template discovery in `prompts/` is non-recursive unless settings or packages add paths explicitly.

Extension command format:

```typescript
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export default function (pi: ExtensionAPI) {
  pi.registerCommand("hotpot-execute", {
    description: "Execute and review the active Hotpot task",
    handler: async (args, ctx) => {
      await ctx.waitForIdle();
      pi.sendUserMessage(`Execute the active Hotpot task. Args: ${args}`);
    }
  });
}
```

Hotpot implication: generic Pi `commands/*.md` should map to Pi prompt templates, not to an OpenCode-compatible command directory. Hotpot itself no longer relies on prompt templates for its three slash commands — see the dedicated section below.

### Hotpot Pi extension commands

Hotpot registers `/hotpot-new`, `/hotpot-execute`, and `/hotpot-finish-work` through `pi.registerCommand` inside `assets/platforms/pi/extensions/hotpot/index.ts` (installed at `.pi/extensions/hotpot/index.ts`). Each handler:

1. Calls `ensureContext(ctx.cwd)` to bootstrap the Hotpot env-var map (`ROOT_DIR`, `HOTPOT_NEW_PROMPT`, `HOTPOT_EXECUTE_PROMPT`, `HOTPOT_FINISH_WORK_PROMPT`, `HOTPOT_TDD_PROTOCOL_PROMPT`, `HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`, `HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`).
2. Calls a shared `buildPiCommandMessage(...)` helper to assemble a single user-voice message. The current body shape is **front-loaded for weak-instruction-following models** (see "Four failure modes" below). New ordering: opening lead-in `A new Hotpot <workflowName> request from me:` → user-input block (`<<< {LABEL} >>> ... <<< END {LABEL} >>>` wrapping the raw `args`) → `If the block above is empty: <emptyArgsBehavior>` Exception → `YOUR FIRST TOOL CALL MUST BE \`Read("<workflowPromptPath>")\`` first-tool-call directive → `FORBIDDEN` list (no `ls`/`tree`/`git log`/`git status`/skill auto-invocation/`project-structure-explorer`/`skill-creator`/clarification questions/generic greetings) → `@.hotpot/prompts/<name>.md → absolute path` substitution table → Pi-specific Platform note about same-session phase separation. The user-input block sits at the top to mitigate the third failure mode (attention loss on user-message body) — weaker models that lose attention mid-message still latch onto the first paragraph. No `=== USER ACTIVE REQUEST ===` ceremonial framing — `role:user` already supplies that semantic, and the extra wrapping caused weaker models to treat the body as documentation.
3. Calls `pi.sendUserMessage(text)` so the model sees the content as an actual user message — bypassing the prompt-template absorption failure mode described above. (`sendUserMessage` is on `ExtensionAPI`, returning `void`; **not** on the handler's `ctx: ExtensionCommandContext`. Using `ctx.sendUserMessage` was a real regression — see the post-validation fix notes in `migrate-pi-commands-to-extension`.) Before calling `pi.sendUserMessage`, every handler MUST also assign the closure-scoped `pendingWorkflow` field (`{ command, workflowPromptPath, userInputLabel }`). The next `pi.on("context", ...)` invocation (Pi fires `context` before every provider request) consumes this marker exactly once and injects a third `role:system` message restating the workflow path, the user-input block markers, and the FORBIDDEN behaviors — this is the per-turn reinforcement layer of the third failure mode mitigation.
4. Arms the closure-scoped `pendingFirstToolGuard` field immediately before `pi.sendUserMessage(text)`. The single `pi.on("tool_call", ...)` hook evaluates this runtime first-tool guard before bash env injection. While armed, the only allowed first tool call is `read` with `path` (or defensive `filePath`) exactly equal to the command's workflow prompt (`HOTPOT_NEW_PROMPT`, `HOTPOT_EXECUTE_PROMPT`, or `HOTPOT_FINISH_WORK_PROMPT`). Any other tool call — `bash`, `ls`, `find`, skill-loading tools, `read` on `docs/ARCH.md` / `AGENTS.md`, etc. — returns `{ block: true, reason }` with an English correction telling the model which Hotpot slash command was just invoked, where the user-input block is, and that the next call must be `Read("<workflowPromptPath>")`. A blocked call keeps the guard armed; the exact workflow `read` disarms it so later workflow exploration proceeds normally. This first-tool guard is a hard runtime constraint layer, not a replacement for the user-message and `pendingWorkflow` prompt layers.

**Per-turn provider context (lightweight)**: the `pi.on("context", ...)` handler now injects a **lightweight system message** listing only `ROOT_DIR` and `HOTPOT_LANGUAGE` (plus the language directive), rather than expanding `Object.entries(hotpot)` with every env field. This mirrors the Claude/Codex PreToolUse lightweight context strategy — the model can resolve prompt files via `$ROOT_DIR/.hotpot/prompts/<name>.md` without seeing every `HOTPOT_*_PROMPT` path on every turn. The full env contract (`HotpotContext`) is still available internally: `ensureContext`, `injectHotpotEnv` (bash `tool_call`), and slash command builders (`buildPiCommandMessage`'s `hotpot` parameter) still use the complete bootstrap map. Only the model-visible provider context was trimmed.

#### Four failure modes the message body and runtime guard must survive

1. **Prompt-template absorption** (legacy, fixed by moving to `pi.sendUserMessage`). When Hotpot delivered slash-command content through `.pi/prompts/hotpot-*.md` thin shells, Pi projects loading `AGENTS.md` / `CLAUDE.md` / global skills lists absorbed the template content as system-level background documentation. The model fell back to "What would you like me to do?" instead of starting the workflow.
2. **Skill auto-invocation hijack** (current, mitigated by message-body design). Even with `role:user` delivery, weaker instruction-following models (observed with `kimi-k2.6` on `moonshotai-cn`) hallucinate generic "explore the project" intent and auto-invoke the global `project-structure-explorer` skill before reading the workflow file. In a real Pi session log, this manifested as: line 4 user message arrives intact → line 5 model thinks "user wants me to explore the project structure" and runs `ls -la` → line 7 model Reads the skill's `SKILL.md` → line 9 model hallucinates "look at the issue mentioned in the PR" and runs `git log` → line 11 model fabricates user input as "嗯" and replies "你好！有什么我可以帮你的吗？". Mitigation: front-load the message with an unambiguous first-tool-call directive (`YOUR FIRST TOOL CALL MUST BE \`Read\` ON THIS EXACT PATH`) and a `FORBIDDEN` list naming the observed distractions.
3. **Attention loss on user-message body** (current, mitigated by message reordering + per-turn system injection). Even with `role:user` delivery + front-loaded `FIRST TOOL CALL` directive + FORBIDDEN list, severely weak instruction-following models still lose attention on the mid-message `<<< INITIAL TASK IDEA >>>` block entirely. Evidence: Pi session `/Users/bytedance/.pi/agent/sessions/--Users-bytedance-RustProjects-hotpot--/2026-05-21T09-37-43-485Z_019e49e5-e43d-7545-96ee-db13bb965132.jsonl`, line 4 user message arrives intact → line 5 model thinking literally writes "The user hasn't asked anything yet" and runs `pwd && ls -la` → line 7 model hallucinates an `agent-browser` extension → line 16 model closes with "I'm ready to help with the skill-creator skill. What would you like to do?". Mitigation has two layers: (i) `buildPiCommandMessage` reorders the body so the `<<< userInputLabel >>>` block is the FIRST paragraph (right after the lead-in `A new Hotpot <workflowName> request from me:`), giving weaker models a high-attention slot for the user's actual request; (ii) the `hotpotExtension` closure exposes a single-shot `pendingWorkflow` marker that each handler sets immediately before `pi.sendUserMessage` — the next `pi.on("context", ...)` invocation pushes a third `role:system` message explicitly naming the workflow absolute path, the `<<< userInputLabel >>>` block markers, and the FORBIDDEN behaviors (`ls`/`tree`/`git log`/`git status`/`project-structure-explorer`/`skill-creator`/greeting), then resets the marker. If a future model still ignores even this dual-layer mitigation, escalate by tightening the message further or recommending a stronger model.
4. **Prompt-only first-tool failure** (current, fixed by runtime first-tool guard). The 2026-05-22 Pi session `/Users/bytedance/.pi/agent/sessions/--Users-bytedance-RustProjects-hotpot--/2026-05-22T02-26-08-908Z_019e4d81-218c-7e8d-8446-e2ce64ff183b.jsonl` showed that prompt-only mitigation was insufficient even after user-input-first ordering and `pendingWorkflow` system reinforcement. With `deepseek-v4-pro` (and the user also reported switching across `kimi-k2.6`), line 4 contained the full `INITIAL TASK IDEA` block plus first-tool-call directive, but line 5 still read `docs/ARCH.md` and ran `find`; later turns ran `ls`, read `AGENTS.md`, hallucinated unrelated tasks, treated input as empty/hello, and asked what to do. Mitigation: `pendingFirstToolGuard` makes the first tool call a runtime-enforced contract. It blocks every non-workflow first call with a corrective `{ block: true, reason }`, keeps armed after bad attempts, and disarms only after the model reads the exact workflow prompt path.

`hotpot init --platform pi` (and `hotpot update`) additionally runs a one-shot cleanup that removes the deprecated `.pi/prompts/hotpot-{new,execute,finish-work}.md` thin shells from any project that still has them. Cleanup is idempotent: missing paths are skipped silently; real IO errors propagate up.

When future Hotpot slash commands are added on Pi, register them with `pi.registerCommand` + `buildPiCommandMessage` (do not reintroduce thin shells), and add any newly deprecated paths to `cleanup_deprecated_pi_prompts` in `src/assets/platforms/pi.rs`. When tuning the message body, preserve both the leading first-tool-call directive and the `FORBIDDEN` list — they are load-bearing for weaker models, not stylistic. Each handler MUST also assign the closure-scoped `pendingWorkflow` field immediately before `pi.sendUserMessage`; without it, the third failure mode's per-turn system reinforcement is silently disabled and weaker models will regress to attention loss on the user-message body. Each handler MUST also arm `pendingFirstToolGuard`; without it, the fourth failure mode can bypass prompt-only defenses and execute the wrong first tool call.

## Agents

Pi intentionally does not include built-in subagents. Its design pushes workflow-specific behavior into extensions, skills, prompt templates, and packages.

Agent-like mechanisms:

- Skills provide reusable on-demand capabilities.
- Prompt templates provide manual slash-command workflows.
- Extensions can change prompts, intercept input, register tools, and send user messages.
- Extensions can create, fork, switch, and navigate sessions from command handlers.

Skill locations:

- Global skills live in `~/.pi/agent/skills/` and `~/.agents/skills/`.
- Project skills live in `.pi/skills/` and `.agents/skills/` from the current directory or ancestors.
- Packages can provide `skills/` directories or `pi.skills` entries.
- Settings can list skills in `skills`.
- CLI can load skills with `--skill <path>`.

Skill format:

```markdown
---
name: hotpot-execute
description: Execute and review Hotpot tasks. Use when the user asks to run the active Hotpot task.
allowed-tools: read bash edit write grep find ls
disable-model-invocation: false
---

# Hotpot Execute

Read the active task, implement the checkbox plan, run validation, and report results.
```

Skill rules:

- Pi implements the Agent Skills standard and validates leniently.
- `name` is required by the standard and must match the parent directory for directory skills.
- `description` is required; skills with missing descriptions are not loaded.
- Unknown frontmatter fields are ignored.
- Name must be lowercase letters, numbers, and hyphens, up to 64 characters.
- Skills are progressively disclosed: startup includes names/descriptions, full `SKILL.md` loads on demand.
- Skills register as `/skill:name` commands when `enableSkillCommands` is true.
- Pi can load Claude Code or Codex skill directories by adding their paths to settings.

Settings example:

```json
{
  "skills": [
    "~/.claude/skills",
    "~/.codex/skills",
    "../.claude/skills"
  ],
  "enableSkillCommands": true
}
```

Hotpot implication: Pi has no direct equivalent to `.opencode/agents/` or `.codex/agents/*.toml`. Convert Hotpot agent behavior into Skills and extension-mediated workflows.

## Hooks And Plugins

Pi uses TypeScript extensions and Pi packages rather than separate hook JSON files.

Extension locations:

- Global single-file extensions live in `~/.pi/agent/extensions/*.ts`.
- Global directory extensions live in `~/.pi/agent/extensions/*/index.ts`.
- Project single-file extensions live in `.pi/extensions/*.ts`.
- Project directory extensions live in `.pi/extensions/*/index.ts`.
- Settings can list extension paths in `extensions`.
- CLI can load extension sources with `-e` or `--extension`.

Extension skeleton:

```typescript
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

export default function (pi: ExtensionAPI) {
  pi.on("session_start", async (_event, ctx) => {
    ctx.ui.notify("Hotpot extension loaded", "info");
  });

  pi.on("tool_call", async (event) => {
    if (event.toolName === "bash" && event.input.command?.includes("rm -rf")) {
      return { block: true, reason: "Blocked dangerous command" };
    }
  });

  pi.registerTool({
    name: "read_issue_candidates",
    label: "Read Issue Candidates",
    description: "Read temporary Hotpot issue candidates",
    parameters: Type.Object({}),
    async execute(_toolCallId, _params, _signal, _onUpdate, ctx) {
      return {
        content: [{ type: "text", text: `Read candidates from ${ctx.cwd}` }],
        details: {}
      };
    }
  });
}
```

Important event families:

- Resource discovery: `resources_discover`.
- Session lifecycle: `session_start`, `session_shutdown` (Hotpot uses this to run `hotpot vuepress stop --if-running` so any VuePress dev server started during `/hotpot:new` is released on session close — see **VuePress Integration** in `docs/ARCH.md`), `session_before_switch`, `session_before_fork`, `session_before_compact`, `session_compact`, `session_before_tree`, `session_tree`.
- Agent lifecycle: `before_agent_start`, `agent_start`, `agent_end`, `turn_start`, `turn_end`.
- Message lifecycle: `message_start`, `message_update`, `message_end`.
- Provider lifecycle: `context`, `before_provider_request`, `after_provider_response`.
- Tool lifecycle: `tool_execution_start`, `tool_call`, `tool_result`, `tool_execution_end`.
- Input lifecycle: `input`.
- User shell lifecycle: `user_bash`.
- Model lifecycle: `model_select`, `thinking_level_select`.

Pi packages bundle extensions, skills, prompt templates, and themes.

Package manifest format:

```json
{
  "name": "hotpot-pi-package",
  "keywords": ["pi-package"],
  "pi": {
    "extensions": ["./extensions"],
    "skills": ["./skills"],
    "prompts": ["./prompts"],
    "themes": ["./themes"]
  }
}
```

Package conventions if no `pi` manifest exists:

- `extensions/` loads `.ts` and `.js` files.
- `skills/` recursively finds `SKILL.md` folders and top-level `.md` skill files.
- `prompts/` loads `.md` files.
- `themes/` loads `.json` files.

Hotpot implication: Pi package distribution should bundle Hotpot prompt templates, skills, and TypeScript extensions together.

## Tools

Pi has built-in tools, CLI tool filters, and extension-registered custom tools.

Built-in tools listed by the docs:

- `read`
- `bash`
- `edit`
- `write`
- `grep`
- `find`
- `ls`

CLI tool controls:

```bash
pi --tools read,grep,find,ls -p "Review the code"
pi --no-builtin-tools -e ./my-extension.ts
pi --no-tools -p "Answer without tools"
```

Custom tool definition:

```typescript
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

export default function (pi: ExtensionAPI) {
  pi.registerTool({
    name: "hotpot_status",
    label: "Hotpot Status",
    description: "Show the active Hotpot task status",
    parameters: Type.Object({
      path: Type.Optional(Type.String({ description: "Project path" }))
    }),
    async execute(toolCallId, params, signal, onUpdate, ctx) {
      return {
        content: [{ type: "text", text: `Project: ${params.path ?? ctx.cwd}` }],
        details: { toolCallId }
      };
    }
  });
}
```

Tool behavior notes:

- Tool parameters use TypeBox schemas.
- `execute` receives `toolCallId`, params, abort signal, update callback, and extension context.
- Extensions can override built-in tools.
- `tool_call` can block or mutate tool input before execution.
- `tool_result` can modify tool output after execution.
- Custom rendering can alter how tools and messages appear in the TUI.
- Pi intentionally does not include built-in MCP; use extensions or external processes if MCP-like behavior is needed.

Hotpot implication: the OpenCode review-memory plugin tools should become Pi extension tools if Hotpot targets Pi.
