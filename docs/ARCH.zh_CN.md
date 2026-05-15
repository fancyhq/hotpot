# Hotpot 架构速览

面向规划、实现、扩展 Hotpot 的 agent 的快速入口。在改动命令、子代理、提示词或 CLI 子命令前先读这一份。

## Hotpot 是什么

面向编码 agent（Claude Code、OpenCode、Codex、Pi）的跨平台任务编排器。一次完整的 Hotpot 生命周期：

1. 把用户的想法捕获为一份完整的任务文件。
2. 通过执行子代理实施任务。
3. 通过只读的 review 子代理审查结果（review 会读取项目沉淀下来的记忆）。
4. 循环执行 修复 → review，直到结果可接受（最多 2 轮）。
5. 用户确认后，把有价值的经验晋升进长期共享记忆。

所有命令都由用户**手动触发**。Hotpot 不会自动启动任何工作——它只在用户、文件系统与子代理之间做编排。

## 核心概念

- **任务文件**（`<workspace>/tasks/<YYYY-MM-DD>-<title>.md`）：自包含的 handoff 文档。必须含 `## Task`、`## Plan`、`## Execution Instructions`。`## Plan > ### Mode` 携带 `- tdd: true|false` 决定 TDD 与默认两种流程。`<title>` 是 `task create --title` 传入的 kebab-case slug；CLI 在拼文件名时会把 `title` 内残留的连续空白折成单个 `-` 作为兜底（仅防御性兜底，`/hotpot:new` 已要求 AI 直接产出 kebab-case，不做更激进的 slugify）。
- **任务台账**（`<workspace>/overview.jsonl`）：每个用户一份，强制不变式："同一时刻最多一条 `active=true && status=In Progress` 的行"。
- **Issue 候选**（`<workspace>/issue-candidates.jsonl`）：临时、按用户隔离的 review 记忆草稿，由 execute 缓冲并在 finish-work 中决策晋升或丢弃。
- **Issue 记忆**（`.hotpot/issues.jsonl`）：共享、长期。只能通过 `hotpot issues promote` 在用户确认后追加。
- **共享 prompt**（`.hotpot/prompts/`）：跨平台的 LLM 提示词资产，由 `hotpot init` / `hotpot update` 安装，不要手改。

## 目录结构

```
<project>/
├── .hotpot/
│   ├── config.toml                      # 项目配置（如 language）
│   ├── issues.jsonl                     # 共享、长期记忆
│   ├── prompts/                         # 已安装的提示词资产（勿改）
│   ├── brainstorm/<session>/            # 可视化伴侣临时产物
│   └── workspaces/<username>/
│       ├── overview.jsonl
│       ├── issue-candidates.jsonl
│       └── tasks/<YYYY-MM-DD>-<title>.md
├── .claude/  .opencode/  .codex/  .pi/   # 各平台配置目录
```

路径解析：`src/paths.rs`。用户名解析链（`src/context.rs::resolve_username`）：`HOTPOT_USERNAME` → `git config --local user.name` → `git config --global user.name` → `"default"`。

## 命令

四个手动入口。各平台分别用自己的机制承载，语义一致。

| 命令 | 作用 |
|---|---|
| `hotpot init` | 安装指定平台的资产与共享 prompt。幂等。`--platform {claude\|opencode\|codex\|pi\|all}`。 |
| `hotpot update` | 协作者 day-1 入口。自动探测已安装平台，刷新资产、bootstrap 当前用户 workspace、合并 hotpot 段到 `.gitignore`、跑健康自检。 |
| `/hotpot:new` | 头脑风暴 → 用户批准设计 → `hotpot task create [--switch\|--inactive]` → 写入 handoff 任务文件。new 阶段不改业务代码。 |
| `/hotpot:execute` | 取活动任务 → 调起执行子代理 → 收集 diff 与相关记忆 → 调起只读 review 子代理 → 修复循环（≤ 2 轮）→ 缓冲 issue 候选 → 让用户挑选保留范围 → 通过 `hotpot issues candidate add` 落盘。 |
| `/hotpot:finish-work` | 确认完成 → 汇总候选 → 用户批准晋升 → `hotpot issues promote` → `hotpot issues candidate clear` → 可选 git commit → `hotpot task done [--commit <SHA>]` → 可选切换并续作下一条 In-Progress 任务。 |

各平台具体承载方式：

- **Claude Code**：`.claude/commands/hotpot/*.md` + `.claude/agents/hotpot-{execution,review}.md`。
- **OpenCode**：`.opencode/commands/hotpot/*.md` + `.opencode/agents/hotpot-{execution,review}.md` + `.opencode/plugins/` 下的 TypeScript 插件。
- **Codex**：`.codex/skills/hotpot-*/SKILL.md` + `.codex/agents/hotpot-{execution,review}.toml`（没有项目级 Markdown 命令体系）。
- **Pi**：`.pi/prompts/hotpot-*.md` + `.pi/extensions/`（无原生子代理——同会话分阶段执行；review 阶段仍是只读）。

## 端到端流程（单个任务）

```
new → 任务文件写好 → execute → 执行子代理
                              → review 子代理（注入相关记忆）
                              → 修复循环（≤ 2 轮）
                              → 提议候选（用户挑选）
                              → 落盘批准的候选
finish-work → 汇总候选 → 用户批准晋升
            → 写入 issues.jsonl → 清空候选
            → 可选 git commit → 标记任务 Done [+ SHA]
            → 可选切换到下一条 In-Progress 任务 → 只跑执行阶段
```

状态变更走 CLI 子命令（**不要**直接改文件）：

