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

- **任务文件**（`<workspace>/tasks/<YYYY-MM-DD>-<title>.md`）：自包含的 handoff 文档。必须含 `## Task`、`## Plan`、`## Execution Instructions`。`## Plan > ### Mode` 携带 `- tdd: true|false` 决定 TDD 与默认两种流程。`## Plan > ### Execution Strategy` 携带 `- git-worktree: true|false`，它是执行阶段是否使用 worktree 的权威决策。`<title>` 是 `task create --title` 传入的 kebab-case slug；CLI 在拼文件名时会把 `title` 内残留的连续空白折成单个 `-` 作为兜底（仅防御性兜底，`/hotpot:new` 已要求 AI 直接产出 kebab-case，不做更激进的 slugify）。
- **任务台账**（`<workspace>/overview.jsonl`）：每个用户一份，强制不变式："同一时刻最多一条 `active=true && status=In Progress` 的行"。
- **Issue 候选**（`.hotpot/issue-candidates.jsonl`）：项目级共享的临时 review 记忆候选，由 execute 缓冲并在 finish-work 中决策晋升或丢弃。旧版 `.hotpot/workspaces/<username>/issue-candidates.jsonl` 会一次性迁移进这个全局文件，然后清空旧文件。
- **Issue 记忆**（`.hotpot/issues.jsonl`）：共享、长期。只能通过 `hotpot issues promote` 在用户确认后追加。
- **共享 prompt**（`.hotpot/prompts/`）：跨平台的 LLM 提示词资产，由 `hotpot init` / `hotpot update` 安装，不要手改。
- **VuePress hub**（`.hotpot-hub/`，opt-in）：用户跑 `hotpot vuepress install`（或 `hotpot init --enable-vuepress`）才会部署的 VuePress 项目根目录，承载 `pnpm docs:dev` 以便在浏览器中渲染任务文件。它与 `.hotpot/config.toml::[vuepress] enabled` 以及两份 opt-in prompt（`.hotpot/prompts/vuepress.md` / `vuepress-style.md`）构成原子状态——三者必须同步——只能通过 `hotpot vuepress install` / `uninstall` 维护。手动改 `enabled` 会使三者失同步。

## 目录结构

```
<project>/
├── .hotpot/
│   ├── config.toml                      # 项目配置（language、[vuepress]）
│   ├── issues.jsonl                     # 共享、长期记忆
│   ├── issue-candidates.jsonl           # 共享、临时 review 记忆候选
│   ├── prompts/                         # 已安装的提示词资产（勿改）
│   ├── brainstorm/<session>/            # 可视化伴侣临时产物；stop 会清理 brainstorm session 目录
│   └── workspaces/<username>/
│       ├── overview.jsonl
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
| `hotpot update` | 协作者 day-1 入口。自动探测已安装平台，刷新资产、bootstrap 当前用户 workspace、合并 hotpot 段到 `.gitignore`、跑健康自检。`--force` 会覆盖内容不同的 Hotpot 私有 owned 模板，同时保留 merge / config / 用户自有资产的既有策略。 |
| `hotpot vuepress {install,uninstall,start,stop,status}` | 管理 opt-in VuePress 集成。`install` 部署 `.hotpot-hub/` + `pnpm install` + opt-in prompts + 翻 `[vuepress] enabled = true`；`uninstall` 反向回滚；`start`/`stop`/`status` 通过 `.hotpot-hub/vuepress.runtime.json` 管理 `pnpm docs:dev` 进程。详见 **VuePress 集成**。 |
| `/hotpot:new` | 头脑风暴 → 用户批准设计 → 决定执行策略（`## Plan > ### Execution Strategy`，包含 `git-worktree: true|false`）→ `hotpot task create [--switch\|--inactive]` → 写入 handoff 任务文件。启用 VuePress 时收尾流程额外询问用户是否在浏览器查看并跑 `hotpot vuepress start`。new 阶段不改业务代码。 |
| `/hotpot:execute` | 入口跑 `hotpot vuepress stop --if-running` 释放 `/hotpot:new` 可能启动的 dev server → 取并读取活动任务 → 解析 `## Plan > ### Execution Strategy` → 按 `git-worktree: true|false` 创建、复用或禁止 worktree → 调起执行子代理 → 收集 diff 与相关记忆 → 调起只读 review 子代理 → 修复循环（≤ 2 轮）→ 缓冲 issue 候选 → 让用户挑选保留范围 → 通过 `hotpot issues candidate add` 落盘。 |
| `/hotpot:finish-work` | 确认完成 → 汇总候选 → 用户批准晋升 → `hotpot issues promote` → `hotpot issues candidate clear` → 可选 git commit → `hotpot task done [--commit <SHA>]` → 当 task commit 由 finish-work 自动创建时，自动把 task-done ledger diff 提交为 `chore: record task Done` → 可选切换并续作下一条 In-Progress 任务。 |

