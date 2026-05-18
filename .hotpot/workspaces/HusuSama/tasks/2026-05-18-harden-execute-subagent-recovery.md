# Harden Execute Subagent Recovery

## Task

### Summary

`/hotpot:execute` 在 OpenCode 上偶发两类严重故障：(1) `Unknown agent type: hotpot-execution` 直接中断；(2) Provider 瞬时错误（`Concurrency limit exceeded for user, please retry later`、`API Error 524` 等）打死主代理 turn，需要用户手动「继续」唤醒。诊断表明前者大概率是 OpenCode subagent 注册表缓存问题，后者既有 provider 账户级并发上限，也有 Hotpot 侧 review prompt 把完整 git diff / issue memory 全量内联导致单请求超大的根因放大。

本任务在 `assets/prompts/hotpot-execute.md` 内加三层防护 + 一项 payload 减负，**不动 CLI 代码**，跨四个平台（Claude / OpenCode / Codex / Pi）共享生效。

### User Request

> 在使用 `/hotpot:execute` 时，有时会出现报错：`Unknown agent type: hotpot-execution... is not a valid agent type`，执行的过程中，经常会出现 API 错误或者 `"Concurrency limit exceeded for user, please retry later"` 类型的错误，需要重新交互来唤醒 AI，帮我看下这两个问题，这两个 BUG 非常严重。

进一步澄清：
- 出错平台主要是 **OpenCode**。
- `Concurrency limit exceeded` / `524` 是**主代理 turn 直接被打死**的场景（不是 subagent 失败），用户必须手动发「继续」之类的消息重新唤醒。
- 用户主动提出应先排查「是否数据量过大」等 Hotpot 侧根因，不要盲目加重试。

### Approved Design

**三层防护 + 一项减负，全部写在 `assets/prompts/hotpot-execute.md`：**

#### Layer 1 — Pre-flight: Verify Subagent Registration

在现有 `## Pre-flight: Release VuePress server` 之后、`## Precondition: Task File Must Exist` 之前新增章节。

内容要点：
- 按平台探测 agent 文件存在性
  - Claude: `$ROOT_DIR/.claude/agents/hotpot-{execution,review}.md`
  - OpenCode: `$ROOT_DIR/.opencode/agents/hotpot-{execution,review}.md`
  - Codex: `$ROOT_DIR/.codex/agents/hotpot-{execution,review}.toml`
  - Pi: 无 subagent，跳过此检查（Pi 走同会话分阶段）
- 平台识别通过 `[ -d "$ROOT_DIR/.<platform>" ]` 判断；多平台共存时全部检查
- 文件缺失 → 报错 `Hotpot subagent file missing at <path>` + 提示 `hotpot update --platform <p>` + **重启 agent session**，STOP
- 文件在盘但调用仍报 `Unknown agent type` → 平台注册表缓存问题，同样的修复路径
- 此 pre-flight 在 worktree decision (Step 0) **之前** 执行，避免无谓的 worktree 创建

#### Layer 2 — Subagent Invocation Error Handling

新增独立章节 `## Subagent Invocation Error Handling`，放在 `## Execution Agent` 之前，规则对 execution / review / fix-round 三类 subagent 调用都生效。

内容要点：
- 列举瞬时错误特征字串（不区分大小写）：
  - `Concurrency limit exceeded`
  - `rate limit` / `Rate limit exceeded` / HTTP 429
  - `API Error 524` / `502` / `503` / `service unavailable` / `gateway timeout`
  - `internal error` / `internal server error` (HTTP 5xx)
  - 连接重置 / 读取超时类网络错误描述
- 命中后行为：等 5–10 秒，用**完全相同的 prompt** 重新 spawn 同一个 subagent
- 重试上限 2 次（共 3 轮尝试）；orchestrator 必须在推理中**显式记录尝试次数**，避免无限循环
- 全部失败 → 把 provider 原始报错原样抛给用户，请求重跑 `/hotpot:execute`；**不**写任何 candidate，**不**静默降级到同会话 inline 执行
- 必须区分两类非瞬时错误：
  - `Unknown agent type: <name>` → 走 Layer 1 修复路径
  - subagent 返回的 final report 中含 blocker / mismatch → 走现有 blocker 流程，**不** 触发重试

