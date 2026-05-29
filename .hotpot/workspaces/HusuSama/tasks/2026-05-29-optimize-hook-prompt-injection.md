# Optimize Hook Prompt Injection

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| Done | true | 4 | high |
:::

---

## Task

### Summary

::: info
优化 Hotpot hooks 的 prompt 投递机制，降低每轮用户消息和工具调用带来的 token 消耗。执行 agent 需要保留现有 hook 命令入口和 Rust `Context` 中的 prompt 路径字段，但把模型可见的 hook 注入内容压缩为 `ROOT_DIR` 与 `HOTPOT_LANGUAGE`，并让其它 prompt 路径在 prompt 文案中通过 `ROOT_DIR` 拼接得到。
:::

### User Request

::: info 用户原始需求
用户希望优化当前 hooks 逻辑，解决两个问题：当前投递 prompt 过于频繁，容易造成大量 token 消耗；当前投递的 prompt 数据量过大。

用户要求：

- 修改 prompt 投送机制，在 `Edit|Write` 和使用 `hotpot` 命令前推送数据，不再在用户消息投送时投递 prompt。
- 修改 prompt 数据，Rust 代码中无需移除类似 `HOTPOT_NEW_PROMPT` 等变量，但推送的 prompt 中应该仅保留 `ROOT_DIR` 和 `HOTPOT_LANGUAGE`，在 prompt 内容中直接通过 `ROOT_DIR` 拼接其它文件路径，避免 hooks 投递大量 prompt 数据。
- 优化 Rust 代码中针对 hooks 的数据。

经确认，本任务采用建议边界：保留 hook 命令入口和完整 bootstrap/env 契约，重点压缩模型可见的 hook prompt 数据；不把 `hotpot hook codex pre-tool-use` 这类 hook 命令入口视为 env 压缩对象。
:::

### Approved Design

::: tip
本任务只优化模型可见 hook prompt，不移除 Rust `Context` 里的 `HOTPOT_NEW_PROMPT`、`HOTPOT_EXECUTE_PROMPT`、`HOTPOT_FINISH_WORK_PROMPT`、`HOTPOT_TDD_PROTOCOL_PROMPT`、review-memory prompt 字段，也不破坏 `hotpot hook bootstrap --format json/shell` 给 OpenCode/Pi 插件和 shell 命令使用的完整 env 契约。

实现方向：

1. Claude/Codex 不再通过 `UserPromptSubmit` 每轮用户消息投递 prompt 或语言上下文；改为在工具前触发。
2. 工具前触发范围覆盖 `Edit|Write`，并覆盖使用 `hotpot` 命令前的 Bash 场景。Claude 需要从当前只匹配 `Bash` 调整为能覆盖 `Edit|Write` 与 Bash 中的 `hotpot` 命令；Codex 需要从当前只匹配 `Edit|Write` 扩展为也覆盖 Bash 中的 `hotpot` 命令。
3. Rust hook 的模型可见上下文由完整 `context_lines(context)` 改为轻量上下文，只列出 `ROOT_DIR`、`HOTPOT_LANGUAGE` 和短语言指令。其它 prompt 路径不再逐项投递给模型，平台 prompt/skill 文案改为通过 `ROOT_DIR/.hotpot/prompts/<name>.md` 拼接定位。
4. `hotpot hook bootstrap`、OpenCode `shell.env`、Pi `injectHotpotEnv()` 的完整 env 注入暂时保留，避免破坏 review-memory 工具、Pi slash command builder、Codex skill 入口和现有脚本。
5. 更新 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md`，明确新的 hook 投递时机、轻量模型上下文和完整 bootstrap/env 契约之间的边界。
:::

### Alternatives Considered

- 全量压缩 bootstrap/env 契约：可以进一步减少 shell env 字段，但会影响 OpenCode/Pi 插件类型、Pi slash command 路径构造、review-memory 工具和 Codex skill 中的 env 引用，兼容风险较高，本任务不采用。
- 只改触发时机、不改 prompt 数据：风险较低，但无法解决当前投递数据量过大的核心问题，本任务不采用。
- 推荐并已批准的方案：保留完整 env 契约，仅压缩模型可见 hook prompt，并把 prompt 路径改为 `ROOT_DIR` 拼接；这能显著减少 token 消耗，同时兼容现有平台插件。

### Requirements

- Claude/Codex 不再在 `UserPromptSubmit` 时投递模型 prompt 数据。
- `Edit|Write` 前必须投递轻量 Hotpot 上下文。
- 使用 `hotpot` 命令前必须投递轻量 Hotpot 上下文。
- 模型可见 hook prompt 只列出 `ROOT_DIR` 和 `HOTPOT_LANGUAGE`，并包含必要的简短语言指令。
- Rust `Context` 中类似 `HOTPOT_NEW_PROMPT` 的字段无需移除。
- `hotpot hook bootstrap --format json/shell` 的完整 env 契约保持兼容，除非执行中发现必须调整并先向用户报告 blocker。
- 平台 prompt/skill 中需要 prompt 路径时，通过 `ROOT_DIR/.hotpot/prompts/<name>.md` 拼接，不依赖模型上下文里逐项投递的 `HOTPOT_*_PROMPT`。
- 同步更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`，因为本任务影响 execution flow / hook 注入架构。

