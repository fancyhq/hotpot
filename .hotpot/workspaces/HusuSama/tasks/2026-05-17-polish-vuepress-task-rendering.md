---
title: 优化任务文件在 VuePress 中的渲染效果（skill 模板强化 + SCSS 增强）
description: 改写 vuepress-style.md 为强制模板版，新增 .vuepress/styles/index.scss 注入主题增强样式
date: 2026-05-17
category: [Task]
tag: [vuepress, prompt-template, scss, theme, ai-output-quality]
---

# 优化任务文件在 VuePress 中的渲染效果

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | false | 3 | medium |
:::

---

## Task

### Summary

AI 当前按 `assets/prompts/vuepress-style.md` 写出的任务文件在 VuePress 中渲染效果差，主要四个症状：

1. `h2 / h3 / h4 / h5` 视觉层级在默认主题里区分不够。
2. 页面顶部缺速览块（Status / TDD / 任务数 / 风险），用户进入页面要往下翻很久才知道任务是啥。
3. AI 几乎不主动用 VuePress 的 `::: tip / warning / details` 容器，Risks / Non-Goals 这类语义鲜明的段落和正文长得一样。
4. `**Files:**` 用粗体加缩进 list、命令验证步骤用纯散文，结构性信息散落在散文里。

本任务从两侧夹击：在 **skill 侧**把 `vuepress-style.md` 从「列出可用功能」改写成「强制模板 + 推荐项」，强约束 AI 写出语义化结构；在 **样式侧**新增 `assets/vuepress/docs/.vuepress/styles/index.scss`（约 150 行）通过默认主题的自动 SCSS 加载机制注入 heading 层级强化、container 配色、表格美化、task-list 进度可视化。

### User Request

::: info 用户原话
帮我优化一下 markdown 的写法，目前 AI 编写的任务文件在 vuepress 中显示效果很差，建议你设计一下，可以是 skill 改进，也可以是模板文件增加样式
:::

用户在 AskUserQuestion 中明确：四个痛点（视觉层级、顶部速览、callout 不用、代码块/表格散乱）全选；范围选「纯 skill 改 + 1 个 SCSS 文件」；TDD 选 Skip。

### Approved Design

#### 改动 1：`assets/prompts/vuepress-style.md` 改写为「强制模板 + 推荐项」版

::: warning 改动概述

文件结构调整为 4 个区块：

1. **顶部 4 条 MUST 规则**（醒目，AI 一进文件就看到）
2. **2 条 SHOULD 推荐项**
3. **可选扩展**（保留现有 section 1-5 的部分内容：YAML frontmatter、container 语法、代码块高亮、链接、表格 —— 但缩减为「快速参考」）
4. **禁用列表**（不变，section 6 原文）+ **结构锚点保留**（不变，section 7 原文）

**4 条 MUST 规则**（具体示例代码块在本 warning 容器之外，避免 `:::` 嵌套触发 MUST 5 bug）：

- **MUST ①** — H1 与 `## Task` 之间必须放 Overview 容器（4 列状态表 Status / TDD / Tasks / Risk）。`Status` 永远填 `In Progress`（任务新创建时只可能是这个状态，Done/Cancelled 阶段会被 hotpot-execute / finish-work 流程改）；`TDD` 跟 `## Plan > ### Mode` 的 `tdd:` 取值；`Tasks` 是 `### Implementation Tasks` 下的 `#### Task N` 数量；`Risk` 是 AI 自评 low/medium/high。
- **MUST ②** — `### Risks and Watchouts` 内容包 `::: warning` 容器。外层 H3 标题**保留**（机器锚点 + ToC），warning 容器只包内容主体。
- **MUST ③** — `### Non-Goals` 内容包 `::: details` 折叠。理由：长任务文件 Non-Goals 段落往往 5-10 条，默认收起避免视觉拥挤。
- **MUST ④** — `### File Map` 与每个 `#### Task N` 下的 `**Files:**` 必须用 3 列表格。`Action` 限定枚举值 `Modify` / `Create` / `Delete` / `Test`。

