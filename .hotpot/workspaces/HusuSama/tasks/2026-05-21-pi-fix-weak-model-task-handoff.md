# Pi 弱模型任务交接修复：消息体重排 + per-turn 系统级强提醒

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | false | 6 | medium |
:::

---

## Task

### Summary

::: info
Pi 平台的 `kimi-k2.6`（`moonshotai-cn` 提供商）在已有 front-loaded `FIRST TOOL CALL` 指令 + `FORBIDDEN` 列表防御下仍然识别不到用户请求——比 `docs/platforms/pi.md` 已记录的 "Skill auto-invocation hijack" 严重一档：模型把 `<<< INITIAL TASK IDEA >>>` 整段完全注意力丢失，思维链直接写 "user hasn't asked anything yet"，跑 `pwd && ls -la`，再幻觉无关任务（agent-browser → skill-creator），最终以 greeting 收尾。本任务两层叠加修复（A 消息体重排 + B per-turn 系统级强提醒），并同步把第三档失败模式与对策写进 `docs/platforms/pi.md` 和 ARCH。
:::

### User Request

::: info 用户原话
任务里修复后，使用 pi 还是出现任务识别的问题，这里是对话记录：`/Users/bytedance/.pi/agent/sessions/--Users-bytedance-RustProjects-hotpot--/2026-05-21T09-37-43-485Z_019e49e5-e43d-7545-96ee-db13bb965132.jsonl`

随后在 AskUserQuestion 中选择"走 /hotpot:new 立 D 任务（推荐）"，确认按 A + B + C 三段叠加方案推进，验证以手动 kimi-k2.6 + claude-sonnet 两模型跑为准；TDD 模式 Skip。
:::

### Approved Design

::: tip
**A. 消息体重排（`buildPiCommandMessage`）**

把 `<<< INITIAL TASK IDEA / EXECUTION NOTES / FINISH NOTES >>>` 用户输入块从消息中段前移到**第一段**——开篇即用 `A new Hotpot <workflowName> request from me:` 作引导，紧跟用户输入块与空参 Exception；其后才是 `YOUR FIRST TOOL CALL MUST BE Read(...)` 指令、`FORBIDDEN` 列表、`@path` 替换表、Platform note。同步压缩冗余措辞（"Do not summarize it — execute it" / "use this verbatim as the ..." 等），降低消息总长，让弱模型在 attention 早期就锁定用户真实意图。

**B. per-turn 系统级强提醒**

在 `hotpotExtension` 工厂闭包内新增状态字段：

```typescript
let pendingWorkflow: { command: string; workflowPromptPath: string; userInputLabel: string } | undefined;
```

三个 `pi.registerCommand` handler 在调用 `pi.sendUserMessage(text)` 之前赋值 `pendingWorkflow`；现有 `pi.on("context", ...)` 已在每次 provider 请求前触发——在其末尾追加：若 `pendingWorkflow` 非空，则多 push 一条 `role: "system"` 消息，明确点出（i）用户刚 invoke 哪个 slash command、（ii）用户请求在最近一条 user 消息的 `<<< ${userInputLabel} >>> ... <<< END ${userInputLabel} >>>` 块里、（iii）首步必须 `Read("<absolute workflowPromptPath>")`、（iv）禁止 `ls / tree / git log / git status / 任意 skill 自动调用（特别是 project-structure-explorer / skill-creator）/ 任意 greeting`。注入完成立即 `pendingWorkflow = undefined`，保证只在下一轮 provider 请求生效一次。

设计要点：

1. 标记**单次消费**——避免在后续工作流轮次重复注入污染上下文。
2. handler 每次进入都**无条件覆盖**——若上一轮 handler 设置的标记尚未被消费（极少见的异常情形），后一个 handler 入口的赋值天然清掉旧值。
3. 三档防护互补——A 在消息体内提升用户意图显著度（适用所有模型）；B 在 system 上下文层强行复述（适用 attention 极弱的模型）；如果二者仍压不住，则触发文档里"换模型"建议路径。

**C. 文档同步**

