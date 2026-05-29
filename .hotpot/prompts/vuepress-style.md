## VuePress Task File Writing Conventions (Template-Enforced)

This file is loaded by `/hotpot:new` when VuePress is enabled and is
**layered on top of** standard markdown — MUST rules must apply, SHOULD
are encouraged, the rest is reference. Standard GFM (headings,
paragraphs, lists, task lists `- [ ] / - [x]`, tables, code fences,
links, images, blockquotes) all work normally.

## MUST Rules

The six rules below are non-negotiable. They are how the AI-authored
task file lands in VuePress with usable visual hierarchy, semantic
callouts, and scannable structure. Rule 5 is the defensive escape
hatch against the markdown-hint container parser bug that has
historically caused duplicate H2/H3 anchors to leak from example
code blocks; Rule 6 is the defensive escape hatch against the
backslash-backtick pseudo-escape that leaks `<word>`-shaped tokens
out of inline code into raw markdown and crashes the Vue SFC
compiler.

### MUST 1 — Insert an Overview container between H1 and `## Task`

Right after the H1 title and before `## Task`, emit a single
`::: info Overview` container with a 1-row, 4-column status table.

````markdown
# <Task Title>

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | <true|false> | <N> | <low|medium|high> |
:::

## Task
````

Field semantics:

- `Status` — always `In Progress` for a freshly created task (Done /
  Cancelled are written later by `hotpot-execute` / `finish-work`).
- `TDD` — the literal `true` or `false` from `## Plan > ### Mode > - tdd:`.
- `Tasks` — count of `#### Task N` blocks under `### Implementation Tasks`.
- `Risk` — AI self-assessment, one of `low` / `medium` / `high`.

### MUST 2 — Wrap `### Risks and Watchouts` body in a `::: warning` container

The H3 heading **stays** (it is a structural / ToC anchor); only the
body content goes inside the warning container.

````markdown
### Risks and Watchouts

::: warning
- risk 1
- risk 2
:::
````

### MUST 3 — Wrap `### Non-Goals` body in a `::: details` collapsible

Non-Goals lists are typically 5–10 entries on long tasks; collapsing
them reduces visual clutter without losing information.

````markdown
### Non-Goals

::: details Non-Goals
- not doing X
- not doing Y
:::
````

### MUST 4 — `### File Map` and every per-task `**Files:**` block must be a 3-column table

`Action` is one of the enum values `Modify` / `Create` / `Delete` / `Test`.

````markdown
### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/vuepress.rs` | Modify | Add detach logic |
| `assets/foo.scss` | Create | New style enhancement layer |
| `tests/bar.rs` | Test | Verify detach behavior |

#### Task 1: ...

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/vuepress.rs` | Modify | Fix start() detach |
````

### MUST 5 — Never embed `:::` inside a fenced code block within a `:::` container

VuePress 2's markdown-hint container parser DOES NOT respect fenced
code block boundaries when scanning for its own closing `:::`. If a
`::: container` body contains a fenced code block whose body in turn
contains `:::`, the inner `:::` is greedily matched as the outer
container's close — everything that should still be inside the
fenced block leaks out as real markdown. Symptom: H2/H3 headings
from inside example code blocks render as REAL section headings in
VuePress, polluting the ToC with duplicate "Task" / "Risks" / etc.
entries.

````markdown
DO NOT WRITE — outer ":::" wraps inner ":::" through a fence:

::: details Demo
```markdown
::: warning
- example
:::
```
:::
````

````markdown
SAFE — keep the fenced example at top level, no outer ":::" wrapper:

```markdown
::: warning
- example
:::
```
````

In other words: a `:::` container body may contain a fenced code
block ONLY IF that fenced block does not itself contain `:::`. To
show a `:::` example, host the fenced block at the top level of the
document, not inside another `:::` container.

### MUST 6 — Never write `\`` inside an inline code span; use double-backtick wrapping for embedded backticks

Markdown does NOT treat `\`` (backslash + backtick) as an escaped
backtick. A single-backtick inline span closes at the very next
single backtick in the source, regardless of any preceding
backslash. The sequence `` `outer \`inner\` more` `` therefore
parses as **three fragments**:

`````text
Source:  `outer \`inner\` more`

Parses as:
  bt-1 … bt-2  →  inline code, body = "outer \"
  bt-2 … bt-3  →  PLAIN TEXT       = "inner\"   ← leaks out of code formatting
  bt-3 … bt-4  →  inline code, body = " more"
