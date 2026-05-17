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
- **VuePress hub**（`.hotpot-hub/`，opt-in）：用户跑 `hotpot vuepress install`（或 `hotpot init --enable-vuepress`）才会部署的 VuePress 项目根目录，承载 `pnpm docs:dev` 以便在浏览器中渲染任务文件。它与 `.hotpot/config.toml::[vuepress] enabled` 以及两份 opt-in prompt（`.hotpot/prompts/vuepress.md` / `vuepress-style.md`）构成原子状态——三者必须同步——只能通过 `hotpot vuepress install` / `uninstall` 维护。手动改 `enabled` 会使三者失同步。

## 目录结构

```
<project>/
├── .hotpot/
│   ├── config.toml                      # 项目配置（language、[vuepress]）
│   ├── issues.jsonl                     # 共享、长期记忆
│   ├── prompts/                         # 已安装的提示词资产（勿改）
│   ├── brainstorm/<session>/            # 可视化伴侣临时产物
│   └── workspaces/<username>/
│       ├── overview.jsonl
│       ├── issue-candidates.jsonl
│       └── tasks/<YYYY-MM-DD>-<title>.md
├── .hotpot-hub/                          # VuePress hub（opt-in；已加入 .gitignore）
│   ├── package.json                     # vuepress + theme 依赖
│   ├── docs/                            # 通过用户软链汇集任务文件
│   └── vuepress.runtime.json            # dev-server 的 pid/port/url/ttl 状态
├── .claude/  .opencode/  .codex/  .pi/   # 各平台配置目录
```

路径解析：`src/paths.rs`。用户名解析链（`src/context.rs::resolve_username`）：`HOTPOT_USERNAME` → `git config --local user.name` → `git config --global user.name` → `"default"`。

## 命令

slash 命令是 AI 工作流；CLI 子命令是状态和资源管理。

| 命令 | 作用 |
|---|---|
| `hotpot init` | 安装指定平台的资产与共享 prompt。幂等。`--platform {claude\|opencode\|codex\|pi\|all}`。`--enable-vuepress`（或交互式 yes）会额外跑 `hotpot vuepress install`。 |
| `hotpot update` | 协作者 day-1 入口。自动探测已安装平台，刷新资产、bootstrap 当前用户 workspace、合并 hotpot 段到 `.gitignore`、跑健康自检。 |
| `hotpot vuepress {install,uninstall,start,stop,status}` | 管理 opt-in VuePress 集成。`install` 部署 `.hotpot-hub/` + `pnpm install` + opt-in prompts + 翻 `[vuepress] enabled = true`；`uninstall` 反向回滚；`start`/`stop`/`status` 通过 `.hotpot-hub/vuepress.runtime.json` 管理 `pnpm docs:dev` 进程。详见 **VuePress 集成**。 |
| `/hotpot:new` | 头脑风暴 → 用户批准设计 → `hotpot task create [--switch\|--inactive]` → 写入 handoff 任务文件。启用 VuePress 时收尾流程额外询问用户是否在浏览器查看并跑 `hotpot vuepress start`。new 阶段不改业务代码。 |
| `/hotpot:execute` | 入口跑 `hotpot vuepress stop --if-running` 释放 `/hotpot:new` 可能启动的 dev server → 取活动任务 → 调起执行子代理 → 收集 diff 与相关记忆 → 调起只读 review 子代理 → 修复循环（≤ 2 轮）→ 缓冲 issue 候选 → 让用户挑选保留范围 → 通过 `hotpot issues candidate add` 落盘。 |
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

## VuePress 集成

VuePress 是 **opt-in**：禁用项目里**不会**出现 VuePress 相关 prompt、env-var 或 hub 文件。启用 / 禁用都是原子 CLI 操作——手动改 config 标志是不被支持的用法。

### 原子状态——必须同步的三件套

1. `.hotpot/config.toml::[vuepress] enabled = true`
2. `.hotpot-hub/` 存在且 `package.json` + `pnpm install` 已完成
3. `.hotpot/prompts/vuepress.md` + `.hotpot/prompts/vuepress-style.md` 存在

`hotpot vuepress install` 把三者原子地落到位；`hotpot vuepress uninstall` 反向回滚（prompts → docs 软链 → `.hotpot-hub/` → config 开关）。`hotpot vuepress start` 启动前会跑 `verify_install_consistency`，发现任一不一致就 bail 并提示修复，避免 pnpm 失败时给出难解读的报错。