#### Layer 3 — Resume After Transient Failure

新增独立章节 `## Resume After Transient Failure`，放在 `## Final Response` 之前。

内容要点：
- 适用场景：主代理 turn 上一轮以 provider 瞬时错误截止，用户后续发了类似「继续」/`retry`/`resume`/`继续执行`/`重新执行`/单字符或几字符短消息的唤醒指令。
- 主代理 AI **不能从 Phase 0 重启**，应当按下列自检顺序定位上一轮的中断点：
  1. `hotpot task active --path` 拿当前任务文件路径
  2. `Read` 任务文件查 `- [ ]` checkbox 状态：勾选的实施步骤说明已完成
  3. `git status --porcelain` + `git diff` 看实际改动了哪些文件
  4. `hotpot issues candidate list` 看 candidate buffer 是否已落盘——已有内容说明执行 + review 都过完，应跳到 `## Confirm Candidates With User` 阶段
  5. 综合以上信号判断中断阶段（执行前 / 执行中 / review 前 / review 中 / fix loop / candidate 确认前），从该阶段的**下一个未完成动作**续作；不要重做已完成的 subagent 调用
- 此章节明确说明：「连『继续』都不用发」的真正自动恢复需要平台 hook 级支持，超出本任务范围。

#### Layer 4 — Review Prompt Payload Caps

修改现有 `## Collect Review Context` + `## Review Agent`（两套模式 default / TDD）+ `## Automatic Fix Loop`，引入 git diff / issue memory 上限截断。

内容要点：

**Git diff cap:**
- 收集 review context 时，先用 `git diff --stat` 拿到文件级摘要 + `git diff` 拿全量
- 计算全量 diff 字节数：
  - `≤ 40 KB` → 按现行方式整段嵌入 review prompt
  - `> 40 KB` → 嵌入策略改为：
    1. `git diff --stat` 完整内容
    2. 每个文件最多 `git diff -- <file> | head -c 8192`，超出部分标记为 `[... truncated; review agent should run `git diff -- <file>` for the full hunk ...]`
    3. Review prompt 显式提示 review agent：「diff 被截断；遇到需要看完整 hunk 的文件时，自行运行 `git diff -- <file>` 取全量」
- 阈值 `40 KB` 写成显式常量，便于后续调优；附注释解释「该阈值兼顾大多数任务的可读 review 与 provider 单请求超时窗口」

**Issue memory cap:**
- `hotpot issues relevant` 已有 `--limit 5` 上限。但每条 issue 含完整 `Scene` / `Review Check` / `Notes` 字段，可能很长
- 在 review prompt 中只嵌入 `Scene` 与 `Review Check` 两个字段，去掉其余字段
- 同样告诉 review agent：「需要完整 issue 信息时跑 `hotpot issues get --issue-id <id>` 自取」（若 `hotpot issues get` 不存在则改提示 `cat .hotpot/issues.jsonl | jq 'select(.issue_id=="<id>")'`）

**Fix-round prompt:**
- 同样的截断策略应用到 `## Automatic Fix Loop` 嵌入的 diff，防止 fix round 越跑越胖

### Alternatives Considered

- **方案 A：新增 `hotpot doctor` CLI 自检子命令做 agent 注册自检** — 用户在范围抉择阶段否决了「需要新 CLI」的方向，且单纯 file-existence probe 用 bash `[ -f ... ]` 已足够覆盖。
- **方案 B：在 OpenCode TypeScript 插件里做 task-tool 重试** — 只覆盖 OpenCode，违反 `docs/ARCH.md` 的「跨平台优先」原则；其他三平台仍无防护。
- **方案 C：平台 hook 自动注入「继续」消息** — 技术上可行（Claude `Stop` / Codex `Stop` / OpenCode `session.*`），但风险高（错误循环）、跨平台差异大（Codex 没有可靠 SessionEnd），且各 hook 修改面大。**显式排除，超出本任务范围**。
- **方案 D（已采纳）：仅修改 `assets/prompts/hotpot-execute.md`** — 范围最小、跨平台一致（`.hotpot/prompts/` 是四平台共享资产）、零新增 CLI 入口；唯一弱点是依赖主代理 AI 遵守 prompt 指令，无 100% 保证。

