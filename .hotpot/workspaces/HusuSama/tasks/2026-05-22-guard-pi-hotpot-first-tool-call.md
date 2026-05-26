# Guard Pi Hotpot First Tool Call

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | true | 5 | high |
:::

---

## Task

### Summary

::: info
为 Pi 平台的 Hotpot slash command 增加首轮工具调用运行时护栏，避免弱模型在 `/hotpot-new`、`/hotpot-execute`、`/hotpot-finish-work` 刚投递 workflow 后忽略用户任务、运行探索命令或重新询问需求。现有方案已经把用户输入前置并增加 `pendingWorkflow` system 强提醒，但 2026-05-22 的 Pi 会话仍复现失败；本任务把关键约束下沉到 Pi extension 的 `tool_call` hook，在工作流刚启动后的第一轮只允许读取指定 workflow prompt，错误工具调用会被扩展拦截并返回纠正信息。
:::

### User Request

::: info 用户原话
当前使用 pi 时，AI 会一直忽略我提交的任务，比如在使用 hotpot-new 后，AI 会在查询一些文件后，又开始询问需要做什么，目前已进行过多次修复。对话原始数据可以查看这个文件：`/Users/bytedance/.pi/agent/sessions/--Users-bytedance-RustProjects-hotpot--/2026-05-22T02-26-08-908Z_019e4d81-218c-7e8d-8446-e2ce64ff183b.jsonl`

目前我也切换过多个模型，比如 kimi-k2.6、deepseek-v4-pro 等。
:::

### Approved Design

::: tip
批准方案：在 Pi extension 中实现“首轮工具调用运行时护栏”，不再只依赖模型遵守 prompt。

核心设计：

1. 在 `assets/platforms/pi/extensions/hotpot/index.ts` 中扩展现有 `pendingWorkflow` 机制，新增一个专门用于首轮工具调用校验的闭包状态，例如 `pendingFirstToolGuard`。三个 Hotpot slash command handler 在调用 `pi.sendUserMessage(text)` 前同时设置 `pendingWorkflow` 与 `pendingFirstToolGuard`。
2. `pendingFirstToolGuard` 记录 command、期望 workflow prompt 绝对路径、用户输入块 label，以及可选的 raw args 预览。它只覆盖刚启动工作流后的第一轮工具调用，不影响正常 `/hotpot:execute` 后续实施阶段。
3. 在已有 `pi.on("tool_call", ...)` hook 中先执行 Hotpot first-tool guard，再执行现有 bash env 注入逻辑。若 guard 处于 armed 状态，只有读取期望 workflow prompt 的 `read` 工具调用可以通过；其它任何工具调用，包括 `bash`、`ls`、`find`、读取 `AGENTS.md`、读取 `docs/ARCH.md`、技能加载类工具或读取非 workflow 文件，都必须被拦截。
4. 被拦截时，hook 返回 Pi 支持的 block 结果，例如 `{ block: true, reason: <纠正文案> }`。纠正文案必须明确告诉模型：最近触发的是哪个 Hotpot slash command，用户真实请求在最近 user message 的 `<<< LABEL >>>` 块中，下一步必须调用 `Read("<workflowPromptPath>")`，不得探索项目、调用技能、问“需要做什么”或 greeting。
5. 当模型成功发起正确的 `read` 工具调用并且路径等于期望 workflow prompt 绝对路径时，guard 立即解除，后续工具调用恢复正常；如果第一次工具调用被拦截，guard 继续保持 armed，直到模型改为读取 workflow prompt。
6. 保留现有 `buildPiCommandMessage` 用户输入前置与 `pendingWorkflow` system 强提醒；新护栏是运行时硬约束层，不替代提示词层。
7. 同步更新 `docs/platforms/pi.md`、`docs/ARCH.md`、`docs/ARCH.zh_CN.md`，把 2026-05-22 新失败样本与“prompt-only 防线不足，必须用 tool_call guard”写入长期文档。
:::

### Alternatives Considered

- **继续调提示词**：改动小，但最近任务已经完成用户输入前置与 per-turn system 强提醒，2026-05-22 日志仍然失败，说明 prompt-only 方案无法稳定约束弱模型；不采用。
- **建议用户只使用强模型**：可以规避部分弱模型问题，但用户已切换过 kimi-k2.6、deepseek-v4-pro 等多个模型，且 Hotpot 需要跨平台和跨模型鲁棒性；不作为唯一方案。
- **移除 Pi Hotpot slash commands**：能避免误导但会损害 Pi 平台支持；不采用。
- **推荐方案：tool_call 运行时护栏 + 现有提示词层保留**：把“第一个工具必须读取 workflow”从软提示变成扩展层校验，最小化依赖模型自觉遵守指令；采用。

