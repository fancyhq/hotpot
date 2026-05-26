# migrate-pi-commands-to-extension

## Task

### Summary

把 Pi 三个 Hotpot slash 命令（`/hotpot-new` / `/hotpot-execute` / `/hotpot-finish-work`）从 prompt template thin shell 升级到 Pi extension command（`pi.registerCommand` + `ctx.sendUserMessage`）。前一任务 `strengthen-pi-new-arguments-prompt`（commit `628f586`）已证明 prompt body 强化在加载 AGENTS.md 的 Pi 项目里会被整体当作背景文档吸收——AI 收到的 `/hotpot-new <text>` 不被识别为 user active request，回退到泛化问候。解决路径是把"用户的请求"用 `ctx.sendUserMessage` 作为**实际 user 消息**发送，避开 prompt template 被系统级吸收的失效模式。同时一次性清理三份废弃 `.pi/prompts/hotpot-*.md` thin shell（含本仓 + 其他用户项目通过 `hotpot init` 触发的 one-shot cleanup）。

### User Request

> 继续修复 pi

补充确认（brainstorm）：
- 范围：三个命令一起升级（预防性，避免后续 execute / finish-work 重复同样回归）。
- 旧装 prompt template：彻底删除，并加入 `hotpot init --platform pi` 的 one-shot cleanup。
- TDD：Skip（TypeScript 部分无测试栈；Rust 部分用普通单元测试覆盖 cleanup fn 即可）。

### Approved Design

#### Architecture transition

```
旧：用户键入 /hotpot-new <text>
    → Pi 解析 .pi/prompts/hotpot-new.md 模板
    → 模板内容（包括 $ARGUMENTS、framing）作为「prompt」推给 AI
    → 在加载 AGENTS.md 的项目里被 AI 整体当背景文档，回退到泛化问候

新：用户键入 /hotpot-new <text>
    → Pi 触发 pi.registerCommand("hotpot-new", handler)
    → handler 拼一段 user-voice 消息（含 args 包裹、强制 framing、$HOTPOT_NEW_PROMPT 绝对路径、@path 替换表、Platform note、空参数 fallback）
    → 通过 ctx.sendUserMessage(text) 作为「user 消息」发送
    → AI 必须响应这条 user 消息，直接进入 workflow
```

#### Pi extension command handler 消息 schema

handler 在拼消息时**必须从 `HotpotContext` 解析绝对路径**，不要让 AI 自己 echo env var——绝对路径直接给 AI，最少一步操作。

`/hotpot-new` 消息模板（伪代码）：

```
=== USER ACTIVE REQUEST ===
I just invoked `/hotpot-new` in my Pi session. Begin the Hotpot new-task workflow now.

<<< INITIAL TASK IDEA (verbatim from `/hotpot-new` arguments) >>>
${args}
<<< END INITIAL TASK IDEA >>>

The block above IS my initial task idea. Do not ask me again — proceed directly to brainstorming using it as the starting point.

Read the full workflow body at this absolute path first:
  ${hotpot.HOTPOT_NEW_PROMPT}

Pi has no `@path` expansion. When the workflow body references `@.hotpot/prompts/<name>.md`, substitute the matching absolute path and use `Read`:
- @.hotpot/prompts/output-language.md       → ${hotpot.ROOT_DIR}/.hotpot/prompts/output-language.md
- @.hotpot/prompts/tdd-protocol.md          → ${hotpot.HOTPOT_TDD_PROTOCOL_PROMPT}
- @.hotpot/prompts/record-issue-candidate.md → ${hotpot.HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT}
- @.hotpot/prompts/summarize-issue-candidates.md → ${hotpot.HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT}
- @.hotpot/prompts/get-issue.md             → ${hotpot.ROOT_DIR}/.hotpot/prompts/get-issue.md
- @.hotpot/prompts/hotpot-execute.md        → ${hotpot.HOTPOT_EXECUTE_PROMPT}
- @.hotpot/prompts/hotpot-finish-work.md    → ${hotpot.HOTPOT_FINISH_WORK_PROMPT}

Pi has no dedicated subagents — run execution and review phases as strictly separated phases in the same session, announcing each phase explicitly.

**Exception (empty arguments)**: If the `INITIAL TASK IDEA` block above contains no non-whitespace text, ask me exactly one concise question to obtain the initial task idea instead of proceeding to brainstorming.
=== END USER ACTIVE REQUEST ===
```

空参数分支：handler 在 `args.trim() === ""` 时仍走同一份模板（Exception 段会触发；handler 不需要二次分支，模板内部覆盖了），保持单条码路径。