`````

The middle fragment escapes code formatting entirely. If it
contains a `<word>`-shaped token (e.g. `<name>.md`,
`<workflowName>`, `<TaskTitle>`), VuePress runs the rendered HTML
through Vue's template compiler — an unregistered `<word>` is
parsed as an unknown component with no matching `</word>` close,
the SFC compile fails, and the dev server returns its error
overlay instead of the task content.

To embed literal backticks inside an inline code span, **wrap the
span in double-backticks** (markdown standard) and write the
embedded single backticks as themselves. Single backticks inside a
double-backtick span are literal characters.

`````markdown
DO NOT WRITE — backslash-backtick leaks the middle fragment as raw text:

`YOUR FIRST TOOL CALL MUST BE \`Read("<path>")\` — DO NOTHING ELSE FIRST.`
`When the workflow references \`@.hotpot/prompts/<name>.md\`, substitute it.`
`````

`````markdown
SAFE — double-backtick outer wrapper, single backticks inside are literal:

``YOUR FIRST TOOL CALL MUST BE `Read("<path>")` — DO NOTHING ELSE FIRST.``
``When the workflow references `@.hotpot/prompts/<name>.md`, substitute it.``
`````

Alternative: split the sentence into multiple single-backtick spans
joined by plain text — e.g. write `Read("<path>")` as its own
inline code segment with the surrounding instruction text outside
the code. This is verbose but mechanically safe.

The bug is invisible in plain-markdown rendering (GitHub, most
editors) because they do not run a Vue compile pass — they simply
render the leaked middle fragment as ordinary text. VuePress is
where it becomes a hard error, so a task file that "looks fine in
the editor" can still crash the dev server. When in doubt, audit
inline code spans for `\`` and rewrite them with double-backticks.

## SHOULD Recommendations

The seven recommendations below (A–G) are strongly encouraged but may
be omitted on shorter / denser task files where extra containers
would hurt readability. They cover visual containerization (A, F, G),
data structuring (B, D), prose formatting (C), and section
delimiting (E).

### SHOULD A — Use semantic containers for Summary and Approved Design

Wrap the `### Summary` body in `::: info` and the `### Approved Design`
body in `::: tip`. Headings stay outside the container.

````markdown
### Summary

::: info
<one-paragraph summary>
:::

### Approved Design

::: tip
<design narrative + sub-sections>
:::
````

### SHOULD B — Use 2-column `| Command | Expected |` tables for validation steps

When a Task's validation is a sequence of commands with deterministic
expected outputs, prefer a 2-column table over prose. Steps containing
long explanations may stay as `- [ ]` checkboxes.

````markdown
| Command | Expected |
| ------- | -------- |
| `cargo build` | passes, no new warnings |
| `hotpot vuepress start --port 8080` | single-line JSON, returns immediately |
````

### SHOULD C — Prefer markdown ordered/unordered lists over inline `(1)(2)(3)`

When enumerating items in prose, break them into a real markdown
list. Inline parenthesized numbers `(1) ... (2) ... (3) ...` read
as a wall of text in VuePress; an actual list is scannable.

````markdown
BAD: 主要四个症状：(1) 视觉层级混乱；(2) 缺速览；(3) 不用容器；(4) 表格散乱。

GOOD:
主要四个症状：

1. 视觉层级混乱
2. 缺速览
3. 不用容器
4. 表格散乱
````

### SHOULD D — Bold the `Step N` marker in each implementation checkbox

Implementation Task steps are scanned visually during execution.
Bolding the step marker makes the sequence count immediately
readable from across the page.

````markdown
- [ ] **Step 1**: Inspect `src/foo.rs` lines 42-58.
- [ ] **Step 2**: Run `cargo build`; expect no new warnings.
````

### SHOULD E — Add `---` (hr) before each top-level `##` mandatory section

The three Hotpot mandatory H2s (`## Task` / `## Plan` /
`## Execution Instructions`) benefit from an explicit horizontal
rule above them. The CSS in `.vuepress/styles/index.scss` already
styles H2s with a left accent bar + bottom border, but a separator
strengthens visual chunking between major sections.

````markdown
:::

---

## Plan

### Mode
````

### SHOULD F — Use `:::` containers instead of raw `>` blockquotes

VuePress 2's default theme styles `::: tip / info / warning / details`
containers much more prominently than plain blockquotes. Reserve `>`
for actual literary quotations; for user requests, design callouts,
or design notes, prefer a semantic container.

````markdown
BAD: > 用户原话：帮我做 X

GOOD:
::: info 用户原话
帮我做 X
:::
````

### SHOULD G — Wrap every `#### Task N` body in a `::: info Task body` container

For every `#### Task N` block under `### Implementation Tasks`,
keep the H4 heading OUTSIDE and wrap the entire body (the `**Files:**`
table + the `**Steps:**` checkbox list) inside a single
`::: info Task body` container. This turns each Task into a
visually self-contained card without sacrificing any of the
machine-readable anchors.