各平台具体承载方式：

- **Claude Code**：`.claude/commands/hotpot/*.md` + `.claude/agents/hotpot-{execution,review}.md`。
- **OpenCode**：`.opencode/commands/hotpot/*.md` + `.opencode/agents/hotpot-{execution,review}.md` + `.opencode/plugins/` 下的 TypeScript 插件。
- **Codex**：`.codex/skills/hotpot-*/SKILL.md` + `.codex/agents/hotpot-{execution,review}.toml`（没有项目级 Markdown 命令体系）。
- **Pi**：`.pi/extensions/` —— 通过 `pi.registerCommand` 注册 `/hotpot-new` / `/hotpot-execute` / `/hotpot-finish-work` 三个 slash command，handler 用 `pi.sendUserMessage`（该方法在 `ExtensionAPI` 上，**不**在 handler 的 `ctx: ExtensionCommandContext` 上）以 user 消息发送 workflow 提示；不再使用 prompt template thin shell；无原生子代理——同会话分阶段执行；review 阶段仍是只读。每个 handler 还会在 `pi.sendUserMessage` 前 arm `pendingFirstToolGuard`；同一个 `tool_call` hook 必须在 workflow prompt 成功读取前阻止除精确读取该 workflow prompt 之外的任何首个工具调用，读取成功后再 disarm，恢复正常 workflow 探索。

## 端到端流程（单个任务）

```
new → 任务文件写好（含 Execution Strategy）
    → execute 消费策略 → 执行子代理
                     → review 子代理（注入相关记忆）
                     → 修复循环（≤ 2 轮）
                     → 提议候选（用户挑选）
                     → 落盘批准的候选
finish-work → 汇总候选 → 用户批准晋升
            → 写入 issues.jsonl → 清空候选
            → 可选 git commit → 标记任务 Done [+ SHA]
            → task commit 为自动创建时，自动提交 task-done ledger diff
            → 可选切换到下一条 In-Progress 任务 → 只跑执行阶段
```

状态变更走 CLI 子命令（**不要**直接改文件）：

- `hotpot task create [--switch|--inactive] --title <t>` —— 强制单 active 不变式；冲突时以 `ACTIVE_CONFLICT:` 前缀 bail（按机器可读 token 处理，**不要翻译**）。
- `hotpot task list --json`、`hotpot task active [--path|--count]`
- `hotpot task done [--task-id <id>] [--commit <sha>]` —— ledger 更新后，
  CLI 还会同步任务 Markdown 文件的 VuePress Overview `Status` 单元格为 `Done`
  （仅 VuePress 启用项目）。I/O 错误输出 `eprintln!` warning 但不导致命令失败。
  跳过类结果（VuePress 未启用、文件缺失、无 Overview 表）静默忽略。
  同步实现在 `src/task/markdown.rs::sync_task_file_status`。
- `hotpot task cancel`、`hotpot task resume`
- `hotpot issues relevant --changed-file <p> --keyword <k> --limit 5`
- `hotpot issues promote`（stdin JSONL → `{"promoted":N}`）
- `hotpot issues candidate {list,add,clear}`（`add` 从 stdin 读 JSONL → `{"added":N}`）

worktree 执行契约：

