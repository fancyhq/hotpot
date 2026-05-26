# strengthen-pi-new-arguments-prompt

## Task

### Summary

修复 Pi 平台 `/hotpot-new <初始想法>` 的"参数虽然送达但 AI 仍忽略文本、直接进入 brainstorm 追问"的回归。上次任务 `fix-pi-new-arguments`（commit `51bc25b`）已在 `assets/platforms/pi/prompts/hotpot-new.md` 引入 `$ARGUMENTS`，Pi 也确实把命令后的文本注入了 prompt，但当前模板仅使用「if non-empty, treat it as ...」这种**软性条件式**措辞，被共享 body (`assets/prompts/hotpot-new.md` 的 `## Command Usage` / `## Brainstorming Flow`) 中"Ask clarifying questions"的更强指令盖过——AI 看到了 `$ARGUMENTS` 注入的文本却仍认为没有 initial task idea。本任务通过**强化 Pi shell 模板措辞**对症修复，不动共享 workflow，不动其他三个平台资产。

### User Request

> 针对 pi agent ，在 prompt 中已经添加了 "$ARGUMENT" 等，在pi中使用的时候，也投递了这些数据，但是 pi 在执行时，AI 依然认为没有给其发送任务，帮我修复一下这个问题

补充确认（brainstorm）：
- 受影响命令：仅 `/hotpot-new`（execute / finish-work 暂不在本任务范围内）。
- 现象：Pi AI 直接进入 brainstorm，但忽略 `$ARGUMENTS` 注入的用户文本，不把它当作 initial idea，而是继续追问"你的初始想法是什么"。

### Approved Design

只改 Pi `hotpot-new.md` 模板措辞，把 `$ARGUMENTS` 行升级为**显式分隔块 + 无条件指令 + anti-question 强制句**的组合，让共享 body 看到时无法用「ask one concise question」覆盖。同步更新 `src/assets/platforms/pi.rs` 的资产回归测试匹配新措辞，并在 `docs/platforms/pi.md` 补一段 Hotpot Pi 模板**注入命令参数的正确模式**说明。

模板新措辞（实现时严格按此结构）：

```
<<< INITIAL TASK IDEA (verbatim from `/hotpot-new` arguments) >>>
$ARGUMENTS
<<< END INITIAL TASK IDEA >>>

The block above IS the user's initial task idea. Proceed directly to brainstorming using it as the starting point. Your first brainstorm message MUST explicitly reference or paraphrase the idea above before asking any clarifying question. Do NOT ask another question to obtain the initial task idea — you already have it.

If the block above is empty (no arguments supplied), follow the shared workflow's fallback and ask exactly one concise question for the initial task idea.
```

关键点：
1. 用 `<<< INITIAL TASK IDEA ... >>>` / `<<< END INITIAL TASK IDEA >>>` 显式分隔块，让 AI 把它识别为数据载荷而不是叙述。
2. 把 `treat it as` 这种软建议改成 `IS the user's initial task idea` + `Proceed directly` 无条件指令。
3. 新增**强制 anti-question 句**：`Your first brainstorm message MUST explicitly reference or paraphrase the idea above` 和 `Do NOT ask another question to obtain the initial task idea — you already have it.`，对冲共享 body `## Brainstorming Flow` 中的 "Ask clarifying questions" 强指令。
4. 空参数 fallback 独立成段，仅在分隔块内为空时触发。

### Alternatives Considered

- **方案 B（Pi extension 接管命令）**：用 `pi.registerCommand("hotpot-new", ...)` + `ctx.sendUserMessage()` 自己组装消息绕过 prompt template 变量。控制力最强，但引入「Pi 走 extension、其它平台走 prompt template」的架构分叉，与本症状的最小修复半径不匹配，未采用。
- **方案 C（在 Pi shell 里 override 共享 body 条款）**：显式追加一段「override：Pi 已注入 idea，忽略共享 body `## Command Usage` 第二点」。比方案 A 激进，且需要追踪共享 body 具体条款表述，将来文案漂移时容易失同步，未采用。
- **推荐方案 A**：症状根因是「软条件被强指令覆盖」，最小、对症、不引入新机制、不耦合共享 body 文案；已有的回归测试可平滑升级。

### Requirements