**2 条 SHOULD**：

- **SHOULD A**：`### Summary` 段落用 `::: info` 容器；`### Approved Design` 段落用 `::: tip` 容器。推荐不强制——文档过满时 AI 可以省略，但出现在长任务里强烈鼓励。
- **SHOULD B**：每个 Task 的「运行 X 预期 Y」验证步骤建议用 2 列表格 `| Command | Expected |` 替代散文。步骤含长解释时允许保持 `- [ ]` checkbox 形态。

**保留不动**：

- 原 section 6（禁用列表：Vue 组件 / `[[toc]]` / `<<< @/path` / Mermaid / 数学 / 内联 color HTML）——这些必须继续禁，否则破坏默认主题渲染。
- 原 section 7（Hotpot 强制英文锚点：`## Task` / `## Plan` / `## Execution Instructions` / `### Mode` / 各 `### Summary` 等子级）——这是与 hotpot-execute 流的解析契约，**绝不可翻译或重命名**。

:::

**MUST ① 示例** — Overview 容器：

````markdown
# <Task Title>

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | <true|false> | <N> | <low|medium|high> |
:::

## Task
````

**MUST ② 示例** — Risks warning：

````markdown
### Risks and Watchouts

::: warning
- risk 1
- risk 2
:::
````

**MUST ③ 示例** — Non-Goals details：

````markdown
### Non-Goals

::: details Non-Goals
- 不做 X
- 不做 Y
:::
````

**MUST ④ 示例** — File Map 与 `**Files:**` 表格：

````markdown
### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/vuepress.rs` | Modify | 加 detach |
| `assets/foo.scss` | Create | 新增样式增强层 |
| `tests/bar.rs` | Test | 验证 detach 行为 |

