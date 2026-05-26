# HOTPOT AGENTS

## Compatibility

`HOTPOT` should maintain broad compatibility. When adapting to different agent tools, use these files to understand each platform's basic configuration format.

- For `claude code` compatibility, reference @docs/platforms/claude-code.md
- For `opencode` compatibility, reference @docs/platforms/opencode.md
- For `codex` compatibility, reference @docs/platforms/codex.md
- For `pi` compatibility, reference @docs/platforms/pi.md

## Design

You must first read the @docs/ARCH.md document to understand the project's current execution flow and architecture. When the features you add or modify affect the execution flow, you must update the @docs/ARCH.md and @docs/ARCH.zh_CN.md documents.

- `ARCH.md`: intended for `agent` consumption and native English readers; write it in English.
- `ARCH.zh_CN.md`: has the same content as `ARCH.md` but written in Simplified Chinese for native Chinese readers; write it in Simplified Chinese.

## Conventions

- Every function, file, etc. you write must include `doc` comments. Important parameters, and any code whose purpose may be ambiguous, should also be commented. Comments use a bilingual Chinese + English style, for example:

```rust
//! <English description of the module.>
//!
//! <中文模块描述>

/// <English short description of the function.>
/// <中文简短描述>
///
/// <English extended description, e.g. parameter descriptions, examples, etc.>
fn some_function(arg1: String, arg2: i32) -> Result<String> {
    // <English description>
    // <中文描述>
    todo!()
}
```

- Output in code must use English, for example:
- If a file exceeds 1000 lines, consider whether it should be split by functionality while remaining in the same module. **User confirmation is required before splitting.**

```rust
// bad
println!("你好")
bail!("你好")

// good
println!("Hello")
bail!("Hello")
```

## Notes

- When writing files, use the binary name `hotpot`.
- The project should minimize dependencies on other programming languages unless a compatible `Agent` tool must use that language, such as the `typescript` used by `opencode`. For languages without such restrictions, prefer `shell` scripts and keep Windows compatibility in mind.
- `hotpot` is a global command-line tool and requires the `agent` to provide environment variables such as `ROOT_DIR` through hooks or similar mechanisms in order to work correctly. Be mindful of these dependencies.
