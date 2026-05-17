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
- Template discovery in `prompts/` is non-recursive unless settings or packages add paths explicitly.

Extension command format:

```typescript
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export default function (pi: ExtensionAPI) {
  pi.registerCommand("hotpot-execute", {
    description: "Execute and review the active Hotpot task",
    handler: async (args, ctx) => {
      await ctx.waitForIdle();
      await ctx.sendUserMessage(`Execute the active Hotpot task. Args: ${args}`);
    }
  });
}
```

Hotpot implication: `commands/*.md` should map to Pi prompt templates, not to an OpenCode-compatible command directory. Use extension commands when command logic needs programmatic session control.

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