`/hotpot-execute` 与 `/hotpot-finish-work` 的消息只调整以下点：
- 命令名引用：`/hotpot-new` → `/hotpot-execute` / `/hotpot-finish-work`
- 分隔块标签：`INITIAL TASK IDEA` → `EXECUTION NOTES` / `FINISH NOTES`
- workflow body 路径：`HOTPOT_NEW_PROMPT` → `HOTPOT_EXECUTE_PROMPT` / `HOTPOT_FINISH_WORK_PROMPT`
- "begin the Hotpot new-task workflow" → "begin the Hotpot execute workflow" / "begin the Hotpot finish-work workflow"
- `/hotpot-execute` 的 `@path` 替换表去掉 `hotpot-execute.md` / `hotpot-finish-work.md` 两项（因为 execute body 不引用它们）；保留 `output-language.md` / `tdd-protocol.md` / `record-issue-candidate.md` / `summarize-issue-candidates.md` / `get-issue.md`
- `/hotpot-finish-work` 的 `@path` 替换表只保留 `output-language.md` / `summarize-issue-candidates.md` / `get-issue.md` / `hotpot-execute.md`

为减少 handler 内消息文本的重复度，三个 handler 应共享一个内部 helper `buildPiCommandMessage(cmd, args, hotpot, options)`，options 携带分隔块标签、workflow body env-var、`@path` 子集等差异。

#### One-shot cleanup of deprecated thin shells

`src/assets/platforms/pi.rs` 提供 `cleanup_deprecated_pi_prompts(root: &Path) -> Result<()>`：
- 删除 `<root>/.pi/prompts/hotpot-new.md`、`<root>/.pi/prompts/hotpot-execute.md`、`<root>/.pi/prompts/hotpot-finish-work.md` 三个具体路径
- 任何文件不存在时跳过该路径（不报错）
- 文件存在时 `std::fs::remove_file(&path)?` 删除
- 整个 fn 整体返回 `Result<()>`；任一 `remove_file` 真正 IO 失败时返回 `Err`，触发 `hotpot init` 上层报错（避免静默丢错）
- 该 fn 在 Pi platform install/update 流程末尾被调用，调用点位于 `src/commands/init/pi.rs` 或等价的 Pi platform install entry

不影响其他平台的 `.pi/` 之外的目录；不递归扫描；不删非 Hotpot 文件。

#### Asset list edits

`src/assets/platforms/pi.rs::ASSETS`：移除三条 `Asset::owned(".pi/prompts/hotpot-new.md", ...)` / `hotpot-execute.md` / `hotpot-finish-work.md`。保留 `.pi/extensions/hotpot/index.ts`。

`src/assets/platforms/pi.rs` 既有测试：
- `pi_new_prompt_template_passes_command_arguments` —— 删除（断言对象已不存在）
- `pi_prompt_source_and_installed_template_stay_in_sync` —— 删除（断言对象已不存在）

新增测试 `cleanup_deprecated_pi_prompts_*`（见 Plan）。

#### Repository file deletions

`assets/platforms/pi/prompts/hotpot-new.md`、`hotpot-execute.md`、`hotpot-finish-work.md` 三份源资产 git 删除。若 `assets/platforms/pi/prompts/` 目录变空，删目录。

### Alternatives Considered

- **方案 A（前一任务路径）prompt body 层强化**：失败。AGENTS.md 等竞争上下文会把整段 prompt 当背景文档吸收，AI 不进入 workflow。本任务的设计就是从此处升级而来。
- **方案 C（保留旧 prompt template 做 fallback）**：未知风险。Pi 在 prompt template 与 extension command 同名时的解析顺序无文档化保证；既可能 extension command 胜出（旧 prompt 永远不触发，等于无效残留），也可能两者并发触发（产生重复或冲突回应）。本任务选择彻底删除旧 prompt template 以消除歧义。
- **方案 D（在通用资产引擎里加 deprecated_paths 机制）**：长期复用价值高，但本任务只需要 retire 三个具体路径，普通引擎扩展属于过度抽象；Pi platform 专属的 cleanup fn 已足够，未来如有其他平台同类需求再抽公共层。
- **推荐方案（B + cleanup）**：三命令全部走 extension command 体系；旧 prompt template 通过 Pi platform install 末尾 one-shot 调用 cleanup fn 抹除。被采用。

### Requirements

