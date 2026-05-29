<!-- Documentation reference for Claude Code platform integration formats. -->

# Claude Code Platform Reference

This document summarizes the Claude Code formats that matter when adapting Hotpot commands, agents, hooks, plugins, and tools.

Sources checked: `https://code.claude.com/docs/en/overview`, `https://code.claude.com/docs/zh-CN/hooks-guide`, Skills, Sub-agents, Hooks, and MCP documentation.

## Commands And Skills

Claude Code supports both project Markdown commands and Skills. Skills are the newer reusable capability format, but project `.claude/commands/` directories are still used in real projects for manual slash-command workflows.

Preferred skill locations:

- Personal skills live in `~/.claude/skills/<skill-name>/SKILL.md`.
- Project skills live in `.claude/skills/<skill-name>/SKILL.md`.
- Plugin skills live under a plugin `skills/<skill-name>/SKILL.md`.
- Enterprise-managed skill locations can also be provided by policy.

Command locations:

- `.claude/commands/deploy.md` still creates `/deploy`.
- Nested command files are used by projects for namespaced manual workflows, such as `.claude/commands/trellis/...` in the Trellis project.
- If a skill and legacy command share a name, the skill wins.

Skill format:

```markdown
---
name: hotpot-execute
description: Execute and review the active Hotpot task.
argument-hint: "[instructions]"
allowed-tools: Read, Write, Edit, MultiEdit, Glob, Grep, Bash
model: sonnet
effort: high
context: fork
agent: hotpot-execution
user-invocable: true
---

# Hotpot Execute

Resolve the active task and implement it.

Arguments: $ARGUMENTS
First arg: $1
Session: ${CLAUDE_SESSION_ID}
```

Important skill frontmatter:

- `name` is optional in Claude Code but useful for portability.
- `description` and `when_to_use` guide automatic invocation.
- `argument-hint` documents manual slash-command arguments.
- `arguments` can define named arguments.
- `disable-model-invocation: true` makes the skill manual-only.
- `user-invocable` controls direct user invocation.
- `allowed-tools` pre-approves tools while the skill is active.
- `model` and `effort` tune execution.
- `context: fork` runs in an isolated subagent context.
- `agent` selects a built-in or custom subagent.
- `hooks`, `paths`, and `shell` can be scoped to the skill.

Template features:

- `$ARGUMENTS` expands all user arguments.
- `$ARGUMENTS[N]` and `$N` expand positional arguments.
- Named arguments can be referenced as `$name`.
- `${CLAUDE_SESSION_ID}`, `${CLAUDE_EFFORT}`, and `${CLAUDE_SKILL_DIR}` are available.
- Inline `!` shell interpolation and fenced `!` command blocks can inject command output unless disabled by settings.

Hotpot implication: for explicit manual slash-command workflows, install Hotpot command files under `.claude/commands/hotpot/`. Use `.claude/skills/<name>/SKILL.md` only when Hotpot needs a reusable skill that can be discovered or invoked as a skill rather than a command-file entrypoint.

Hooks do not create slash commands. The hooks guide describes hooks as lifecycle automation that runs when events occur. The `/hooks` menu is a read-only browser for configured hooks. `UserPromptExpansion` can observe or block expansion of an already existing user-typed skill or command, but it is not the mechanism that registers that skill or command.

Hotpot implication: do not implement Claude Code Hotpot commands by adding hooks alone. Use `.claude/commands/` files for manual slash-command entrypoints, and optionally add Skills later for reusable capability packaging. Hooks are optional supporting automation for validation, notifications, safety gates, review-memory lifecycle, or context injection.

## Agents

Claude Code custom subagents are Markdown files with YAML frontmatter and body instructions.

Agent locations and priority include:

- Managed settings.
- `--agents` CLI JSON.
- Project `.claude/agents/`.
- User `~/.claude/agents/`.
- Plugin `agents/` directories.

Agent format:

```markdown
---
name: hotpot-review
description: Reviews Hotpot task changes and reports concrete findings.
tools: Read, Glob, Grep, Bash
model: sonnet
permissionMode: default
maxTurns: 12
effort: high
color: purple
---

You are a read-only reviewer. Inspect the task, diff, and validation output.
Return findings first, ordered by severity.
```

Required fields:

- `name`
- `description`

Useful optional fields:

- `tools` and `disallowedTools` control tool access.
- `model` can be `sonnet`, `opus`, `haiku`, a full model ID, or `inherit`.
- `permissionMode` controls approval behavior.
- `maxTurns` limits agent loop length.
- `skills` scopes available skills.
- `mcpServers` scopes MCP servers.
- `hooks` can be defined in the agent frontmatter.
- `memory` can include `user`, `project`, or `local` memory.
- `background`, `effort`, `isolation: worktree`, `color`, and `initialPrompt` tune execution.

Invocation patterns:

- Natural language delegation.
- `@"code-reviewer (agent)"` mentions.
- `claude --agent code-reviewer` for a session-wide agent.
- `.claude/settings.json` with `{ "agent": "code-reviewer" }`.

Limitations:

- Subagents cannot spawn other subagents.
- Tool restrictions can also restrict `Agent(subagent)` spawning.

