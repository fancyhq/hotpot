# 把 VuePress 分支闸门从 env-var 切换到文件存在检测

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | false | 5 | low |
:::

---

## Task

### Summary

::: info
`assets/prompts/hotpot-new.md` 当前用 `$HOTPOT_VUEPRESS_ENABLED == "true"` 作为
"是否走 VuePress 写作规范 + 收尾流程"的闸门。但只有 Claude Code / Codex / Pi
把这个 env-var 推进了 AI 对话上下文；**OpenCode 插件只通过 `shell.env`
把它塞进 shell 子进程**，AI 对话本身**永远看不到**——于是 OpenCode 上闸门
silent 失败，brainstorm 结束既不询问是否在浏览器查看，写出的 task 文件也不应用
VuePress 写作约定。本任务把闸门改为"`.hotpot/prompts/vuepress.md` 与
`vuepress-style.md` 双双在盘上"的**文件存在检测**，让四个平台跨平台一致。
:::

### User Request

::: info 用户原话
> 在 `opencode` 中，`brainstorming` 结束未提示用户是否使用 `vuepress` 查看，
> 写入的文件也未使用 `vuepress` 规定格式，需要验证所有平台的一致性，是否仅
> claude code 做了特殊处理。

后续 brainstorm 已确认：Claude Code 没有"特殊处理"——Claude / Codex / Pi
三平台的 hook 都把 VuePress 三件套推进了 AI 对话；唯独 OpenCode 插件只做了
`shell.env` 而漏了 AI 对话注入。最终选择"切换闸门为文件存在检测"作为
**最小爆炸半径**的跨平台修复（拒绝改 OpenCode 插件、拒绝改 echo workaround、
拒绝三件套大改 hook plumbing）。
:::

### Approved Design

::: tip

**单一闸门规则**：以 `.hotpot/prompts/vuepress.md` 与 `vuepress-style.md`
是否双双在盘上作为唯一闸门。`hotpot vuepress install` / `uninstall` 的原子
状态已经保证这两个文件与 `[vuepress] enabled = true` 同步落地 / 同步移除，
所以"文件在盘上"在语义上等价于"VuePress 启用"，而且**跨四个平台一致可观测**
（每个平台的 AI 都能用 Bash `test -f` 来检测）。

**关键不变式**：

1. `HOTPOT_VUEPRESS_ENABLED` env-var 本身**保留**：bootstrap / hook 仍发
   它，OpenCode/Pi 插件类型字段仍带它，`src/context.rs` 单元测试仍用它。
   只是把它从"AI 闸门"降级为"信息字段"。
2. `## Optional: VuePress Integration` 段重写为"先 Bash `test -f` 探测，再
   按结果走 BEFORE / AFTER 两步"的形态，措辞跨平台中立（不绑定到 `Read`
   也不绑定到 `ls`，让各平台 AI 用自己 Bash 工具实现）。
3. 文档 / 代码注释里所有"env-gate"措辞替换为"file-existence gate"，保持
   `src/` 注释、`docs/ARCH*.md` 表格、`assets/prompts/vuepress.md` 内部
   引文在新词条下一致。
4. 验证阶段额外跑 grep 审计，扫描"env-gate"残留 + 扫描 `\$HOTPOT_VUEPRESS_ENABLED ==`
   在 prompt 资产里的残留，确保没遗漏。

**新闸门逻辑（伪代码，将写进 `hotpot-new.md`）**：

```bash
[ -f "$ROOT_DIR/.hotpot/prompts/vuepress.md" ] \
  && [ -f "$ROOT_DIR/.hotpot/prompts/vuepress-style.md" ] \
  && echo enabled \
  || echo disabled
```

- `enabled` → BEFORE 写 `.md` 先 Read `vuepress-style.md`、AFTER 写完
  Read `vuepress.md`。
- `disabled` → 跳过整段，走 `## Final Response` 默认收尾。

:::

### Alternatives Considered

- **(B) 扩展 OpenCode 插件，把 VuePress 三件套推进 AI 对话**：可行但
  OpenCode plugin API 并未文档化"inject system message"事件，只能 hack
  `tool.execute.before` 返回值，**修复局限于 OpenCode 且留下隐患**。未选。
