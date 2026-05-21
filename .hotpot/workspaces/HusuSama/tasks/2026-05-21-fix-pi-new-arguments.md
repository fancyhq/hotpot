# fix-pi-new-arguments

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | false | 3 | medium |
:::

---

## Task

### Summary

::: info
修复 Pi 平台 `/hotpot-new <initial task idea>` 无法把命令后的用户输入传入 Hotpot new-task workflow 的问题。当前 Pi prompt template 只有 `argument-hint`，共享 `hotpot-new` workflow 因看不到初始想法而会继续追问，用户在 Pi 中表现为对话反复回答 `ready`、无法进入可用的任务创建流程。
:::

### User Request

::: info 用户原话
在 `pi` 中，使用 `hotpot-new` 命令，与 `pi` 的对话，会一直重复回答 `ready` ，无法使用，看是否需要在给 `pi` 的 `prompt` 中，插入 `$@` 类似的变量占位
:::

用户已批准推荐方案：修改 Pi `hotpot-new` prompt template，使用户在 `/hotpot-new ...` 后输入的初始任务想法通过 Pi 模板变量传入共享 workflow；补充资产内容回归测试；更新 Pi 平台文档说明 Hotpot prompt templates 必须显式包含 `$ARGUMENTS` / `$@` 这类变量。

### Approved Design

::: tip
采用最小跨平台风险修复：只改变 Pi 的 `hotpot-new` prompt template，不改变共享 `assets/prompts/hotpot-new.md` 的通用流程，也不改变 Claude / OpenCode / Codex 资产。

实现应在 `assets/platforms/pi/prompts/hotpot-new.md` 的模板正文中显式注入 Pi 模板参数，例如新增 `Initial task idea from command arguments: $ARGUMENTS` 及简短说明：如果该值非空，必须把它当作用户提供的初始任务想法；如果为空，才按共享 workflow 的规则询问一次初始任务想法。这样 `/hotpot-new 修复 pi 参数传递` 会把自然语言参数直接带进模型上下文。

测试应覆盖资产内容本身，防止以后重写 Pi template 时再次丢失参数占位。执行代理可在 `src/assets/platforms/pi.rs` 增加模块内单元测试，使用 `include_str!(...)` 或已注册 asset 内容断言 `.pi/prompts/hotpot-new.md` 包含 `$ARGUMENTS`，并包含把非空 arguments 当作 initial task idea 的指令。测试不需要启动真实 Pi。

文档应更新 `docs/platforms/pi.md`，在 prompt template 规则或 Hotpot implication 附近说明：Pi 模板不会自动把命令参数附加给模型；模板正文必须显式引用 `$ARGUMENTS` / `$@` / `$1` 等变量，否则 slash 命令后的文字只会显示在 UI hint 中或被丢失，Hotpot `hotpot-new` 会看不到初始 idea。
:::

### Alternatives Considered

- 只修改 `assets/platforms/pi/prompts/hotpot-new.md`：最快，但没有回归保护，后续资产重写容易再次丢失 `$ARGUMENTS`。
- 同时审计并修改 `hotpot-execute` / `hotpot-finish-work`：可能有价值，但本次用户症状只指向 `hotpot-new` 的初始任务想法缺失；扩大范围会增加无关行为变更风险。
- 在 Pi extension 中用 `pi.registerCommand()` 包装 `/hotpot-new` 并手动 `sendUserMessage(args)`：控制力更强，但比 prompt template 修复更复杂，也偏离当前 Pi 资产采用 prompt template 的设计。
- 推荐方案：在 Pi `hotpot-new` template 中显式注入 `$ARGUMENTS`，补资产测试和平台文档。该方案改动最小，直接匹配 Pi 文档的模板变量规则，并覆盖本次问题。

### Requirements