- `docs/platforms/pi.md` 的 "Two failure modes the message body must survive" 段落升级为 "Three failure modes"，加入第三档（attention loss on user-message body）并描述对策（消息体重排 + per-turn 系统级强提醒）。同步更新该文件中描述 Pi extension command 消息体结构的描述以反映新顺序。
- `docs/ARCH.md` + `docs/ARCH.zh_CN.md` 的 "Notes For Future Agents" 末尾关于 Pi `pi.registerCommand` / `buildPiCommandMessage` 的那条 bullet 补一句：新增 slash command 时必须同时在 handler 内 `pi.sendUserMessage` 之前赋值 `pendingWorkflow` 闭包字段，并在 `buildPiCommandMessage` 中保留"用户输入块前置"的顺序。
:::

### Alternatives Considered

- **只做 A（重排不加 system 注入）** — 代价低，但实测中 `kimi-k2.6` 在中段已完全注意力丢失，重排能否单独压住未知；放弃因鲁棒性不足。
- **只做 B（保留中段位置，仅加 system 注入）** — 不改消息体能减小回归面，但若 system message 同样在 attention 内被淹没（kimi-k2.6 已表现出系统级 skill 描述也能压过 user 消息），单 B 也不稳；放弃。
- **改用 stronger model 不动代码** — 真正能根治，但需要修改用户配置且绕过问题；放弃作为唯一方案，仅作为 A+B 仍失败时的最终建议路径。
- **推荐方案（A + B + C）** — 双层防御叠加：A 提升消息体内显著度，B 在 system 上下文强行复述，C 把新档失败模式与对策固化到长期文档以防回归。

### Requirements

- `buildPiCommandMessage` 的输出顺序变为：开篇引导 → 用户输入块 → 空参 Exception → `FIRST TOOL CALL` 指令 → `FORBIDDEN` 列表 → `@path` 替换表 → Platform note。
- `buildPiCommandMessage` 整体长度不长于当前版本（压缩冗余措辞）。
- `pendingWorkflow` 闭包字段在三个 slash command handler 内被赋值，且**在调用** `pi.sendUserMessage` **之前**赋值。
- `pi.on("context", ...)` 注入第三条 `role: "system"` 消息**当且仅当** `pendingWorkflow` 非空，且注入后立即清空。
- 第三条 system 消息文本必须显式列出：workflow 绝对路径、`<<< ... >>>` 块作为用户请求标识、`ls / tree / git log / git status / project-structure-explorer / skill-creator / greeting` 等 FORBIDDEN 行为清单。
- `docs/platforms/pi.md` 的 "failure modes the message body must survive" 段落计数从 "Two" 改为 "Three"，并补全第三档（描述 + 证据：session 日志路径 + line 4/5/16）。
- `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md` 的 Pi 相关 "Notes For Future Agents" bullet 同步更新为反映 `pendingWorkflow` 契约。
- 部署：`assets/platforms/pi/extensions/hotpot/index.ts` 改完后通过 `hotpot init --platform pi` 或 `hotpot update` 同步刷新 `.pi/extensions/hotpot/index.ts`，保证 `diff` 为空。
- 验证：`kimi-k2.6 / moonshotai-cn` 实跑 `/hotpot-new "<非空 idea>"`，首次 tool call 即 `Read` workflow 绝对路径；无 exploration 命令；无 skill 自动调用；无 greeting 回退；模型在首回复中正确复述用户输入。
- 验证：`claude-sonnet`（或同档强模型）实跑同一命令无回归。

### Non-Goals

::: details Non-Goals
- 不改 `hotpot-execute` / `hotpot-finish-work` 的工作流体（`assets/prompts/hotpot-*.md`）——本任务只动 Pi extension 投递层与 Pi 平台文档。
- 不引入 TypeScript 测试框架——本仓库 `.pi/extensions/` 此前从未配测试栈；验证以端到端肉眼跑为准（见 Validation）。
- 不重写其他平台（Claude / OpenCode / Codex）的 slash command 投递路径——它们没有同类问题。
- 不调整 `pi.on("context", ...)` 已有的两条 system message（hotpot env-var dump、language directive）——仅在末尾追加第三条。
- 不重构 `assets/platforms/pi/extensions/hotpot/index.ts` 中与 slash command 投递无关的部分（`registerTool` 三件套、bash hook、user_bash、session_shutdown）。
- 不动 `.hotpot/issue-candidates.jsonl` / `overview.jsonl` 等持久化数据格式。
:::