````markdown
#### Task 1: <Small Executable Unit>

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/foo.rs` | Modify | ... |

**Steps:**

- [ ] **Step 1**: ...
- [ ] **Step 2**: ...

:::
````

CRITICAL — the H4 `#### Task N` MUST stay OUTSIDE the container.
Putting the Task name only inside the container as `task: ...` plain
text BREAKS:

- `hotpot-execute` / `hotpot-review` subagent parsing of per-task
  boundaries (they grep `#### Task N` H4).
- `hotpot-review` TDD conformance check ("for every `#### Task N`
  listed in the task file's `## Plan > ### Implementation Tasks`").
- `- [ ]` checkbox progress tracking — if you also remove the
  checkboxes, `execution` agent cannot mark progress.

Steps and Files MUST remain real markdown (checkboxes + table) inside
the container; the container is a pure visual wrapper, not a
replacement for the structure.

INTERACTION WITH MUST 5: a Task step description that itself shows
example markdown containing `:::` (e.g. a teaching task explaining
container syntax) will trigger the markdown-hint parser bug and
cause the outer `::: info Task body` to close early. In that case,
either rewrite the step to not embed `:::` in its example, or skip
the `::: info Task body` wrapper for that specific Task.

## Quick Reference: VuePress markdown extras

Optional extensions the default theme supports — use when helpful.

### Frontmatter

Only `title` is required. `category` / `tag` group entries on index
pages. Do NOT use `sidebar` / `prev` / `next` / `permalink` (conflicts
with auto sidebar generation).

````markdown
---
title: Task title
description: One-line description
date: 2026-05-17
category: [Task]
tag: [<keywords>]
---
````

### Custom containers (full list)

````markdown
::: tip Tip
:::

::: warning Caution
:::

::: danger Danger
:::

::: info Info
:::

::: details Click to expand
:::
````

Full markdown is supported inside containers.

### Code blocks with line highlighting and titles

`````markdown
```rust title="src/main.rs" {2,4-6}
fn main() {
    println!("highlighted");
    let x = 1;
    let y = 2;
    let z = 3;
    println!("{}", x + y + z);
}
```
`````

### Links

- In-project links use relative paths with `.md` suffix:
  `[spec](./vuepress-style.md)` — VuePress rewrites to `.html` at build.
- External links are normal: `[label](https://...)`.
- Do NOT use `[[wiki-style]]` internal links.

### Tables

Standard GFM tables with alignment markers:

````markdown
| Field | Type   | Description |
| :---- | :----: | ----------: |
| id    | string | Task ID     |
| date  | string | YYYY-MM-DD  |
````

## Disabled Features

The following features must NOT be used (either the default theme does
not render them, or they cannot be reliably emitted by an AI):

- Vue components: `<Badge>` / `<RouteLink>` / `<CodeGroup>` /
  `<CodeTabs>` / `<Tabs>` etc. They depend on theme/plugin runtime and
  fall back to raw tags in plain-markdown rendering.
- `[[toc]]` inline table-of-contents — default-theme support is
  inconsistent.
- `<<< @/path/to/file` code import — requires extra
  markdown-power plugin config not enabled in this template.
- Mermaid diagrams (` ```mermaid ` blocks) / math expressions
  (`$...$` etc.) — depend on plugins not enabled in this template.
- Color / style raw HTML such as `<span style="color:red">` — same
  reason.

## Hotpot Mandatory Anchors

Hotpot still requires the task `.md` to contain `## Task` / `## Plan` /
`## Execution Instructions` and the other structural sections.
**These English anchors MUST NOT be renamed or translated** (not even
when the project `output-language` is set to a non-English value).
VuePress will render them as ordinary `<h2>` headings — no special
handling needed — but their presence is the hard contract
`/hotpot:execute` relies on to parse the task file.

## General principles

- Standard GFM markdown (including task lists `- [ ]`) is **safe by
  default**. Write the way you would write plain markdown.
- The 6 MUST rules above must be followed; the 7 SHOULD recommendations
  may be omitted when the document is already dense (but try to apply
  most of them — they materially improve scannability for both AI and
  human reviewers).
- Quick-reference extensions are "use them if they help" — they are
  **not mandatory**. Simple tasks are fine with pure markdown.
- The disabled list is "using them will break things" — those genuinely
  must be avoided.
- When unsure whether a non-standard extension is supported by
  VuePress, fall back to the most basic markdown.
- The primary goal of the task file is correct parsing by
  `/hotpot:execute` (structural English anchors intact); a pretty
  render in VuePress is secondary.