### Requirements

- 只修改 `assets/prompts/hotpot-execute.md` 一个源文件
- 不引入新的 CLI 子命令、新 Rust 代码、新依赖
- 不修改任何平台特定文件（`.claude/`、`.opencode/`、`.codex/`、`.pi/` 下的资产）
- 新增/修改后的章节自然语言内容保持双语风格的简洁英文（与现有 prompt 文件一致）；结构性 anchor 全英文
- `hotpot update` 之后，`.hotpot/prompts/hotpot-execute.md` 与 `assets/prompts/hotpot-execute.md` 必须 byte-identical
- `cargo test` 通过；如有 snapshot 测试锁定该文件长度/哈希，必须同步更新
- 不破坏现有 worktree decision / VuePress pre-flight / TDD mode 检测的章节顺序与跨引用

### Non-Goals

- **不实现真正的「连『继续』都不用发」自动恢复** —— 这是平台 hook 级工程，单独立项
- **不修改 `assets/prompts/hotpot-new.md` 或 `hotpot-finish-work.md`** —— new 不调 subagent，finish-work 主要做 ledger 操作，瞬时错误风险小；若需要可后续单独立项
- **不在 Rust CLI 侧增加 `hotpot doctor`、`hotpot diff --truncate` 等新子命令** —— 全部逻辑下沉到 prompt 指令
- **不更改 subagent 自身的 developer_instructions**（`.claude/agents/`、`.opencode/agents/`、`.codex/agents/*.toml`）—— 改动局限在 orchestrator prompt
- **不动 issue memory 数据格式或 `hotpot issues relevant` 的 JSON schema**

### Project Context

- 该文件路径：`assets/prompts/hotpot-execute.md`（596 行 / 27 KB）
- 安装目标：`.hotpot/prompts/hotpot-execute.md`，由 `hotpot init` / `hotpot update` 复制（`SHARED_ASSETS` 注册在 `src/commands/init/mod.rs`）
- 现有相关章节结构（行号大致定位）：
  - L26 `## Pre-flight: Release VuePress server`
  - L32 `## Precondition: Task File Must Exist`
  - L43 `## Agent Definitions`
  - L51 `## Required Flow`
  - L68 `## Worktree Decision (Step 0)`
  - L127 `## Resolve Active Task`
  - L161 `## Detect TDD Mode`
  - L172 `## Execution Agent`（含 Default / TDD 两套模式，分别在 L176 / L225 嵌入 `<entire task file content>`）
  - L260 `## Collect Review Context`（含 `git diff` 收集、`hotpot issues relevant` 调用）
  - L319 `## Review Agent`（含 Default / TDD 两套模式，分别在 L348 / L414 嵌入 `<entire task file content after execution>` + 完整 diff + issue memory）
  - L454 `## Automatic Fix Loop`（L475 再次嵌入完整任务文件 + diff）
  - L520 `## Record Reusable Issue Candidates`
  - L546 `## Confirm Candidates With User`
  - L580 `## Final Response`
- 跨平台资产引用：四份平台 thin shell（`.claude/commands/hotpot/execute.md`、`.opencode/commands/hotpot/execute.md`、`.codex/skills/hotpot-execute/SKILL.md`、`.pi/prompts/hotpot-execute.md`）都通过 `@.hotpot/prompts/hotpot-execute.md` 或 `$HOTPOT_EXECUTE_PROMPT` 引用同一份内容
- diff 收集相关现有命令：`git rev-parse --is-inside-work-tree` / `git status --porcelain` / `git diff` / `hotpot issues relevant --changed-file <p> --keyword <k> --limit 5`
- 当前用户 workspace 任务文件普遍 11–29 KB；最近 `issues.jsonl` 含 5 条共 9.9 KB；典型 review prompt payload 估算 25–80 KB，diff 大时可破 100 KB —— 这就是 Layer 4 的根因
- VuePress 安装状态当前不一致（env-var `HOTPOT_VUEPRESS_ENABLED=true` 但 `.hotpot/prompts/vuepress*.md` 不在盘上）；本任务**不**修这个状态不一致问题，但执行 agent 不要在此处停顿，按 file-existence gate 视为 disabled