### Requirements

- `/hotpot-new`、`/hotpot-execute`、`/hotpot-finish-work` 三个 Pi command 都必须设置首轮工具调用 guard。
- guard armed 时，唯一允许的首个工具调用是读取对应 workflow prompt 绝对路径：`HOTPOT_NEW_PROMPT`、`HOTPOT_EXECUTE_PROMPT` 或 `HOTPOT_FINISH_WORK_PROMPT`。
- guard 必须拦截错误首个工具调用并返回明确纠正文案，不能静默放行。
- 被拦截后 guard 必须保持 armed，直到模型正确读取 workflow prompt。
- 正确读取 workflow prompt 后 guard 必须解除，避免影响后续 workflow 正常探索、执行、review。
- 现有 bash env 注入逻辑必须保留；guard 的顺序应确保错误 bash 首调不会被当作正常命令执行。
- 现有 `pendingWorkflow` system 强提醒与 `buildPiCommandMessage` 用户输入前置保持兼容，不删除。
- 必须覆盖 2026-05-22 日志中的失败模式：模型没有先读取 workflow，而是读 `docs/ARCH.md`、跑 `find`、`ls`、再把用户输入幻觉成别的任务或问候。
- 文档必须说明这已经是第四档 Pi failure mode：prompt-only mitigation 失效后需要 runtime guard。
- 部署副本 `.pi/extensions/hotpot/index.ts` 必须通过 `cargo run -- init --platform pi` 从源资产同步，不手改。

### Non-Goals

::: details Non-Goals
- 不改 Claude、OpenCode、Codex 平台行为。
- 不重写 Hotpot shared workflow prompt 的业务流程。
- 不引入新的 Pi prompt-template thin shell。
- 不让 extension 自动完成整个 `/hotpot:new` 流程；它只负责拦截错误首个工具调用并纠偏。
- 不拦截 workflow 读取成功之后的正常工具调用。
- 不改变 `overview.jsonl`、task 文件格式或 issue-memory 数据格式。
- 不把用户任务内容解析成结构化字段写入持久状态；用户原始输入仍由 user message 与 workflow prompt 驱动。
:::

### Project Context