- `/hotpot:new` 必须在创建任务记录与写任务文件前解决执行策略。策略写在 `## Plan > ### Execution Strategy` 下，必须包含 `- git-worktree: true` 或 `- git-worktree: false`。
- `/hotpot:execute` 只消费任务文件中的策略；它不会询问用户是否使用 worktree。缺失或非法的 `git-worktree` 值会使执行停止，并要求重新运行 `/hotpot:new` 或修订任务文件。
- 当 `git-worktree: true` 时，execute 复用 `hotpot worktree path` 返回的已附着 worktree；如果没有附着，则运行 `hotpot worktree create`。创建失败是 blocker，不会提示降级。
- 当 `git-worktree: false` 时，execute 在当前 checkout 中运行。如果 `hotpot worktree path` 报告已有附着 worktree，execute 会停止，因为任务文件策略与 ledger 状态冲突。

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
| VuePress opt-in prompts | `VUEPRESS_OPT_IN_ASSETS`（`src/assets/vuepress_opt_in.rs`） | 仅 `hotpot vuepress install` | `vuepress.md`（收尾流程）+ `vuepress-style.md`（markdown 写作规范）。`hotpot-new.md` 的 file-existence gate 只在这两份文件都在盘上时 Read 它们——而它们在盘上恰好等价于 VuePress 已安装（由 `hotpot vuepress install` / `uninstall` 维护的原子状态）。四个平台都靠 Bash `test -f` 直接观测，所以 OpenCode（其插件不把 `HOTPOT_VUEPRESS_ENABLED` 推进 AI 对话）也能跟 Claude / Codex / Pi 走同样的分支。 |
| VuePress hub 项目 | `VUEPRESS_HUB_ASSETS`（`src/assets/vuepress_hub.rs`） | 仅 `hotpot vuepress install` | `.hotpot-hub/` 内 `package.json`、`pnpm-lock.yaml`、`docs/README.md` 以及**五份 `.vuepress/` 文件**（`config.js` / `client.js` / `sidebar.js` / `styles/index.scss` / `components/TaskIndex.vue`）。前四份是紧密耦合的运行时文件（`config.js` ↔ `client.js` ↔ `sidebar.js` ↔ `TaskIndex.vue` 通过编译期 `__HOTPOT_TASK_INDEX__` 注入串联）；`styles/index.scss` 是**独立装饰层**，由 `@vuepress/theme-default` 通过 `styles/index.scss` 约定自动加载——可安全编辑或删除，不会破坏首页 TaskIndex 注入链。真正的 `pnpm install` + `sync_tasks_links`（幂等：清掉 stale 链、为新用户建链、保留已有链）由 `vuepress::install_hub` 编排，不由资产引擎完成。 |

### 服务生命周期——三层防护

`hotpot vuepress start` spawn 出的 `pnpm docs:dev` 进程不能泄漏到用户 session 之外。三层独立防护：

1. **`/hotpot:execute` 入口 stop**（主路径）。prompt 第一步就是 `hotpot vuepress stop --if-running`。幂等，覆盖"用户 new 完直接进入 execute"的常规路径。
2. **`SessionEnd` / `session_shutdown` hook**（第 2 层）。Claude Code、OpenCode、Pi 三平台 session 关闭时都会触发对应事件，调同一份幂等 stop。**Codex 文档里没有 SessionEnd 事件**——Codex 用户只能靠第 3 层。
3. **`--ttl` 懒过期**（兜底）。`start` 在 `vuepress.runtime.json` 写入 `expires_at`（默认 30 分钟）。下次 `status` 或 `start` 调用时检查这个时间戳并 kill 过期进程。Codex 的唯一安全网。

`runtime.json` 在 `.hotpot-hub/vuepress.runtime.json`（hub 内，uninstall 删 hub 时自然连带清理）。stale 状态（pid 已死 / ttl 过期）通过下次读取时懒清理，不需要后台轮询。

VuePress 的公共 env-var 契约：`HOTPOT_VUEPRESS_ENABLED` 始终输出（`"true"`/`"false"`）；`HOTPOT_VUEPRESS_PORT` + `HOTPOT_VUEPRESS_URL` 仅启用时输出。

## npm 分发

项目提供了一个轻量 npm wrapper 包（位于 `npm/`，发布为 `@fancyhq/hotpot`），用户可通过 `npm install -g @fancyhq/hotpot` 全局安装。安装后的 CLI 命令仍然是 `hotpot`。

### 架构

- `npm/package.json` — 定义包元数据（发布为 `@fancyhq/hotpot`）、`bin.hotpot` 入口和 `postinstall` 脚本。
- `npm/bin/hotpot.js` — CLI 入口；将所有参数和 stdio 转发到同一 `bin/` 目录下的原生 Rust 二进制。
- `npm/scripts/install.js` — postinstall 脚本；检测 `process.platform` / `process.arch`，映射到 release asset label，从 GitHub Releases 下载对应压缩包（`https://github.com/fancyhq/hotpot/releases/download/<tag>/hotpot-<tag>-<label>.<ext>`），解压二进制到 `bin/`，并设置可执行权限。