### Project Context

- 关键证据：Pi session 日志 `/Users/bytedance/.pi/agent/sessions/--Users-bytedance-RustProjects-hotpot--/2026-05-21T09-37-43-485Z_019e49e5-e43d-7545-96ee-db13bb965132.jsonl`，line 4 用户消息完整投递；line 5 模型 thinking `"The user hasn't asked anything yet"` 并跑 `pwd && ls -la`；line 7、10 幻觉 `agent-browser` 扩展；line 16 收尾 `"I'm ready to help with the skill-creator skill. What would you like to do?"`。
- 涉及的源文件：`assets/platforms/pi/extensions/hotpot/index.ts`（唯一源，由 `hotpot init --platform pi` 同步到 `.pi/extensions/hotpot/index.ts`）。
- 涉及的文档：`docs/platforms/pi.md`（Pi 平台权威说明）、`docs/ARCH.md` + `docs/ARCH.zh_CN.md`（架构总览）。
- 上游契约：`pi.on("context", ...)` 返回 `{ messages: [...] }`，每次 provider 请求前触发；`pi.sendUserMessage(text)` 在 `ExtensionAPI`（`pi` 参数）上而非 handler 的 `ctx`——已通过 `migrate-pi-commands-to-extension` 任务确认且写进 `pi.md`。
- 验证模型：`moonshotai-cn` 提供商 `kimi-k2.6` 模型（必跑），以及一档强模型（如 `claude-sonnet-4-6` 或同档）作回归对比。

---

## Plan

### Mode

- tdd: false

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Modify | 重排 `buildPiCommandMessage`；新增 `pendingWorkflow` 闭包字段；三个 handler 内赋值；扩展 `pi.on("context", ...)` 注入与清空逻辑 |
| `.pi/extensions/hotpot/index.ts` | Modify | 由 `hotpot init --platform pi` 同步刷新（不手改） |
| `docs/platforms/pi.md` | Modify | "Two failure modes" → "Three failure modes"；新增第三档描述与对策；同步消息体结构描述 |
| `docs/ARCH.md` | Modify | Pi `pi.registerCommand` / `buildPiCommandMessage` bullet 补 `pendingWorkflow` 契约 |
| `docs/ARCH.zh_CN.md` | Modify | 中文版同步上一项 |

### Implementation Tasks

#### Task 1: 重排 `buildPiCommandMessage` 输出顺序并压缩冗余措辞

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Modify | `buildPiCommandMessage` 函数体重排 + doc-comment 更新 |

**Steps:**

- [x] **Step 1**：阅读 `assets/platforms/pi/extensions/hotpot/index.ts` 当前 `buildPiCommandMessage` 实现（约 lines 122–162）与其 doc-comment（约 lines 67–122）。
- [x] **Step 2**：把函数体内的字符串数组顺序改为下列结构（顺序很重要，元素之间留必要空行）：
  1. 开篇引导：`A new Hotpot ${opts.workflowName} request from me:`
  2. 用户输入块：`<<< ${opts.ideaBlockLabel} >>>` / `${opts.args}` / `<<< END ${opts.ideaBlockLabel} >>>`
  3. 空参 Exception 单行：`If the block above is empty: ${opts.emptyArgsBehavior}`
  4. 首步指令单段（整段拼接成一行）：``YOUR FIRST TOOL CALL MUST BE `Read("${opts.workflowPromptPath}")` — DO NOTHING ELSE FIRST. That file is the Hotpot ${opts.workflowName} workflow; follow it end-to-end.``
  5. `FORBIDDEN before you Read the workflow:` 列表（保留四条 bullet 但去掉 "Do not summarize it — execute it" 这种废话；显式补 `skill-creator` 进 skill 黑名单）
  6. `@path` 替换表：``When the workflow file references `@.hotpot/prompts/<name>.md` (Pi has no `@path` expansion), substitute these absolute paths and `Read`:`` + `${refLines}`
  7. Platform note：保持现有 "Pi has no dedicated subagents..." 整段不动