#### Task 1: ...

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/vuepress.rs` | Modify | 修 start() detach |
````

#### 改动 2：新增 `assets/vuepress/docs/.vuepress/styles/index.scss`

::: warning 改动概述

VuePress 默认主题约定：`docs/.vuepress/styles/index.scss` 会被主题自动 `@import`（无需 config.js 注册）。`package.json` 里 `sass-embedded@^1.86.0` 已经在 devDependencies——原作者预留了 SCSS 通道。

新增文件结构（6 个 section，bilingual 注释，纯 SCSS 无外部依赖）。SCSS 内容轮廓见下方折叠块。

:::

::: details SCSS 内容轮廓（不是最终代码，执行 agent 落地时按此结构展开 + 实测调参）

````scss
/*!
 * Hotpot VuePress task page enhancements.
 * Loaded automatically by @vuepress/theme-default through the
 * `styles/index.scss` convention. Pure CSS variables —
 * dark-mode adapts automatically via --vp-c-* tokens.
 *
 * Hotpot 任务文件渲染增强样式。默认主题约定 styles/index.scss
 * 自动 @import；全部走 --vp-c-* CSS 变量，dark mode 自动跟随。
 */

/* ── 1. Page chrome | 页面整体 ─────────────────────────────── */
.theme-default-content {
  max-width: 980px;
  line-height: 1.72;
}

/* ── 2. Heading hierarchy | 标题层级强化 ──────────────────── */
.theme-default-content h2 {
  font-size: 1.55rem;
  margin-top: 2.2rem;
  padding: 0.35rem 0 0.45rem 0.8rem;
  border-bottom: 1px solid var(--vp-c-divider);
  border-left: 4px solid var(--vp-c-accent);
  background: linear-gradient(
    to right,
    var(--vp-c-bg-alt) 0%,
    transparent 30%
  );
  border-radius: 3px 0 0 0;
}

.theme-default-content h3 {
  font-size: 1.2rem;
  padding-left: 0.65rem;
  border-left: 3px solid var(--vp-c-accent-soft, var(--vp-c-divider));
  margin-top: 1.8rem;
}

.theme-default-content h4 {
  display: inline-block;
  font-size: 1rem;
  background: var(--vp-c-bg-alt);
  padding: 0.25rem 0.7rem;
  border-radius: 5px;
  margin-top: 1.4rem;
}

/* TDD Red/Green/Refactor 五级标题：CSS 圆点 + 文字着色 */
.theme-default-content h5[id="red"]    { color: #d04848; }
.theme-default-content h5[id="green"]  { color: #3aa55e; }
.theme-default-content h5[id="refactor"] { color: #4285f4; }
.theme-default-content h5[id="red"]::before,
.theme-default-content h5[id="green"]::before,
.theme-default-content h5[id="refactor"]::before {
  content: "";
  display: inline-block;
  width: 0.7em;
  height: 0.7em;
  border-radius: 50%;
  margin-right: 0.55em;
  vertical-align: 0;
}
.theme-default-content h5[id="red"]::before    { background: #d04848; }
.theme-default-content h5[id="green"]::before  { background: #3aa55e; }
.theme-default-content h5[id="refactor"]::before { background: #4285f4; }

/* ── 3. Containers | 自定义容器配色 ──────────────────────── */
.theme-default-content .custom-container {
  border-left-width: 4px;
  border-radius: 4px;
  margin: 1rem 0;
}

/* Overview 顶部 info 容器内的表格收紧 cell padding */
.theme-default-content .custom-container.info table {
  margin: 0.4rem 0 0;
  font-size: 0.92rem;
  th, td { padding: 0.4rem 0.7rem; }
}

/* ── 4. Tables | 表格美化 ───────────────────────────────── */
.theme-default-content table {
  display: table;
  width: 100%;
  border-collapse: collapse;
  margin: 1rem 0;
}
.theme-default-content table th {
  background: var(--vp-c-bg-alt);
  font-weight: 600;
  text-align: left;
}
.theme-default-content table tr:nth-child(even) td {
  background: var(--vp-c-bg-alt);
}
.theme-default-content table code {
  font-size: 0.88em;
  padding: 0.12rem 0.4rem;
}

/* ── 5. Task list | 任务 checkbox 视觉 ───────────────────── */
.theme-default-content .task-list-item {
  list-style: none;
  padding-left: 0;
}
.theme-default-content .task-list-item input[type="checkbox"] {
  margin-right: 0.5rem;
  transform: translateY(1px);
  cursor: default;
}
/* markdown-it-task-lists 给已勾选项标 .task-list-item-checked */
.theme-default-content .task-list-item-checked {
  color: var(--vp-c-text-mute);
  text-decoration: line-through;
}

/* ── 6. Anchor accents | 三个 Hotpot 固定锚点的微图标 ────── */
.theme-default-content h2[id="task"]::before { content: "📋 "; }
.theme-default-content h2[id="plan"]::before { content: "🗺️ "; }
.theme-default-content h2[id="execution-instructions"]::before { content: "🚀 "; }
````

**注意**：上述 emoji 是「示例 emoji」——执行 agent 落地时如果用户后续表达不想要 emoji，改成纯 CSS 几何形状（如 `::before { content: ""; width: 6px; height: 6px; ... background: var(--vp-c-accent); }`）。本任务**默认带 emoji**——三个锚点 emoji 是「装饰性 hint」不是契约，可单文件移除。

:::

#### 改动 3：登记 + 文档同步

::: warning 改动概述

`src/assets/vuepress_hub.rs::VUEPRESS_HUB_ASSETS` 增加一条 `Asset::owned` 条目；同时更新该常量上方 doc comment 的「四个 `.vuepress/` 文件紧密耦合」表述——SCSS 是独立装饰层，与 config.js / sidebar.js / TaskIndex.vue 互不依赖。

`docs/ARCH.md` 与 `docs/ARCH.zh_CN.md` 在 **VuePress 集成 > 资产分级** 表格的 hub 行里把「四份 `.vuepress/` 文件」改成「五份 `.vuepress/` 文件」并补一行说明 `styles/index.scss` 是装饰层。

:::

### Alternatives Considered

- **方案 A：只改 skill，不动 SCSS**。零样式工作量，但 VuePress 默认主题对 h2/h3/h4 字号区分本来就弱，光改写作约束治标不治本。**未采纳**。
- **方案 B：只加 SCSS，不动 skill**。AI 不主动用 container/表格的话再好的 CSS 也只能润色 plain markdown，Risks/Non-Goals/File Map 这些语义信息仍然糊在散文里。**未采纳**。
- **方案 C：双管齐下**（采纳）—— skill 强约束 4 条 + 推荐 2 条；SCSS 6 个 section 共 ~150 行。内容质量与渲染质量同步提升。
- **方案 D：方案 C + 额外写一份「黄金范例任务文件」**。范围大（4-5 个文件），但本任务文件本身就**已经按新模板写**（Overview 容器、Risks warning、Non-Goals details、File Map 表格），相当于内嵌示范，不必单独再写一份范例文件。**未采纳**。

### Requirements

- `assets/prompts/vuepress-style.md` 改写后必须包含 4 条 MUST 规则（Overview / Risks warning / Non-Goals details / File Map 表格），措辞清晰，每条带「正确示例」markdown 代码片段。
- 保留原 section 6（禁用清单）与 section 7（英文锚点约束）—— 这两段是契约，不能删。
- 新增 `assets/vuepress/docs/.vuepress/styles/index.scss`，文件存在、UTF-8 编码、首行起一个 `/*! ... */` 双语注释块声明用途。
- `src/assets/vuepress_hub.rs::VUEPRESS_HUB_ASSETS` 必须新增 SCSS 对应的 `Asset::owned(".hotpot-hub/docs/.vuepress/styles/index.scss", include_str!("../../assets/vuepress/docs/.vuepress/styles/index.scss"))` 条目。
- `cargo build` 通过，无新增 warning。
- `hotpot vuepress install --force` 跑通，落地后 `.hotpot-hub/docs/.vuepress/styles/index.scss` 存在且内容与 `assets/` 源文件一致。
- 浏览器打开 `http://localhost:8080/HusuSama/polish-vuepress-task-rendering`（即本任务）肉眼验证：Overview 块、Risks warning 块、Non-Goals details 块、File Map 表格、heading 层级强化都生效。
- `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md` 资产分级表 hub 行同步更新「五份 `.vuepress/` 文件」。