- 架构要求：先读 `docs/ARCH.md`；影响执行流时同步更新 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md`。
- Pi 集成源文件是 `assets/platforms/pi/extensions/hotpot/index.ts`；安装副本是 `.pi/extensions/hotpot/index.ts`，由 `src/assets/platforms/pi.rs` 的 `Asset::owned` 安装。
- 当前 Pi extension 已通过 `pi.registerCommand` 注册 `/hotpot-new`、`/hotpot-execute`、`/hotpot-finish-work`，并用 `pi.sendUserMessage(text)` 发送 user-voice workflow 消息。
- 当前 `buildPiCommandMessage` 已把用户输入块前置，格式为 `A new Hotpot <workflowName> request from me:` 后紧跟 `<<< INITIAL TASK IDEA >>>` 等块。
- 当前 `pendingWorkflow` 在 handler 调用 `pi.sendUserMessage(text)` 前设置，在下一次 `pi.on("context", ...)` 中单次注入 system 强提醒后清空。
- 当前 `pi.on("tool_call", ...)` 已拦截 bash 工具调用并注入 Hotpot env：`event.input.command = export ...; ${command}`。新 guard 应合并进这个 hook，避免注册多个互相覆盖或顺序不明的 tool_call handler。
- `docs/platforms/pi.md` 记录了三类既有失败模式：prompt-template absorption、skill auto-invocation hijack、attention loss on user-message body。本次新增失败样本是第四类：即使用户输入前置与 system 强提醒都存在，模型仍执行错误首个工具调用。
- 2026-05-22 失败日志路径：`/Users/bytedance/.pi/agent/sessions/--Users-bytedance-RustProjects-hotpot--/2026-05-22T02-26-08-908Z_019e4d81-218c-7e8d-8446-e2ce64ff183b.jsonl`。关键观察：line 4 的 user message 完整包含 `INITIAL TASK IDEA` 和 first-tool-call 强约束；line 5 模型没有先 Read workflow，而是读 `docs/ARCH.md` 并跑 `find`；后续多次把用户任务幻觉成其它需求或把输入当空/hello。
- 既有相关任务：`.hotpot/workspaces/HusuSama/tasks/2026-05-21-pi-fix-weak-model-task-handoff.md` 已完成消息体重排与 `pendingWorkflow` 强提醒，本任务不要重复做同一层防御。

---

## Plan

### Mode

- tdd: true

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Modify | 增加首轮工具调用 guard 状态、判断函数、tool_call 拦截逻辑，并保持现有 bash env 注入。 |
| `.pi/extensions/hotpot/index.ts` | Modify | 通过 `cargo run -- init --platform pi` 同步源资产到安装副本。 |
| `docs/platforms/pi.md` | Modify | 记录第四档 Pi failure mode 与 runtime guard 设计契约。 |
| `docs/ARCH.md` | Modify | 英文架构说明同步 Pi slash command 首轮工具调用 guard 契约。 |
| `docs/ARCH.zh_CN.md` | Modify | 中文架构说明同步上一项。 |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Test | 若仓库无 TS 测试框架，至少新增可运行的轻量导出/测试入口或用脚本化断言验证 guard 纯函数；执行 agent 必须先调查可行测试方式。 |

### Implementation Tasks

#### Task 1: 为 Pi first-tool guard 提取可测试状态与判定逻辑

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Test | 先建立能验证 guard 判定逻辑的测试或脚本化断言。 |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Modify | 新增 guard 状态类型与纯判定函数，供 tool_call hook 调用。 |

##### Red

- [x] R1: 先检查项目是否已有 TypeScript 测试命令；读取 `Cargo.toml`、`.pi/package.json`、根目录 package 配置和现有 extension 写法，确定最小可运行测试方式。若没有现成 TS 测试框架，在同一文件中导出纯函数并新增临时脚本化验证命令，避免引入新依赖。
- [x] R2: 为 guard 判定写失败测试，覆盖 `hotpot-new` armed 状态下 `toolName="read"` 且 path 等于 `HOTPOT_NEW_PROMPT` 时返回 allow + disarm；`toolName="bash"`、`toolName="read"` 但 path 为 `docs/ARCH.md`、`toolName="ls"` 时返回 block + keep armed。测试名使用 `firstToolGuardAllowsOnlyWorkflowRead`。
- [x] R3: 运行选定的精确测试命令；若使用脚本化验证，命令必须明确执行 `firstToolGuardAllowsOnlyWorkflowRead` 场景；**expect failure**，失败原因应是 guard 判定函数尚不存在或行为未实现。

##### Green

- [x] G1: 在 `assets/platforms/pi/extensions/hotpot/index.ts` 中新增双语 doc comment 的类型，例如 `type PendingFirstToolGuard = { command: string; workflowPromptPath: string; userInputLabel: string; };`，并新增纯函数，例如 `evaluateFirstToolGuard(guard, toolName, input)`。
- [x] G2: 判定函数必须兼容 Pi read 工具参数名；先从日志确认实际 read tool 参数为 `path`，同时防御性支持 `filePath`，但不要猜测其它未见字段。
- [x] G3: 判定函数返回结构应能表达 allow/disarm 与 block/reason/keep-armed，reason 文案包含 command、workflowPromptPath、userInputLabel 和禁止探索提示。
- [x] G4: 运行 R3 的精确测试命令；**expect pass**。
- [x] G5: 运行 `cargo test`；**expect no other regressions**。

##### Refactor

- [x] F1: 检查新类型和函数命名是否清晰、是否有重复字符串；如 correction reason 过长，提取小函数并保留双语 doc comment，否则写 `no refactor needed`。
- [x] F2: 若发生 refactor，重跑 R3 的精确测试命令与 `cargo test`；**expect pass**。否则标记 `skipped (no refactor)`。

:::

#### Task 2: 在三个 Pi slash command handler 中 arm first-tool guard

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Test | 验证三个 handler 都会在发送 user message 前设置 guard。 |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Modify | 在 `hotpot-new`、`hotpot-execute`、`hotpot-finish-work` handler 中设置 guard。 |

##### Red

- [x] R1: 为 handler arm 行为写失败测试或脚本化源码断言，测试名使用 `handlersArmFirstToolGuardBeforeSendUserMessage`。断言三个 handler 都设置 `pendingFirstToolGuard = { ... }`，且位置在各自 `pi.sendUserMessage(text)` 之前。
- [x] R2: 运行精确测试命令；**expect failure**，因为 `pendingFirstToolGuard` 尚未被 handler 设置。

##### Green

- [x] G1: 在 `hotpotExtension` 工厂函数内、靠近现有 `let pendingWorkflow` 的位置新增 `let pendingFirstToolGuard: PendingFirstToolGuard | undefined;`，并写双语注释说明它是运行时首轮工具调用护栏。
- [x] G2: 在 `hotpot-new` handler 的 `pi.sendUserMessage(text)` 前设置 `pendingFirstToolGuard`，`workflowPromptPath: hotpot.HOTPOT_NEW_PROMPT`，`userInputLabel: "INITIAL TASK IDEA"`。
- [x] G3: 在 `hotpot-execute` handler 的 `pi.sendUserMessage(text)` 前设置 `pendingFirstToolGuard`，`workflowPromptPath: hotpot.HOTPOT_EXECUTE_PROMPT`，`userInputLabel: "EXECUTION NOTES"`。
- [x] G4: 在 `hotpot-finish-work` handler 的 `pi.sendUserMessage(text)` 前设置 `pendingFirstToolGuard`，`workflowPromptPath: hotpot.HOTPOT_FINISH_WORK_PROMPT`，`userInputLabel: "FINISH NOTES"`。
- [x] G5: 确保 `pendingWorkflow` 仍按原逻辑设置，不被替换或删除。
- [x] G6: 运行 R2 的精确测试命令；**expect pass**。
- [x] G7: 运行 `cargo test`; **expect no other regressions**。

##### Refactor

- [x] F1: 若三个 handler 的 guard 设置重复过多，可新增小 helper，例如 `armFirstToolGuard(command, workflowPromptPath, userInputLabel)`；helper 必须有双语 doc comment。若重复可接受且 helper 会增加复杂度，写 `no refactor needed`。
- [x] F2: 若发生 refactor，重跑 R2 的精确测试命令与 `cargo test`；**expect pass**。否则标记 `skipped (no refactor)`。

:::

#### Task 3: 在 Pi tool_call hook 中拦截错误首个工具调用

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Test | 验证错误首调被 block、正确 workflow read 放行并 disarm、bash env 注入仍保留。 |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Modify | 合并 guard 到现有 `pi.on("tool_call", ...)` hook。 |

##### Red

- [x] R1: 写失败测试或脚本化验证 `toolCallHookBlocksWrongFirstToolAndKeepsGuard`：armed 后第一次 `bash` 或读取 `docs/ARCH.md` 必须返回 block，guard 保持 armed。
- [x] R2: 写失败测试或脚本化验证 `toolCallHookAllowsWorkflowReadAndDisarms`：armed 后 `read` 期望 workflow prompt 必须不 block，并清空 guard。
- [x] R3: 写失败测试或脚本化验证 `toolCallHookStillInjectsHotpotEnvForLaterBash`：guard 已解除后 bash command 仍会被注入 Hotpot env export。
- [x] R4: 运行精确测试命令；**expect failure**，因为 hook 尚未接入 guard。

##### Green

- [x] G1: 修改现有 `pi.on("tool_call", async (event, ctx) => { ... })`，在最前面检查 `pendingFirstToolGuard`。
- [x] G2: 若 guard 判定为 block，返回 `{ block: true, reason }`，不要继续执行 bash env 注入，也不要清空 guard。
- [x] G3: 若 guard 判定为 allow + disarm，设置 `pendingFirstToolGuard = undefined`，然后继续原有逻辑；如果工具不是 bash，正确 workflow read 应直接 return 或落到后续非 bash return，但不能 block。
- [x] G4: 保留原有 bash env 注入行为：当 guard 不存在且 `event.toolName === "bash"` 时，继续用 `ensureContext(ctx.cwd)` 注入 Hotpot env。
- [x] G5: correction reason 必须使用英文输出，因为它是工具层返回给模型的操作指令；用户-facing 最终报告仍按中文。
- [x] G6: 运行 R4 的精确测试命令；**expect pass**。
- [x] G7: 运行 `cargo test`; **expect no other regressions**。

##### Refactor

- [x] F1: 检查 `tool_call` hook 是否仍保持单一清晰流程：guard check → bash env injection → return。避免新增第二个 `pi.on("tool_call")` 造成顺序不确定；如有重复则合并。
- [x] F2: 若发生 refactor，重跑 R4 的精确测试命令与 `cargo test`；**expect pass**。否则标记 `skipped (no refactor)`。

:::

#### Task 4: 同步部署副本并更新 Pi/ARCH 文档

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.pi/extensions/hotpot/index.ts` | Modify | 通过 init 同步新 extension 源资产。 |
| `docs/platforms/pi.md` | Modify | 记录 runtime guard 与新失败日志。 |
| `docs/ARCH.md` | Modify | 英文架构说明同步首轮工具调用 guard 契约。 |
| `docs/ARCH.zh_CN.md` | Modify | 中文架构说明同步首轮工具调用 guard 契约。 |