- **(C) Workflow 第一步显式 `echo $HOTPOT_VUEPRESS_ENABLED`**：零插件改动，
  但每次都多一次 Bash 调用，且 `enabled` 与 install 状态可能不同步
  （env-var 从 `[vuepress] enabled` 派生，但 install 才真的落盘文件——
  文件存在检测才是 ground truth）。未选。
- **(D) 三件套打平：扩 OpenCode 插件 + 改 Codex SessionStart + 改 Claude SubagentStart 包含 VuePress 三件套**：最深的契约修复，但本次只是 prompt
  分支问题，杀鸡用牛刀，且影响多个 hook 注入点，回归面大。未选。
- **(A) 切换为文件存在检测（已选）**：单一改动点 + 跨平台对称 + 不动
  插件代码 + 不动 hook 契约 + 与 ARCH 已声明的"opt-in 原子状态"语义一致。

### Requirements

- `assets/prompts/hotpot-new.md` 的 `## Optional: VuePress Integration` 段
  完全重写，不再依赖 `$HOTPOT_VUEPRESS_ENABLED`。
- 改完后 `hotpot init` 必须能把新 prompt 同步到 `.hotpot/prompts/hotpot-new.md`，
  且 `hotpot vuepress install` 必须能把新版 `vuepress.md` 同步到
  `.hotpot/prompts/vuepress.md`（执行阶段需运行这两条命令完成落盘）。
- `cargo build` 通过，`cargo test` 全绿（注释 / 文本改动不应影响任何测试）。
- 全仓库 grep `env-gate` 与 prompt 资产里的 `\$HOTPOT_VUEPRESS_ENABLED ==`
  应只剩当前任务文件本身的引用（任务文件位于 workspaces 目录、不参与
  AI 加载链路，不影响行为）。

### Non-Goals

::: details Non-Goals

- 不删除 `HOTPOT_VUEPRESS_ENABLED` env-var 本身，也不改 `hotpot hook bootstrap`
  / Claude PreToolUse / Codex PreToolUse / OpenCode shell.env / Pi context
  push 中关于 VuePress 三件套的任何逻辑。
- 不改任一平台的插件 / 扩展代码（`assets/platforms/**/plugins/`,
  `assets/platforms/**/extensions/`）。
- 不修复 Claude/Codex 在非 PreToolUse 场景下 env-var 可能不可见的脆性
  （那是 SessionStart / SubagentStart / UserPromptSubmit 不包含 VuePress
  三件套的副带问题，本次不在范围内）。
- 不动 `src/context.rs` 的 `resolve_vuepress_enabled[_with_source]` 解析链路
  和它的单元测试。
- 不动 `assets/prompts/hotpot-execute.md` / `hotpot-finish-work.md`——这两份
  prompt 调 `hotpot vuepress stop --if-running` 是无条件幂等调用，与闸门
  无关。

:::

### Project Context

- 仓库根：`/Users/bytedance/RustProjects/hotpot`。
- 资产源（被 `hotpot init` 安装到 `.hotpot/prompts/`）：`assets/prompts/`。
- VuePress opt-in 资产源（被 `hotpot vuepress install` 安装）：源在
  `assets/prompts/vuepress.md` 与 `assets/prompts/vuepress-style.md`，对应
  `src/assets/vuepress_opt_in.rs` 的 `VUEPRESS_OPT_IN_ASSETS` 表。
- VuePress 状态当前为 `enabled`，故 `.hotpot/prompts/vuepress*.md` 已在盘上，
  改完后 install 一次即可覆盖。
- 测试用 `cargo run --` 取代 `hotpot`（本仓库未安装 hotpot 二进制；见
  `AGENTS.md`）。
- 当前任务记录：`task_id=u5hmCl5Hme`, `time=2026-05-18`,
  `title=unify-vuepress-gate-to-file-check`, `active=true`,
  `status=In Progress`.

---

## Plan

### Mode

