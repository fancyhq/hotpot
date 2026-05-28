<!-- Documentation reference for Codex platform integration formats. -->

# Codex Platform Reference

This document summarizes the Codex formats that matter when adapting Hotpot commands, agents, hooks, plugins, and tools.

Sources checked: `https://developers.openai.com/codex`, especially CLI Slash Commands, Subagents, Hooks, Plugins, Build Plugins, and Config Reference.

## Commands

Codex CLI currently documents built-in slash commands rather than a project-local Markdown command format.

Documented built-ins include:

- `/permissions`
- `/agent`
- `/apps`
- `/plugins`
- `/clear`
- `/compact`
- `/copy`
- `/diff`
- `/exit`
- `/experimental`
- `/feedback`
- `/init`
- `/logout`
- `/mcp`
- `/mention`
- `/model`
- `/fast`
- `/plan`
- `/goal`
- `/personality`
- `/ps`
- `/stop`
- `/fork`
- `/side`
- `/resume`
- `/new`
- `/quit`
- `/review`
- `/status`
- `/debug-config`
- `/statusline`
- `/title`
- `/keymap`

Important command notes:

- `/init` generates project instructions such as `AGENTS.md`.
- `/review` reviews working-tree changes.
- `/mcp` lists MCP servers and tools.
- `/plugins` opens plugin management.
- `/agent` switches active agent threads.

Hotpot implication: do not assume `commands/*.md` can be installed as native Codex slash commands. For Codex, model reusable Hotpot workflows as Skills inside a Codex plugin, instructions in `AGENTS.md`, or explicit prompts that delegate to custom TOML agents.

## Agents

Codex supports built-in and custom subagents. Custom agents are TOML files.

Locations:

- Global custom agents live in `~/.codex/agents/`.
- Project custom agents live in `.codex/agents/`.
- Each TOML file defines one custom agent.

Built-in agents:

- `default`
- `worker`
- `explorer`

Agent format:

```toml
name = "hotpot-execution"
description = "Implements Hotpot tasks from full task handoff files."
sandbox_mode = "workspace-write"
model = "gpt-5.1-codex"
model_reasoning_effort = "high"
nickname_candidates = ["executor", "hotpot worker"]

developer_instructions = """
You implement the task described in the provided Hotpot task file.
Treat the Task section as source of truth, follow the Plan, update checkboxes,
and stop on blockers.
"""

[mcp_servers.hotpot]
command = "hotpot"
args = ["mcp", "serve"]

[[skills.config]]
path = "./skills/hotpot-execute"
enabled = true
```

Required fields:

- `name`
- `description`
- `developer_instructions`

Useful optional fields:

- `nickname_candidates`
- `model`
- `model_reasoning_effort`
- `sandbox_mode`, commonly `read-only` or `workspace-write`
- `mcp_servers`
- `skills.config`
- Other Codex `config.toml` keys layered into spawned sessions

Global subagent settings can be configured under `[agents]`:

```toml
[agents]
max_threads = 6
max_depth = 1
job_max_runtime_seconds = 1800
```

Behavior notes:

- Subagent workflows are enabled by default.
- Subagents are spawned only when explicitly requested.
- Custom agent files act as configuration layers for spawned sessions.
- If a custom agent uses a built-in name such as `explorer`, the custom agent takes precedence.

Hotpot implication: the repository's `.codex/agents/hotpot-execution.toml` and `.codex/agents/hotpot-review.toml` pattern is the native Codex agent format.

## Hooks And Plugins

Codex hooks are lifecycle hooks behind the `hooks` feature flag.

Enable hooks:

```toml
[features]
hooks = true
```

Hook locations:

- `~/.codex/hooks.json`
- `~/.codex/config.toml`
- `<repo>/.codex/hooks.json`
- `<repo>/.codex/config.toml`
- Installed plugin hook manifests

Project hooks require a trusted `.codex/` layer. Hooks from all matching sources run; higher-precedence layers do not replace lower-precedence hooks.

JSON hook format:

```json
{
  "PreToolUse": [
    {
      "matcher": "Bash",
      "hooks": [
        {
          "type": "command",
          "command": "python3 .codex/hooks/block-dangerous-bash.py"
        }
      ]
    }
  ]
}
```

TOML hook format:

```toml
[[hooks.PreToolUse]]
matcher = "Bash"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "python3 .codex/hooks/block-dangerous-bash.py"
```

Documented events:

- `SessionStart`
- `PreToolUse`
- `PermissionRequest`
- `PostToolUse`
- `UserPromptSubmit`
- `Stop`

**Codex does NOT expose a session-close event** in the documented clinic. `Stop` fires after every assistant turn, which is the wrong semantics for one-shot resource cleanup (it would tear down state mid-conversation). Consequence: Hotpot's VuePress server can't be released via a Codex hook the way it is on Claude Code (`SessionEnd`), OpenCode (`session.deleted`), and Pi (`session_shutdown`). Codex users rely instead on (1) the `/hotpot:execute` pre-flight `hotpot vuepress stop --if-running` for the normal path, and (2) the `--ttl 1800` lazy-expiry in `vuepress start` as a final safety net for the close-without-execute path. The CLI stop path now performs strong cleanup itself: Unix process group first, Windows process tree, stale runtime cleanup, and a conservative runtime-port fallback for Hotpot-owned VuePress/Vite/pnpm processes. See **VuePress Integration** in `docs/ARCH.md`.

Matcher notes:

- `*`, empty, or omitted matcher means match all.
- Matchers are regex strings.
- `PermissionRequest`, `PostToolUse`, and `PreToolUse` match tool names such as `Bash`, `Edit`, `Write`, `apply_patch`, and MCP names.
- `SessionStart` matches startup kinds such as `startup`, `resume`, and `clear`.
- `UserPromptSubmit` and `Stop` ignore matchers.

Command hook input includes JSON fields such as `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `model`, and `turn_id`.

Hook output can:

- Add `systemMessage`.
- Deny a `PreToolUse` through `hookSpecificOutput.permissionDecision = "deny"`.
- Use legacy `decision = "block"` or exit code `2` with stderr.
- Allow or deny `PermissionRequest`; deny wins.
- Add `additionalContext` or replace/block tool results in `PostToolUse`.
- Continue a task from `Stop` by returning `decision = "block"` with a reason.

Plugins bundle Codex resources.

Minimal plugin structure:

```text
my-plugin/
├── .codex-plugin/
│   └── plugin.json
├── skills/
│   └── hotpot-execute/
│       └── SKILL.md
├── .mcp.json
├── hooks/
│   └── hooks.json
└── assets/
```

Minimal `plugin.json`:

```json
{
  "name": "hotpot",
  "version": "1.0.0",
  "description": "Hotpot task workflows for Codex",
  "skills": "./skills/"
}
```

Full plugin manifests can include `author`, `homepage`, `repository`, `license`, `keywords`, `skills`, `mcpServers`, `apps`, `hooks`, and `interface`. Paths must be relative to the plugin root and start with `./`.

Hotpot implication: Codex plugin packaging is the best Codex equivalent to OpenCode commands plus plugins. Use plugin Skills for workflow prompts, plugin MCP for tools, and plugin hooks for lifecycle behavior.

## Tools

Codex tool extension is primarily through MCP servers, plugins, and built-in tool configuration.

Relevant config areas:

```toml
[features]
multi_agent = true
hooks = true
shell_tool = true
unified_exec = true
apps = true
skill_mcp_dependency_install = true

[mcp_servers.hotpot]
command = "hotpot"
args = ["mcp", "serve"]

sandbox_mode = "workspace-write"
approval_policy = "on-request"
```

Plugin MCP formats:

- `plugin.json` can point `mcpServers` at a `.mcp.json` file.
- `plugin.json` can define inline MCP server configuration.
- If a plugin has `./hooks/hooks.json` and omits explicit hook config, Codex can still load that default hook file.

Hotpot implication: Codex does not use OpenCode-style TypeScript custom tools. If Hotpot needs custom callable tools in Codex, expose them as MCP tools and load them through `config.toml`, an agent TOML file, or a plugin manifest.