### Non-Goals

::: details Non-Goals
- 不删除 `src/context.rs::Context` 中现有 `HOTPOT_*_PROMPT` 字段。
- 不移除 `hotpot hook bootstrap` 给 shell/plugin 使用的完整字段，除非后续单独任务批准。
- 不重写 Hotpot new/execute/finish-work 主流程。
- 不改变 Hotpot task ledger、issue candidates、VuePress 或 worktree 业务逻辑。
- 不引入新的跨语言依赖。
:::

### Project Context

当前关键文件和已知现状：

- `src/commands/hook.rs` 实现 Claude/Codex hook 输出、`print_shell_exports`、`context_lines`、`shell_export_assignments` 和相关测试。
- `src/context.rs` 定义公开 `Context` 字段，包括 `ROOT_DIR`、`HOTPOT_LANGUAGE`、多个 `HOTPOT_*_PROMPT` 和 VuePress env；这些字段是 OpenCode/Pi 插件读取的公共契约。
- `assets/platforms/claude/settings.json` 当前 `PreToolUse` 只匹配 `Bash`，并配置 `UserPromptSubmit` 调用 `hotpot hook claude user-prompt-submit`。
- `assets/platforms/codex/config.toml` 当前 `PreToolUse` 匹配 `Edit|Write`，并配置 `UserPromptSubmit` 调用 `hotpot hook codex user-prompt-submit`。
- `assets/platforms/opencode/plugins/hotpot-bash-before.ts` 当前通过 `shell.env` 向 Bash 注入完整 bootstrap JSON；OpenCode 没有同样的 UserPromptSubmit hook 投递模型上下文。
- `assets/platforms/pi/extensions/hotpot/index.ts` 当前 `pi.on("context", ...)` 每次 provider request 注入完整 Hotpot context 和语言指令；`tool_call` 对 bash 注入完整 env，并有 slash-command first-tool guard。
- `assets/platforms/codex/skills/hotpot-new/SKILL.md` 等 Codex skills 当前直接引用 `$HOTPOT_*_PROMPT`；执行时应改为指导通过 `$ROOT_DIR/.hotpot/prompts/...` 定位，避免依赖模型上下文逐项投递。
- `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md` 当前描述每个平台逐轮注入语言，包括 Claude/Codex `UserPromptSubmit` 与 Pi `context`；需要同步新的投递策略。

---

## Plan

### Mode

- tdd: true

### Execution Strategy