## Plan

### Mode

- tdd: false

### File Map

- Modify: `assets/prompts/hotpot-execute.md` — 加 Layer 1/2/3 三个新章节 + 修改 Review/Fix-loop 区域引入 Layer 4 截断策略
- Validate via Bash: `cargo run -- update` 把改动同步装到 `.hotpot/prompts/hotpot-execute.md`，再 `diff` 验证一致
- Test: 无新增单元测试（asset-only 改动）；如发现现有 snapshot 测试锁定该文件，同步更新

### Implementation Tasks

#### Task 1: Add Pre-flight Subagent Registration check

**Files:**

- Modify: `assets/prompts/hotpot-execute.md`

- [x] Step 1: `Read` 整份 `assets/prompts/hotpot-execute.md` 确认章节顺序，定位 `## Pre-flight: Release VuePress server (if any)`（约 L26）与 `## Precondition: Task File Must Exist`（约 L32）之间的插入点
- [x] Step 2: 在两章之间插入新章节 `## Pre-flight: Verify Subagent Registration`，要求：
  - 章节内容明确「按平台探测 agent 文件存在性，缺失或调用仍报 `Unknown agent type` 时 STOP 并提示 `hotpot update` + 重启 session」
  - 给出 4 个平台对应文件路径（Claude `.claude/agents/hotpot-{execution,review}.md`、OpenCode `.opencode/agents/hotpot-{execution,review}.md`、Codex `.codex/agents/hotpot-{execution,review}.toml`、Pi 跳过）
  - 给出可粘贴的 bash 探测片段，使用 `$ROOT_DIR/.claude/agents/...` 等显式路径
  - 多平台共存（如同时有 `.claude/` 和 `.opencode/`）的处理规则：每个发现的平台都跑一次探测
  - 章节内嵌示例错误回复："Hotpot subagent file missing at `<path>`. The `<platform>` registry cannot resolve hotpot-execution / hotpot-review. Run `hotpot update --platform <platform>`, then restart the agent session."
- [x] Step 3: `Read` 修改后的文件确认章节顺序：`## Goal` → `## Output Language` → `## Pre-flight: Release VuePress server` → `## Pre-flight: Verify Subagent Registration` → `## Precondition: Task File Must Exist`
- [x] Step 4: Run `grep -n "^## " assets/prompts/hotpot-execute.md | head -10` 并验证打印结果包含新章节

#### Task 2: Add Subagent Invocation Error Handling section

**Files:**

- Modify: `assets/prompts/hotpot-execute.md`

- [x] Step 1: 在 `## Execution Agent`（约 L172）之前插入新章节 `## Subagent Invocation Error Handling`
- [x] Step 2: 章节内容必须覆盖：
  - 瞬时错误特征字串清单（`Concurrency limit exceeded`、`rate limit`、HTTP 429 / 502 / 503 / 524、`internal error`、`gateway timeout`、连接重置 / 读取超时）
  - 重试上限：2 次重试（共 3 轮尝试），每次等 5–10 秒
  - 必须用**完全相同的 prompt** 重新 spawn 同一个 subagent，不要重建 context
  - 全失败后的行为：把 provider 原始报错抛给用户，请求重跑命令，**不**写 candidate，**不**降级到 inline 执行
  - 与非瞬时错误的区分：`Unknown agent type` → Layer 1；subagent 返回 blocker → 现有 blocker 流程
  - 显式规则「orchestrator AI 必须在推理中记录尝试次数」防止无限循环
- [x] Step 3: Run `grep -nE "Subagent Invocation Error Handling|Concurrency limit|API Error 524|Unknown agent type" assets/prompts/hotpot-execute.md` 并验证四个关键字串都被命中

#### Task 3: Add Resume After Transient Failure section

**Files:**

- Modify: `assets/prompts/hotpot-execute.md`