- `/hotpot-new <text>` 在加载 AGENTS.md 的真实 Pi 会话里：AI 首条 brainstorm 消息必须**显式引用或释义** `<text>`，不得追问 initial idea。
- `/hotpot-new`（空参数）在真实 Pi 会话里：AI 第一条消息必须是 "ask exactly one concise question to obtain the initial task idea" 形式（不能直接进入 brainstorm）。
- `/hotpot-execute` 在真实 Pi 会话里：AI 必须从 `$HOTPOT_EXECUTE_PROMPT` 读取 workflow 并进入 execute 流程（带 args 时把 args 当 execution notes 注入；空 args 时仍正常进入但不追问）。
- `/hotpot-finish-work` 在真实 Pi 会话里：AI 必须从 `$HOTPOT_FINISH_WORK_PROMPT` 读取 workflow 并进入 finish-work 流程。
- 三个 handler 共享 helper 减少消息文本漂移面。
- 任意 Pi 项目跑 `hotpot init --platform pi` 后，`.pi/prompts/hotpot-{new,execute,finish-work}.md` 三个旧 thin shell 自动被删除（若存在）；不存在时无错。
- `cargo test` 全量通过；不增引 warning。
- 不动 `assets/prompts/hotpot-*.md` 共享 body；不动其他平台资产。
- `docs/platforms/pi.md` 更新为反映新架构；`docs/ARCH.md` + `docs/ARCH.zh_CN.md` 同步更新 Pi 行的描述（这次确实改变了跨平台执行流契约）。
- 代码与测试断言保持 English；文档遵循双语注释约定（Rust doc 双语；`docs/platforms/pi.md` 用英文段；ARCH 双语同步）。

### Non-Goals

- 不动其他平台（Claude / OpenCode / Codex）的 new / execute / finish-work 资产。
- 不动共享 workflow body（`assets/prompts/hotpot-*.md`）。
- 不动 Pi extension 中现有的 record/read/clear `issue_candidate` 工具、`pi.on("context", ...)` 推送、`pi.on("tool_call", ...)` env-var export、`pi.on("session_shutdown", ...)` VuePress cleanup 等既有逻辑。
- 不引入 TypeScript 测试基础设施（项目原本不存在 TS lint/test）；本任务交付不带 TS 自动化测试。
- 不实施 Pi 端到端自动化测试——靠用户在真实 Pi 会话端到端验证。
- 不修复 Pi 自身 UI 或推理模型行为。
- 不为通用资产引擎引入 deprecated_paths 通用机制（见 Alternatives D）。

### Project Context

- Pi extension 源：`assets/platforms/pi/extensions/hotpot/index.ts`；安装拷贝：`.pi/extensions/hotpot/index.ts`。
- Pi extension 已有 `ensureContext(cwd)` / `HotpotContext` 类型与 bootstrap 流程；本任务直接复用。
- Pi extension API：`pi.registerCommand(name, { description, handler })`，handler 参数 `(args: string, ctx)`；`ctx.sendUserMessage(text: string)` 把 text 当 user 消息发送（参见 `docs/platforms/pi.md` 现有示例）。
- 资产清单：`src/assets/platforms/pi.rs::ASSETS` 是 `Asset::owned` 项列表；三份 prompt template 是 `Asset::owned(".pi/prompts/hotpot-*.md", include_str!(...))` 风格。
- Pi platform install entry：`src/commands/init/pi.rs`（或类似——执行时具体路径让 execution agent 通过 `grep` 在 `src/commands/init/` 下确认）。需要把 `cleanup_deprecated_pi_prompts` 调用接进该 entry 的末尾，使 `hotpot init --platform pi` / `hotpot update` 都触发。
- `assets/platforms/pi/prompts/` 目前仅有三份待删 prompt（这点在 `ls -la` 中已验证）；执行时再次 verify。
- `HotpotContext` 字段包括 `ROOT_DIR`、`HOTPOT_NEW_PROMPT`、`HOTPOT_EXECUTE_PROMPT`、`HOTPOT_FINISH_WORK_PROMPT`、`HOTPOT_TDD_PROTOCOL_PROMPT`、`HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`、`HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`，刚好覆盖三个 handler 需要替换的所有 `@path`。
- 共享 body `.hotpot/prompts/hotpot-{new,execute,finish-work}.md` 内有 `@.hotpot/prompts/...` 形式引用——helper 必须把这些列在 `@path` 替换表里（参考前一任务的 Pi thin shell `.pi/prompts/hotpot-new.md` 替换表，逐项继承）。
- `docs/ARCH.md` 当前 Pi 平台描述："Pi: `.pi/prompts/hotpot-*.md` + `.pi/extensions/`（no native subagents — single-session phased execution; review phase stays read-only）"——需要更新为 "Pi: `.pi/extensions/`（slash commands via `pi.registerCommand`；no prompt-template thin shells；no native subagents）"。
- `docs/ARCH.zh_CN.md` 对应位置同步双语更新。
- 仓库 `cargo test` 基线：86 个测试通过；当前 3 个 `dead_code` warning 是基线不算回归。删除两个 prompt-template 测试后，total 应降到 84。
- `docs/platforms/pi.md` 之前 6 句关于"three-part pattern + Exception + USER ACTIVE REQUEST framing"的整段需要替换为"Pi extension command 架构"说明——这块内容已在前一任务 `strengthen-pi-new-arguments-prompt` 中沉淀过，本任务把它精简为"已废弃，现走 extension command"+ 指向新章节。