- git-worktree: false
- rationale: 用户选择在当前 checkout 执行。任务虽然影响多平台 hooks 和文档，但不使用隔离 worktree；执行 agent 必须尊重当前工作区已有改动，不要回滚非本任务修改。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/commands/hook.rs` | Modify | 调整 Claude/Codex hook 模型可见上下文、删除或弱化 UserPromptSubmit 投递、补充轻量 prompt helper 与测试。 |
| `assets/platforms/claude/settings.json` | Modify | 调整 PreToolUse matcher 覆盖 `Edit|Write` 和使用 `hotpot` 命令前的 Bash 场景，并移除 UserPromptSubmit hook 配置。 |
| `assets/platforms/codex/config.toml` | Modify | 调整 PreToolUse matcher 覆盖 `Edit|Write` 与 Bash/hotpot 场景，并移除 UserPromptSubmit hook 配置。 |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Modify | 压缩 Pi provider-context 中模型可见 Hotpot 数据，只保留 `ROOT_DIR`、`HOTPOT_LANGUAGE` 和必要工作流强化；保留 bash env 注入完整契约。 |
| `assets/platforms/opencode/plugins/hotpot-bash-before.ts` | Modify | 如需要，明确 `shell.env` 保留完整 env 是工具进程契约，不是模型 prompt 投递；避免新增模型可见大上下文。 |
| `assets/platforms/codex/skills/hotpot-new/SKILL.md` | Modify | 将 prompt 路径说明改为通过 `$ROOT_DIR/.hotpot/prompts/...` 拼接，减少对逐项 prompt env 的依赖。 |
| `assets/platforms/codex/skills/hotpot-execute/SKILL.md` | Modify | 同步 Codex execute skill 的 prompt 路径说明。 |
| `assets/platforms/codex/skills/hotpot-finish-work/SKILL.md` | Modify | 同步 Codex finish-work skill 的 prompt 路径说明。 |
| `docs/ARCH.md` | Modify | 更新 hook/prompt 投递架构、公共契约说明和未来 agent 注意事项。 |
| `docs/ARCH.zh_CN.md` | Modify | 同步中文架构文档。 |
| `docs/platforms/claude-code.md` | Modify | 如 matcher 或 UserPromptSubmit 策略变化需要平台说明，更新 Claude hook 参考。 |
| `docs/platforms/codex.md` | Modify | 如 matcher 或 UserPromptSubmit 策略变化需要平台说明，更新 Codex hook 参考。 |
| `docs/platforms/pi.md` | Modify | 更新 Pi context 注入只保留轻量模型上下文的说明。 |

### Implementation Tasks

#### Task 1: Lock Down Lightweight Hook Output Tests

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/commands/hook.rs` | Test | 先用失败测试固定新的轻量模型上下文契约。 |

##### Red

- [x] R1: 在 `src/commands/hook.rs` 的 `#[cfg(test)] mod tests` 中添加或调整测试 `claude_pre_tool_use_uses_lightweight_prompt_context`，构造 `Context::from_payload` 后调用 Claude PreToolUse 响应 helper，断言模型可见内容包含 `ROOT_DIR` 和 `HOTPOT_LANGUAGE`，且不包含 `HOTPOT_NEW_PROMPT`、`HOTPOT_EXECUTE_PROMPT`、`HOTPOT_FINISH_WORK_PROMPT`、`HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`。
- [x] R2: 在同一测试模块添加或调整测试 `codex_pre_tool_use_uses_lightweight_prompt_context`，断言 `build_codex_response(CodexHookCommand::PreToolUse, ...)` 的 `systemMessage` 和 `hookSpecificOutput.additionalContext` 都符合轻量上下文约束。
- [x] R3: 运行 `cargo test claude_pre_tool_use_uses_lightweight_prompt_context codex_pre_tool_use_uses_lightweight_prompt_context`；预期失败，失败原因应指向当前 hook 输出仍包含完整 prompt/env 路径字段或缺少新 helper。

##### Green

- [x] G1: 在 `src/commands/hook.rs` 中新增或重构一个小 helper，例如 `prompt_context_message(context, intro)` 或类似命名，用于模型可见 hook 输出，只生成 `ROOT_DIR`、`HOTPOT_LANGUAGE` 和短语言指令；保持双语 doc comment。
- [x] G2: 将 Claude/Codex `PreToolUse` 的模型可见输出改用轻量 helper，不再调用会列出完整字段的 `context_lines(context)`。
- [x] G3: 保留 `print_shell_exports`、`print_json`、OpenCode/Pi 所需 `Context` 序列化字段和 `shell_export_assignments` 的完整契约，除非测试显示必须局部调整。
- [x] G4: 运行 `cargo test claude_pre_tool_use_uses_lightweight_prompt_context codex_pre_tool_use_uses_lightweight_prompt_context`；预期通过。
- [x] G5: 运行 `cargo test hook::tests` 或 `cargo test commands::hook` 中实际可用的 hook 相关测试目标；预期无回归。