- [x] **Step 3**：删除当前函数中冗余措辞——具体是：原 `That file is the Hotpot ${opts.workflowName} workflow. Follow it end-to-end. Do not summarize it — execute it.` 段已并入 Step 2 的第 4 项压缩版；原 `My input for the workflow — use this verbatim as the "${opts.ideaBlockLabel}", do NOT re-ask:` 整段删除（新顺序下用户输入已在顶部，此引导句无用）；原 ``If the `${opts.ideaBlockLabel}` block above contains no non-whitespace text between the `<<<` and `>>>` markers, ${opts.emptyArgsBehavior}`` 整段删除（已被 Step 2 第 3 项的压缩版替代）。
- [x] **Step 4**：更新 `buildPiCommandMessage` 上方的 doc-comment：在现有"Two known failure modes"段落末尾追加第三档（attention loss on user-message body）的双语描述，并把"## Two known failure modes"标题改为"## Three known failure modes"。简要说明本次重排把用户输入块移到顶部的设计意图。
- [x] **Step 5**：保持函数签名（参数 / 返回类型）不变；只动函数体与 doc-comment。
- [x] **Step 6**：运行 `cargo run -- init --platform pi` 重新生成 `.pi/extensions/hotpot/index.ts`（Task 3 会再跑一遍正式部署，这里只是先验证编译/资产语法）。

**Validation:**

| Command | Expected |
| ------- | -------- |
| `diff assets/platforms/pi/extensions/hotpot/index.ts .pi/extensions/hotpot/index.ts && echo SAME` | 输出 `SAME`（init 已同步） |
| `grep -n 'A new Hotpot' assets/platforms/pi/extensions/hotpot/index.ts` | 命中函数体内的开篇引导字符串 |
| `grep -c 'Do not summarize it' assets/platforms/pi/extensions/hotpot/index.ts` | `0`（冗余措辞已删） |
| `grep -n 'skill-creator' assets/platforms/pi/extensions/hotpot/index.ts` | 命中至少一行（FORBIDDEN 列表已扩展） |
| `wc -l assets/platforms/pi/extensions/hotpot/index.ts` | 函数体行数不长于改前（手工对比即可） |

:::

#### Task 2: 引入 `pendingWorkflow` 闭包状态 + `pi.on("context", ...)` 注入与清空

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Modify | 闭包加状态字段；三个 handler 赋值；context 事件读取/注入/清空 |

**Steps:**

- [x] **Step 1**：在 `hotpotExtension` 工厂函数（`export default function hotpotExtension(pi: ExtensionAPI)`）顶部、紧挨 `let context: HotpotContext | undefined;` 之后，声明新字段：

  ```typescript
  let pendingWorkflow:
    | { command: string; workflowPromptPath: string; userInputLabel: string }
    | undefined;
  ```

  上面加一段双语 doc-comment 说明：闭包级状态，handler 设置、context 事件消费、单次消费、为何这样设计（指向 `docs/platforms/pi.md` 的 third failure mode）。