`[vuepress]` 表由 `enable_in_config_toml`（`src/vuepress.rs`）写入，包含双语警告注释；任何场景都**不要**绕过这个写入器手改 config。

### 资产分级

| 类别 | 来源 | 安装时机 | 用途 |
|---|---|---|---|
| 共享资产 | `SHARED_ASSETS`（`src/assets/shared.rs`） | 每次 `hotpot init` | 跨平台 prompt（output-language、tdd-protocol、hotpot-new/execute/finish-work 等）。 |
| VuePress opt-in prompts | `VUEPRESS_OPT_IN_ASSETS`（`src/assets/vuepress_opt_in.rs`） | 仅 `hotpot vuepress install` | `vuepress.md`（收尾流程）+ `vuepress-style.md`（markdown 写作规范）。`hotpot-new.md` 的 env-gate 只在 `$HOTPOT_VUEPRESS_ENABLED == "true"` 时 Read 它们；禁用项目里磁盘上没有这两份文件，AI 上下文自然清洁。 |
| VuePress hub 项目 | `VUEPRESS_HUB_ASSETS`（`src/assets/vuepress_hub.rs`） | 仅 `hotpot vuepress install` | `.hotpot-hub/` 内 `package.json`、`pnpm-lock.yaml`、`docs/README.md` 以及**五份 `.vuepress/` 文件**（`config.js` / `client.js` / `sidebar.js` / `styles/index.scss` / `components/TaskIndex.vue`）。前四份是紧密耦合的运行时文件（`config.js` ↔ `client.js` ↔ `sidebar.js` ↔ `TaskIndex.vue` 通过编译期 `__HOTPOT_TASK_INDEX__` 注入串联）；`styles/index.scss` 是**独立装饰层**，由 `@vuepress/theme-default` 通过 `styles/index.scss` 约定自动加载——可安全编辑或删除，不会破坏首页 TaskIndex 注入链。真正的 `pnpm install` + `sync_tasks_links`（幂等：清掉 stale 链、为新用户建链、保留已有链）由 `vuepress::install_hub` 编排，不由资产引擎完成。 |

### 服务生命周期——三层防护

`hotpot vuepress start` spawn 出的 `pnpm docs:dev` 进程不能泄漏到用户 session 之外。三层独立防护：

1. **`/hotpot:execute` 入口 stop**（主路径）。prompt 第一步就是 `hotpot vuepress stop --if-running`。幂等，覆盖"用户 new 完直接进入 execute"的常规路径。
2. **`SessionEnd` / `session_shutdown` hook**（第 2 层）。Claude Code、OpenCode、Pi 三平台 session 关闭时都会触发对应事件，调同一份幂等 stop。**Codex 文档里没有 SessionEnd 事件**——Codex 用户只能靠第 3 层。
3. **`--ttl` 懒过期**（兜底）。`start` 在 `vuepress.runtime.json` 写入 `expires_at`（默认 30 分钟）。下次 `status` 或 `start` 调用时检查这个时间戳并 kill 过期进程。Codex 的唯一安全网。

`runtime.json` 在 `.hotpot-hub/vuepress.runtime.json`（hub 内，uninstall 删 hub 时自然连带清理）。stale 状态（pid 已死 / ttl 过期）通过下次读取时懒清理，不需要后台轮询。

