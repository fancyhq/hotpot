# Fix Codex VuePress New And Stop

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| Done | true | 4 | high |
:::

---

## Task

### Summary

::: info
修复 Codex 平台下 VuePress 集成的两个连锁问题：`hotpot-new` 必须在写任务文件前就确定 VuePress 格式，避免先创建普通 Markdown 再回头改写；`hotpot-execute` 入口调用 `hotpot vuepress stop --if-running` 后必须可靠释放由 `hotpot-new` 启动的 VuePress dev server，避免用户继续手动 `lsof` 和 `kill`。
:::

### User Request

用户报告两个问题：

1. 在 Codex 中使用 `hotpot-new` 时，Codex 会先创建普通 Markdown 任务文件，然后发现启用了 VuePress，又回头修改这个 Markdown 任务文件；需要优化为一次性写对。
2. 在 Codex 启动 VuePress 服务后，执行 `hotpot-execute` 时没有关闭 VuePress 服务，用户还需要手动通过 `lsof` 找到 `pid` 并通过 `kill` 关闭；需要排查并修复服务生命周期。

用户已确认按推荐方案创建本任务：启用 TDD，并使用 `git-worktree: true`。

### Approved Design

::: tip
本任务同时修复 prompt 编排和 Rust 服务停止逻辑。

`hotpot-new` 侧采用前置决策方案：共享 prompt 必须在写任务文件之前完成 VuePress gate，读取 `vuepress-style.md`，并要求 Codex 使用 `apply_patch Add File` 一次性写入最终 VuePress 版本。Codex skill 需要补充平台专属防漂移说明，避免模型先按普通 Markdown 完成任务文件再做二次修改。

`hotpot vuepress stop` 侧采用 CLI 兜底方案：不能只信任 runtime 里记录的父进程 `pid`。Unix 上 `start` 通过 `setsid()` 让子进程成为新 session leader，因此 stop 应优先按进程组终止，必要时再按端口或 runtime 端口做受限清理；Windows 继续保留 `taskkill /T` 的进程树语义。`status` 的过期清理也应复用同样的停止能力，避免 TTL 清理只杀父 pid。

架构文档需要同步更新，因为本任务改变了 `/hotpot:new` 的 VuePress 写入顺序以及 VuePress 服务生命周期防线。
:::

### Alternatives Considered

- 只修改 `.hotpot/prompts/hotpot-new.md` 当前安装副本：能立即改善本仓库体验，但不修复源资产，其他项目或后续 `hotpot update` 仍会复发，因此不采用。
- 只修改 prompt，不动 `src/vuepress.rs`：能减少任务文件二次改写，但不能解决服务残留；用户仍可能需要手动 `lsof` / `kill`，因此不采用。
- 只在 `hotpot-execute.md` 里增加更多停止说明：如果 CLI 的 `stop` 不可靠，prompt 重复调用也只是重复失败，不能成为根修复。
- 采纳方案：源 prompt、当前安装 prompt、Codex skill、Rust VuePress lifecycle、架构文档一起更新，并用测试覆盖关键行为。

### Requirements