- tdd: false

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/hotpot-new.md` | Modify | 把 `## Optional: VuePress Integration` 段从 env-gate 改为 file-existence gate（主改动） |
| `assets/prompts/vuepress.md` | Modify | line 12 的 "per the env-gate in" 引文改为 "per the file-existence gate in" |
| `docs/ARCH.md` | Modify | 资产分级表 line 111 + VuePress Integration 主段把 "env-gate" 措辞改为 "file-existence gate" 并补一句解释 |
| `docs/ARCH.zh_CN.md` | Modify | 同 ARCH.md 的中文同步 |
| `src/assets/vuepress_opt_in.rs` | Modify | 文件头注释 "env-gate" 措辞替换 |
| `src/assets/mod.rs` | Modify | line 269/274 注释 "env-gate" 措辞替换 |
| `src/context.rs` | Modify | line 96/100/685 注释里 "prompt env-gate" 措辞替换 |
| `.hotpot/prompts/hotpot-new.md` | Modify | 由 `cargo run -- init --platform all` 自动从资产源刷新（执行阶段验证一致） |
| `.hotpot/prompts/vuepress.md` | Modify | 由 `cargo run -- vuepress install` 自动从资产源刷新 |

### Implementation Tasks

#### Task 1: 重写 `assets/prompts/hotpot-new.md` 的 VuePress Integration 段

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/hotpot-new.md` | Modify | 主分支闸门改写 |

**Steps:**

- [ ] **Step 1**: 用 `Read` 工具读 `assets/prompts/hotpot-new.md` 的 line 495-510
  区域，定位 `## Optional: VuePress Integration` 段（精确边界：从
  `## Optional: VuePress Integration` 到 `## Final Response` 之前）。
- [ ] **Step 2**: 用 `Edit` 工具把整段（包含标题行的前一行起到 `## Final Response`
  之前的空行止）替换为新版。新版 MUST 满足：
  - 开头 1 段说明"不要依赖 `$HOTPOT_VUEPRESS_ENABLED`——env-var 在 OpenCode
    上不进 AI 对话；用文件存在检测，跨平台一致"。
  - 给出 Bash 探测命令（用 fenced code block；MUST 5：不要把它放在 `:::`
    容器里嵌套——它会去到 task 文件里就是 task 文件，但 prompt 资产本身就是
    AI 读取的源，不要包 `:::`）：
    ```bash
    [ -f "$ROOT_DIR/.hotpot/prompts/vuepress.md" ] && \
      [ -f "$ROOT_DIR/.hotpot/prompts/vuepress-style.md" ] && \
      echo enabled || echo disabled
    ```
  - `enabled` 分支：保留原 BEFORE / AFTER 两步语义不变，仅把"`$HOTPOT_VUEPRESS_ENABLED == "true"` 时"措辞换成"探测返回 `enabled` 时"。
  - `disabled` 分支：保留"忽略本段、不要 Read 那两个文件、走默认 `## Final Response`"
    语义，仅把 env-var 等式换成"探测返回 `disabled`"。
  - 显式说明：**只能在探测返回 `enabled` 后才 Read `vuepress*.md`**，避免
    禁用项目 Read 到不存在文件。
- [ ] **Step 3**: 跑 `grep -n 'HOTPOT_VUEPRESS_ENABLED' assets/prompts/hotpot-new.md`，
  **期望输出为空**（资产源里此 env-var 应完全消失）。
- [ ] **Step 4**: 跑 `grep -n 'file-existence gate\|test -f' assets/prompts/hotpot-new.md`，
  **期望命中新闸门描述至少 1 次**。

:::