##### Refactor

- [x] F1: 检查 `context_lines`、`shell_context_message`、`codex_shell_context_message`、`language_directive_message` 是否命名仍准确；如果完整上下文只用于 env/export hint，应重命名或添加注释避免误用。已添加注释说明角色，命名仍准确。
- [x] F2: 如发生重构，重新运行 `cargo test hook::tests` 或对应 hook 测试命令；预期通过。否则标记 `skipped (no refactor)`。

:::

#### Task 2: Adjust Platform Hook Trigger Timing

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/platforms/claude/settings.json` | Modify | 移除用户消息投递，改为工具前投递。 |
| `assets/platforms/codex/config.toml` | Modify | 移除用户消息投递，扩展工具前投递范围。 |
| `src/commands/hook.rs` | Test | 覆盖 UserPromptSubmit 行为变化或命令枚举兼容。 |

##### Red

- [x] R1: 添加或调整测试 `codex_user_prompt_submit_is_no_longer_prompt_delivery`，明确 `CodexHookCommand::UserPromptSubmit` 不再返回包含 `systemMessage`/`additionalContext` 的 prompt 投递内容，或如果保留子命令兼容，则返回最小 allow/no-op JSON。
- [x] R2: 添加资产断言测试 `platform_hook_assets_do_not_register_user_prompt_submit_for_prompt_delivery`。测试读取 `assets/platforms/claude/settings.json` 和 `assets/platforms/codex/config.toml`，断言不再配置 `hotpot hook ... user-prompt-submit`。
- [x] R3: 添加资产断言测试 `platform_pre_tool_use_assets_cover_edit_write_and_hotpot_bash`。测试断言 Claude/Codex 的 PreToolUse matcher 或配置能覆盖 `Edit|Write` 和 Bash/hotpot 命令场景；如果平台 matcher 无法按命令内容过滤，应记录实现选择并用测试锁定实际配置。
- [x] R4: 运行 `cargo test codex_user_prompt_submit_is_no_longer_prompt_delivery platform_hook_assets_do_not_register_user_prompt_submit_for_prompt_delivery platform_pre_tool_use_assets_cover_edit_write_and_hotpot_bash`；预期失败。

##### Green

- [x] G1: 更新 `assets/platforms/claude/settings.json`：移除 `UserPromptSubmit` hook 配置；调整 `PreToolUse` 配置覆盖 `Edit|Write` 和使用 `hotpot` 命令前的 Bash 场景。若 Claude matcher 只能按工具名匹配，则配置为覆盖 `Bash|Edit|Write`，并在 hook 输出或文档中说明 Bash 侧只在 Hotpot 相关命令前有意义但平台无法更细过滤。
- [x] G2: 更新 `assets/platforms/codex/config.toml`：移除 `UserPromptSubmit` hook 配置；调整 `PreToolUse` matcher 覆盖 `Edit|Write` 和 Bash/hotpot 场景。若 Codex matcher 只能按工具名匹配，则使用 `Bash|Edit|Write` 或等效正则，并在文档中记录限制。
- [x] G3: 在 `src/commands/hook.rs` 中保留 `UserPromptSubmit` 子命令的向后兼容 no-op 输出，或如果删除子命令则同步 clap/tests/assets，避免已安装旧配置直接崩溃。优先选择兼容 no-op，因为旧项目可能还未运行 `hotpot update`。
- [x] G4: 运行 R4 中的三个测试；预期通过。
- [x] G5: 运行 `cargo test`；预期通过或仅暴露与本任务无关的既有失败，需记录。

##### Refactor

- [x] F1: 检查 hook enum doc comment、statusMessage 和测试名是否准确表达"用户消息不再投递 prompt，工具前投递轻量上下文"。
- [x] F2: 如发生重构，重新运行 `cargo test`；预期通过。否则标记 `skipped (no refactor)`。

:::

#### Task 3: Move Prompt Path Guidance To ROOT_DIR Composition

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/platforms/codex/skills/hotpot-new/SKILL.md` | Modify | Codex new skill 不再依赖模型上下文逐项投递 prompt env。 |
| `assets/platforms/codex/skills/hotpot-execute/SKILL.md` | Modify | Codex execute skill 同步路径拼接规则。 |
| `assets/platforms/codex/skills/hotpot-finish-work/SKILL.md` | Modify | Codex finish skill 同步路径拼接规则。 |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Modify | Pi provider-context 模型可见数据压缩；slash command 内部可继续使用完整 bootstrap context。 |
| `src/commands/hook.rs` | Test | 用资产测试锁定 prompt 路径说明。 |