- `hotpot-new` 在 VuePress 启用时必须先读取 VuePress 写作规范，再创建任务文件。
- Codex 的 `hotpot-new` skill 必须明确禁止“先普通 Markdown，再 VuePress 改写”的两阶段写法。
- `assets/prompts/hotpot-new.md` 与 `.hotpot/prompts/hotpot-new.md` 必须保持本次关键行为一致。
- `hotpot vuepress stop --if-running` 必须在 runtime pid 存活、父 pid 已死但端口仍被 VuePress 子进程占用、TTL 过期清理等场景下尽力释放 dev server。
- Unix 停止逻辑必须考虑 `setsid()` 后的进程组；Windows 必须保留进程树终止能力。
- 所有新增或修改的 Rust 函数、重要 helper 和文件级说明必须包含中英双语 doc comments。
- CLI 输出仍使用 English。
- 修改执行流后必须更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`。

### Non-Goals

::: details Non-Goals
- 不重新设计 VuePress hub 的目录结构。
- 不替换 VuePress、Vite、pnpm 或主题依赖。
- 不新增后台常驻 watchdog 或轮询 daemon。
- 不改变 `vuepress.runtime.json` schema，除非测试证明没有 schema 变更无法可靠修复；若必须变更，需要在报告中明确说明兼容影响。
- 不把 Codex 的 `Stop` hook 当作 SessionEnd 使用；`docs/platforms/codex.md` 已说明 Codex 没有合适的 session-close 事件。
- 不把任务文件写入职责迁移到 Rust CLI；本任务只修复 prompt 编排和服务生命周期。
:::

### Project Context

- `docs/ARCH.md` 说明 VuePress 是 opt-in，并且 `/hotpot:new` 在 VuePress 启用时会走 `.hotpot/prompts/vuepress.md` 的收尾流程。
- `.hotpot/prompts/hotpot-new.md` 当前的 `## Optional: VuePress Integration` 位于写文件规则之后，虽然要求写前读取 `vuepress-style.md`，但 Codex 容易先按通用模板完成普通 Markdown，再发现可选分支并改写。
- `.codex/skills/hotpot-new/SKILL.md` 是 Codex 的薄壳，负责指向 `$HOTPOT_NEW_PROMPT` 并补充 Codex 路径替换说明；这里适合添加 Codex 专属的写文件顺序约束。
- `src/vuepress.rs::start` 在 Unix 使用 `pre_exec(setsid)`，在 Windows 使用 `CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`，并把 `child.id()` 写入 `.hotpot-hub/vuepress.runtime.json`。
- `src/vuepress.rs::stop` 当前读取 runtime pid，调用 `terminate_pid(state.pid, false)`，等待 3 秒后再 `terminate_pid(state.pid, true)`，最后删除 runtime；Unix 的 `terminate_pid` 当前是 `kill <pid>`，不是 `kill -<pgid>`。
- `hotpot-execute.md` 已要求入口无条件运行 `hotpot vuepress stop --if-running`；如果执行后仍残留，主要修复点应在 `src/vuepress.rs` 的停止语义。

---

## Plan

### Mode

- tdd: true

### Execution Strategy

- git-worktree: true
- rationale: 本任务会同时修改共享 prompt、Codex skill、Rust 服务生命周期和架构文档。隔离 worktree 可以降低服务测试和 prompt 资产修改对主 checkout 的干扰。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/hotpot-new.md` | Modify | 调整 VuePress gate 的位置和强度，要求写任务文件前完成格式决策 |
| `.hotpot/prompts/hotpot-new.md` | Modify | 同步当前项目已安装 prompt，让本仓库后续 Codex `hotpot-new` 立即生效 |
| `.codex/skills/hotpot-new/SKILL.md` | Modify | 添加 Codex 专属约束，禁止先普通 Markdown 再 VuePress 改写 |
| `src/vuepress.rs` | Modify | 增强 stop/status 的进程树、进程组和端口兜底清理 |
| `src/commands/vuepress.rs` | Modify | 如需暴露或记录 stop 行为变化，调整 CLI 参数说明或 doc comments |
| `docs/ARCH.md` | Modify | 更新 `/hotpot:new` VuePress 写入顺序和服务生命周期说明 |
| `docs/ARCH.zh_CN.md` | Modify | 同步中文架构文档 |
| `docs/platforms/codex.md` | Modify | 如 Codex 生命周期说明需要补充，更新 Codex 平台文档 |
| `tests` or inline Rust tests in `src/vuepress.rs` | Test | 覆盖停止逻辑和 prompt 关键指令 |

### Implementation Tasks

#### Task 1: 固化 `hotpot-new` 的 VuePress 写入顺序

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/hotpot-new.md` | Modify | 把 VuePress gate 明确提升为写 task file 前置步骤 |
| `.hotpot/prompts/hotpot-new.md` | Modify | 同步当前已安装 prompt |
| `.codex/skills/hotpot-new/SKILL.md` | Modify | Codex 专属约束一次性 `Add File` 最终内容 |

##### Red

- [ ] R1: 在合适的 Rust inline test 或脚本化验证位置新增文本断言测试，测试名使用 `vuepress_new_prompt_requires_prewrite_style_gate`，读取 `assets/prompts/hotpot-new.md` 并断言 `## Optional: VuePress Integration` 明确要求在创建任务文件前完成 VuePress gate 与读取 `vuepress-style.md`。
- [ ] R2: 在同一测试或相邻测试中新增 `codex_new_skill_forbids_two_phase_vuepress_task_write`，读取 `.codex/skills/hotpot-new/SKILL.md` 并断言包含禁止先写普通 Markdown 再改写 VuePress 的 Codex 专属说明。
- [ ] R3: 分别运行 `cargo test vuepress_new_prompt_requires_prewrite_style_gate` 和 `cargo test codex_new_skill_forbids_two_phase_vuepress_task_write`；预期失败，失败原因应指向缺少新的 prompt / skill 约束文本。