VuePress 的公共 env-var 契约：`HOTPOT_VUEPRESS_ENABLED` 始终输出（`"true"`/`"false"`）；`HOTPOT_VUEPRESS_PORT` + `HOTPOT_VUEPRESS_URL` 仅启用时输出。

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
- Hook / bootstrap 之间的公共 env-var 契约：`ROOT_DIR`、`HOTPOT_USERNAME`、`HOTPOT_LANGUAGE`、`HOTPOT_ISSUE_CANDIDATES_FILE`、`HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`、`HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`、`HOTPOT_TDD_PROTOCOL_PROMPT`、`HOTPOT_NEW_PROMPT`、`HOTPOT_EXECUTE_PROMPT`、`HOTPOT_FINISH_WORK_PROMPT`，外加 VuePress 三件套（`HOTPOT_VUEPRESS_ENABLED` 总是有；`HOTPOT_VUEPRESS_PORT` / `HOTPOT_VUEPRESS_URL` 仅启用时有——见 **VuePress 集成**）。新增 env-var 时必须同步扩展 `hotpot hook bootstrap` 的输出与 `src/context.rs::Context`，四个平台才能拿到。
- **面向 agent 的 env-var path 字段不变式**：`Context` 上所有 path 字段（`ROOT_DIR`、`HOTPOT_ISSUE_CANDIDATES_FILE`、所有 `HOTPOT_*_PROMPT`）在**所有平台**都按 POSIX 形式输出——`dunce::canonicalize` 负责剥离 Windows 的 `\\?\` verbatim 前缀，`path_to_agent_string`（`src/context.rs`）再把残留的 `\` 全部替换为 `/`。Windows 的 fs API 原生接受正斜杠分隔符，所以这层规范化是无损的；其目的在于杜绝反斜杠流入下游 markdown 渲染（`\h`、`\R` 等会被当 escape 吃掉）、URI 拼接（`\\?\` 会被 URI 编码成 `%3F`，产生 `file://%3F\D:\...`）以及 JSON-in-prompt 展开（反斜杠需要二次转义，下游消费方经常算错）。**任何新增的 path 类字段都必须经过 `path_to_agent_string`**——禁止直接 `.display().to_string()` 后塞进 `Context`；任何会喂给 `Context` 的路径解析也必须用 `dunce::canonicalize` 而非 `std::fs::canonicalize`。
- TDD 模式改动必须**同步**落到四个平台的 `new` / `execute` 资产，以及 `hotpot-execution` / `hotpot-review` 子代理。共享协议必须通过 `@.hotpot/prompts/tdd-protocol.md`（Claude/OpenCode）或 `$HOTPOT_TDD_PROTOCOL_PROMPT`（Codex/Pi）引用，**禁止**内联拷贝其内容。
- 重写 `overview.jsonl` / `issues.jsonl` / `issue-candidates.jsonl` 的临界区都要经过 `src/lock.rs::with_file_lock`。硬约束：**锁持有期间禁止 spawn 子进程**——平台 hook 可能回调 hotpot，触发嵌套死锁。
- 代码里的输出保持英文（见 `AGENTS.md`）；自然语言对外回复按 `.hotpot/config.toml::language` 决定。解析逻辑在 Rust（`src/context.rs::resolve_language[_with_source]`，链路：env `HOTPOT_LANGUAGE` → `<root>/.hotpot/config.toml` 顶层 `language` → `"English"`）。每个平台 hook 都会**逐轮**重新注入，对抗"一次指令长会话漂移"：Claude 走 `PreToolUse` + `SubagentStart` + `UserPromptSubmit`；Codex 走 `PreToolUse` + `SessionStart` + `UserPromptSubmit`；OpenCode 插件走 `shell.env`；Pi 走 `pi.on("context", …)`。`src/commands/hook.rs::language_directive_message` 是这一行简短指令的唯一来源；详细规范仍在 `assets/prompts/output-language.md`，被四份主工作流 prompt（`hotpot-new.md` / `hotpot-execute.md` / `hotpot-finish-work.md` / `tdd-protocol.md`）通过 `@.hotpot/prompts/output-language.md`（Claude/OpenCode）或 `$ROOT_DIR/.hotpot/prompts/output-language.md`（Codex/Pi——thin shell 显式列出该路径替换）引用。结构性锚点（CLI flag、JSON 字段、`ACTIVE_CONFLICT:`、markdown 章节标题、`tdd: true|false`、kebab-case slug）无论 language 取何值都**必须保持英文**。
- `hotpot task create` 只追加 `overview.jsonl` 一行；**不会**创建任务的 `<time>-<title>.md` 文件。`.md` 的创建归 `/hotpot:new` slash command 负责，通过平台的「创建文件」工具完成（Claude `Write`、OpenCode `write`、Codex `apply_patch *** Add File`、Pi `write`）。slash command prompt 必须显式禁止「先 Read 探测再 Write」——`task create` 之后该路径不存在是正常态，不是错误。CLI 会在 `task create` 中尽力 `create_dir_all` `<workspace>/tasks/` 作为兜底（非致命副作用），但 slash command 不能把这点当成契约依赖。