### Non-Goals

::: details Non-Goals

- **不改** `assets/vuepress/docs/.vuepress/config.js`、`client.js`、`sidebar.js`、`components/TaskIndex.vue`——首页索引已美观，且改它们会破坏 `__HOTPOT_TASK_INDEX__` 注入链。
- **不引入新 markdown-it 插件**（不加 mermaid、数学、`<<<` import、`[[toc]]`）——禁用清单要继续保留。
- **不改** `assets/prompts/hotpot-new.md` 主流程——VuePress style 是它在 `$HOTPOT_VUEPRESS_ENABLED == "true"` 时 Read 的下游文件，主流程契约不动。
- **不改** `### Mode > - tdd: true|false` 解析锚点 / `## Task` / `## Plan` / `## Execution Instructions` 这些机器锚点；只在视觉层加装饰。
- **不写**「黄金范例任务文件」（本任务自身就是范例，不需要单独存一份）。
- **不动** TaskIndex.vue 已经存在的 scoped CSS。
- **不改** `Cargo.toml` 任何依赖（SCSS 通过 `include_str!` 嵌入，运行期不需 sass 编译；编译期由 VuePress 端 `sass-embedded` 处理，依赖已在 `package.json`）。
- **不动** `hotpot vuepress` CLI 子命令任何逻辑（install / uninstall / start / stop / status 全部不变）。
- **不强制** AI 在所有 `### Summary` 都包 `::: info`（SHOULD 而非 MUST）。

:::

### Project Context

::: tip 项目上下文