##### Red

- [x] R1: 写文档/部署失败断言命令：`diff assets/platforms/pi/extensions/hotpot/index.ts .pi/extensions/hotpot/index.ts && echo SAME` 当前应失败或无法证明新 guard 已同步；`grep -c 'first-tool' docs/platforms/pi.md` 与 `grep -c '2026-05-22T02-26-08-908Z_019e4d81' docs/platforms/pi.md` 当前应不满足要求。
- [x] R2: 运行 R1 中的精确命令；**expect failure**，因为部署副本和文档尚未同步。

##### Green

- [x] G1: 运行 `cargo run -- init --platform pi`，同步 `.pi/extensions/hotpot/index.ts`；不要手改部署副本。
- [x] G2: 在 `docs/platforms/pi.md` 的 Hotpot Pi extension commands / failure modes 章节中新增第四档失败模式，描述 2026-05-22 日志、受影响模型、prompt-only mitigation 失效，以及 runtime first-tool guard 的契约。
- [x] G3: 在 `docs/ARCH.md` 的 Pi notes 中追加：每个 Pi Hotpot slash command handler 除设置 `pendingWorkflow` 外，还必须 arm 首轮工具调用 guard；tool_call hook 必须在 workflow prompt 被读取前 block 其它工具调用。
- [x] G4: 在 `docs/ARCH.zh_CN.md` 对应位置追加 G3 的简体中文版本，保留代码标识符英文。
- [x] G5: 运行 R1 的精确命令；**expect pass**。
- [x] G6: 运行 `cargo test`; **expect no other regressions**。