Hotpot implication: the existing `.claude/agents/hotpot-execution.md` and `.claude/agents/hotpot-review.md` pattern is native Claude Code subagent format.

## Hooks And Plugins

Claude Code hooks run at lifecycle events and can be declared globally, per project, by plugins, or inside skill/agent frontmatter. They automate behavior around an existing session, skill, command, tool call, subagent, or configuration change; they are not command registration files.

Hook locations:

- `~/.claude/settings.json`
- `.claude/settings.json`
- `.claude/settings.local.json`
- Managed policy settings
- Plugin `hooks/hooks.json`
- Skill or agent frontmatter

Settings hook shape:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "python3 .claude/hooks/block-dangerous-bash.py",
            "timeout": 5,
            "statusMessage": "Checking bash command"
          }
        ]
      }
    ]
  }
}
```

Handler types:

- `command`
- `http`
- `mcp_tool`
- `prompt`
- `agent`

Common handler fields:

- `type`
- `if`
- `timeout`
- `statusMessage`
- `once` for skill hooks

Common events include:

- `SessionStart`
- `Setup`
- `UserPromptExpansion`
- `PreToolUse`
- `PermissionRequest`
- `PermissionDenied`
- `PostToolUse`
- `PostToolUseFailure`
- `PostToolBatch`
- `Notification`
- `SubagentStart`
- `SubagentStop`
- `TaskCreated`
- `TaskCompleted`
- `Stop`
- `StopFailure`
- `InstructionsLoaded`
- `ConfigChange`
- `CwdChanged`
- `FileChanged`
- `WorktreeCreate`
- `WorktreeRemove`
- `PreCompact`
- `PostCompact`
- `SessionEnd` — Hotpot wires this to `hotpot hook claude session-end`, which runs `vuepress stop_if_running` so a `pnpm docs:dev` started during `/hotpot:new` is released even when the user closes the session without proceeding to `/hotpot:execute`. See **VuePress Integration** in `docs/ARCH.md`.

Hotpot-specific hook notes:
- `PreToolUse` now uses `"Bash|Edit|Write"` matcher (tool-name regex) and injects **lightweight model-visible context** containing only `ROOT_DIR` and `HOTPOT_LANGUAGE` plus a short language directive. The `Bash` arm matches every Bash call, not only `hotpot` command content; the payload is kept lightweight so this broader trigger is acceptable for Hotpot Bash commands. The old full `context_lines` output (with every `HOTPOT_*_PROMPT` path) is no longer delivered to the model — prompt files are resolved via `$ROOT_DIR/.hotpot/prompts/<name>.md` instead.
- Hotpot does not configure or handle per-user-message prompt hooks; prompt/language delivery happens through `PreToolUse` and review-memory delivery through `SubagentStart`.

Matcher notes:

- Tool events match tool names, including MCP names such as `mcp__server__tool`.
- `SubagentStart` and `SubagentStop` match agent type.
- Matcher interpretation depends on exact or regex-like content.

Command hooks receive JSON on stdin with fields such as `session_id`, `transcript_path`, `cwd`, and `permission_mode`. They can return JSON decisions. Paths can use `${CLAUDE_PROJECT_DIR}`, `${CLAUDE_PLUGIN_ROOT}`, and `${CLAUDE_PLUGIN_DATA}`.

Important hook limitations:

- Hooks cannot trigger `/` commands or tool calls directly.
- `UserPromptExpansion` runs when a user-typed skill or command expands into a prompt, before that prompt reaches Claude.
- `UserPromptExpansion` matchers filter by the existing skill or command name.
- Hook stdout can inject additional context for some events, and hook output can allow, ask, deny, or block supported events, depending on the event schema.
- The `/hooks` menu is read-only; hooks are added by editing settings, plugin hook files, or skill/agent frontmatter.

Hotpot implication: Claude Code can support Hotpot safety gates and review-memory lifecycle integration through hooks, but hooks should not be used as the primary command entrypoint. Custom tools are usually better supplied through MCP than through hooks.

## Tools

Claude Code extends tools primarily through MCP servers and tool permissions.

MCP setup examples:

```bash
claude mcp add --transport http hotpot http://localhost:3000/mcp
claude mcp add hotpot -- hotpot mcp serve
claude mcp list
claude mcp get hotpot
claude mcp remove hotpot
```

MCP scopes:

- Local project scope in `~/.claude.json`.
- Project scope in `.mcp.json`, which is shareable.
- User scope in `~/.claude.json`.
- Plugin-provided MCP servers.
- Claude.ai connectors.

Precedence is local, project, user, plugin, then Claude.ai connectors.

`.mcp.json` supports environment expansion:

```json
{
  "mcpServers": {
    "hotpot": {
      "command": "hotpot",
      "args": ["mcp", "serve"],
      "env": {
        "HOTPOT_ROOT": "${PWD:-.}"
      }
    }
  }
}
```

MCP integration details:

- MCP resources can be referenced with `@` mentions.
- MCP prompts appear as slash commands like `/mcp__servername__promptname`.
- MCP tool search can defer loading until relevant.
- Plugin MCP servers can live in plugin `.mcp.json` or `plugin.json`.

Hotpot implication: if Hotpot needs Claude Code custom tools equivalent to OpenCode plugin tools, package them as an MCP server or a plugin-bundled MCP server.