- `/hotpot-new <文本>` 在 Pi 中必须让 AI 第一条 brainstorm 消息**显式引用或释义**用户给的 `<文本>`，不得在 idea 已存在时再追问 initial idea。
- 空参数情况下，仍保持「问一次 initial idea」的默认 fallback 行为。
- 不修改共享 `assets/prompts/hotpot-new.md`。
- 不修改 Claude、OpenCode、Codex 的 `new` 资产。
- 不修改 Pi `hotpot-execute.md` / `hotpot-finish-work.md`（本任务非目标，避免扩大范围）。
- 更新 `src/assets/platforms/pi.rs` 的资产回归测试匹配新措辞；新增断言：anti-question 强制句存在。
- `docs/platforms/pi.md` 补充一段 Hotpot Pi 模板「命令参数注入正确模式」说明，覆盖：分隔块 / 无条件指令 / anti-question 三要素，并说明软性 hint 会被共享 body 的 `Ask clarifying questions` 盖过这一原因。
- 代码与测试断言保持 English；文档遵循双语注释约定（项目惯例：英文段 + 中文段，本任务文档新增段落是用户面向的英文 doc 段，保持英文即可）。

### Non-Goals

- 不修复 Pi 自身 UI / 模型推理行为。
- 不改造 Pi extension 命令注册体系。
- 不动 `hotpot-execute` / `hotpot-finish-work` Pi 模板（即便它们也缺 `$ARGUMENTS`，也留给后续任务处理，避免本任务扩大范围）。
- 不改 `docs/ARCH.md` / `docs/ARCH.zh_CN.md`：本修复只是 Pi shell 文案调整，不改变跨平台执行流契约。
- 不引入新 cargo 依赖。
- 不实施 Pi 端到端自动化测试。

### Project Context

- Pi 模板源：`assets/platforms/pi/prompts/hotpot-new.md`；安装拷贝：`.pi/prompts/hotpot-new.md`（两份内容应一致，由 `hotpot init` / `hotpot update` 同步）。
- 资产注册：`src/assets/platforms/pi.rs::ASSETS` 已包含此 prompt；已有 `#[cfg(test)] mod tests` 里的 `pi_new_prompt_template_passes_command_arguments` 测试需要更新断言（旧断言："treat it as the user's initial task idea" / "If it is empty" / "ask one concise question" 会因措辞调整而部分失效）。
- 共享 body：`assets/prompts/hotpot-new.md` 的 `## Command Usage` 第二点要求"If no task idea was provided, ask one concise question"，`## Brainstorming Flow` 第二点要求"Ask clarifying questions one at a time"——这两条强指令是 AI 忽略软性 hint 的直接原因。
- Pi 模板变量规则：`docs/platforms/pi.md` 明确 Pi 不会自动把命令参数附加给模型，模板正文必须显式引用 `$ARGUMENTS` / `$@` / `$1`。
- `assets/platforms/pi/prompts/hotpot-new.md` 当前已知行：第 9-11 行为旧的软性条件式 `$ARGUMENTS` 段；修复需替换为新结构。
- 项目使用 `cargo test`（本仓库未安装 `hotpot` 二进制时改用 `cargo run --`）。
- 现有资产引擎为 `src/assets/` 下的 owned/merge 资产清单。本次只动 prompt 模板文件内容 + 同模块的回归测试，不需要碰资产引擎本身。

## Plan

### Mode

- tdd: false

### File Map

- Modify: `assets/platforms/pi/prompts/hotpot-new.md` — 把 `$ARGUMENTS` 软性条件式段替换为「分隔块 + 无条件指令 + anti-question」三要素结构。
- Modify: `.pi/prompts/hotpot-new.md` — 与上面 asset 内容一致；测试时本地 Pi 直接读这份文件（按 `docs/platforms/pi.md` 末尾"同步资产源与实际 prompt"提醒）。
- Modify: `src/assets/platforms/pi.rs` — 升级 `pi_new_prompt_template_passes_command_arguments` 测试断言，新增 anti-question 强制句断言；保留 `$ARGUMENTS` 存在性断言。
- Modify: `docs/platforms/pi.md` — 在 Hotpot prompt template 规则附近追加一段「命令参数注入正确模式」说明。
- 不动: `assets/prompts/hotpot-new.md`，`assets/platforms/{claude,opencode,codex}/`，Pi 其它两份 prompt，`src/` 其它文件，`docs/ARCH.md`，`docs/ARCH.zh_CN.md`。

### Implementation Tasks

#### Task 1: Rewrite Pi `hotpot-new` template to enforce initial-idea injection