## Plan

### Mode

- tdd: false

### File Map

- Modify: `assets/platforms/pi/extensions/hotpot/index.ts` — 新增三个 `pi.registerCommand` 注册 + `buildPiCommandMessage` helper；保留所有既有逻辑。
- Modify: `.pi/extensions/hotpot/index.ts` — 与 asset 源完全同步（asset 引擎应自动 reinstall，但本任务 verify diff 为空）。
- Modify: `src/assets/platforms/pi.rs` — 从 `ASSETS` 移除三条 prompt template；删除既有 `pi_new_prompt_template_passes_command_arguments` 与 `pi_prompt_source_and_installed_template_stay_in_sync` 两个测试；新增 `cleanup_deprecated_pi_prompts` fn 及单元测试。
- Modify: `src/commands/init/pi.rs`（执行时 `grep` 验证准确路径）— 在 Pi platform install 流程末尾调用 `cleanup_deprecated_pi_prompts(root)?`。
- Modify: `docs/platforms/pi.md` — 删除上一任务沉淀的 prompt template 三 / 四要素段；新增"Hotpot Pi extension commands"小节描述 `pi.registerCommand` 入口、消息 schema、cleanup 行为。
- Modify: `docs/ARCH.md` — 更新 Pi 平台行描述。
- Modify: `docs/ARCH.zh_CN.md` — 同步中文 Pi 行描述。
- Delete: `assets/platforms/pi/prompts/hotpot-new.md`、`hotpot-execute.md`、`hotpot-finish-work.md` 三份源资产。
- Delete（条件）: `assets/platforms/pi/prompts/` 目录（如果删完三个文件后空了）。
- 不动: `assets/prompts/hotpot-*.md`、`assets/platforms/{claude,opencode,codex}/`、其他 Pi extension 既有逻辑。

### Implementation Tasks

#### Task 1: Add `pi.registerCommand` handlers + shared helper in Pi extension

**Files:**

- Modify: `assets/platforms/pi/extensions/hotpot/index.ts`
- Modify: `.pi/extensions/hotpot/index.ts`

- [x] Step 1: 打开 `assets/platforms/pi/extensions/hotpot/index.ts`，在 `export default function hotpotExtension(pi: ExtensionAPI) {` 内 `pi.on("session_shutdown", ...)` 之前的合适位置（既有 hooks 之间或之后均可，**不要破坏既有 hooks 顺序**），新增三个 `pi.registerCommand` 注册块（见 Approved Design "Pi extension command handler 消息 schema"）。
- [x] Step 2: 在 `export default function` 外（文件顶部 helper 区，紧邻 `bootstrapHotpot` / `ensureJsonlFile` 等既有 helper）新增 `buildPiCommandMessage(opts: { command, args, hotpot, ideaBlockLabel, workflowPromptPath, atPathRefs })`，返回拼好的消息 string。`atPathRefs` 是 `{ shortRef: string; absolutePath: string }[]`。
- [x] Step 3: 三个 handler 分别调用 `buildPiCommandMessage` 时传不同 `command` / `ideaBlockLabel` / `workflowPromptPath` / `atPathRefs`：
  - `hotpot-new`：label `INITIAL TASK IDEA`；workflowPromptPath `hotpot.HOTPOT_NEW_PROMPT`；atPathRefs 包含 output-language / tdd-protocol / record-issue-candidate / summarize-issue-candidates / get-issue / hotpot-execute / hotpot-finish-work 全部 7 项。
  - `hotpot-execute`：label `EXECUTION NOTES`；workflowPromptPath `hotpot.HOTPOT_EXECUTE_PROMPT`；atPathRefs 5 项（去掉 hotpot-execute / hotpot-finish-work）。
  - `hotpot-finish-work`：label `FINISH NOTES`；workflowPromptPath `hotpot.HOTPOT_FINISH_WORK_PROMPT`；atPathRefs 4 项（去掉 tdd-protocol、record-issue-candidate、hotpot-finish-work；保留 output-language / summarize-issue-candidates / get-issue / hotpot-execute）。
