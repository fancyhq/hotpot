# Fix VuePress Task URL Stem

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | false | 2 | low |
:::

---

## Task

### Summary

::: info
修正 `/hotpot:new` 在 VuePress 收尾流程中给出的任务浏览链接：当前说明把任务文件名 `2025-03-02-mock-task.md` 截成 `mock-task`，但 VuePress 实际路由需要不带 `.md` 的完整任务文件名 stem，即 `2025-03-02-mock-task`。这会导致用户启动 VuePress 后拿到错误链接，无法直接打开刚创建的任务页面。
:::

### User Request

::: info 用户原话
在启动 vuepress 服务后，给出的链接地址错误，应该使用不带后缀的完整任务文件名称，比如 2025-03-02-mock-task，现在是使用的 mock-task，未包含完整的文件名
:::

### Approved Design

::: tip
按推荐方案创建本任务：修正 VuePress 收尾 prompt 的链接拼接规则，让浏览器 URL 使用完整任务文件 stem，而不是只使用日期后面的 title slug。具体目标是把 `assets/prompts/vuepress.md` 中的 `<task-slug>` 概念改为完整任务页面路径段，例如 `2025-03-02-mock-task.md` → `2025-03-02-mock-task`，并同步更新当前项目已安装的 `.hotpot/prompts/vuepress.md`，使当前开发仓库立即使用正确说明。

执行阶段还需要检查 VuePress hub 路由、侧边栏或任务索引生成是否已经按完整 stem 工作。如果路由实现本来正确，只更新 prompt 文案与示例；如果发现 hub 资产中也存在同类截断逻辑，则在同一任务内修复对应源资产并验证。
:::

### Alternatives Considered

- 只修改当前项目 `.hotpot/prompts/vuepress.md`：能立刻修复本仓库提示，但不会修复 Hotpot 源资产，后续 `hotpot update` 或其他项目安装仍会得到错误说明，因此不采用。
- 只修改源资产 `assets/prompts/vuepress.md`：能修复未来安装和更新，但当前已安装 prompt 仍然错误，当前项目的 `/hotpot:new` 收尾流程不会立即变好，因此不采用。
- 同步修改源资产与当前已安装 prompt，并检查 VuePress hub 路由是否一致：覆盖当前行为和未来安装路径，范围仍集中在 VuePress task URL 说明上，因此批准采用。

### Requirements

- VuePress 收尾流程给出的浏览 URL 必须使用完整任务文件名 stem：`<url>/<HOTPOT_USERNAME>/<YYYY-MM-DD-title>`。
- 示例必须明确说明 `2025-03-02-mock-task.md` 会生成 `2025-03-02-mock-task`，不能再生成 `mock-task`。
- 同步更新 `assets/prompts/vuepress.md` 和 `.hotpot/prompts/vuepress.md`，避免源资产与当前安装资产分叉。
- 检查 `.hotpot-hub` 源资产中与路由、侧边栏、任务索引相关的代码或模板，确认没有同类“去掉日期前缀”的错误。
- 保持结构性 token、命令名、路径、JSON key、`HOTPOT_USERNAME` 等机器可读内容为 English。
- 输出给用户的自然语言保持中文，代码和命令输出保持 English。

### Non-Goals

::: details Non-Goals
- 不重新设计 VuePress hub 页面、样式或 TaskIndex UI。
- 不改变 Hotpot 任务文件命名约定 `<YYYY-MM-DD>-<title>.md`。
- 不改变 `hotpot vuepress start` 的进程管理、端口、TTL 或 stop 逻辑。
- 不改变 `/hotpot:new` 的任务创建流程、active task 处理或 TDD 模式判断。
- 不引入新的路由格式或兼容旧错误 URL 的重定向机制，除非探索发现当前 VuePress 实现已经需要最小修复。
:::

### Project Context

已检查项目架构与相关文件：

- `docs/ARCH.md` 说明 VuePress hub 是 opt-in，`/hotpot:new` 在 VuePress 启用时会走 `.hotpot/prompts/vuepress.md` 的收尾流程。
- `.hotpot/prompts/hotpot-new.md` 要求先用文件存在性 gate 判断 VuePress 是否启用；当前仓库 `.hotpot/prompts/vuepress.md` 和 `.hotpot/prompts/vuepress-style.md` 都存在，说明 VuePress 已启用。
- `assets/prompts/vuepress.md` 是源资产，通过 `src/assets/vuepress_opt_in.rs` 注册并安装到 `.hotpot/prompts/vuepress.md`。
- 当前 `assets/prompts/vuepress.md` 与 `.hotpot/prompts/vuepress.md` 都在 Step 3 中写着：浏览 URL 为 `<url>/<HOTPOT_USERNAME>/<task-slug>`，并定义 `<task-slug>` 是“日期前缀和 `.md` 扩展名之间的 kebab-case slug”，示例 `2026-05-17-add-vuepress-link.md` → `add-vuepress-link`。这正是用户报告的错误模式。
- `src/task/storage.rs::get_task_filename` 生成任务文件 stem 为 `<time>-<sanitized-title>`，例如 `2026-05-18-fix-vuepress-task-url-stem`。VuePress 链接说明应沿用这个完整 stem。
- `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md` 当前未细写 VuePress 任务页 URL 规则；如果实现改动只限 prompt 文案，架构执行流不变，可以不更新架构文档。若执行阶段发现需要改 VuePress hub 路由生成逻辑，则需要同步更新两份架构文档。

---

## Plan

### Mode

