# Bash Before Username Resolution Design

## Summary

Implement username resolution in `.opencode/plugins/bash-before.ts` so each shell command receives `HOTPOT_USERNAME` for the current session without mutating persistent system or git configuration.

## Goals

- Preserve the existing `ROOT_DIR` injection behavior.
- Resolve a username once per session and reuse it for every `shell.env` call.
- Prefer a preconfigured environment variable over git-derived values.
- Fall back to an interactive prompt only when no environment or git username is available.
- Keep the change limited to `.opencode/plugins/bash-before.ts`.

## Non-Goals

- Persisting the username to system environment variables.
- Writing the username back to git config.
- Adding new config files or long-term storage.
- Refactoring other plugin files such as `hooks/opencode/plugins/bash-before.ts`.

## Existing Context

The current plugin exports `bashBefore` and does two things:

- Handles `shell.env` and injects `ROOT_DIR = ctx.directory`.
- Handles `session.created` but currently contains only comments describing the intended username behavior.

There is no existing helper or test infrastructure around this plugin in the current workspace.

## Recommended Approach

Use a session-scoped cache inside the plugin closure.

- On `session.created`, resolve the username once and store it in a local variable such as `sessionUsername`.
- On every `shell.env` call, inject `ROOT_DIR` and, when available, inject `HOTPOT_USERNAME` from the cached value.

This keeps the logic simple:

- `event` determines the value.
- `shell.env` distributes the value to each shell invocation.

## Rejected Alternatives

### Resolve on every `shell.env` call

This avoids dependence on event ordering but would make the first shell execution responsible for interactive prompting and increases the risk of repeated checks or repeated prompts.

### Write to `process.env`

This would allow reuse across more code paths in the same Node process, but it broadens the scope beyond the current session abstraction and creates implicit shared state.

## Resolution Order

The username resolution sequence is:

1. Read `process.env.HOTPOT_USERNAME`.
2. If it is present after trimming whitespace, use it immediately.
3. Otherwise attempt git-based lookup.
4. If git lookup does not produce a non-empty value, prompt the user interactively.

### Git Lookup Rules

- First determine whether `ctx.directory` is inside a git work tree.
- If it is inside a git work tree, try repository-local `git config user.name` first.
- If the repository-local value is missing or empty, try `git config --global user.name`.
- If the directory is not a git work tree, skip straight to `git config --global user.name`.
- Any git command failure is treated as a non-fatal miss and resolution continues.

### Prompt Rules

- Prompt only when no environment variable or git value can be resolved.
- Keep prompting until the user provides a non-empty trimmed value.
- The prompt text should explain that setting `HOTPOT_USERNAME` can persist the username for future runs.

## State Model

The plugin maintains one in-memory username value for the current session.

- The resolved username is cached in the plugin closure.
- The cache is not written back to `process.env`.
- The cache is not written to disk.
- The cache exists only for the lifetime of the current plugin/session instance.

## Implementation Sketch

Keep the change in one file and add only small helpers:

- A helper to safely execute git commands and return a trimmed string or no value.
- A helper to detect whether the current directory is in a git work tree.
- A helper to prompt for a username via stdin/stdout.
- A helper to resolve the final username according to the defined precedence.

The main plugin flow remains:

- `event({ event })` initializes the session cache during `session.created`.
- `"shell.env"(input, output)` injects `ROOT_DIR` and `HOTPOT_USERNAME` when the cache is populated.

## Error Handling

- Git failures must not crash plugin startup.
- Empty strings from environment, git, or prompt input are treated as missing values.
- Interactive prompting continues until a usable username is entered.
- If `session.created` does not occur for some reason, `shell.env` still continues to inject `ROOT_DIR`; the implementation may optionally resolve lazily later, but the primary design assumes `session.created` runs first.

## Verification Plan

After implementation, verify:

- `ROOT_DIR` is still injected on every `shell.env` call.
- `HOTPOT_USERNAME` is injected when present in the environment.
- Git-derived usernames populate the session cache when the environment variable is absent.
- Prompt fallback occurs only when both environment and git sources are unavailable.
- No persistent configuration is modified.

## Open Decisions Closed During Brainstorming

- Scope: current session only.
- Injection: every `shell.env` invocation.
- Precedence: environment variable first, then git, then prompt.
- Prompt fallback: yes, even when git exists but yields no username.

## Expected Outcome

After the change, shell commands launched through this plugin should reliably receive `HOTPOT_USERNAME` within the current session while keeping the implementation local, minimal, and non-persistent.