##### Green

- [ ] G1: 修改 `assets/prompts/hotpot-new.md`，把 VuePress 探测和 `vuepress-style.md` 读取要求写成写任务文件之前的硬前置顺序，并保留文件存在 gate 的跨平台依据。
- [ ] G2: 修改 `.codex/skills/hotpot-new/SKILL.md`，说明 Codex 在 VuePress enabled 时必须先读取 style prompt，再用 `apply_patch` 的 `*** Add File` 一次性写最终 VuePress 任务文件，禁止先创建普通 Markdown 后二次修改。
- [ ] G3: 同步修改 `.hotpot/prompts/hotpot-new.md`，保持当前项目安装副本与源 prompt 的关键行为一致。
- [ ] G4: 分别运行 `cargo test vuepress_new_prompt_requires_prewrite_style_gate` 和 `cargo test codex_new_skill_forbids_two_phase_vuepress_task_write`；预期通过。

##### Refactor

- [ ] F1: 检查新增 prompt 文案是否重复、是否与 `Writing The Task File` 章节冲突；如重复，合并为单一权威顺序。
- [ ] F2: 运行 `diff -u assets/prompts/hotpot-new.md .hotpot/prompts/hotpot-new.md`；预期除安装副本允许的上下文差异外，本次 VuePress 关键段落无差异。若存在非预期差异，修正后重跑 Task 1 测试。

:::

#### Task 2: 增强 VuePress stop 的进程树和端口兜底

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/vuepress.rs` | Modify | 修复 runtime pid 不足以释放 VuePress 子进程的问题 |
| `src/commands/vuepress.rs` | Modify | 如有必要，更新 stop CLI 说明 |
| `src/vuepress.rs` tests | Test | 覆盖 process group、stale runtime 和 port fallback |

##### Red

- [ ] R1: 在 `src/vuepress.rs` 测试模块新增 `stop_uses_process_group_on_unix_when_runtime_pid_is_alive`。在 Unix cfg 下断言 stop 终止逻辑会针对 runtime pid 的 process group 发出 graceful 和 forceful 终止意图；在非 Unix cfg 下测试应跳过或断言 Windows 仍使用 tree kill helper。
- [ ] R2: 新增 `stop_if_running_cleans_runtime_when_pid_dead_but_port_has_vuepress_owner`，通过可测试 helper 模拟 runtime pid 已死但 runtime port 仍有可识别 VuePress/Vite/pnpm 进程占用，断言 `stop_if_running` 会调用端口兜底清理并删除 runtime。
- [ ] R3: 新增 `status_expired_ttl_reuses_stop_cleanup`，模拟 TTL 过期且进程仍存在，断言 status 不再只调用单 pid terminate，而是复用与 stop 一致的清理路径。
- [ ] R4: 分别运行 `cargo test stop_uses_process_group_on_unix_when_runtime_pid_is_alive`、`cargo test stop_if_running_cleans_runtime_when_pid_dead_but_port_has_vuepress_owner` 和 `cargo test status_expired_ttl_reuses_stop_cleanup`；预期失败，失败原因应暴露当前只杀 runtime pid 或缺少端口兜底。

##### Green

- [ ] G1: 在 `src/vuepress.rs` 中抽出可测试的停止计划 helper，例如按平台返回 `ProcessCleanupPlan`，并补中英双语 doc comments。
- [ ] G2: Unix 侧优先终止 `setsid()` 创建的 process group，使用 `kill -TERM -<pid>` 和必要时 `kill -KILL -<pid>`；保留单 pid 兜底以处理异常平台行为。
- [ ] G3: Windows 侧保留 `taskkill /PID <pid> /T` 和 forceful `/F` 行为，避免回归现有进程树终止能力。
- [ ] G4: 为 runtime port 增加受限兜底：仅在 runtime 存在、端口来自 Hotpot runtime、且能识别为本 hub 的 VuePress/Vite/pnpm 相关进程时才终止；不能误杀任意占用同端口的用户进程。
- [ ] G5: 让 `stop`、`stop_if_running`、readiness 失败清理、TTL 过期的 `status` 分支复用同一清理 helper。
- [ ] G6: 运行 Task 2 的三个精确测试；预期通过。
- [ ] G7: 运行 `cargo test vuepress`；预期 VuePress 相关测试全部通过。

##### Refactor

- [ ] F1: 检查 `src/vuepress.rs` 中新增 helper 的命名、错误处理和注释，确保 CLI 输出仍为 English，内部注释保持中英双语。
- [ ] F2: 运行 `cargo test vuepress` 和 `cargo fmt --check`；预期通过。若格式检查失败，运行 `cargo fmt` 后重跑测试。

:::

#### Task 3: 更新架构和平台文档

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify | 记录新建任务文件前置 VuePress gate 与 stop 清理语义 |
| `docs/ARCH.zh_CN.md` | Modify | 同步中文说明 |
| `docs/platforms/codex.md` | Modify | 如需补充 Codex 无 SessionEnd 下的 stop 兜底，更新平台文档 |

##### Red

- [ ] R1: 新增或扩展文本断言测试 `arch_documents_vuepress_prewrite_gate_and_process_cleanup`，读取 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md`，断言两份文档都说明 `/hotpot:new` 在写任务文件前完成 VuePress style gate，并说明 `stop` 使用 runtime pid、进程组或进程树、TTL/端口兜底等防线。
- [ ] R2: 运行 `cargo test arch_documents_vuepress_prewrite_gate_and_process_cleanup`；预期失败，失败原因应指向架构文档缺少新语义。