**Files:**

- Modify: `assets/platforms/pi/prompts/hotpot-new.md`
- Modify: `.pi/prompts/hotpot-new.md`

- [x] Step 1: 打开 `assets/platforms/pi/prompts/hotpot-new.md`，定位现有的 `Initial task idea from command arguments: $ARGUMENTS` 起始的两段（约第 9-11 行）。
- [x] Step 2: 用以下精确结构替换上述两段（其它行——frontmatter、用户面向 invocation 行、`$HOTPOT_NEW_PROMPT` Read 指令段、Pi `@path` 替换表、Platform note、Output Language 段——一律保持原样，不动）：

  ```
  <<< INITIAL TASK IDEA (verbatim from `/hotpot-new` arguments) >>>
  $ARGUMENTS
  <<< END INITIAL TASK IDEA >>>

  The block above IS the user's initial task idea. Proceed directly to brainstorming using it as the starting point. Your first brainstorm message MUST explicitly reference or paraphrase the idea above before asking any clarifying question. Do NOT ask another question to obtain the initial task idea — you already have it.

  If the block above is empty (no arguments supplied), follow the shared workflow's fallback and ask exactly one concise question for the initial task idea.
  ```

  注意：
  - 三处 `<<< ... >>>` 行字面必须完全一致（含两个空格、`/hotpot-new`、`verbatim`、`END INITIAL TASK IDEA` 等）。
  - 不要在分隔块内对 `$ARGUMENTS` 加引号、加 escape 或额外空白——Pi 模板变量是直接字符串替换。
  - 「— you already have it」中是 EM DASH（U+2014），与 `assets/platforms/pi/prompts/hotpot-new.md` 现有 Unicode 标点习惯一致；不要替换成两个 ASCII 减号。
  - 段间空行保持单个空行。
- [x] Step 3: 在 Pi 模板现有的 `The full workflow is defined at $HOTPOT_NEW_PROMPT ...` 行之前确保有一个空行，使新结构和后续段落视觉分离。
- [x] Step 4: 复制相同改动到 `.pi/prompts/hotpot-new.md`（保持 asset 源与 installed prompt 一致；这是 `docs/platforms/pi.md` 在 Pi 章节末尾明确要求的同步惯例）。
- [x] Step 5: 用 `diff -u assets/platforms/pi/prompts/hotpot-new.md .pi/prompts/hotpot-new.md` 验证两份文件完全一致；expect 空输出。

#### Task 2: Update regression test to assert the strengthened wording

**Files:**

- Modify: `src/assets/platforms/pi.rs`

- [x] Step 1: 打开 `src/assets/platforms/pi.rs`，定位 `#[cfg(test)] mod tests` 内的 `pi_new_prompt_template_passes_command_arguments` 测试。
- [x] Step 2: 保留断言：`template.contains("$ARGUMENTS")`（不变）。
- [x] Step 3: 删除旧断言：
  - `template.contains("treat it as the user's initial task idea")`
  - `template.contains("If it is empty")`
  - `template.contains("ask one concise question")`
- [x] Step 4: 新增以下断言（顺序无所谓，但每条对应一个 invariant，错误消息保持英文，简洁说明意图）：
  - `template.contains("<<< INITIAL TASK IDEA")` — 分隔块开头存在
  - `template.contains("<<< END INITIAL TASK IDEA >>>")` — 分隔块结尾存在
  - `template.contains("IS the user's initial task idea")` — 无条件指令存在
  - `template.contains("MUST explicitly reference or paraphrase")` — anti-skip 强制句存在
  - `template.contains("Do NOT ask another question to obtain the initial task idea")` — anti-question 强制句存在
  - `template.contains("If the block above is empty")` — 空参数 fallback 仍在
- [x] Step 5: 运行 `cargo test -p hotpot pi_new_prompt_template_passes_command_arguments`（如包名识别失败则用 `cargo test pi_new_prompt_template_passes_command_arguments`）；expect 通过。
- [x] Step 6: 运行 `cargo test`；expect 全量通过、无新增 warning（已有 3 个无关 dead_code warning 是基线，不算回归）。

#### Task 3: Document the Pi-template argument-injection pattern

**Files:**

- Modify: `docs/platforms/pi.md`

