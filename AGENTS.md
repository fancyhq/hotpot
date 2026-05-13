# HOTPOT AGENTS

## Compatibility

`HOTPOT` should maintain broad compatibility. When adapting to different agent tools, use these files to understand each platform's basic configuration format.

- For `claude code` compatibility, reference @docs/platforms/claude-code.md
- For `opencode` compatibility, reference @docs/platforms/opencode.md
- For `codex` compatibility, reference @docs/platforms/codex.md
- For `pi` compatibility, reference @docs/platforms/pi.md

## Notes

- When writing files, use the binary name `hotpot`; when testing, use `cargo run --` instead of `hotpot`.
- Every function and every newly created file should include `doc` comments. Important parameters and code whose purpose is unclear should also be documented.
- The project should minimize dependencies on other programming languages unless a compatible `Agent` tool must use that language, such as the `typescript` used by `opencode`. For languages without such restrictions, prefer `shell` scripts and keep Windows compatibility in mind.
- `hotpot` is a global command-line tool and requires the `agent` to provide environment variables such as `ROOT_DIR` through hooks or similar mechanisms in order to work correctly. Be mindful of these dependencies.