##### Green

- [ ] G1: 更新 `docs/ARCH.md` 的 `/hotpot:new` 命令说明、VuePress Integration 的 service lifecycle 段和必要的 Design Principles。
- [ ] G2: 用简体中文同步更新 `docs/ARCH.zh_CN.md`，内容与英文版语义一致。
- [ ] G3: 检查 `docs/platforms/codex.md` 中 Codex 无 SessionEnd 的说明；如当前文本仍准确，只补充 `hotpot vuepress stop --if-running` 现在包含 CLI 侧强清理语义；如无需修改，在执行报告中说明。
- [ ] G4: 运行 `cargo test arch_documents_vuepress_prewrite_gate_and_process_cleanup`；预期通过。

##### Refactor

- [ ] F1: 对比 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md` 的 VuePress 章节，确认顺序和信息密度一致。
- [ ] F2: 运行 `rg -n "VuePress|vuepress|SessionEnd|ttl|process group|进程组" docs/ARCH.md docs/ARCH.zh_CN.md docs/platforms/codex.md`，确认没有旧结论与新实现矛盾。

:::

#### Task 4: 端到端验证 Codex 流程与服务释放

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `assets/prompts/hotpot-new.md` | Test | 验证 prompt 顺序最终符合 Codex 使用方式 |
| `.codex/skills/hotpot-new/SKILL.md` | Test | 验证 Codex skill 约束存在 |
| `src/vuepress.rs` | Test | 验证 start/stop/status 生命周期 |
| `.hotpot-hub/vuepress.runtime.json` | Test | 运行时验证 stop 后被删除 |

##### Red

- [ ] R1: 在实现完成前记录当前端到端复现命令：`cargo run -- vuepress status`、`cargo run -- vuepress start --port 8080`、`cargo run -- vuepress stop --if-running`、`cargo run -- vuepress status`。若当前环境已有服务运行，先只执行 `status` 并记录状态，不要误杀用户进程。
- [ ] R2: 如能安全启动 VuePress，运行 `cargo run -- vuepress start --port 8080` 后用 `lsof -i :8080` 或 macOS/Linux 等价命令记录实际持有端口的进程树；再运行 `cargo run -- vuepress stop --if-running`。旧实现可能留下端口占用；若无法复现，也要保留命令输出作为基线。

##### Green

- [ ] G1: 运行 `cargo test`；预期全部 Rust 测试通过。
- [ ] G2: 运行 `cargo run -- vuepress start --port 8080`；预期命令返回单行 JSON，包含 `url` 和 `pid`，且 `.hotpot-hub/vuepress.runtime.json` 存在。
- [ ] G3: 运行 `cargo run -- vuepress stop --if-running`；预期返回成功并删除 `.hotpot-hub/vuepress.runtime.json`。
- [ ] G4: 运行 `cargo run -- vuepress status`；预期输出 `{"running":false}`。
- [ ] G5: 在 macOS/Linux 上运行 `lsof -i :8080`；预期没有 Hotpot VuePress/Vite/pnpm 进程继续占用该端口。如果端口被非 Hotpot 用户进程占用，不要 kill，记录并说明。
- [ ] G6: 检查新建任务文件 prompt 文案：`rg -n "BEFORE|before writing|vuepress-style|Add File|普通 Markdown|VuePress" assets/prompts/hotpot-new.md .hotpot/prompts/hotpot-new.md .codex/skills/hotpot-new/SKILL.md`；预期能看到写前 gate 和 Codex 一次性 `Add File` 约束。

##### Refactor

- [ ] F1: 运行 `cargo fmt --check`；预期通过。
- [ ] F2: 运行 `git diff --check`；预期无尾随空格或 whitespace error。
- [ ] F3: 检查 `git diff -- src/vuepress.rs src/commands/vuepress.rs assets/prompts/hotpot-new.md .codex/skills/hotpot-new/SKILL.md docs/ARCH.md docs/ARCH.zh_CN.md docs/platforms/codex.md`，确认没有 unrelated refactor。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo test vuepress_new_prompt_requires_prewrite_style_gate` and `cargo test codex_new_skill_forbids_two_phase_vuepress_task_write` | prompt 与 Codex skill 顺序约束测试通过 |
| `cargo test stop_uses_process_group_on_unix_when_runtime_pid_is_alive`, `cargo test stop_if_running_cleans_runtime_when_pid_dead_but_port_has_vuepress_owner`, and `cargo test status_expired_ttl_reuses_stop_cleanup` | VuePress stop 清理语义测试通过 |
| `cargo test arch_documents_vuepress_prewrite_gate_and_process_cleanup` | 架构文档同步测试通过 |
| `cargo test vuepress` | VuePress 相关测试通过 |
| `cargo test` | 全量 Rust 测试通过 |
| `cargo fmt --check` | 格式检查通过 |
| `git diff --check` | 无 whitespace error |
| `cargo run -- vuepress start --port 8080` | 返回单行 JSON，包含 `url` 和 `pid` |
| `cargo run -- vuepress stop --if-running` | 返回成功，释放 Hotpot VuePress dev server 并删除 runtime |
| `cargo run -- vuepress status` | 输出 `{"running":false}` |
| `lsof -i :8080` | 没有 Hotpot VuePress/Vite/pnpm 进程残留；非 Hotpot 进程只记录不终止 |