- [x] **Step 2**：扩展现有 `pi.on("context", async (_event, ctx) => { ... })` 回调（约 lines 194–227）。在原 `return { messages: [...] }` 之前先准备一个 `messages` 局部数组：

  ```typescript
  const messages: { role: "system"; content: string }[] = [
    { role: "system", content: <existing hotpot env-var dump> },
    { role: "system", content: <existing language directive> },
  ];

  if (pendingWorkflow) {
    messages.push({
      role: "system",
      content: [
        `IMPORTANT: A Hotpot slash command (/${pendingWorkflow.command}) was just invoked.`,
        `The user's actual request is in the most recent user message between \`<<< ${pendingWorkflow.userInputLabel} >>>\` and \`<<< END ${pendingWorkflow.userInputLabel} >>>\` markers — read it carefully before doing anything else.`,
        `Your FIRST tool call MUST be \`Read("${pendingWorkflow.workflowPromptPath}")\`. Do NOT run \`ls\` / \`tree\` / \`git log\` / \`git status\` / any other exploration command first. Do NOT invoke any skill (especially \`project-structure-explorer\` or \`skill-creator\`). Do NOT reply with a greeting like "What would you like me to do?" or "你好" — just start the workflow.`,
      ].join(" "),
    });
    pendingWorkflow = undefined;
  }

  return { messages };
  ```

  保留原两条 system message 的注释（hotpot env-var dump 段 + language directive 段）；新追加的第三条上面加一段双语 doc-comment 说明用途。

- [x] **Step 3**：在 `pi.registerCommand("hotpot-new", { handler: async (args, ctx) => { ... } })` 中，在 `pi.sendUserMessage(text)` **这一行之前**插入：

  ```typescript
  pendingWorkflow = {
    command: "hotpot-new",
    workflowPromptPath: hotpot.HOTPOT_NEW_PROMPT,
    userInputLabel: "INITIAL TASK IDEA",
  };
  ```

- [x] **Step 4**：同样的赋值模式镜像到 `pi.registerCommand("hotpot-execute", ...)`，对应 `workflowPromptPath: hotpot.HOTPOT_EXECUTE_PROMPT`、`userInputLabel: "EXECUTION NOTES"`。

- [x] **Step 5**：同样的赋值模式镜像到 `pi.registerCommand("hotpot-finish-work", ...)`，对应 `workflowPromptPath: hotpot.HOTPOT_FINISH_WORK_PROMPT`、`userInputLabel: "FINISH NOTES"`。

- [x] **Step 6**：在 `assets/platforms/pi/extensions/hotpot/index.ts` 顶部的注释段 `// ── Slash-command registrations ──...` 内补一行说明：每个 handler 在 `pi.sendUserMessage` 之前必须设置 `pendingWorkflow`，新增 slash command 时同理。

- [x] **Step 7**：手工源码 review：grep `pi.sendUserMessage` 应找到 3 处，每处的紧前一行必须是 `pendingWorkflow = { ... };`。

**Validation:**

| Command | Expected |
| ------- | -------- |
| `grep -c 'pendingWorkflow = {' assets/platforms/pi/extensions/hotpot/index.ts` | `3`（三个 handler 各一处赋值） |
| `grep -c 'pendingWorkflow = undefined' assets/platforms/pi/extensions/hotpot/index.ts` | `1`（context 事件内的单次消费） |
| `grep -n 'pi.sendUserMessage' assets/platforms/pi/extensions/hotpot/index.ts` | 命中 3 处 |
| 手工读源码 | 每处 `pi.sendUserMessage(text)` 紧前一行必是 `pendingWorkflow = { ... };` |
| `grep -n 'A Hotpot slash command (' assets/platforms/pi/extensions/hotpot/index.ts` | 命中 context 事件注入文案 |

:::