支持平台：Linux x86_64/aarch64、macOS x86_64/aarch64、Windows x86_64。不支持的平台输出英文错误信息并以退出码 1 终止。

### 版本同步

npm 包版本通过 `release-please` 与 Rust crate 版本保持同步。`release-please-config.json` 在 `extra-files` 数组中列出了 `npm/package.json`，因此 Release PR 会根据 conventional commits 自动同时更新 `Cargo.toml` 和 `npm/package.json`。

### 发布流程

`.github/workflows/release-please.yml` 包含一个 `publish-npm` job，在 release 创建后、`build-release-assets` 完成后运行。它会检出 release tag、执行 `npm pack --dry-run` 验证、然后通过 `npm publish ./npm --access public` 及 `NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}` 发布。`--access public` 是必需的，因为 scoped 包（`@fancyhq/hotpot`）在 npm 上默认为私有。

手动重建 workflow `.github/workflows/rebuild-release-assets.yml` **不**发布 npm——它只重建已有 tag 的二进制资产并上传。

### Wrapper 可执行权限契约

npm CLI 入口 `npm/bin/hotpot.js` 在 Unix 上必须保持可执行权限，以便 fish 等 shell（会检查 symlink 目标的 executable bit）能正确执行安装后的 `hotpot` 命令。以下机制保证这一要求：

- **源文件权限**：`npm/bin/hotpot.js` 以可执行权限提交到仓库（`chmod +x`，mode `0o755`）。Git 会追踪 mode 变更（`100644 → 100755`）。
- **Tarball 验证**：`npm pack --dry-run --json ./npm` 会输出 tarball 中每个文件的 mode。`npm/scripts/install.test.js` 中的回归测试断言 `bin/hotpot.js` 条目的可执行位已设置（`mode & 0o111 !== 0`）。
- **确定性（无网络）测试**：`node --test npm/scripts/install.test.js` 运行完整测试套件，包括可执行位检查、bin 映射验证、tarball 文件完整性以及 `setExecutable` 辅助函数测试，全程不执行任何网络 I/O。

`npm/scripts/install.js` 中的 `setExecutable` 辅助函数（用于下载后的原生二进制）也已导出并通过 `node:test` 独立测试。任何移除 npm wrapper 可执行权限或原生二进制 chmod 调用的修改都会被 CI 拦截。

### 发布前提

- 仓库必须配置 `NPM_TOKEN` secret，其值应为具有 `@fancyhq/hotpot` 包发布权限的 npm automation token。
- npm 安装需要网络能访问 GitHub Releases。离线环境或 GitHub 被阻断时安装将失败。
- 如果你的 npm 配置了自定义或内部 registry 镜像，`@fancyhq/hotpot` 包可能无法在该 registry 中找到。可使用 `--registry https://registry.npmjs.org` 进行单次安装，或使用 `npm config set @fancyhq:registry https://registry.npmjs.org/` 设置持久化的 scope 级别覆盖。

## 多渠道分发

Hotpot 通过多个包管理器渠道分发。本节涵盖每个渠道的发布流程、版本同步和前置条件。

### 渠道概览

| 渠道 | 类型 | 自动发布 | 所需 Secret | 状态 |
|------|------|----------|-------------|------|
| GitHub Release（二进制资产） | 直接下载 | 是（资产构建并上传） | `GITHUB_TOKEN`（内置） | ✅ 生产 |
| npm（`@fancyhq/hotpot`） | npm registry | 是（workflow `publish-npm` job） | `NPM_TOKEN` | ✅ 生产 |
| crates.io（`hotpot-ai`） | Rust crate registry | 是（workflow `publish-crates-io` job） | `CARGO_REGISTRY_TOKEN` | ✅ 新增 |

### 资产命名规则

所有二进制压缩包遵循以下命名格式：

```
hotpot-${TAG}-${ASSET_LABEL}${EXT}
```

其中：
- `${TAG}` 是完整的 GitHub Release tag（例如 `hotpot-v0.3.2`）
- `${ASSET_LABEL}` 标识平台（例如 `windows-x86_64`、`macos-aarch64`、`linux-x86_64`）
- `${EXT}` Linux/macOS 为 `.tar.gz`，Windows 为 `.zip`

当 `TAG=hotpot-v0.3.2` 时，文件名示例为 `hotpot-hotpot-v0.3.2-windows-x86_64.zip`。

### 包命名与发布契约