- **VuePress hub 资产源路径**：`D:/RustProjects/hotpot/assets/vuepress/` ——逐字节对应到 `<project>/.hotpot-hub/` 安装目标，由 `src/assets/vuepress_hub.rs::VUEPRESS_HUB_ASSETS` 数组登记。
- **SCSS 自动加载机制**：VuePress 2 + `@vuepress/theme-default` 默认 `@import` `docs/.vuepress/styles/index.scss`（若存在）。无需 config.js 任何变更。`package.json` devDependencies 已含 `sass-embedded@^1.86.0`，sass 编译器在 `pnpm install` 时已部署。
- **AssetEngine 行为**：`Asset::owned(target, content)` 表示「Hotpot 私有，覆盖式安装」；`hotpot vuepress install --force` 与 `hotpot update` 都会重新落盘所有 owned 资产。新增条目即可自动随 install/update 分发。
- **运行时验证设备**：本地 Windows，VuePress dev server 通过 `cargo run -- vuepress start --port 8080` 启动；浏览器路径模式 `http://localhost:8080/<HOTPOT_USERNAME>/<task-slug>`（slug 是 filename 去日期与 `.md` 后缀部分）。
- **CSS 变量参考**：默认主题暴露 `--vp-c-accent`、`--vp-c-bg-alt`、`--vp-c-divider`、`--vp-c-text`、`--vp-c-text-mute`、`--vp-c-bg`、`--vp-c-bg-soft` 等。全部走变量，dark mode 自动适配。
- **markdown-it-task-lists 配置**：`config.js` 已 `md.use(markdownItTaskLists, { enabled: true, label: true })`；已勾选项自动带 `.task-list-item-checked` class。
- **同名旧 SCSS 文件**：`.hotpot-hub/docs/.vuepress/styles/` 目录现在**不存在**——新建即可，无需删除旧文件。
- **VuePress 文档**：默认主题样式覆盖约定参见 `https://ecosystem.vuejs.press/themes/default/styles.html`（VuePress 2 RC）。

:::

---

## Plan

### Mode

- tdd: false   <!-- machine-readable; the execute flow parses this line. -->

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/vuepress-style.md` | Modify | 改写为「强制模板 + 推荐项」版，加 4 条 MUST + 2 条 SHOULD |
| `assets/vuepress/docs/.vuepress/styles/index.scss` | Create | 新增 ~150 行主题增强 SCSS（heading 层级、容器、表格、task-list） |
| `src/assets/vuepress_hub.rs` | Modify | `VUEPRESS_HUB_ASSETS` 加 SCSS 条目；更新顶部 doc comment 说明从 4 个 .vuepress 文件变 5 个 |
| `docs/ARCH.md` | Modify | 资产分级表 hub 行从「四份」改「五份」+ 一行 SCSS 装饰层说明 |
| `docs/ARCH.zh_CN.md` | Modify | 同上中文版同步 |

### Implementation Tasks

#### Task 1：改写 `assets/prompts/vuepress-style.md`

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/vuepress-style.md` | Modify | 重写为强制模板版 |

**Steps:**

- [x] **Step 1**：用 Read 读完 `assets/prompts/vuepress-style.md` 现有内容（约 149 行），理清原有 7 个 section 的结构。
- [x] **Step 2**：保留原文件**最顶部的导言 + section 6（禁用列表）+ section 7（结构锚点）**，**重写**中间内容为以下 4 个新 section：
  1. `## MUST Rules`（4 条强制规则，每条带正确示例代码片段）
  2. `## SHOULD Recommendations`（2 条推荐项）
  3. `## Quick Reference: VuePress markdown extras`（缩减版的原 section 1-5：frontmatter、container、code block、links、tables，每条只保留 1 个示例）
  4. （原 section 6/7 保留原样作为 `## Disabled Features` 与 `## Hotpot Mandatory Anchors`）