- [x] Step 4: handler 实现中先 `const hotpot = await ensureContext(ctx.cwd);` → 再 `const text = buildPiCommandMessage(...);` → `await ctx.sendUserMessage(text);`。注意 args 是字符串可能为空，handler 不做 trim 早返回——所有空参数行为由消息文本内的 `Exception (empty arguments)` 段处理。
- [x] Step 5: 给三个 `pi.registerCommand` 块加 `description` 字段（描述简短英文，与既有 prompt template frontmatter `description` 对齐）。
- [x] Step 6: 给新增 helper / handler 加双语 JSDoc 注释（英文段 + 中文段），与文件既有注释风格一致。
- [x] Step 7: 把同样改动复制到 `.pi/extensions/hotpot/index.ts`（asset 源与 installed 一致）。
- [x] Step 8: 用 `diff -u assets/platforms/pi/extensions/hotpot/index.ts .pi/extensions/hotpot/index.ts` 验证两份完全一致；expect 空输出。

#### Task 2: Remove deprecated Pi prompt template assets

**Files:**

- Modify: `src/assets/platforms/pi.rs`
- Delete: `assets/platforms/pi/prompts/hotpot-new.md`
- Delete: `assets/platforms/pi/prompts/hotpot-execute.md`
- Delete: `assets/platforms/pi/prompts/hotpot-finish-work.md`
- Delete（条件）: `assets/platforms/pi/prompts/` 目录（empty 时）

- [x] Step 1: 用 `grep -n 'hotpot-new\|hotpot-execute\|hotpot-finish-work' src/assets/platforms/pi.rs` 定位 `ASSETS` 数组中三条 `Asset::owned(".pi/prompts/hotpot-*.md", include_str!(...))` 项；删除这三项（保留 `.pi/extensions/hotpot/index.ts` 项）。
- [x] Step 2: 删除 `src/assets/platforms/pi.rs` 中 `#[cfg(test)] mod tests` 里的 `pi_new_prompt_template_passes_command_arguments` 与 `pi_prompt_source_and_installed_template_stay_in_sync` 两个测试函数（断言对象已不存在）。
- [x] Step 3: 删除 `assets/platforms/pi/prompts/hotpot-new.md`、`hotpot-execute.md`、`hotpot-finish-work.md` 三份源文件（`git rm`）。
- [x] Step 4: 检查 `assets/platforms/pi/prompts/` 是否空（`ls assets/platforms/pi/prompts/ 2>/dev/null`）。若空，删目录（`rmdir`）。
- [x] Step 5: 跑 `cargo build` 验证 `include_str!` 引用一致——expect compile pass，无 missing-file 错。

#### Task 3: Implement and wire `cleanup_deprecated_pi_prompts`

**Files:**

- Modify: `src/assets/platforms/pi.rs`
- Modify: `src/commands/init/pi.rs`（执行时 `grep` 验证精确路径）

- [x] Step 1: 在 `src/assets/platforms/pi.rs` 文件顶部新增 `pub(crate) fn cleanup_deprecated_pi_prompts(root: &Path) -> std::io::Result<()>`：
  - 硬编码 list `[".pi/prompts/hotpot-new.md", ".pi/prompts/hotpot-execute.md", ".pi/prompts/hotpot-finish-work.md"]`
  - 循环 `for rel in LIST { let abs = root.join(rel); if abs.exists() { std::fs::remove_file(&abs)?; } }`
  - 加双语 doc 注释说明这是一次性废弃路径清理，未来如再废弃可扩展 list
- [x] Step 2: `grep -rn 'platforms::pi\|init_pi\|update_pi' src/commands/init/` 确认 Pi platform install entry 在 `src/commands/init/pi.rs` 还是 `src/commands/init/mod.rs`；找到 entry 函数（候选名 `init_pi` / `install_pi` / `pi::install`）。
- [x] Step 3: 在该 entry 函数 **既有资产 install 流程完成之后**（最后一步）调用 `crate::assets::platforms::pi::cleanup_deprecated_pi_prompts(root)?;`，把任何 IO 错向上传播。
- [x] Step 4: 在 `src/assets/platforms/pi.rs::tests` 内新增三条单元测试（用 `tempfile` crate 创建临时 root，已在项目里）：
  - `cleanup_deprecated_pi_prompts_removes_all_three`：在 tempdir 里 `mkdir -p .pi/prompts/` + 写三份占位文件 → 跑 fn → assert 三文件不存在 + fn 返回 `Ok(())`
  - `cleanup_deprecated_pi_prompts_is_noop_when_absent`：tempdir 里不创任何文件 → 跑 fn → assert 返回 `Ok(())`
  - `cleanup_deprecated_pi_prompts_handles_partial_existence`：tempdir 里只创 `.pi/prompts/hotpot-new.md` → 跑 fn → assert 该文件不存在 + fn 返回 `Ok(())`
- [x] Step 5: 运行 `cargo test cleanup_deprecated_pi_prompts`，expect 三条测试通过。
- [x] Step 6: 运行 `cargo test`，expect 全量通过；前面删除的两个 prompt-template 测试不再出现；total = 84（基线 86 - 2）+ 3（新加）= 87。
- [x] Step 7: 在本仓 root 跑 `cargo run -- init --platform pi 2>&1 | tail`：expect 成功；之后 `ls .pi/prompts/ 2>&1`：expect 三个旧 thin shell 都不存在或目录不存在。