crates.io 包名 `hotpot-ai` 与 release tag、资产命名以及安装后的 CLI 命令名是分离的。以下契约维护了这种分离：

- **`release-please-config.json`** 显式设置了根 package 的 `component: "hotpot"`。这确保 release-please 生成的 release tag 格式为 `hotpot-v<version>`，而不会从 Cargo 包名派生为 `hotpot-ai-v<version>`。
- **`Cargo.toml`** 保留显式的 `[[bin]] name = "hotpot"` 目标，因此 `cargo install hotpot-ai` 安装后的 CLI 命令为 `hotpot`（而非 `hotpot-ai`）。
- **`npm/package.json`** 暴露 `"bin": { "hotpot": "bin/hotpot.js" }`，因此 `npm install -g @fancyhq/hotpot` 使得 PATH 上可用的 CLI 命令名为 `hotpot`。
- **Release 资产文件名**遵循 `hotpot-${TAG}-${ASSET_LABEL}${EXT}` 格式（例如 `hotpot-hotpot-v0.3.2-windows-x86_64.zip`）。资产名中的 TAG 是完整的 GitHub Release tag（`hotpot-v<version>`），而非 Cargo 包名。
- **压缩包内部结构**始终包含名为 `hotpot`（Windows 上为 `hotpot.exe`）的单一二进制文件，绝不会是 `hotpot-ai`。

这些契约防止 crates.io 包名变更泄漏到 GitHub Release tag、下载 URL 或用户最终使用的命令名中。未来对分发渠道的任何更改都必须保持这种分离。

### 发布工作流 Job 依赖图

```
release-please
  ├── build-release-assets（5 平台矩阵构建）
  │     └── publish-npm（构建完成后）
  └── publish-crates-io（独立，仅需 tag）
```

所有 job 都受 `needs.release-please.outputs.release_created == 'true'` 门控，因此仅在新 release 创建时运行（普通 `main` push 不会触发）。

### crates.io 渠道

`publish-crates-io` job（位于 `.github/workflows/release-please.yml`）：
1. 检出 release tag。
2. 通过 `cargo package --locked --no-verify` 验证包。
3. 通过 `cargo publish --locked --token "$CARGO_REGISTRY_TOKEN"` 发布到 crates.io。

**前置条件：**
- `CARGO_REGISTRY_TOKEN` 仓库 secret 配置了 crates.io API token。
- `Cargo.toml` 必须包含 `description`、`readme`、`keywords`、`categories` 等元数据。
- 包名称为 `hotpot-ai`；`Cargo.toml` 保留显式 `[[bin]]` 目标 `hotpot`，因此 `cargo install hotpot-ai` 安装后的 CLI 命令仍为 `hotpot`。

### 版本同步

所有渠道的版本通过 `release-please` 同步：
- `release-please-config.json` 的 `extra-files` 数组列出了 `release-please` 在 Release PR 生成期间更新的文件：
  - `Cargo.lock`（Rust lockfile）
  - `npm/package.json`（npm 包版本）

### 延后渠道：Homebrew、Scoop、winget

当前实现**不**维护或发布 Homebrew、Scoop、winget manifests。这些渠道已延后，因为仓库内本地 manifest 不能提供用户预期的直接安装体验，例如 `brew install hotpot`、`scoop install hotpot` 或 `winget install fancyhq.hotpot`。

后续工作应为每个渠道增加真正的分发路径，例如 Homebrew tap 或 Homebrew Core PR、Scoop bucket 条目，以及 `microsoft/winget-pkgs` 提交流程。

### 手动重建 workflow

`.github/workflows/rebuild-release-assets.yml` workflow 仅重建并上传已有 tag 的二进制资产。它**不**：
- 发布到 npm 或 crates.io。

如需发布包，请使用完整的 `release-please.yml` workflow。

## 设计原则

- **任务文件即契约**：新增的抽象优先映射到任务文件已有章节，不要另起 sidecar 文件。
- **编排归 slash command，智能归子代理**：命令文件负责收集上下文（路径、diff、记忆），并以明确 prompt 调起子代理。子代理不能调用其他子代理。
- **记忆流水线刻意分两段**：候选是轻量的项目级共享临时记录；晋升后的 issue 是重要的共享长期记忆。**禁止绕过候选阶段**。
- **review 永远只读**：Pi 的同会话回退也不例外。
- **状态变更走 CLI 子命令**，不要在 slash command prompt 里直接改文件。
- **幂等**：`hotpot init` / `hotpot update` 重复执行必须安全。`hotpot update --force` 是显式替换内容不同的 Hotpot 私有 owned 模板的逃生口；它不能把 `Merge*` 资产或 `CreateIfMissing` seed 变成整文件覆盖。
- **跨平台优先**：任何新行为都要按四个平台一并设计（或显式声明只覆盖子集并说明原因）。只在一个平台落地视为兼容性回退。