- [x] **Step 3**：4 条 MUST 规则照本任务文件 `### Approved Design > 改动 1` 的 4 个 details 块的示例 markdown 写入，**完整保留**示例代码块（用四个反引号外层 fence 包三个反引号内层）。
- [x] **Step 4**：保留原文件结尾的「General principles」段落（关于"标准 markdown 优先"的 fallback 原则），但加一句新原则：「4 条 MUST 规则必须遵守；2 条 SHOULD 可在文档过满时省略」。
- [x] **Step 5**：导言段补一句明确：「This file is loaded by `/hotpot:new` when VuePress is enabled and **is layered on top of** standard markdown — MUST rules must apply, SHOULD are encouraged, the rest is reference.」
- [x] **Step 6**：Read 写完的文件做一次自查，确认：(a) 4 条 MUST 都包含示例代码；(b) `## Hotpot Mandatory Anchors` section 完整保留；(c) `## Disabled Features` section 完整保留；(d) 文件总行数不超过 250 行（保持简洁可读）。

:::

#### Task 2：新增 `assets/vuepress/docs/.vuepress/styles/index.scss`

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/vuepress/docs/.vuepress/styles/index.scss` | Create | 主题增强样式 |

**Steps:**

- [x] **Step 1**：用 Glob 确认 `assets/vuepress/docs/.vuepress/styles/` 目录是否存在；不存在直接通过 Write 创建文件（Write 会自动建父目录）。
- [x] **Step 2**：写入完整 SCSS 文件，结构遵循本任务文件 `### Approved Design > 改动 2 > SCSS 内容轮廓` 里的 6 个 section 顺序：(1) 页面整体；(2) 标题层级；(3) 容器；(4) 表格；(5) 任务 checkbox；(6) 三个 Hotpot 固定锚点的微装饰。每个 section 顶部一个 bilingual 注释块。
- [x] **Step 3**：文件首行起一个 `/*! ... */` 注释块说明用途、声明依赖（CSS 变量自动 dark mode 适配）、说明可单文件删除而不影响 VuePress 启动。
- [x] **Step 4**：**特别处理三个 anchor 微装饰（SCSS 第 6 块）的 emoji**——采用方案 A：用 emoji（`📋` `🗺️` `🚀`）。若执行 agent 觉得 emoji 不合适当下 user-content 规范，**作为 blocker 停下来询问**，不要擅自换成几何形状或删除。
- [x] **Step 5**：`h5[id="red|green|refactor"]` 的圆点用纯 CSS 几何形（不带 emoji）—— Red/Green/Refactor 是 TDD 流程的机器化标识，emoji 反而不专业，圆点纯 CSS 更克制。
- [x] **Step 6**：所有颜色优先用 `--vp-c-*` 变量；只有 TDD 三色（红 `#d04848` / 绿 `#3aa55e` / 蓝 `#4285f4`）写死十六进制，确保深浅色模式下都能识别。
- [x] **Step 7**：行数控制：80-180 行；超过 180 行说明 section 拆得太碎，需要合并。
- [x] **Step 8**：Read 写完的 SCSS 文件做语法自查（虽然不进 cargo 编译，但要保证 sass-embedded 接收时不会 panic）：每条 rule 末尾分号、`}` 配对、注释 `/* */` 闭合。

:::