##### Refactor

- [x] F1: 检查文档是否只在 Pi 相关章节追加内容，没有误改其它平台说明；若需要压缩重复描述，保持 `docs/platforms/pi.md` 详细、ARCH 简洁。否则写 `no refactor needed`。
- [x] F2: 若发生 refactor，重跑 R1 的精确命令与 `cargo test`；**expect pass**。否则标记 `skipped (no refactor)`。

:::

#### Task 5: 端到端验证 Pi new-task 失败样本被拦截纠正

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `/Users/bytedance/.pi/agent/sessions/...jsonl` | Test | 用真实 Pi session 日志验证首轮错误工具调用不再执行。 |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Test | 若端到端发现 guard 文案或状态问题，回到源文件修正。 |

##### Red

- [x] R1: 记录现有失败日志 `/Users/bytedance/.pi/agent/sessions/--Users-bytedance-RustProjects-hotpot--/2026-05-22T02-26-08-908Z_019e4d81-218c-7e8d-8446-e2ce64ff183b.jsonl` 中的 baseline：首个 assistant tool call 不是 workflow Read，而是 `docs/ARCH.md` read + `find`。
- [x] R2: 将该 baseline 写入执行报告，作为端到端 Red 证据；**expect failure baseline confirmed**。

##### Green

- [ ] G1: 在 Pi 客户端开启新 session，确保已加载同步后的 `.pi/extensions/hotpot/index.ts`；如 Pi 需要 reload，执行 Pi 的 `/reload` 或重启 session。
- [ ] G2: 使用弱模型优先复现，至少包括用户提到的 `deepseek-v4-pro` 或 `kimi-k2.6` 中一个；发送 `/hotpot-new "将所有模板的 cargo run -- 移除，仅保留 hotpot 的说明，用户安装必须直接使用 hotpot"`。
- [ ] G3: 检查新 session 日志：如果模型第一步尝试错误工具调用，必须出现 tool_call block/correction，且错误命令没有真实执行；随后模型应改为读取 `$HOTPOT_NEW_PROMPT`。
- [ ] G4: 如果模型第一步直接读取 `$HOTPOT_NEW_PROMPT`，也视为通过；记录 guard 没有误伤正确行为。
- [ ] G5: 确认模型进入 `/hotpot:new` workflow 后没有重新询问“需要做什么”，而是围绕 `INITIAL TASK IDEA` 继续 brainstorm。
- [ ] G6: 至少再跑一次 `/hotpot-execute` 或 `/hotpot-finish-work` 空参路径的 smoke test，确认 guard 不破坏其它 command 的 workflow prompt 首读。
- [ ] G7: 运行 `cargo test`; **expect no other regressions**。