## 给后续 agent 的注意事项

- 在 `assets/platforms/<platform>/` 下新增的任何文件，都必须同时在 `src/commands/init/<platform>.rs::ASSETS` 数组里登记，否则 `hotpot init` 不会安装。Hotpot 私有文件用 `Asset::owned(...)`；与用户内容共存的平台主配置文件用 `Asset::merge_json(...)` / `Asset::merge_toml(...)`（锚点表在 `src/commands/init/merge.rs`）。
- 跨平台共用的 LLM 提示词放 `assets/prompts/`，由 `src/commands/init/mod.rs::SHARED_ASSETS` 统一登记一次，会被装到每个项目的 `.hotpot/prompts/`。运行时路径由 `src/context.rs::prompt_path` 解析。
- Hook / bootstrap 之间的公共 env-var 契约：`ROOT_DIR`、`HOTPOT_USERNAME`、`HOTPOT_LANGUAGE`、`HOTPOT_ISSUE_CANDIDATES_FILE`、`HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`、`HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`、`HOTPOT_TDD_PROTOCOL_PROMPT`、`HOTPOT_NEW_PROMPT`、`HOTPOT_EXECUTE_PROMPT`、`HOTPOT_FINISH_WORK_PROMPT`，外加 VuePress 三件套（`HOTPOT_VUEPRESS_ENABLED` 总是有；`HOTPOT_VUEPRESS_PORT` / `HOTPOT_VUEPRESS_URL` 仅启用时有——见 **VuePress 集成**）。新增 env-var 时必须同步扩展 `hotpot hook bootstrap` 的输出与 `src/context.rs::Context`，四个平台才能拿到。
- **面向 agent 的 env-var path 字段不变式**：`Context` 上所有 path 字段（`ROOT_DIR`、`HOTPOT_ISSUE_CANDIDATES_FILE`、所有 `HOTPOT_*_PROMPT`）在**所有平台**都按 POSIX 形式输出——`dunce::canonicalize` 负责剥离 Windows 的 `\\?\` verbatim 前缀，`path_to_agent_string`（`src/context.rs`）再把残留的 `\` 全部替换为 `/`。Windows 的 fs API 原生接受正斜杠分隔符，所以这层规范化是无损的；其目的在于杜绝反斜杠流入下游 markdown 渲染（`\h`、`\R` 等会被当 escape 吃掉）、URI 拼接（`\\?\` 会被 URI 编码成 `%3F`，产生 `file://%3F\D:\...`）以及 JSON-in-prompt 展开（反斜杠需要二次转义，下游消费方经常算错）。**任何新增的 path 类字段都必须经过 `path_to_agent_string`**——禁止直接 `.display().to_string()` 后塞进 `Context`；任何会喂给 `Context` 的路径解析也必须用 `dunce::canonicalize` 而非 `std::fs::canonicalize`。
- TDD 模式改动必须**同步**落到四个平台的 `new` / `execute` 资产，以及 `hotpot-execution` / `hotpot-review` 子代理。共享协议必须通过 `@.hotpot/prompts/tdd-protocol.md`（Claude/OpenCode）或 `$HOTPOT_TDD_PROTOCOL_PROMPT`（Codex/Pi）引用，**禁止**内联拷贝其内容。
- 重写 `overview.jsonl` / `issues.jsonl` / `.hotpot/issue-candidates.jsonl` 的临界区都要经过 `src/lock.rs::with_file_lock`。全局 candidates 锁也保护从 `.hotpot/workspaces/<username>/issue-candidates.jsonl` 进行的一次性旧文件迁移。硬约束：**锁持有期间禁止 spawn 子进程**——平台 hook 可能回调 hotpot，触发嵌套死锁。
- 双语注释遵循英文在前原则（见 `AGENTS.md`）：英文段落/句子在前，中文简短补充在后。详细技术说明优先保留英文完整性。该规则适用于所有 Rust（`//!`、`///`、`//`）、TypeScript（`//`、`/* */`）和配置文件（`#`）注释。
- 代码里的输出保持英文（见 `AGENTS.md`）；自然语言对外回复按 `.hotpot/config.toml::language` 决定。解析逻辑在 Rust（`src/context.rs::resolve_language[_with_source]`，链路：env `HOTPOT_LANGUAGE` → `<root>/.hotpot/config.toml` 顶层 `language` → `"English"`）。每个平台 hook 都会**逐轮**重新注入，对抗"一次指令长会话漂移"：Claude 走 `PreToolUse` + `SubagentStart` + `UserPromptSubmit`；Codex 走 `PreToolUse` + `SessionStart` + `UserPromptSubmit`；OpenCode 插件走 `shell.env`；Pi 走 `pi.on("context", …)`。`src/commands/hook.rs::language_directive_message` 是这一行简短指令的唯一来源；详细规范仍在 `assets/prompts/output-language.md`，被四份主工作流 prompt（`hotpot-new.md` / `hotpot-execute.md` / `hotpot-finish-work.md` / `tdd-protocol.md`）通过 `@.hotpot/prompts/output-language.md`（Claude/OpenCode）或 `$ROOT_DIR/.hotpot/prompts/output-language.md`（Codex/Pi——thin shell 显式列出该路径替换）引用。结构性锚点（CLI flag、JSON 字段、`ACTIVE_CONFLICT:`、markdown 章节标题、`tdd: true|false`、kebab-case slug）无论 language 取何值都**必须保持英文**。
- `hotpot task create` 只追加 `overview.jsonl` 一行；**不会**创建任务的 `<time>-<title>.md` 文件。`.md` 的创建归 `/hotpot:new` slash command 负责，通过平台的「创建文件」工具完成（Claude `Write`、OpenCode `write`、Codex `apply_patch *** Add File`、Pi `write`）。slash command prompt 必须显式禁止「先 Read 探测再 Write」——`task create` 之后该路径不存在是正常态，不是错误。CLI 会在 `task create` 中尽力 `create_dir_all` `<workspace>/tasks/` 作为兜底（非致命副作用），但 slash command 不能把这点当成契约依赖。
- Pi 不再使用 prompt template thin shell 承接 slash commands。新增 Hotpot Pi slash command 时必须在 `assets/platforms/pi/extensions/hotpot/index.ts` 里调 `pi.registerCommand` 注册，并通过共享 helper `buildPiCommandMessage` 拼装 user-voice 消息（分隔块 + framing + workflow prompt 绝对路径 + `@.hotpot/prompts/*` 替换表 + Platform note + 空参数 Exception）。handler 的 `@path` 替换表必须与共享 workflow body 保持同步——`assets/prompts/hotpot-*.md` 增删 `@.hotpot/prompts/<name>.md` 引用时，Pi handler 的 `atPathRefs` 也要同步增删。任何新废弃的 Pi 资产路径都必须加入 `src/assets/platforms/pi.rs::cleanup_deprecated_pi_prompts` 列表，使 `hotpot init` / `hotpot update` 能幂等清理。`buildPiCommandMessage` 的消息体必须把用户输入块（`<<< userInputLabel >>>`）放在**第一段**——在 first-tool-call 指令之前；这种"用户输入前置"顺序是 Pi 第三档失败模式（用户消息体 attention 丢失，详见 `docs/platforms/pi.md`）的对策。每个 handler 还必须在调 `pi.sendUserMessage` **之前**立即赋值闭包级 `pendingWorkflow` 字段（位于 `assets/platforms/pi/extensions/hotpot/index.ts`），下一次 `pi.on("context", ...)` 事件才能注入 per-turn system 强提醒消息——重述 workflow 路径、用户输入块分隔符、FORBIDDEN 行为列表。每个 handler 还必须用同一组 command、workflow prompt path、user-input label arm `pendingFirstToolGuard`；现有唯一 `pi.on("tool_call", ...)` hook 必须先判定这个运行时 first-tool guard，再执行 bash env 注入：非 workflow 首调返回 `{ block: true, reason }` 并保持 armed，只有精确 workflow `read` 才清空 guard。第四档 Pi failure mode 已证明 prompt-only 防线可能失效，因此 runtime guard 是承载约束的关键层。新增 Pi slash command 时四处都要同步扩展：handler 注册、`buildPiCommandMessage` 调用（含正确的 `ideaBlockLabel` / `atPathRefs`）、`pendingWorkflow` 赋值、`pendingFirstToolGuard` 赋值。