- `hotpot task create [--switch|--inactive] --title <t>` —— 强制单 active 不变式；冲突时以 `ACTIVE_CONFLICT:` 前缀 bail（按机器可读 token 处理，**不要翻译**）。
- `hotpot task list --json`、`hotpot task active [--path|--count]`
- `hotpot task done [--task-id <id>] [--commit <sha>]`、`hotpot task cancel`、`hotpot task resume`
- `hotpot issues relevant --changed-file <p> --keyword <k> --limit 5`
- `hotpot issues promote`（stdin JSONL → `{"promoted":N}`）
- `hotpot issues candidate {list,add,clear}`（`add` 从 stdin 读 JSONL → `{"added":N}`）

## 设计原则

- **任务文件即契约**：新增的抽象优先映射到任务文件已有章节，不要另起 sidecar 文件。
- **编排归 slash command，智能归子代理**：命令文件负责收集上下文（路径、diff、记忆），并以明确 prompt 调起子代理。子代理不能调用其他子代理。
- **记忆流水线刻意分两段**：候选轻量、按用户隔离；晋升后的 issue 重要且全局共享。**禁止绕过候选阶段**。
- **review 永远只读**：Pi 的同会话回退也不例外。
- **状态变更走 CLI 子命令**，不要在 slash command prompt 里直接改文件。
- **幂等**：`hotpot init` / `hotpot update` 重复执行必须安全。
- **跨平台优先**：任何新行为都要按四个平台一并设计（或显式声明只覆盖子集并说明原因）。只在一个平台落地视为兼容性回退。

## 给后续 agent 的注意事项

- 在 `assets/platforms/<platform>/` 下新增的任何文件，都必须同时在 `src/commands/init/<platform>.rs::ASSETS` 数组里登记，否则 `hotpot init` 不会安装。Hotpot 私有文件用 `Asset::owned(...)`；与用户内容共存的平台主配置文件用 `Asset::merge_json(...)` / `Asset::merge_toml(...)`（锚点表在 `src/commands/init/merge.rs`）。
- 跨平台共用的 LLM 提示词放 `assets/prompts/`，由 `src/commands/init/mod.rs::SHARED_ASSETS` 统一登记一次，会被装到每个项目的 `.hotpot/prompts/`。运行时路径由 `src/context.rs::prompt_path` 解析。
- Hook / bootstrap 之间的公共 env-var 契约：`ROOT_DIR`、`HOTPOT_USERNAME`、`HOTPOT_LANGUAGE`、`HOTPOT_ISSUE_CANDIDATES_FILE`、`HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`、`HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`、`HOTPOT_TDD_PROTOCOL_PROMPT`。新增 env-var 时必须同步扩展 `hotpot hook bootstrap` 的输出，四个平台才能拿到。
- TDD 模式改动必须**同步**落到四个平台的 `new` / `execute` 资产，以及 `hotpot-execution` / `hotpot-review` 子代理。共享协议必须通过 `@.hotpot/prompts/tdd-protocol.md`（Claude/OpenCode）或 `$HOTPOT_TDD_PROTOCOL_PROMPT`（Codex/Pi）引用，**禁止**内联拷贝其内容。
- 重写 `overview.jsonl` / `issues.jsonl` / `issue-candidates.jsonl` 的临界区都要经过 `src/lock.rs::with_file_lock`。硬约束：**锁持有期间禁止 spawn 子进程**——平台 hook 可能回调 hotpot，触发嵌套死锁。
- 代码里的输出保持英文（见 `AGENTS.md`）；自然语言对外回复按 `.hotpot/config.toml::language` 决定。解析逻辑在 Rust（`src/context.rs::resolve_language[_with_source]`，链路：env `HOTPOT_LANGUAGE` → `<root>/.hotpot/config.toml` 顶层 `language` → `"English"`）。每个平台 hook 都会**逐轮**重新注入，对抗"一次指令长会话漂移"：Claude 走 `PreToolUse` + `SubagentStart` + `UserPromptSubmit`；Codex 走 `PreToolUse` + `SessionStart` + `UserPromptSubmit`；OpenCode 插件走 `shell.env`；Pi 走 `pi.on("context", …)`。`src/commands/hook.rs::language_directive_message` 是这一行简短指令的唯一来源；详细规范仍在 `assets/prompts/output-language.md`，被四份主工作流 prompt（`hotpot-new.md` / `hotpot-execute.md` / `hotpot-finish-work.md` / `tdd-protocol.md`）通过 `@.hotpot/prompts/output-language.md`（Claude/OpenCode）或 `$ROOT_DIR/.hotpot/prompts/output-language.md`（Codex/Pi——thin shell 显式列出该路径替换）引用。结构性锚点（CLI flag、JSON 字段、`ACTIVE_CONFLICT:`、markdown 章节标题、`tdd: true|false`、kebab-case slug）无论 language 取何值都**必须保持英文**。
- `hotpot task create` 只追加 `overview.jsonl` 一行；**不会**创建任务的 `<time>-<title>.md` 文件。`.md` 的创建归 `/hotpot:new` slash command 负责，通过平台的「创建文件」工具完成（Claude `Write`、OpenCode `write`、Codex `apply_patch *** Add File`、Pi `write`）。slash command prompt 必须显式禁止「先 Read 探测再 Write」——`task create` 之后该路径不存在是正常态，不是错误。CLI 会在 `task create` 中尽力 `create_dir_all` `<workspace>/tasks/` 作为兜底（非致命副作用），但 slash command 不能把这点当成契约依赖。