##### Red

- [x] R1: 添加资产测试 `codex_skills_reference_prompt_paths_via_root_dir`，读取三个 Codex skill 文件，断言主流程 prompt 和 `@.hotpot/prompts/...` 替换说明使用 `$ROOT_DIR/.hotpot/prompts/<file>.md`，且不再把 `$HOTPOT_NEW_PROMPT`、`$HOTPOT_EXECUTE_PROMPT`、`$HOTPOT_FINISH_WORK_PROMPT`、`$HOTPOT_TDD_PROTOCOL_PROMPT` 作为模型必须依赖的路径来源。
- [x] R2: 添加资产测试 `pi_context_message_is_lightweight`，读取 `assets/platforms/pi/extensions/hotpot/index.ts`，断言 provider `context` system message 不再通过 `Object.entries(hotpot)` 展开完整 Hotpot context 给模型，而是显式列出 `ROOT_DIR` 与 `HOTPOT_LANGUAGE`。
- [x] R3: 运行 `cargo test codex_skills_reference_prompt_paths_via_root_dir pi_context_message_is_lightweight`；预期失败。

##### Green

- [x] G1: 更新三个 Codex skill 文件：把 "full workflow defined at `$HOTPOT_*_PROMPT`" 改为 "resolve as `$ROOT_DIR/.hotpot/prompts/hotpot-*.md` and Read"；所有 nested `@.hotpot/prompts/<name>.md` 映射也改为 `$ROOT_DIR/.hotpot/prompts/<name>.md`。
- [x] G2: 更新 `assets/platforms/pi/extensions/hotpot/index.ts` 的 `pi.on("context", ...)`：第一条 system message 只列 `ROOT_DIR`、`HOTPOT_LANGUAGE` 和简短说明；保留 `ensureContext`、`injectHotpotEnv`、slash command builder 内部使用完整 `HotpotContext` 的行为。
- [x] G3: 如 TypeScript 注释涉及"context 注入完整 env"，同步改为 "provider context 轻量，bash/tool env 完整"。
- [x] G4: 运行 R3 中的两个测试；预期通过。
- [x] G5: `cargo test` 覆盖资产断言：118 passed。

##### Refactor

- [x] F1: 检查 Codex skill 与 Pi extension 中是否仍有用户可见文案暗示每轮用户消息都会收到完整 env；修正不准确描述。所有引用已更新。
- [x] F2: 如发生重构，重新运行 R3/G5 的验证命令；预期通过。否则标记 `skipped (no refactor)`。

:::

#### Task 4: Update Architecture And Platform Documentation

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify | 架构说明必须反映新的 hook 投递机制。 |
| `docs/ARCH.zh_CN.md` | Modify | 中文架构说明同步。 |
| `docs/platforms/claude-code.md` | Modify | Claude hook matcher/UserPromptSubmit 策略变化需要平台说明。 |
| `docs/platforms/codex.md` | Modify | Codex hook matcher/UserPromptSubmit 策略变化需要平台说明。 |
| `docs/platforms/pi.md` | Modify | Pi provider context 从完整上下文改为轻量上下文。 |

##### Red