### Risks and Watchouts

::: warning
- Unix 上负 pid kill 表示按 process group 发信号，必须确保只对 Hotpot 自己通过 `setsid()` 启动并记录在 runtime 的 pid 使用，避免误伤当前 shell 的进程组。
- 端口兜底清理必须保守，只能处理能确认属于 `.hotpot-hub` 或 Hotpot VuePress 命令线的进程，不能杀任意占用同端口的用户服务。
- `.hotpot/prompts/hotpot-new.md` 是当前项目安装副本，`assets/prompts/hotpot-new.md` 是源资产；两者都要更新，否则当前开发仓库和未来安装行为会分叉。
- `git-worktree: true` 下执行 agent 需要确认 `.hotpot/` 安装副本是否在 worktree 中可见；如果不可见，应修改源资产并在主 checkout 通过安全命令同步安装副本，不能静默跳过当前项目修复。
- 修改 prompt 时不要破坏 `ACTIVE_CONFLICT:`、`tdd: true|false`、`git-worktree: true|false`、`## Task` / `## Plan` / `## Execution Instructions` 等机器可解析锚点。
- 服务验证可能需要占用 8080 端口；如果端口被非 Hotpot 进程占用，不要强杀，改用配置端口或记录 blocker。
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
- Because `tdd: true`, follow Red → Green → Refactor for every `#### Task N` and include failing-test evidence before implementation evidence.
- Because `git-worktree: true`, perform source edits inside the attached Hotpot worktree after `/hotpot:execute` creates or resolves it. Only use main checkout operations when Hotpot commands require main checkout state, and report that distinction clearly.