- `/hotpot-new <文本>` 在 Pi 中必须把 `<文本>` 明确传入 Hotpot new-task workflow，作为 initial task idea 使用。
- 当 `$ARGUMENTS` 为空时，仍保持共享 workflow 的默认行为：询问一次初始任务想法。
- 不修改共享 `assets/prompts/hotpot-new.md` 的跨平台流程，除非执行中发现必须同步澄清；如需修改共享流程，必须同时检查其它平台影响。
- 添加回归测试，验证 Pi `hotpot-new` 资产包含参数变量和使用说明。
- 更新 `docs/platforms/pi.md`，记录 Pi prompt templates 必须显式引用参数变量。
- 保持代码输出、测试断言消息为 English；文档和 Rust doc comments 如新增应遵循项目双语注释约定。

### Non-Goals

::: details Non-Goals
- 不修复 Pi 自身的 UI 或模型行为。
- 不新增 Pi subagent 能力；Pi 仍按同会话 fallback 运行。
- 不重构 Hotpot prompt asset 安装系统。
- 不改变 Claude、OpenCode、Codex 的 `new` 命令行为。
- 不在本任务中实现真实 Pi 端到端自动化测试。
- 不创建单独计划文件；本文件就是执行交接文档。
:::

### Project Context

- Hotpot 是跨平台任务编排工具，Pi 平台资产位于 `assets/platforms/pi/`，安装登记位于 `src/assets/platforms/pi.rs`。
- Pi `hotpot-new` 模板当前为 `assets/platforms/pi/prompts/hotpot-new.md`，frontmatter 有 `argument-hint: "[initial task idea]"`，但正文没有 `$ARGUMENTS` / `$@` / `$1`，因此命令后的参数没有进入模型可见 prompt。
- `docs/platforms/pi.md` 已记录 Pi prompt template 支持变量：`$1`、`$2`、`$@`、`$ARGUMENTS`、`${@:N}`、`${@:N:L}`，并给出示例 `Arguments: $ARGUMENTS`。
- `.hotpot/prompts/hotpot-new.md` 的共享 workflow 明确要求：如果命令后有文本，视为 initial task idea；如果没有，才询问初始任务想法。
- 当前仓库没有 `tests/` 目录，单元测试主要以内联 `#[cfg(test)]` 模块存在；可在 `src/assets/platforms/pi.rs` 加小型 asset 内容测试。
- 按 `AGENTS.md`，如修改执行流文档需更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`；本任务主要修复 Pi prompt 参数传递，不改变 Hotpot 总体执行流。若执行代理判断这是平台行为约束，应只在必要时补充架构文档。

---

## Plan

### Mode

- tdd: false

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/platforms/pi/prompts/hotpot-new.md` | Modify | 显式注入 Pi 命令参数，指导 agent 把非空 `$ARGUMENTS` 当作 initial task idea。 |
| `src/assets/platforms/pi.rs` | Test | 增加资产内容回归测试，防止 Pi `hotpot-new` 模板再次丢失参数占位。 |
| `docs/platforms/pi.md` | Modify | 记录 Hotpot Pi prompt templates 必须显式引用参数变量，否则命令参数不会传入模型上下文。 |
| `docs/ARCH.md` | Modify | 仅当执行代理确认该修复改变或澄清架构级 Pi 平台约束时更新。 |
| `docs/ARCH.zh_CN.md` | Modify | 若更新 `docs/ARCH.md`，必须同步更新中文架构文档。 |

### Implementation Tasks

#### Task 1: Fix Pi hotpot-new argument injection

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/platforms/pi/prompts/hotpot-new.md` | Modify | 让 Pi prompt template 正文包含用户命令参数。 |

**Steps:**

- [x] **Step 1**: Inspect `assets/platforms/pi/prompts/hotpot-new.md` and confirm the body currently lacks `$ARGUMENTS`, `$@`, or `$1` outside documentation examples.
- [x] **Step 2**: Add a concise section near the invocation line, before reading `$HOTPOT_NEW_PROMPT`, with wording equivalent to: `Initial task idea from command arguments: $ARGUMENTS`.
- [x] **Step 3**: Add an explicit instruction: if the command arguments value is non-empty, treat it as the user's initial task idea for the shared workflow; if empty, follow the shared workflow and ask one concise question.
- [x] **Step 4**: Keep the existing substitution table, Pi no-subagent note, and output-language section intact.
- [x] **Step 5**: Ensure the final template remains valid Pi prompt template markdown with the existing frontmatter and `argument-hint`.

:::

#### Task 2: Add regression coverage for the Pi template asset

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/assets/platforms/pi.rs` | Test | Assert the bundled Pi new-task template preserves argument injection. |