#### Task 4: Update platform & architecture docs

**Files:**

- Modify: `docs/platforms/pi.md`
- Modify: `docs/ARCH.md`
- Modify: `docs/ARCH.zh_CN.md`

- [x] Step 1: 打开 `docs/platforms/pi.md`，定位上一任务沉淀的 "Inlining `$ARGUMENTS` inside a prose sentence ..." 整个 bullet（约 line 68，长 bullet 包含 three-part pattern + Exception + USER ACTIVE REQUEST 三段说明）。把整个 bullet 替换为一句较短的废弃说明：

  ```
  - **Hotpot 历史经验（已废弃）**: Hotpot 曾通过 prompt template thin shell（`.pi/prompts/hotpot-*.md` + `$ARGUMENTS` 注入 + 分隔块 + Exception 覆盖 + `=== USER ACTIVE REQUEST ===` framing）传递命令参数；该路径在加载 `AGENTS.md` / `CLAUDE.md` / 全局 skills 等竞争上下文的 Pi 项目里会被 AI 整体当作系统级背景文档吸收（参见 `migrate-pi-commands-to-extension`）。Hotpot 现已迁移到 Pi extension command（`pi.registerCommand` + `ctx.sendUserMessage`）传递命令参数。详见下面的 "Hotpot Pi extension commands"。
  ```

- [x] Step 2: 在 "Hotpot implication" 段之后、`## Agents` 之前新增 `### Hotpot Pi extension commands` 子节，覆盖：
  - 三个 `pi.registerCommand("hotpot-{new,execute,finish-work}", ...)` 注册入口
  - handler 流程：`ensureContext(ctx.cwd) → buildPiCommandMessage(...) → ctx.sendUserMessage(text)`
  - 消息内含分隔块 + framing + `$HOTPOT_*_PROMPT` 绝对路径 + Pi `@path` 替换表 + Platform note + Exception (empty arguments)
  - one-shot cleanup 行为：`hotpot init --platform pi` / `hotpot update` 时自动删除 `.pi/prompts/hotpot-{new,execute,finish-work}.md` 三个废弃 thin shell
- [x] Step 3: `docs/ARCH.md` 中找到 "Platform-specific surfaces" 列表里的 Pi 行（当前为 "Pi: `.pi/prompts/hotpot-*.md` + `.pi/extensions/`（no native subagents — single-session phased execution; review phase stays read-only）"）。把它改为 "Pi: `.pi/extensions/` —— `pi.registerCommand` registers `/hotpot-new` / `/hotpot-execute` / `/hotpot-finish-work` slash commands that send user-voice messages via `ctx.sendUserMessage`; no prompt-template thin shells; no native subagents — single-session phased execution; review phase stays read-only"。
- [x] Step 4: 在 ARCH.md "Notes For Future Agents" 末尾追加一条（中英对照其它条目风格）：
  - "Pi 不再使用 prompt template thin shell 承接 slash commands；新增 Hotpot slash command 时必须同时在 `assets/platforms/pi/extensions/hotpot/index.ts` 里调 `pi.registerCommand` + 调用 `buildPiCommandMessage` helper；任何废弃 thin shell 路径必须加入 `src/assets/platforms/pi.rs::cleanup_deprecated_pi_prompts` 的列表，使 `hotpot init` / `update` 能自动清理。"
- [x] Step 5: `docs/ARCH.zh_CN.md` 同步更新 Pi 行的中文描述，以及"给后续 agent 的注意事项"末尾同义补一条。

#### Task 5: Cross-cutting validation

**Files**: 无新增

- [x] Step 1: `cargo test` 全量；expect 全过，no new warning，total = 87（86 - 2 + 3）。如果总数不是 87，列出每个被删/新增测试，分析 deviance。
- [x] Step 2: `cargo run -- init --platform pi` 在本仓跑一次；expect 成功；之后 `ls .pi/prompts/`：expect 三个旧 thin shell 都不存在（或目录已删）。
- [x] Step 3: `diff -u assets/platforms/pi/extensions/hotpot/index.ts .pi/extensions/hotpot/index.ts`：expect 空输出（source 与 installed 一致）。
- [x] Step 4: `grep -n 'pi.registerCommand' .pi/extensions/hotpot/index.ts`：expect 三行命中（hotpot-new / hotpot-execute / hotpot-finish-work）。
- [x] Step 5: `grep -n 'hotpot-new.md\|hotpot-execute.md\|hotpot-finish-work.md' src/assets/platforms/pi.rs`：expect 仅出现在 cleanup_deprecated_pi_prompts 的硬编码 list（即 3-4 条命中，都来自 cleanup fn / 其测试），ASSETS 数组无命中。
- [x] Step 6: 把 `cargo test` 输出末尾 5 行附进 execution report，便于 review 核对。