- [x] Step 1: 打开 `docs/platforms/pi.md`，找到现有的 "Pi does not implicitly append slash-command arguments to the prompt body. Template authors must place `$ARGUMENTS`, `$@`, or positional variables in the Markdown body when the model needs to see command text." 这段（在 Commands / Prompt template rules 区域）。
- [x] Step 2: 紧跟该段后追加一段新内容（英文，2-4 句），覆盖以下三点：

  1. 仅把 `$ARGUMENTS` 内联到散文段落里仍可能被共享 workflow body 中"ask clarifying questions"这类强指令覆盖；
  2. Hotpot Pi 模板（如 `hotpot-new.md`）注入命令参数时应采用 **分隔块 + 无条件指令 + 强制 anti-question 句** 的三要素模式（可引用 `<<< INITIAL TASK IDEA ... >>>` 作为示例形态）；
  3. 这条规则的目的是让 AI 把变量替换后的文本识别为数据载荷，而非可选 hint，从而避免回归到 brainstorm 默认追问。

  注意：不引入新章节标题；以原有段落为锚点直接追加。
- [x] Step 3: 保持文档其它段落、示例、frontmatter 不变。
- [x] Step 4: 用 `grep -n "INITIAL TASK IDEA" docs/platforms/pi.md` 抽检新段落已落入；expect 至少一行命中。

### Validation

- `cargo test pi_new_prompt_template_passes_command_arguments` — 通过，证明新措辞断言齐全。
- `cargo test` — 全量通过，无新增 warning。
- `diff -u assets/platforms/pi/prompts/hotpot-new.md .pi/prompts/hotpot-new.md` — 空输出，证明 asset 源与已安装 prompt 同步。
- `grep -n "INITIAL TASK IDEA" assets/platforms/pi/prompts/hotpot-new.md .pi/prompts/hotpot-new.md docs/platforms/pi.md` — 三份文件均至少一行命中。
- 手动复读 `assets/platforms/pi/prompts/hotpot-new.md`：确认替换后段落顺序为「frontmatter → invocation 行 → 新分隔块 + 无条件指令段 → 空段落 fallback → Read `$HOTPOT_NEW_PROMPT` 指令 → `@path` 替换表 → Platform note → Output Language」。

### Risks and Watchouts

- **`<<<` / `>>>` 字面别误改成 markdown 代码块**：`<<<` 不是 fenced code block 语法，Pi 会原样渲染为字符串载荷给 AI；不要把它包进 ``` ``` ``` 三重反引号，否则 Pi 模板变量替换 + markdown 渲染叠加可能让 `$ARGUMENTS` 被当成代码内字面量而非变量。
- **EM DASH (U+2014)**：在「— you already have it」中保留 EM DASH；避免在编辑器自动替换为两个 ASCII 减号。Rust 测试断言不需要匹配这个 DASH（只匹配 `Do NOT ask another question to obtain the initial task idea`），所以不会因 Unicode 差异 false-fail。
- **`.pi/prompts/hotpot-new.md` 漂移**：本仓库在 `docs/platforms/pi.md` 末尾明确要求 asset 源与 `.pi/prompts/` 实际文件同步；如果忘记同步两份文件，本地 Pi 测试拿到的还是旧 prompt，问题不会消失。Task 1 Step 5 的 `diff` 校验是硬约束。
- **避免误改 `assets/prompts/hotpot-new.md`**：那是共享 body，跨平台共用；本任务不动它。注意路径区分 `assets/prompts/` (共享) vs `assets/platforms/pi/prompts/` (Pi 专属)。
- **测试断言贴文案就够，别贴整段**：维持现有"断言短小稳定 fragment"的风格；不要把整段新措辞作为单个 `contains` 实参，否则未来微调标点会引发 noisy 失败。
- **不要扩张到 execute/finish-work**：用户明确只针对 `/hotpot-new`；扩张范围属于 scope creep，本任务 Non-Goals 已禁止。
- **不要碰 `docs/ARCH.md` / `docs/ARCH.zh_CN.md`**：本次只是 Pi 模板文案调整，不改变跨平台执行流契约或 env-var 契约。

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task. Specifically: do not touch Pi `hotpot-execute.md`/`hotpot-finish-work.md`, other-platform assets, the shared workflow body, or architecture docs.
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff (e.g. if the existing Pi `hotpot-new.md` content layout has drifted significantly since this task was written).
- Run the validation commands before reporting completion. If `cargo test` surfaces a new unrelated warning, include it in the report but do not silence it.