##### Refactor

- [ ] F1: 根据端到端结果检查 correction reason 是否过长或不够明确；若模型被 block 后仍不纠正，缩短并强化 reason，再重复 G2-G5。若一次通过，写 `no refactor needed`。
- [ ] F2: 若发生 refactor，重跑相关精确测试、`cargo test`、`cargo run -- init --platform pi`、部署副本 diff、以及至少一次 Pi `/hotpot-new` 端到端验证；**expect pass**。否则标记 `skipped (no refactor)`。

##### Blocker

- [ ] B1: Pi 真实端到端验证尚未完成。2026-05-22 fix round 复核时，Task 5 Green 的 G1-G6 仍被当前 Pi session 的 repeated stale ctx errors 阻塞；因此尚未验证当前 Pi 版本是否接受 `tool_call` hook 返回的 `{ block: true, reason }`，也尚未完成 `/hotpot-new` 弱模型首调拦截、workflow prompt 后续读取，或 `/hotpot-execute` / `/hotpot-finish-work` smoke test。Task 5 G1-G7/F1-F2 必须保持未勾选，直到在可用 Pi session 中完成真实验证。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo test` | 通过；无非预期回归。 |
| `cargo run -- init --platform pi` | 成功同步 Pi extension 资产。 |
| `diff assets/platforms/pi/extensions/hotpot/index.ts .pi/extensions/hotpot/index.ts && echo SAME` | 输出 `SAME`。 |
| `grep -c 'pendingFirstToolGuard' assets/platforms/pi/extensions/hotpot/index.ts` | 大于等于 1。 |
| `grep -c '2026-05-22T02-26-08-908Z_019e4d81' docs/platforms/pi.md` | 大于等于 1。 |
| Pi `/hotpot-new "将所有模板的 cargo run -- 移除，仅保留 hotpot 的说明，用户安装必须直接使用 hotpot"` | 首轮只能读取 workflow prompt；错误首调被 block 并纠正；不会重新询问用户要做什么。 |
| Pi `/hotpot-execute` 或 `/hotpot-finish-work` smoke test | workflow prompt 首读正常，后续流程不被 guard 误伤。 |

### Risks and Watchouts

::: warning
- Pi `tool_call` block API 的精确返回结构需要按 `docs/platforms/pi.md` 和实际扩展 API 确认；文档示例是 `{ block: true, reason: "..." }`，但 execution agent 必须验证当前版本是否接受该结构。
- 如果 Pi 内置 read 工具参数不是 `path` 或 `filePath`，guard 可能误拦截正确读取；必须从真实 session 日志确认工具参数名。
- guard 状态必须在正确 workflow read 后立即解除，否则会阻断 workflow 后续必要探索。
- guard 如果在错误首调后错误解除，就无法阻止模型继续跑偏；block 分支必须保持 armed。
- 不要注册多个 `tool_call` hook 分散处理 guard 与 bash env 注入，避免事件顺序不清或覆盖返回值。
- correction reason 写给模型，应简短、英文、命令式；过长可能继续稀释注意力。
- 端到端验证依赖真实 Pi 客户端和模型行为；如果当前环境无法启动 Pi，应明确记录缺口，并至少完成脚本化 guard 测试与部署 diff。
:::

---

## Execution Instructions

给 execution sub-agent 提供本任务文件的完整内容。execution agent 必须：

- 先完整读取本文件再编辑。
- 严格按 `## Plan` 的 Task 1 到 Task 5 顺序执行。
- 遵守 `### Mode` 中的 `tdd: true`：每个 Implementation Task 都必须先 Red，再 Green，再 Refactor。
- 保留所有已批准设计、Requirements、Non-Goals 和风险约束。
- 不扩大 scope 到其它平台或 prompt-template 体系。
- 修改过程中同步更新 checkbox 状态（如果执行环境允许）。
- 遇到 Pi extension API、工具参数名或测试命令与本文件假设不一致时，停止并报告 blocker，不要猜测绕过。
- 在报告完成前运行 `### Validation` 中的命令和 Pi 端到端验证；无法运行的项目必须说明原因与剩余风险。

## Open Questions

- 如果真实 Pi extension API 不支持在 `tool_call` hook 中 block 非 bash 工具调用，应改用哪个 Pi 事件层实现同等护栏？execution agent 需要先验证 API；若不支持，停止并汇报替代方案。