- tdd: false

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/vuepress.md` | Modify | 修正 VuePress 收尾流程源资产中的任务 URL 拼接说明与示例。 |
| `.hotpot/prompts/vuepress.md` | Modify | 同步当前项目已安装 prompt，确保当前 `/hotpot:new` 立即给出正确链接。 |
| `src/assets/vuepress_hub.rs` | Test | 检查 hub 资产中的 sidebar / route / task index 逻辑是否按完整文件 stem 生成链接。 |
| `.hotpot-hub/docs/.vuepress/sidebar.js` | Test | 若存在，检查当前安装 hub 的运行时代码是否与源资产链接规则一致。 |
| `docs/ARCH.md` | Modify | 仅当执行阶段改动 VuePress hub 路由生成或执行流时，更新 English 架构说明。 |
| `docs/ARCH.zh_CN.md` | Modify | 仅当执行阶段改动 VuePress hub 路由生成或执行流时，更新中文架构说明。 |

### Implementation Tasks

#### Task 1: 修正 VuePress 收尾 prompt 的任务 URL 规则

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/vuepress.md` | Modify | 源资产必须修正，保证未来安装和更新后的 prompt 正确。 |
| `.hotpot/prompts/vuepress.md` | Modify | 当前项目已安装 prompt 必须同步，保证本仓库后续 `/hotpot:new` 立即正确。 |

**Steps:**

- [x] **Step 1**: 在 `assets/prompts/vuepress.md` 中定位 Step 3 的 URL 说明，确认当前文本定义 `<task-slug>` 为日期前缀和 `.md` 之间的 slug。
- [x] **Step 2**: 将该说明改为使用完整任务文件名 stem，例如命名为 `<task-page>` 或 `<task-file-stem>`，并明确它是任务文件名去掉 `.md` 后的完整值。
- [x] **Step 3**: 更新示例，确保 `2025-03-02-mock-task.md` 或等价示例映射到 `2025-03-02-mock-task`，不要再出现映射到 `mock-task` 的描述。
- [x] **Step 4**: 对 `.hotpot/prompts/vuepress.md` 做同样修改，保持与源资产内容一致；如果只改源资产，当前项目不会立即使用新说明。
- [x] **Step 5**: 运行 `diff -u assets/prompts/vuepress.md .hotpot/prompts/vuepress.md`；预期除路径上下文外没有内容差异。

:::

#### Task 2: 检查 VuePress hub 路由一致性并验证

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/assets/vuepress_hub.rs` | Test | 源资产内可能包含 sidebar、task index 或 route 拼接逻辑，需要确认没有相同截断 bug。 |
| `.hotpot-hub/docs/.vuepress/sidebar.js` | Test | 当前安装 hub 的运行时代码如果存在，也要确认与源资产规则一致。 |
| `docs/ARCH.md` | Modify | 仅当实际执行流或 hub 路由逻辑变化时更新。 |
| `docs/ARCH.zh_CN.md` | Modify | 仅当实际执行流或 hub 路由逻辑变化时更新。 |

**Steps:**

- [x] **Step 1**: 搜索 `src/assets/vuepress_hub.rs`、`.hotpot-hub/docs/.vuepress/` 和相关 VuePress 资产中的 `slug`、`replace`、日期正则、`.md`、`path`、`route`、`sidebar`，确认是否有把 `YYYY-MM-DD-` 前缀剥掉的逻辑。
- [x] **Step 2**: 如果 hub 资产已经以完整文件 stem 生成路由，不修改 hub 代码，只在完成报告中说明 prompt 是唯一错误源。
- [x] **Step 3**: 如果发现 hub 资产也把日期前缀剥掉，则做最小修复，让路由和链接都使用完整文件 stem，并同步更新当前 `.hotpot-hub` 中对应已安装文件。
- [x] **Step 4**: 判断是否影响 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md` 描述。仅当 Step 3 发生路由/执行流代码改动时，同步更新两份架构文档；如果只改 prompt 文案，不更新架构文档。
- [ ] **Step 5**: 运行 `cargo test`；预期测试通过，允许现有 warning 保持不变。
- [x] **Step 6**: 可选运行 `cargo run -- vuepress start --port 8080`；预期命令快速返回 JSON。若服务已运行或端口占用，改用 `cargo run -- vuepress status` 检查，不要强行杀用户进程。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `diff -u assets/prompts/vuepress.md .hotpot/prompts/vuepress.md` | 两份 VuePress prompt 内容保持一致。 |
| `cargo test` | 测试通过；现有 warning 不应新增为错误。 |
| `cargo run -- vuepress start --port 8080` | 如果执行，快速返回单行 JSON，且按修正后的 prompt 应拼出 `<url>/<HOTPOT_USERNAME>/2025-03-02-mock-task` 这种完整 stem URL。 |

### Risks and Watchouts

::: warning
- `.hotpot/prompts/vuepress.md` 是当前项目安装资产，`assets/prompts/vuepress.md` 是源资产；两者必须同步，否则当前行为或未来安装会继续错误。
- 不要把 `<task-page>` 写成带 `.md` 后缀的 URL 段；用户明确要求“不带后缀的完整任务文件名称”。
- 不要继续使用“between the date prefix and `.md` extension”这类描述，它会重新引导 AI 生成 `mock-task`。
- 如果需要改 `src/assets/vuepress_hub.rs`，必须遵守 `AGENTS.md`：新写或修改的 Rust 函数、文件等应具备中英双语 doc comments，输出文本保持 English。
- 若执行流或 VuePress hub 资产行为发生变化，必须同步更新 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md`。
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