**Steps:**

- [x] **Step 1**: Inspect existing inline test style in nearby modules such as `src/commands/mod.rs`, `src/context.rs`, or `src/assets/merge.rs`.
- [x] **Step 2**: Add a small `#[cfg(test)] mod tests` to `src/assets/platforms/pi.rs` if none exists.
- [x] **Step 3**: Add test `pi_new_prompt_template_passes_command_arguments` that loads the bundled `hotpot-new.md` asset content and asserts it contains `$ARGUMENTS`.
- [x] **Step 4**: In the same test, assert the content includes stable English phrases that instruct the agent to treat non-empty command arguments as the initial task idea and to ask only when empty. Prefer checking short phrase fragments rather than the whole paragraph.
- [x] **Step 5**: Run `cargo test pi_new_prompt_template_passes_command_arguments`; expect the new test to pass.

:::

#### Task 3: Document and validate the Pi behavior

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/platforms/pi.md` | Modify | Make the Pi prompt variable requirement discoverable for future platform work. |
| `docs/ARCH.md` | Modify | Only if a concise architecture note is warranted after implementation. |
| `docs/ARCH.zh_CN.md` | Modify | Keep bilingual architecture docs synchronized if `docs/ARCH.md` changes. |

**Steps:**

- [x] **Step 1**: Update `docs/platforms/pi.md` near the prompt template rules or Hotpot implication section to state that Pi does not implicitly append slash-command arguments to prompt templates; template authors must place `$ARGUMENTS`, `$@`, or positional variables in the body.
- [x] **Step 2**: Mention the Hotpot-specific consequence: `hotpot-new` needs this so command text becomes the initial task idea instead of triggering another clarification loop.
- [x] **Step 3**: Decide whether `docs/ARCH.md` / `docs/ARCH.zh_CN.md` need a small Pi platform note. If no execution-flow or architecture contract changed, leave them untouched and mention that decision in the final report.
- [x] **Step 4**: Run `cargo test`; expect all tests to pass. Existing warnings about currently unused methods are acceptable only if they predate this task and no new warnings are introduced.
- [x] **Step 5**: Run `cargo run -- init --platform pi --dry-run` or the closest available dry-run/update command if supported; expect the Pi asset list to include `.pi/prompts/hotpot-new.md` without errors. If no dry-run exists for `init`, run `cargo run -- --help` / `cargo run -- init --help` to identify a safe validation command and report the limitation.

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo test pi_new_prompt_template_passes_command_arguments` | Passes and proves the Pi `hotpot-new` asset contains argument injection. |
| `cargo test` | Passes without regressions. |
| `cargo run -- init --platform pi --dry-run` | If supported, completes and shows Pi assets can still be processed. |
| Manual review of `assets/platforms/pi/prompts/hotpot-new.md` | Confirms `/hotpot-new <initial task idea>` will expose the text through `$ARGUMENTS` before the shared workflow is followed. |

### Risks and Watchouts

::: warning
- Pi template variable expansion syntax must match Pi docs exactly. Prefer `$ARGUMENTS` because `docs/platforms/pi.md` documents it as the full argument string.
- Do not rely on `argument-hint`; it is autocomplete help, not a model-visible argument payload.
- Avoid overfitting tests to the entire template text; assert stable intent-bearing fragments so copy edits do not cause noisy failures.
- `src/assets/platforms/pi.rs` is under 1000 lines; adding a small test module is safe and does not require splitting.
- If updating `docs/ARCH.md`, update `docs/ARCH.zh_CN.md` in the same change to satisfy project rules.
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