### Validation

- `cargo test` — 全量 87 passed / 0 failed，基线 3 个 dead_code warning 可接受，无新增 warning。
- `cargo test cleanup_deprecated_pi_prompts` — 3 个新增单元测试通过。
- `cargo run -- init --platform pi` — 成功；之后 `.pi/prompts/hotpot-{new,execute,finish-work}.md` 三个旧 thin shell 不存在。
- `diff -u assets/platforms/pi/extensions/hotpot/index.ts .pi/extensions/hotpot/index.ts` — 空输出。
- `grep -n 'pi.registerCommand' .pi/extensions/hotpot/index.ts` — 三行命中。
- `assets/platforms/pi/prompts/` 在 git 中已删除（或目录不存在）。
- 手动 Pi 端到端验证（由用户执行，加载了 AGENTS.md 的真实 Pi 会话）：
  - `/hotpot-new 修复 abc` → AI 首条 brainstorm 消息显式引用/释义 "修复 abc"，不追问 initial idea
  - `/hotpot-new`（空参数）→ AI 第一条消息是"ask exactly one concise question"
  - `/hotpot-execute` / `/hotpot-finish-work` 分别正常进入对应 workflow

### Risks and Watchouts

- **`ctx.sendUserMessage` 在用户实际 Pi 版本的可用性**：`docs/platforms/pi.md` 列示了该 API，但本仓无 TS 测试栈，编译期不能 lint。**强烈依赖你的真实 Pi 端到端测试**作为唯一验证；如 API 在你 Pi 版本上不可用，本任务方案需重新评估（可能要回到 `pi.on("context", …)` 推 user-role 消息的更上游方案）。
  - **Post-validation fix 1 (2026-05-21)**: 用户在真实 Pi 会话报错 `Extension "command:hotpot-new" error: ctx.sendUserMessage is not a function`。查 `@earendil-works/pi-coding-agent` 的 `types.d.ts`：`sendUserMessage` 实际在 `ExtensionAPI`（factory 的 `pi` 参数，line 841）上，**不**在 `ExtensionCommandContext`（handler 的 `ctx` 参数）上。`ReplacedSessionContext`（`withSession()` 回调）另有自己的 `sendUserMessage`，与本场景无关。`ExtensionAPI.sendUserMessage` 返回 `void`（非 `Promise<void>`）。修复：把三个 handler 中的 `await ctx.sendUserMessage(text)` 改为 `pi.sendUserMessage(text)`（`pi` 在 factory 词法作用域内），同步更新 `buildPiCommandMessage` 与 Slash-command 注释块；`assets/` 与 `.pi/` 两份 byte-identical（`hotpot init --platform pi --force` 同步）；`cargo test` 87 passed。
  - **Post-validation fix 2 (2026-05-21)**: 用户第二次真实 Pi 验证仍失败——"测试了 pi ，依然存在不能判断任务的问题"。读取 session log `/Users/bytedance/.pi/agent/sessions/.../2026-05-21T09-24-29-417Z_019e49d9-c669-78e1-a7f0-a1c1d79276d6.jsonl` 定位失效链：line 4 用户消息（含完整 framing + INITIAL TASK IDEA + 绝对路径 + @path 表）已正确以 `role:user` 投递；但 `kimi-k2.6` (moonshotai-cn) 模型在 line 5 thinking 直接幻觉为 "user wants me to explore the project structure"，跑 `ls -la`；line 7 thinking "use the project-structure-explorer skill"，全局 skill（`~/.agents/skills/project-structure-explorer/SKILL.md`，描述 `当用户需要查询项目结构、查看目录树、了解项目文件组织时使用此 skill`）被自动调用劫持本轮；line 9 继续幻觉 "look at the issue mentioned in the PR" 跑 `git log`；line 11 干脆幻觉用户输入是 "嗯" 而回应 "你好！有什么我可以帮你的吗？"。模型从未读取 `hotpot-new.md`。根因：长篇结构化消息被弱指令跟随模型当作背景文档，skill auto-invocation 抢先决定行动。修复：重构 `buildPiCommandMessage` 消息体——(1) 去掉 `=== USER ACTIVE REQUEST ===` / `=== END ===` 仪式框（`role:user` 已经隐含"用户请求"语义）；(2) 首行即绝对命令 "YOUR FIRST TOOL CALL MUST BE `Read` ON THIS EXACT PATH" + workflow 文件绝对路径，强迫模型把首个工具调用钉死在 Read workflow；(3) 显式 "FORBIDDEN" 列表逐条枚举 session log 里观测到的干扰行为（`ls`/`tree`/`git log`/`git status`/任何 skill/`project-structure-explorer`/追问/泛化问候）；(4) `@path` 替换表、INITIAL TASK IDEA 块、empty-args Exception、Platform note 保留但移到 FORBIDDEN 列表之后；(5) `buildPiCommandMessage` JSDoc 扩写为"两类需要这条消息体抵御的失效模式"——prompt-template absorption（已修）+ skill auto-invocation hijack（本次）——记录症状与修复策略，供未来 agent 调试同类回归。`assets/` 与 `.pi/` 两份 byte-identical 同步；`cargo test` 87 passed。**仍需用户在真实 Pi 会话端到端重新验证四个用例。** 若 `kimi-k2.6` 仍不响应，备选路径：通过 `pi.on("context", ...)` 注入系统级"忽略 skill 自动调用"指令；或用更强指令跟随模型重测。