- [x] R1: 添加文档断言测试 `architecture_docs_describe_lightweight_hook_prompt_contract`，读取 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md`，断言包含新契约：模型可见 hook prompt 仅投递 `ROOT_DIR` 与 `HOTPOT_LANGUAGE`，prompt 路径通过 `ROOT_DIR/.hotpot/prompts/...` 拼接，完整 bootstrap/env 契约保留给 shell/plugin 使用。
- [x] R2: 添加文档断言测试 `platform_docs_do_not_claim_user_prompt_submit_prompt_delivery`，断言平台文档不再描述 Claude/Codex 通过 `UserPromptSubmit` 每轮投递 prompt 数据，并描述工具前触发范围。
- [x] R3: 运行 `cargo test architecture_docs_describe_lightweight_hook_prompt_contract platform_docs_do_not_claim_user_prompt_submit_prompt_delivery`；预期失败。

##### Green

- [x] G1: 更新 `docs/ARCH.md`：修改 public env-var contract 与 output-language/hook 注入段落，明确 `Context`/bootstrap 字段完整保留，但模型可见 hook prompt 是轻量上下文；Claude/Codex 不再在 `UserPromptSubmit` 投递 prompt。
- [x] G2: 更新 `docs/ARCH.zh_CN.md`，内容与英文版等价，使用简体中文，保留结构性英文 token。
- [x] G3: 更新 `docs/platforms/claude-code.md`、`docs/platforms/codex.md` 和 `docs/platforms/pi.md` 中与 hook/context 注入相关的说明。
- [x] G4: 运行 R3 中的两个测试；预期通过。
- [x] G5: 运行 `cargo test`；预期通过。

##### Refactor

- [x] F1: 检查文档中是否仍出现旧说法，例如 "UserPromptSubmit hooks each carry ROOT_DIR/HOTPOT_LANGUAGE" 或 "Pi context maps Object.entries(hotpot)" 等，并修正为新设计。所有引用已更新。
- [x] F2: 运行 `cargo test`；预期通过。否则标记 `skipped (no refactor)`。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo test` | 所有 Rust 单元测试和资产断言测试通过。 |
| `printf '{"cwd":"/Users/bytedance/RustProjects/hotpot","hook_event_name":"PreToolUse"}' \| cargo run -- hook codex pre-tool-use` | 输出有效 Codex hook JSON，模型可见内容只包含轻量 Hotpot context，不包含逐项 prompt 路径。 |
| `printf '{"cwd":"/Users/bytedance/RustProjects/hotpot","hook_event_name":"PreToolUse"}' \| cargo run -- hook claude pre-tool-use` | 输出有效 Claude hook JSON，`additionalContext` 只包含轻量 Hotpot context。 |
| `cargo run -- hook bootstrap --format json --root-dir /Users/bytedance/RustProjects/hotpot` | 仍输出完整 bootstrap JSON，包括现有 prompt path 字段，保持插件/env 契约兼容。 |
| `cargo run -- hook bootstrap --format shell --root-dir /Users/bytedance/RustProjects/hotpot` | 仍输出完整 shell exports，保持 Hotpot 命令环境兼容。 |

### Risks and Watchouts

::: warning
- Claude/Codex matcher 可能只能按工具名匹配，无法按 Bash 命令内容精确限制到 `hotpot`。如果平台不支持命令内容过滤，优先使用 `Bash|Edit|Write` 并在文档中说明限制，而不是伪造不可执行的 matcher。
- 已安装旧项目可能仍保留 `UserPromptSubmit` 配置；Rust 子命令应尽量保持 no-op 兼容，避免旧配置直接失败。
- Pi slash command first-tool guard 和 workflow reinforcement 是弱模型兼容的关键防线，不能因压缩 context 而删除。
- OpenCode/Pi 的完整 `shell.env` / bash env 注入仍是工具进程契约；不要把“模型 prompt 压缩”误改成破坏插件运行的 env 删除。
- 架构文档必须中英同步；只改英文或只改中文都会留下执行流误导。
:::

---

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task.
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff.
- Run the validation commands before reporting completion.
