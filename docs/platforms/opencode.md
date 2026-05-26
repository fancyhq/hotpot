<!-- Documentation reference for OpenCode platform integration formats. -->

# OpenCode Platform Reference

This document summarizes the OpenCode formats that matter when adapting Hotpot commands, agents, plugins, and tools.

Sources checked: `https://opencode.ai/docs/zh-cn`, especially Commands, Agents, Plugins, Tools, and Custom Tools.

## Commands

OpenCode supports project and global custom commands.

- Project commands live in `.opencode/commands/`.
- Global commands live in `~/.config/opencode/commands/`.
- The Markdown filename becomes the slash command name.
- Markdown commands use YAML frontmatter plus a prompt/template body.
- Commands can also be declared in `opencode.jsonc` under `command.<name>`.
- Custom commands can override built-in commands.

Markdown command format:

```markdown
---
description: Execute the active Hotpot task
agent: hotpot-execution
model: anthropic/claude-sonnet-4-5
---

Resolve the active task and implement it.

Arguments: $ARGUMENTS
First arg: $1
```

JSON command format:

```jsonc
{
  "command": {
    "execute": {
      "template": "Resolve the active task and implement it. Args: $ARGUMENTS",
      "description": "Execute the active Hotpot task",
      "agent": "hotpot-execution",
      "model": "anthropic/claude-sonnet-4-5",
      "subtask": true
    }
  }
}
```

Template features:

- `$ARGUMENTS` expands to all command arguments.
- `$1`, `$2`, and later positional variables expand individual arguments.
- `!` shell interpolation executes a command at the project root and injects its output.
- `@file` references can include project files.
- `agent` can route the command to a configured agent.
- If `agent` targets a subagent, OpenCode starts a subtask by default.
- `subtask = false` keeps a subagent-targeting command in the current session.
- `subtask = true` forces subtask execution.

Hotpot implication: the existing `commands/*.md` files are structurally compatible with OpenCode when installed under `.opencode/commands/hotpot/` or another OpenCode command path.

## Agents

OpenCode supports agents from configuration and Markdown files.

- Project agents live in `.opencode/agents/`.
- Global agents live in `~/.config/opencode/agents/`.
- Agents can also be declared in `opencode.json` under `agent`.
- Markdown filename becomes the agent name.
- Markdown frontmatter configures behavior; body is the agent prompt.

Agent modes:

- `primary`: user-facing primary agents such as Build and Plan.
- `subagent`: delegated agents invoked by `@agent` mentions or command routing.
- `all`: usable as both primary and subagent; this is the default.

Markdown agent format:

```markdown
---
description: Implements Hotpot tasks from task handoff files
mode: subagent
model: anthropic/claude-sonnet-4-5
temperature: 0
steps: 50
tools:
  read: true
  edit: true
  write: true
  bash: true
permission:
  edit: allow
  bash:
    "hotpot *": allow
    "*": ask
color: blue
---

You implement the task described in the provided Hotpot task file.
```

Important fields:

- `description` is required for subagent selection.
- `mode` controls whether the agent is primary, subagent, or both.
- `model`, `temperature`, `top_p`, and provider-specific fields tune generation.
- `steps` limits agent loop length; old `maxSteps` is deprecated.
- `tools` can enable, disable, or wildcard-match tools.
- `permission` controls edit, bash, webfetch, and tool-specific approvals.
- `permission.task` can restrict which subagents this agent may spawn.
- `disable`, `hidden`, and `color` control availability and UI presentation.

Hotpot implication: the existing `.opencode/agents/hotpot-execution.md` and `.opencode/agents/hotpot-review.md` pattern is the native OpenCode agent format.

## Hooks And Plugins

OpenCode uses plugins for lifecycle hooks, custom behavior, and custom tools.

- Project plugins live in `.opencode/plugins/`.
- Global plugins live in `~/.config/opencode/plugins/`.
- NPM plugin packages can be listed in `opencode.json` under `plugin`.
- Local plugin dependencies belong in `.opencode/package.json`.
- Plugins are JavaScript or TypeScript modules.

Plugin skeleton:

```typescript
import type { Plugin } from "@opencode-ai/plugin";

export const ExamplePlugin: Plugin = async ({ project, client, $, directory, worktree }) => {
  return {
    "command.executed": async (input) => {
      console.log("command", input.command);
    },
    "tool.execute.before": async (input) => {
      if (input.tool === "bash" && String(input.args.command).includes("rm -rf")) {
        throw new Error("Blocked dangerous command");
      }
    },
    "shell.env": async () => ({
      HOTPOT_ENABLED: "1"
    })
  };
};
```

Common event families:

- Command events such as `command.executed`.
- File, installation, LSP, message, permission, server, session, and todo events. Hotpot uses session-close events (`session.deleted` / `session.ended` / `session.shutdown` — matched defensively across OpenCode releases) to run `hotpot vuepress stop --if-running`, releasing any VuePress dev server started during `/hotpot:new`. See **VuePress Integration** in `docs/ARCH.md`.
- Tool events such as `tool.execute.before` and `tool.execute.after`.
- Shell environment injection through `shell.env`.
- TUI events such as `tui.prompt.append`, `tui.command.execute`, and `tui.toast.show`.

Hotpot implication: review-memory helpers are best implemented as OpenCode plugins because plugins can expose tools, inject environment variables, and observe command/tool lifecycles.

## Tools

OpenCode has built-in tools plus MCP and custom tools.

Built-in tools include:

- `bash`
- `edit`
- `write`
- `read`
- `grep`
- `glob`
- `lsp` experimental
- `patch`
- `skill`
- `todowrite`
- `webfetch`
- `websearch`
- `question`

Tool permissions are usually controlled through `permission` in config or agent frontmatter. Edit permission covers edit, write, and patch. MCP tools can be controlled with wildcard names such as `mymcp_*`.

Custom tools live in `.opencode/tools/` or `~/.config/opencode/tools/`.

- A default export creates a tool named after the filename.
- Named exports create tools named `<filename>_<exportname>`.
- Custom tools use the `tool` helper from `@opencode-ai/plugin`.
- Arguments use `tool.schema` or Zod schemas.
- The `execute` function receives arguments and a context with `agent`, `sessionID`, `messageID`, `directory`, and `worktree`.
- A custom tool can override a built-in tool if it uses the same name.

Custom tool format:

```typescript
import { tool } from "@opencode-ai/plugin";

export default tool({
  description: "Read Hotpot issue candidates",
  args: {
    limit: tool.schema.number().optional()
  },
  async execute(args, context) {
    return `Reading candidates from ${context.directory}, limit ${args.limit ?? "all"}`;
  }
});
```

Hotpot implication: OpenCode is the richest target for Hotpot because commands, agents, plugins, and custom tools are all natively extensible.