#### Task 3: 同步刷新 `.pi/extensions/hotpot/index.ts` 部署副本

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.pi/extensions/hotpot/index.ts` | Modify | 由 `hotpot init --platform pi` 拷贝刷新；不手改 |

**Steps:**

- [x] **Step 1**：在仓库根目录运行 `cargo run -- init --platform pi`。
- [x] **Step 2**：核对 `diff assets/platforms/pi/extensions/hotpot/index.ts .pi/extensions/hotpot/index.ts && echo SAME`，必须输出 `SAME`。
- [x] **Step 3**：核对部署副本里同样存在 Task 1 / Task 2 的关键 grep 关键词（`A new Hotpot`、`pendingWorkflow`、`A Hotpot slash command (`）。
- [x] **Step 4**：检查 `cargo run -- init --platform pi` 的 stdout 中没有错误信息；如果有，先解决 `hotpot init` 本身的报错再继续。

**Validation:**

| Command | Expected |
| ------- | -------- |
| `diff assets/platforms/pi/extensions/hotpot/index.ts .pi/extensions/hotpot/index.ts && echo SAME` | `SAME` |
| `grep -c 'pendingWorkflow = {' .pi/extensions/hotpot/index.ts` | `3` |
| `grep -c 'A new Hotpot' .pi/extensions/hotpot/index.ts` | `1`（`buildPiCommandMessage` 的开篇引导模板字面值） |
| `grep -c 'A Hotpot slash command (' .pi/extensions/hotpot/index.ts` | `1`（context 事件注入文案） |

:::

#### Task 4: 把第三档失败模式写进 `docs/platforms/pi.md`

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/platforms/pi.md` | Modify | 升级 "Two failure modes" → "Three failure modes"；同步消息体结构描述 |

**Steps:**

- [x] **Step 1**：定位 `docs/platforms/pi.md` 的 "Hotpot Pi extension commands" 子章节中的 "#### Two failure modes the message body must survive" 小节。
- [x] **Step 2**：把小节标题改为 `#### Three failure modes the message body must survive`。
- [x] **Step 3**：在现有两条失败模式之后追加第三档：
  - 编号 3：`Attention loss on user-message body (current, mitigated by message reordering + per-turn system injection).`
  - 简介：即便 `role:user` 投递 + front-loaded `FIRST TOOL CALL` directive + FORBIDDEN list 都到位，更弱的指令跟随模型（实测 `kimi-k2.6` on `moonshotai-cn`）仍会把消息中段的 `<<< INITIAL TASK IDEA >>>` 块完全 attention 丢失。
  - 证据指向 session log 路径 `/Users/bytedance/.pi/agent/sessions/--Users-bytedance-RustProjects-hotpot--/2026-05-21T09-37-43-485Z_019e49e5-e43d-7545-96ee-db13bb965132.jsonl`，特别是 line 4（user message 完整投递）/ line 5（模型 thinking 写 "user hasn't asked anything yet" 并跑 `pwd && ls -la`）/ line 7（幻觉 agent-browser 扩展）/ line 16（收尾 "I'm ready to help with the skill-creator skill. What would you like to do?"）。
  - 对策：（i）`buildPiCommandMessage` 把 `<<< userInputLabel >>>` 块前移到消息首段；（ii）扩展闭包加 `pendingWorkflow` 字段，三个 handler 在 `pi.sendUserMessage` 之前赋值，`pi.on("context", ...)` 单次消费并清空，注入第三条 `role:system` 消息显式点出 workflow 路径、user 消息位置和 FORBIDDEN 列表。
- [x] **Step 4**：往上回读，找到描述 `buildPiCommandMessage` 当前消息体结构的段落（步骤 2 起的列表，"前置 ... `<<< {LABEL} >>> ... <<< END {LABEL} >>>` 分隔符 ..."），更新成"新顺序：开篇引导 → 用户输入块 → 空参 Exception → FIRST TOOL CALL directive → FORBIDDEN list → @path 替换表 → Platform note"。同步把"无 `=== USER ACTIVE REQUEST ===` ceremonial framing"那句保留（依然成立）。
- [x] **Step 5**：在该子章节末尾"When future Hotpot slash commands are added on Pi, register them with `pi.registerCommand` + `buildPiCommandMessage`..." 段落里补一句：handler 内必须在 `pi.sendUserMessage` 之前赋值闭包内的 `pendingWorkflow` 字段，否则第三档 mitigation 失效。
- [x] **Step 6**：把上文里所有 "Two known failure modes" / "two failure modes" 等表述一致替换为 "Three known failure modes" / "three failure modes"。

**Validation:**

| Command | Expected |
| ------- | -------- |
| `grep -c 'Three failure modes' docs/platforms/pi.md` | `≥ 1` |
| `grep -c 'Two failure modes' docs/platforms/pi.md` | `0`（旧表述全部替换） |
| `grep -c 'pendingWorkflow' docs/platforms/pi.md` | `≥ 1`（对策描述命中） |
| `grep -c 'Attention loss on user-message body' docs/platforms/pi.md` | `1` |
| `grep -n '2026-05-21T09-37-43-485Z_019e49e5' docs/platforms/pi.md` | 命中 1 行（证据指向） |

:::

#### Task 5: 同步更新 `docs/ARCH.md` + `docs/ARCH.zh_CN.md` 的 Pi Notes

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify | Pi `pi.registerCommand` / `buildPiCommandMessage` bullet 补 `pendingWorkflow` 契约 |
| `docs/ARCH.zh_CN.md` | Modify | 中文版同步上一项 |

**Steps:**

- [x] **Step 1**：定位 `docs/ARCH.md` "Notes For Future Agents" 末尾 bullet `Pi no longer uses prompt-template thin shells to back its slash commands. ...`。
- [x] **Step 2**：在该 bullet 末尾追加两句：
  - 第一句：`buildPiCommandMessage` body must put the user-input block (`<<< userInputLabel >>>`) FIRST, before the first-tool-call directive; this user-input-first ordering is a mitigation against the third documented Pi failure mode (attention loss on user-message body — see `docs/platforms/pi.md`).
  - 第二句：Each handler MUST also assign the closure-scoped `pendingWorkflow` field (in `assets/platforms/pi/extensions/hotpot/index.ts`) immediately before `pi.sendUserMessage`, so the next `pi.on("context", ...)` event injects a per-turn system reinforcement naming the workflow path, the user-input block markers, and the FORBIDDEN behaviors. Adding a new Pi slash command requires extending all three: the handler registration, the `buildPiCommandMessage` invocation with the right `ideaBlockLabel` / `atPathRefs`, AND the `pendingWorkflow` assignment.
- [x] **Step 3**：把 Step 2 的两句翻译成简体中文，加到 `docs/ARCH.zh_CN.md` 对应 bullet（"Pi 不再使用 prompt template thin shell..."）末尾。注意保持术语一致（`buildPiCommandMessage` / `pendingWorkflow` / `pi.sendUserMessage` 等英文标识符不翻译）。
- [x] **Step 4**：两份文档都不要新增章节或调整其他段落——本任务只追加这两句在已有 bullet 末尾。

**Validation:**

| Command | Expected |
| ------- | -------- |
| `grep -c 'pendingWorkflow' docs/ARCH.md` | `≥ 1` |
| `grep -c 'pendingWorkflow' docs/ARCH.zh_CN.md` | `≥ 1` |
| `grep -n 'user-input-first' docs/ARCH.md` | 命中 1 行 |
| `grep -n '用户输入块' docs/ARCH.zh_CN.md` | 命中 1 行（中文版的等价描述） |

:::

#### Task 6: 端到端验证（kimi-k2.6 + 强模型回归）

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| —— | —— | 纯观察任务；不改文件 |

**Steps:**

- [ ] **Step 1**：在仓库根目录用 Pi 客户端打开新 session，`/model` 切到 `moonshotai-cn / kimi-k2.6`。
- [ ] **Step 2**：发送 `/hotpot-new "做一个用户列表分页演示页面"`（或任意非空中文 idea，确保会触发用户输入识别）。
- [ ] **Step 3**：观察模型首次工具调用，必须满足全部下列断言：
  1. 首次 tool call 是 `Read`，且 `path` 字段等于 `$HOTPOT_NEW_PROMPT` 解析出的绝对路径。
  2. 在该 `Read` 之前**没有任何**其他 tool call（特别是 `bash`/`ls`/`tree` 或对 `AGENTS.md` 的 `Read`）。
  3. 模型 thinking / 文字回复中**没有**出现 `project-structure-explorer` 或 `skill-creator` 等 skill 名。
  4. 模型**没有**用 "What would you like me to do?" / "你好" 等 greeting 回退。
  5. 模型首段 text 输出能正确复述或派生自用户输入（如提到"用户列表"/"分页"），证明它实际看到了 `<<< INITIAL TASK IDEA >>>` 块。
- [ ] **Step 4**：把 Pi session 日志路径与上述 5 条断言的核对结果记录到工作笔记里（执行 review 时附进去）。
- [ ] **Step 5**：`/new` 起一个新 session，`/model` 切到强模型（`claude-sonnet-4-6` 或同档），重复 Step 2 命令。回归断言：模型仍然能正常进入 brainstorm 流程，不被新增的 system message 干扰。
- [ ] **Step 6**：若 Step 3 任一条断言不通过，立即停步并 surface 给用户，讨论：（a）继续微调 mitigation 文案；（b）正式建议切到更强模型；（c）扩大 FORBIDDEN 列表覆盖新观察到的失效路径。

**Validation:**

| Command | Expected |
| ------- | -------- |
| 肉眼核对 kimi-k2.6 session 日志 | Step 3 五条断言全部通过 |
| 肉眼核对强模型 session 日志 | brainstorm 流程正常完成，无回归 |
| 若失败：把失败截图/日志附进任务文件下方的 Open Questions | —— |

:::

### Validation

- `cargo run -- init --platform pi` 静默成功（仅 stdout 报告 init 状态，无 stderr 错误）。
- `diff assets/platforms/pi/extensions/hotpot/index.ts .pi/extensions/hotpot/index.ts && echo SAME` 输出 `SAME`。
- Task 1–5 每个表里的 `grep` 验收全部命中。
- Task 6 的两次端到端跑，kimi-k2.6 通过 5 条断言、强模型无回归。

### Risks and Watchouts

::: warning
- **新增 system message 文案过长可能反过来稀释 user message 注意力。** 第三条 system message 必须保持单段、字数克制；若实测仍未压住，先尝试再缩短文案、再考虑外溢字段而不是无限扩展条目。
- **`pendingWorkflow` 单次消费假设依赖 Pi 的 `context` 事件确实在 `sendUserMessage` 之后的下一次 provider 请求前触发。** 若 Pi 内部存在不触发 `context` 的 turn（例如 streaming 中断重试），可能出现标记未被消费的边角情况；handler 入口的无条件覆盖是这种情况的兜底。
- **重排消息后，已有的 hotpot-execute / hotpot-finish-work 路径必须同步重排。** Task 1 的修改作用于 `buildPiCommandMessage` 本体而非分支，对三个 handler 同时生效；review 时需逐一确认三处都按新顺序产出文本。
- **kimi-k2.6 之外的弱模型可能有别的失效模式。** 本任务只针对当前观察到的第三档，不预防未来出现的第四档；文档同步把"escalate via 加强 mitigation 或换模型"路径保留。
- **`.pi/extensions/hotpot/index.ts` 是部署副本，不要手改。** Task 3 必须通过 `hotpot init --platform pi` 同步——若手改了部署副本，下次 `hotpot update` 会回滚。
:::

---

## Execution Instructions

把本任务文件完整内容交给 execution sub-agent，agent 必须：

- 编辑前先完整 `Read` 本文件。
- 按 `## Plan` 一节的 Task 1 → Task 6 顺序执行；不要乱序（Task 3 必须在 Task 1+2 完成后才跑 init；Task 6 必须在 Task 1–5 全部完成后跑）。
- 保留 `## Task` 一节的全部 Requirements / Non-Goals / Approved Design 决策；不允许扩张 scope。
- 实施时把每个 `- [ ]` checkbox 改成 `- [x]` 以跟踪进度（在写权限允许的情况下）。
- 若发现必需的文件、命令、API、假设与本 handoff 实际不符，立即停步并以阻塞汇报回来。
- 完成所有 Implementation Tasks 后跑一次 `## Plan > ### Validation` 的完整命令清单；任何一条不通过都视为未完成。
- 报告完成时附上：每个 Task 的 grep 验收输出、`diff` 输出、Task 6 的 5 条断言核对结果（哪几条通过、哪几条失败、失败时的具体观察）。

## Open Questions

- 若 Task 6 的 kimi-k2.6 验收即使在 A+B 双层防御下仍失败，是把"建议切到更强模型"正式作为 `docs/platforms/pi.md` 第三档 mitigation 的最终 fallback 写进文档，还是继续迭代文案？这个决策留给完成 Task 6 后再讨论。