#### Task 2: 同步更新 `assets/prompts/vuepress.md` 的内部引文

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/vuepress.md` | Modify | line 12 引文同步 |

**Steps:**

- [ ] **Step 1**: 用 `Edit` 把 `assets/prompts/vuepress.md` line 12 的
  `(per the env-gate in` 替换为 `(per the file-existence gate in`。
- [ ] **Step 2**: 跑 `grep -n 'env-gate' assets/prompts/vuepress.md`，**期望
  输出为空**。

:::

#### Task 3: 更新 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md`

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify | 表 line 111 + VuePress Integration 主段措辞更新 |
| `docs/ARCH.zh_CN.md` | Modify | 同 ARCH.md 的中文同步 |

**Steps:**

- [ ] **Step 1**: 用 `Edit` 把 `docs/ARCH.md` line 111 单元格里
  `The env-gate in \`hotpot-new.md\` only Reads these when \`$HOTPOT_VUEPRESS_ENABLED == "true"\`, so disabled projects keeping no copy on disk is what guarantees a clean AI context.`
  替换为：
  `The file-existence gate in \`hotpot-new.md\` only Reads these when both files are present on disk — which is exactly when VuePress is installed (atomic invariant maintained by \`hotpot vuepress install\` / \`uninstall\`). Same observable on every platform, so OpenCode (which never surfaces the \`HOTPOT_VUEPRESS_ENABLED\` env-var into AI conversation context) behaves identically to Claude / Codex / Pi.`
- [ ] **Step 2**: 同样替换 `docs/ARCH.zh_CN.md` line 111 对应中文文案，措辞翻译为简体中文：
  `\`hotpot-new.md\` 的 file-existence gate 只在这两份文件都在盘上时 Read 它们——而它们在盘上恰好等价于 VuePress 已安装（由 \`hotpot vuepress install\` / \`uninstall\` 维护的原子状态）。四个平台都靠 Bash \`test -f\` 直接观测，所以 OpenCode（其插件不把 \`HOTPOT_VUEPRESS_ENABLED\` 推进 AI 对话）也能跟 Claude / Codex / Pi 走同样的分支。`
- [ ] **Step 3**: 跑 `grep -n 'env-gate' docs/ARCH.md docs/ARCH.zh_CN.md`，
  **期望输出为空**。

:::

#### Task 4: 更新 Rust 源码注释中的 "env-gate" 措辞

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/assets/vuepress_opt_in.rs` | Modify | 文件头注释 |
| `src/assets/mod.rs` | Modify | line 269/274 注释 |
| `src/context.rs` | Modify | line 96/100/685 注释 |

**Steps:**

- [ ] **Step 1**: 用 `Read` 工具读 `src/assets/vuepress_opt_in.rs` 文件头注释
  （约 line 10-30），把所有 "env-gate" 替换为 "file-existence gate"，并把对
  `$HOTPOT_VUEPRESS_ENABLED == "true"` 的引用改为对"两份文件在盘上"的描述。
  双语风格保留（英文一段 + 中文一段）。
- [ ] **Step 2**: 同样处理 `src/assets/mod.rs` line 265-280 区域的双语注释。
- [ ] **Step 3**: 同样处理 `src/context.rs` 的三处："line 96-100 的
  `HOTPOT_VUEPRESS_ENABLED` 字段 doc 注释"以及"line 685 的 serialize 段注释"，
  把"prompt env-gate"措辞改为"prompt file-existence gate"。注意：env-var 本身
  的解析逻辑、`parse_bool` 单测、`resolve_vuepress_enabled` 解析顺序文档**保持
  不变**——它仍是 hook/bootstrap 的契约字段，只是不再是 AI 闸门。
- [ ] **Step 4**: 跑 `grep -rn 'env-gate' src/`，**期望输出为空**。

:::

#### Task 5: 刷新已安装的 `.hotpot/prompts/` 副本 + 整体验证

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.hotpot/prompts/hotpot-new.md` | Modify | 通过 `init --platform all` 刷新 |
| `.hotpot/prompts/vuepress.md` | Modify | 通过 `vuepress install` 刷新 |

**Steps:**

- [ ] **Step 1**: 跑 `cargo run -- init --platform all`，**期望** 退出码 0，
  且 stdout 提到至少一项 prompt 资产被刷新或保持一致（命令幂等）。
- [ ] **Step 2**: 跑 `cargo run -- vuepress install`，**期望** 退出码 0，
  操作幂等，`.hotpot/prompts/vuepress.md` 与 `vuepress-style.md` 内容与 asset
  源一致。
- [ ] **Step 3**: 跑 `diff assets/prompts/hotpot-new.md .hotpot/prompts/hotpot-new.md` 与
  `diff assets/prompts/vuepress.md .hotpot/prompts/vuepress.md`，**期望两份 diff 都为空**。
- [ ] **Step 4**: 跑 `cargo build`，**期望** 退出码 0、无新 warning（注释改动
  不应触发 warning）。
- [ ] **Step 5**: 跑 `cargo test`，**期望** 全绿（特别是 `src/context.rs`
  里 `HOTPOT_VUEPRESS_ENABLED` 相关单元测试仍通过）。
- [ ] **Step 6**: 跑全仓审计 `grep -rn 'env-gate' . --include='*.md' --include='*.rs' --include='*.ts' | grep -v node_modules | grep -v '.hotpot/workspaces/'`，**期望输出为空**
  （workspaces 下的任务文件本身不影响 AI 行为，是历史档案）。
- [ ] **Step 7**: 跑 `grep -rn '\$HOTPOT_VUEPRESS_ENABLED ==' assets/prompts/ .hotpot/prompts/`，
  **期望输出为空**（prompt 资产里再无此 env-var 等式判断）。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo build` | 退出码 0，无新 warning |
| `cargo test` | 全绿，含 `HOTPOT_VUEPRESS_ENABLED` 解析测试 |
| `grep -rn 'env-gate' src/ assets/prompts/ .hotpot/prompts/ docs/ARCH.md docs/ARCH.zh_CN.md` | 空输出 |
| `grep -rn '\$HOTPOT_VUEPRESS_ENABLED ==' assets/prompts/ .hotpot/prompts/` | 空输出 |
| `diff assets/prompts/hotpot-new.md .hotpot/prompts/hotpot-new.md` | 空 diff |
| `diff assets/prompts/vuepress.md .hotpot/prompts/vuepress.md` | 空 diff |

- 人工验证：本地用 OpenCode（如可用）跑一次 `/hotpot:new`，确认 brainstorm
  收尾会问"是否在浏览器查看"并按 vuepress-style.md 写文件。如果本机没装
  OpenCode，至少在 Claude Code 上跑一次确认不出现回归。

### Risks and Watchouts

::: warning

- **风险：覆写 `## Optional: VuePress Integration` 段时误吃掉 `## Final Response`
  起始行**。Edit 时 `old_string` 一定要锚定到段尾的空行，不要包到下一标题。
- **风险：注释改动触发 doc-test 失败**。`src/context.rs` 的 doc-test 不直接
  断言注释文案，应安全；但跑 `cargo test` 时留意 `doc-tests` 段输出。
- **风险：`hotpot vuepress install` 幂等性**。已在 ARCH 声明为幂等，但若有人
  手动改过 `.hotpot-hub/`，install 可能要求 verify_install_consistency。本任务
  应在干净状态下执行；如失败，按 install 提示修复后再继续。
- **约束**：不要改 `hotpot hook bootstrap` 中 `HOTPOT_VUEPRESS_ENABLED` 的
  emit 行为；不要删 `src/context.rs::parse_bool` 单元测试；不要改任一平台
  插件 / 扩展。
- **约束**：新版 prompt 段必须明确说明"探测返回 `disabled` 时不要 Read
  `vuepress*.md`"——禁用项目里这两份文件不在盘上，盲目 Read 会触发
  "File not found" 错误并污染 AI 上下文。

:::

---

## Execution Instructions

把本任务文件完整内容交给 hotpot-execution 子代理。执行子代理 MUST：

- 实施前完整阅读本文件，特别是 `## Task > ### Approved Design` 与
  `### Non-Goals`。
- 按 `## Plan > ### Implementation Tasks` 顺序执行 Task 1 → 5，每步 done
  后勾上 `- [ ]` 复选框。
- 保留所有 `## Task` 里列出的 Requirements 与 Non-Goals，不要扩张 scope
  （特别是：**不要碰任何平台插件 / 扩展代码**；不要删 env-var；不要改 hook
  注入契约）。
- 完成所有实施 Task 后跑 `### Validation` 表里的全部命令，确认全部通过；
  任一失败就停在那里、记录原因，**不要**继续后续 Task。
- 如发现 Plan 与现状偏离（例如 line 号变了、有未预期的 env-gate 残留点），
  停下来报阻塞而不是自行扩范围。
- 当任务文件里某 step 的 grep 命令检测到本任务文件自身（位于
  `.hotpot/workspaces/HusuSama/tasks/`）的匹配项时，视为档案性 false positive，
  忽略即可（任务文件不在 AI 加载链路上）。