#### Task 3：登记到 asset catalog + 同步 ARCH 文档 + 手动验证

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/assets/vuepress_hub.rs` | Modify | 加 SCSS 资产条目 + 更新 doc comment |
| `docs/ARCH.md` | Modify | 资产分级表 hub 行同步 |
| `docs/ARCH.zh_CN.md` | Modify | 同上中文版 |

**Steps:**

- [x] **Step 1**：用 Read 重新读 `src/assets/vuepress_hub.rs`（约 75 行）确认现有 `VUEPRESS_HUB_ASSETS` 7 条目格式。
- [x] **Step 2**：在 `VUEPRESS_HUB_ASSETS` 数组**最后一条 `TaskIndex.vue` 之后**新增一条：

  ```rust
  Asset::owned(
      ".hotpot-hub/docs/.vuepress/styles/index.scss",
      include_str!("../../assets/vuepress/docs/.vuepress/styles/index.scss"),
  ),
  ```

- [x] **Step 3**：更新 `VUEPRESS_HUB_ASSETS` 上方的 doc comment（line 28-45）：原文说「The four `.vuepress/` files (`config.js` / `client.js` / `sidebar.js` / `components/TaskIndex.vue`) are tightly coupled」——改成「Five `.vuepress/` files: the four tightly-coupled runtime files plus the independent `styles/index.scss` decorative layer (loaded automatically by `@vuepress/theme-default`; can be deleted or edited freely without breaking the home-page TaskIndex injection chain)」。中文段同步改动。
- [x] **Step 4**：跑 `cargo build` —— **预期通过**，无与本次相关的新 warning。验证 `include_str!` 在编译期成功嵌入 SCSS 内容。
- [x] **Step 5**：跑 `cargo run -- vuepress install --force` —— **预期** stdout 输出 install 完成的 JSON / log；落地后用 Read 检查 `.hotpot-hub/docs/.vuepress/styles/index.scss` 存在且首行是 `/*!`。
- [x] **Step 6**：跑 `cargo run -- vuepress start --port 8080` —— **预期**单行 JSON `{"url":"http://localhost:8080","pid":<N>}` 立即返回（不阻塞）。
- [x] **Step 7**：**手动浏览器验证**——打开 `http://localhost:8080/HusuSama/polish-vuepress-task-rendering`（本任务页面），肉眼确认：
  - 顶部 Overview 容器（`::: info`）渲染为浅色背景块，内嵌 4 列表格。
  - `### Risks and Watchouts` 下方 `::: warning` 容器是琥珀色 / 橙色调，左侧粗 border。
  - `### Non-Goals` 下方 `::: details Non-Goals` 默认收起，点击展开。
  - 多张 File Map / **Files:** 表格 cell padding 舒适，隔行变色明显。
  - `## Task` / `## Plan` / `## Execution Instructions` 三个 H2 前面带 emoji 装饰（📋 / 🗺️ / 🚀），且 H2 整体有底 border + 左 4px 彩条 + 浅渐变背景。
  - `### Implementation Tasks` 等 H3 左侧 3px 细线 + 字号比 H4 大。
  - `#### Task 1 / 2 / 3` 这些 H4 渲染为内联 tag 风格（背景块 + 圆角）。

  如果上述任一项明显不对（如样式没生效、emoji 渲染成 `?`、表格仍是默认主题样子），**停下作为 blocker**报告，不要绕路改 SCSS。
- [x] **Step 8**：跑 `cargo run -- vuepress stop` 关掉 dev server，确认 stdout 打印 `Stopped vuepress (pid ..., was running on port 8080).`，`.hotpot-hub/vuepress.runtime.json` 被删。
- [x] **Step 9**：用 Read 打开 `docs/ARCH.md`，定位 **VuePress 集成 > 资产分级** 表格的 hub 行（搜「四份 `.vuepress/`」或 `"four"`+`.vuepress`），更新为：

  > `.hotpot-hub/` containing `package.json`, `pnpm-lock.yaml`, `docs/README.md` and the **five `.vuepress/` files** (`config.js`, `client.js`, `sidebar.js`, `styles/index.scss`, `components/TaskIndex.vue`). The first four runtime files are tightly coupled; `styles/index.scss` is an independent decorative layer (loaded automatically by `@vuepress/theme-default`, safe to edit or delete).

- [x] **Step 10**：用 Read 打开 `docs/ARCH.zh_CN.md`，定位对应中文段，做同等中文改动（「四份」→「五份」，并补一句 SCSS 装饰层说明）。
- [x] **Step 11**：跑 `cargo build` 再次确认 ARCH 文档改动不影响 Rust 编译（`include_str!` 链路完整）。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo build` | 通过，无新增 warning |
| `cargo run -- vuepress install --force` | 落地 SCSS 到 `.hotpot-hub/docs/.vuepress/styles/index.scss` |
| `cargo run -- vuepress start --port 8080` | 立即返回 stdout JSON，不卡死 |
| **手动**：浏览器打开本任务页面 | Overview / warning / details / 表格 / heading 层级 全部生效 |
| `cargo run -- vuepress stop` | 正常 kill、runtime.json 被删 |

### Risks and Watchouts

::: warning

- **VuePress 2 RC 版本对 `styles/index.scss` 的自动加载机制有版本依赖**：`@vuepress/theme-default@2.0.0-rc.88` 文档明确支持，但如果未来升级到稳定版 API 收紧，可能需要在 `config.js` 里显式 `import "./styles/index.scss"` 注册。本任务**不需要预先**做这件事——RC.88 上验证通过就行。
- **emoji 在某些字体下渲染异常**：Step 7 验证里若 `📋 🗺️ 🚀` 显示为豆腐块或问号，先确认浏览器/系统字体是否含 emoji 字体。**不在样式层做 fallback**——如果用户后续不喜欢可单独提需求改成纯 CSS 几何形。
- **`.theme-default-content` selector 选择面广**：覆盖范围包括首页（README.md 渲染处），所以 h2 强化样式也会作用到首页 `<h2>Tasks</h2>`。预期效果：首页 Tasks 标题也有左侧彩条 + 底 border，与 TaskIndex.vue 的 `.hotpot-title` 风格冲突。**应对**：TaskIndex.vue 的 `.hotpot-title` 已经是 scoped style，scoped 优先级高于全局 SCSS，应该不冲突；若发现冲突，把全局 SCSS 的 h2 rule 加 `.theme-default-content > h2` 限定 direct child，TaskIndex 内的 `<h2 class="hotpot-title">` 不在 direct child 位置即可豁免。Step 7 验证里**必须打开首页 `http://localhost:8080/`** 顺便确认 Tasks 卡片区不被波及。
- **`pnpm-lock.yaml` 不需要更新**：本次没加新 npm 依赖（sass-embedded 已经在 devDependencies）。如果跑 `pnpm install` 触发了 lock 变更，是 RC 版本漂移导致的副作用，与本任务无关——保持 lock 不动即可，遇到 conflict 用 `pnpm install --frozen-lockfile`。
- **首次访问 dev server 慢**：VuePress 首次 build 含 SCSS 编译，约 5-15 秒。Step 7 验证里给浏览器 30 秒预热时间，避免误判「样式没生效」。
- **AI 第二次写任务文件时的转折**：改 skill 后，AI 下一次跑 `/hotpot:new` 会按新 MUST 规则写。这次任务文件本身已经按新模板写——**确认本任务文件能在新 SCSS 下渲染好**，就等于同时验证了 skill 与样式的协同生效。
- **`docs/ARCH.md` 表格定位**：直接搜 `four \`.vuepress/\`` 字符串可能因引号转义匹配失败，改用搜 `four`+`vuepress` 的关联文本。

:::

---

## Execution Instructions

把本文件**完整内容**交给 execution sub-agent。execution 必须：

- 在改动前先 Read 本文件、Read `assets/prompts/vuepress-style.md`、Read `src/assets/vuepress_hub.rs`、Read `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md` 的 VuePress 集成段。
- 按 `## Plan > ### Implementation Tasks` 顺序执行 Task 1 → Task 2 → Task 3。Task 1 与 Task 2 互相独立可并行；Task 3 依赖前两个完成。
- 执行每一步后把对应 `- [ ]` checkbox 改成 `- [x]`，记录关键验证命令的 stdout 摘要。
- 保留 `### Non-Goals` 列表所有限制——尤其不要顺手改 `config.js` / `client.js` / `sidebar.js` / `TaskIndex.vue`、不引入新 markdown-it 插件、不改 hotpot-new.md 主流程。
- Task 3 的 Step 7 浏览器验证是最终硬判据；若任一肉眼检查项不通过（样式没生效、emoji 渲染异常、表格仍是默认主题），**停下作为 blocker** 报告，不要绕路改实现。
- Step 4 的 SCSS emoji 决策：若执行 agent 怀疑 user-content rules 限制 emoji 使用，**作为 blocker 停下问**，不要擅自删 emoji 或换成几何形。
- 全部自动化命令通过且 Step 7 浏览器验证通过之前，**不要**报 task 完成。