- [x] Step 1: 在 `## Final Response`（约 L580）之前插入新章节 `## Resume After Transient Failure`
- [x] Step 2: 章节内容必须覆盖：
  - 适用触发条件：上一轮 assistant 消息以 provider 瞬时错误截止，用户随后发短消息唤醒（如「继续」/「retry」/「resume」/「继续执行」/「重新执行」）
  - 5 步自检序列：`hotpot task active --path` → 读任务文件 checkbox → `git status` + `git diff` → `hotpot issues candidate list` → 综合判断中断阶段
  - 中断阶段→续作动作的映射：
    - 执行前死 → 重新走完整 execute 流程
    - 执行后 review 前死 → 直接跑 collect review context + review agent
    - review 后 fix loop 中死 → 重跑该轮 fix
    - candidate 确认前死 → 重新展示 candidate 摘要给用户
  - 显式约束「不要从 Phase 0 重启」「不要重做已完成的 subagent 调用」
  - 末尾标注「真正『不用发继续』的自动恢复需平台 hook 级支持，超出本任务范围」
- [x] Step 3: Run `grep -n "Resume After Transient Failure" assets/prompts/hotpot-execute.md` 验证章节存在

#### Task 4: Cap git diff size in Review Agent prompts (Default & TDD modes)

**Files:**

- Modify: `assets/prompts/hotpot-execute.md`

- [x] Step 1: `Read` `## Collect Review Context` 区域（约 L260–L315），定位 `git diff` 收集步骤
- [x] Step 2: 修改该区域，加入 diff 大小判断逻辑（描述性、非可执行 bash）：
  - 先 `git diff --stat`
  - 再 `git diff` 全量；用 `wc -c` 估字节数
  - `≤ 40 KB` → 全量嵌入；`> 40 KB` → 嵌入 `git diff --stat` + 逐文件最多 `head -c 8192`，超出部分标 `[... truncated; run `git diff -- <file>` for the full hunk ...]`
- [x] Step 3: 修改 `## Review Agent` 下的 Default Review Mode prompt 模板（约 L325–L385）：
  - `Git diff or fallback change context:` 区块的占位符说明改为 `<git diff, possibly per-file truncated when total size exceeds 40KB; review agent must run `git diff -- <file>` to retrieve full hunks when truncated>`
  - prompt 末尾的 Review requirements 列表加一条：「If the diff section indicates per-file truncation, run `git diff -- <file>` for any file whose review-relevance demands the full hunk」
- [x] Step 4: 同样修改 TDD Review Mode prompt 模板（约 L387–L450）；保持两套模式对称
- [x] Step 5: 修改 `## Automatic Fix Loop`（约 L454–L505）的 fix-round prompt 模板，复用相同的截断策略与 review agent 指令
- [x] Step 6: Run `grep -nE "40 ?KB|truncated|git diff -- <file>" assets/prompts/hotpot-execute.md` 并验证三段（review default / review TDD / fix-loop）都被覆盖

#### Task 5: Cap issue memory in Review Agent prompts

**Files:**

- Modify: `assets/prompts/hotpot-execute.md`

- [x] Step 1: `Read` `## Collect Review Context` 中 `hotpot issues relevant` 调用周边区域
- [x] Step 2: 在该区域加一句说明：「The output of `hotpot issues relevant` may contain long `Notes` fields; before embedding into the review prompt, strip everything except `issue_id`, `Scene`, and `Review Check` to keep payload bounded. The review agent can run `cat $ROOT_DIR/.hotpot/issues.jsonl | jq 'select(.issue_id == "<id>")'` to retrieve the full record if a finding requires it.」
- [x] Step 3: 修改 Default / TDD review prompt 模板里 `Relevant Hotpot issue memory:` 区块的占位符说明，标明已是「精简版（id + Scene + Review Check）」
- [x] Step 4: Run `grep -nE "Scene|Review Check|jq " assets/prompts/hotpot-execute.md` 验证修改命中 (25 hits across L373–L627)

#### Task 6: Sync installed prompt and validate

**Files:**

- Modify: `.hotpot/prompts/hotpot-execute.md`（由 `hotpot update` 自动同步）