- **TypeScript 编译错误无 CI**：项目无 `tsc --noEmit` 阶段。execution agent 在 Step 1 改完后必须**手动用 `tsc --noEmit assets/platforms/pi/extensions/hotpot/index.ts`** 或类似手段做编译检查；若没装 `tsc`，至少用 `node --check` 对编译产物做 sanity check。如本机也没 Node，至少做 `head` 抽样人工读 + 配合资产同步 diff 验证语法表面无误。
- **Pi 命令名解析冲突**：旧 `.pi/prompts/hotpot-*.md` 在本仓里会被 cleanup 删除；但如果 execution agent 改完代码后忘记跑一次 `hotpot init --platform pi`，本仓 `.pi/prompts/` 可能还残留旧 thin shell——一定要按 Task 5 Step 2 显式跑一次以确认 cleanup 生效。
- **handler 消息文本与共享 body 漂移**：消息里写了 Platform note、`@path` 替换表等内容，与共享 body 的 `Pi has no @path expansion ...` 段有重复义务。若未来共享 body 调整 substitution 列表（如新增 `@.hotpot/prompts/xxx.md` 引用），Pi handler 的 `@path` 替换表也必须同步——这是新维护面，risk 接受。docs/ARCH.md 的"Notes For Future Agents"应明示这个 sync 义务（见 Task 4 Step 4）。
- **资产引擎对删除资产的支持**：删除 ASSETS 数组里的项并不会让现有 `.pi/prompts/` 文件消失——`hotpot init` 默认只装、不卸。本任务的 `cleanup_deprecated_pi_prompts` 是显式补这个 gap；如果执行时发现资产引擎已有内置 retire 机制，应优先用既有机制，但目前**未在代码中观察到此类机制**。
- **`cargo run -- init --platform pi` 在本仓的副作用**：会重新装其它 Pi 资产文件（hooks、settings.json 等），影响 git 工作区。执行时把 init 跑完后 `git status --porcelain` 抽查只有 cleanup 触发的删除 + 资产覆盖（应为同内容覆盖、git diff 空）。若 init 引入意外文件，调整方案。
- **Helper signature 变化预防**：`buildPiCommandMessage` 是新 helper，若 API 形状不稳定，三 handler 维护成本高；设计上锁定 options 字段（`command`、`args`、`hotpot`、`ideaBlockLabel`、`workflowPromptPath`、`atPathRefs`），未来新增字段不破坏既有 caller。
- **ARCH 双语同步漂移**：`docs/ARCH.md` 与 `docs/ARCH.zh_CN.md` 必须同步改 Pi 行；只改一边视为回归（项目惯例）。
- **`assets/platforms/pi/prompts/` 目录删除可能因为 git 索引残留**：单 `rmdir` 不会自动从 git 索引移除空目录。`git rm` 三个文件后该目录在 git 视角已不存在，无需额外操作。

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task. Specifically: do not touch `assets/prompts/hotpot-*.md` shared body, other platform assets, Pi extension既有 `record_issue_candidate` / `read_issue_candidates` / `clear_issue_candidates` / `pi.on(...)` hooks。
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff. Especially: if `src/commands/init/pi.rs` 实际 entry function 名或位置与本 task 假设差异较大；如果 Pi extension TypeScript 没法用本机 `tsc` / `node --check` 做编译检查；如果 `cargo run -- init --platform pi` 在本仓的 side effect 与 Task 5 Step 2 描述不符。
- Run the validation commands before reporting completion. 若 `cargo test` 出现新 warning，附进 report 但不要 `#[allow(...)]` 静默。
- 在最终报告里包含真实 Pi 端到端测试的步骤复述（不要执行，由用户来）以便用户后续验证。