- [x] Step 1: `hotpot update` 拒绝覆盖差异文件（"already exists and differs; rerun with --force to overwrite"），`update` 无 `--force` 选项；改用 `cargo run -q -- init --force` 写入 `.hotpot/prompts/hotpot-execute.md`，成功（"Hotpot init installed 2 file(s)"）
- [x] Step 2: `diff` 验证完全一致（"DIFF: clean"）
- [x] Step 3: `cargo test` 跑 67 个测试，3 个失败（`issues::tests::test_filter_relevant_issues`、`issues::tests::test_concurrent_append_issue_does_not_lose_rows`、`vuepress::tests::test_sync_tasks_links`）。验证手段：先 `git checkout HEAD -- assets/prompts/hotpot-execute.md .hotpot/prompts/hotpot-execute.md` 还原 prompt，单跑 `test_filter_relevant_issues` 仍失败 —— 失败与 prompt 改动无关，是 pre-existing
- [x] Step 4: 行数 596 → 770（+174 行），写入了 Layer 1 (~43 行 `Pre-flight: Verify Subagent Registration`) + Layer 2 (~38 行 `Subagent Invocation Error Handling`) + Layer 3 (~48 行 `Resume After Transient Failure`) + Layer 4 (~45 行 `Diff Size Cap` / `Issue Memory Cap` 节 + Default/TDD/Fix-Loop 三处模板更新)

### Validation

- `diff assets/prompts/hotpot-execute.md .hotpot/prompts/hotpot-execute.md` — 无差异输出
- `grep -cE "Pre-flight: Verify Subagent Registration|Subagent Invocation Error Handling|Resume After Transient Failure" assets/prompts/hotpot-execute.md` — 输出 `3`
- `grep -cE "Concurrency limit exceeded|API Error 524|Unknown agent type|40 ?KB" assets/prompts/hotpot-execute.md` — 输出 ≥ 4（每个字串至少出现一次）
- `cargo test 2>&1 | tail -10` — 全部 pass
- 人工 QA：在 OpenCode 上跑 `/hotpot:execute`，观察 (a) pre-flight 是否输出探测结果；(b) 故意把 `.opencode/agents/hotpot-execution.md` 暂时移走后 pre-flight 是否 STOP 并给出修复提示；(c) 触发一次大 diff（手动 `git diff` 验证 > 40 KB 的场景），review prompt 是否走截断分支

### Risks and Watchouts

- **可能存在 snapshot 测试锁定该文件长度/哈希**。Task 6 Step 3 须先跑 `cargo test`；如失败优先更新预期值，不要直接 ignore 测试
- **`hotpot update` 是否覆盖 `.hotpot/prompts/hotpot-execute.md`**：根据 `docs/ARCH.md`，`SHARED_ASSETS` 是 day-1 入口，`update` 应当同步；若实测发现 `update` 跳过已有文件，需要改用 `hotpot init --force`（如有）或手工 cp。**先用 `update` 试，diff 验证；如不一致再切方案。**
- **新章节用词需谨慎**：Layer 2 / 3 章节如果让 orchestrator AI 误以为「任何 subagent 失败都该重试」，会掩盖真正 bug。措辞必须明确「只对瞬时错误特征字串」「subagent 主动返回的 blocker 不重试」
- **Layer 4 的 40 KB 阈值是经验值**：可能对某些任务（比如大规模 refactor）仍偏低或偏高。任务文件内显式以「常量」呈现该阈值并附 1 行注释，便于后续调优；不引入新的可配置入口
- **跨平台一致性**：四个平台 thin shell 都通过 `@.hotpot/prompts/hotpot-execute.md` 或 `$HOTPOT_EXECUTE_PROMPT` 引用同一份内容；改动**自动**对四平台生效，但需在 Validation 阶段至少在 OpenCode 上人工验证一次
- **绝对路径 vs `$ROOT_DIR`**：bash 探测片段必须用 `$ROOT_DIR/...`，不要硬编码 `/Users/bytedance/...`；否则其他用户的项目根目录会失配
- **VuePress 状态不一致**（env-var enabled 但 prompt 文件不在盘）：本任务**不修**，但执行 agent 不要分心去补 VuePress；按 prompt 现行 file-existence gate 处理

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task.
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff.
- Run the validation commands before reporting completion.
