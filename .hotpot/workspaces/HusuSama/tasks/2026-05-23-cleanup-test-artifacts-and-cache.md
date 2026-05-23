---
title: 清理测试产物与运行缓存残留
description: 删除已废弃的 issues markdown 测试与仓库根 issues.md，隔离会污染真实仓库的测试，并为临时目录与 brainstorming session 增加自动清理
date: 2026-05-23
category: [Task]
tag: [test, cleanup, cache, server, tempfile]
---

# 清理测试产物与运行缓存残留

## Task

### Summary

当前仓库仍存在一组会留下测试/运行残留的行为：部分测试直接在真实仓库根上运行，可能写出 `.hotpot/workspaces/test*` 与根目录 `issues.md`；多处测试 helper 用手写 `env::temp_dir()` 目录且不自动清理；`hotpot server stop` 仅删除 `server.pid`，不会清掉 `.hotpot/brainstorm/<session>` 下的运行缓存。本任务要删除已无价值的 `test_render_issues_to_markdown` 测试，按用户确认删除仓库根已跟踪的 `issues.md`，把其余相关测试迁移到隔离临时项目根并补齐自动清理，同时让 brainstorming server 停止后自动移除 session 缓存目录；平台模板更新仍保持单文件内容替换/合并，不允许目录级覆盖现有 `.claude`、`.opencode` 等用户目录。

### User Request

> 当前项目中，单测完成后会有很多遗留的测试数据，我希望有一个后置的操作能清理这些数据

后续范围确认与调整：

- 用户先选择清理范围为“测试目录 + 运行缓存”。
- 用户选择清理机制为“自动清理优先”。
- 用户补充要求：清理需包含 `.hotpot/workspaces` 下测试创建的 `test` / `test*` 文件夹、项目根目录的 `issues.md`，并尽量让测试不去覆盖目前已存在的 `.claude`、`.opencode` 等目录；更改模板后需要保持“只替换单个文件内容”的语义。
- 在设计收口阶段，用户进一步要求：`src/issues.rs::test_render_issues_to_markdown` 已经不需要，直接删除该测试而不是改造成隔离测试。
- 额外边界确认：仓库根 `issues.md` 当前是 git 已跟踪文件而非未跟踪垃圾文件；在被提醒该事实后，用户仍明确选择将其删除纳入本任务范围。
- TDD 选择：用户确认使用 `tdd: false`。

### Approved Design

本任务采用“隔离 + RAII 自动清理 + 停止时回收 session”的组合方案。

1. 删除已无业务价值且会污染真实仓库根的 `src/issues.rs::test_render_issues_to_markdown`。
2. 按用户显式确认，删除仓库根已跟踪的 `issues.md` 文件，不再把它当作测试输出目标保留。
3. 将仍会直接使用真实仓库根的测试迁移到独立临时项目根，尤其是使用 `resolve_root_dir(None)` 且配合 `test` / `test*` username 的测试，避免继续在当前仓库的 `.hotpot/workspaces/` 下落盘。
4. 将现有手写 `env::temp_dir().join("hotpot-...")` 的测试目录 helper 改为 RAII 持有的 `tempfile::TempDir` 或等价自动清理语义，覆盖目前已知的 `src/lock.rs`、`src/context.rs`、`src/issues.rs`、`src/task/mod.rs`、`src/commands/update.rs`、`src/vuepress.rs` 等测试位置。
5. 增强 `hotpot server stop` / `stop --all`：在停止对应 server 进程后，自动删除整个 `.hotpot/brainstorm/<session>` 目录；若 session 目录已成孤儿且不再有可停止进程，也应以幂等方式清理残留缓存文件（`content/*.html`、`state/server.pid`、`state/server-info`、`state/events` 等）。
6. 平台模板安装/更新语义保持现状：主配置文件继续走 merge，Hotpot 私有资产继续逐文件处理，不引入目录级覆盖；测试也必须在隔离项目根验证，不能拿当前仓库内真实 `.claude`、`.opencode` 目录当夹具直接写入。

这个方案把“止血新残留”和“回收运行缓存”放在首位；对白名单历史残留只做有限处理，不扩展为新的手动 cleanup CLI，也不改 `.hotpot-hub/` 的卸载语义。

### Alternatives Considered

- 只把测试临时目录改成 `TempDir`，不处理真实仓库根污染与运行缓存：能减少 `%TEMP%` 残留，但无法解决 `.hotpot/workspaces/test*`、`issues.md` 以及 `.hotpot/brainstorm/<session>` 残留。未采纳。
- 保留 `test_render_issues_to_markdown`，仅把它迁移到隔离目录：技术上可行，但用户已明确表示该测试“不需要这个测试项目了”，直接删除更小更正确。未采纳。
- 增加一个显式手动 cleanup CLI：能扫历史残留，但用户已选择“自动清理优先”，而且当前问题主要是测试/stop 生命周期设计不当。未采纳。
- 推荐并已批准的方案：删除无用测试与 `issues.md`，隔离会污染真实仓库根的测试，补 RAII 自动清理，并在 `server stop` 时自动移除 session 缓存目录。

### Requirements

- 删除 `src/issues.rs::test_render_issues_to_markdown`。
- 删除仓库根 `issues.md` 文件。
- 修复会把测试输出写到真实仓库 `.hotpot/workspaces/test*` 的测试，改为在隔离临时项目根中运行。
- 将当前已发现的测试临时目录 helper/内联创建点改为自动清理，至少覆盖 `src/lock.rs`、`src/context.rs`、`src/issues.rs`、`src/task/mod.rs`、`src/commands/update.rs`、`src/vuepress.rs` 中相关测试。
- `hotpot server stop --session-dir <dir>` 与 `hotpot server stop --all` 在成功停止后都必须清除对应 `.hotpot/brainstorm/<session>` 目录残留；重复执行应保持幂等。
- 若 `session_dir` 已存在但 `server.pid` 缺失或已不可用，清理逻辑应以“孤儿 session 残留”语义继续做 best-effort 清理，而不是仅返回 `Ok(())` 后留下目录。
- 平台模板更新仍保持单文件语义：`.claude/settings.json`、`.opencode/package.json` 等主配置继续 merge，其余 Hotpot 资产继续逐文件写入；不新增目录级覆盖实现。
- 测试/验证中必须覆盖“不会误伤现有 `.claude`、`.opencode` 目录内容”的边界，测试夹具应使用临时项目根。
- 所有新增/修改的函数、测试 helper、以及意图不明显的关键逻辑，继续遵守 `AGENTS.md` 要求的中英双语 doc comment / 说明风格。

### Non-Goals

- 不新增单独的手动 cleanup CLI。
- 不改变 `.hotpot-hub/` 的安装/卸载语义；本任务只处理 brainstorming session 运行缓存目录。
- 不批量删除除用户显式点名范围外的项目持久数据（例如 `.hotpot/issues.jsonl`、`.hotpot/overview.jsonl`、普通用户 workspace 数据）。
- 不改成目录级模板替换；平台资产仍按当前单文件安装/合并模型工作。
- 不扩展到与本问题无关的生产目录创建逻辑或 worktree 逻辑。

### Project Context

已确认的现状与落点如下：

- `src/issues.rs::test_render_issues_to_markdown` 当前调用 `resolve_root_dir(None)`，然后把 `render_issues_to_markdown(&root_dir)` 的结果直接写到 `format!("{}/issues.md", root_dir)`；这正是仓库根 `issues.md` 被触碰的来源。
- 仓库根当前确实存在 `issues.md`，并且 `git ls-files --error-unmatch issues.md` 证明该文件已被 git 跟踪；因此删除它是一次显式范围决定，不是随手清未跟踪垃圾。
- `src/task/mod.rs` 内有多处测试直接 `resolve_root_dir(None)` 并使用 `username = "test"` 或 `test_*`，这些测试会在当前仓库 `.hotpot/workspaces/<username>/` 下留下真实工作区数据。另有部分测试已经使用 `make_isolated_project_dir()`，但该 helper 仍是手写 tempdir 且未自动清理。
- `src/issues.rs::unique_issue_root`、`src/context.rs::unique_root`、`src/task/mod.rs::make_isolated_project_dir`、`src/commands/update.rs::temp_project_dir`、`src/vuepress.rs::temp_vuepress_root`、`src/lock.rs::temp_data_path` 等测试 helper 仍使用 `std::env::temp_dir()` 手工建目录，生命周期结束后不会自动删除。
- `src/assets/platforms/pi.rs` 已存在自清理的 `ScopedTempDir`，说明该仓库接受“测试作用域结束自动清理”的模式；本任务可参考这种目标，但优先复用 `tempfile::TempDir` 的标准语义。
- `src/assets/mod.rs` 当前安装策略已经区分：Hotpot 私有资产为 `InstallStrategy::Owned` 的逐文件写入，`.claude/settings.json` / `.opencode/package.json` 等主配置走 `MergeJson`，`toml`/文本亦有对应 merge 策略；仓库并不存在“整目录覆盖”安装器。因此本任务要防的是测试污染真实目录，而不是重写安装模型。
- `src/server.rs::stop()` 当前 `--all` 只是遍历 `.hotpot/brainstorm/*` 并对每个目录调用 `stop_session()`；`stop_session()` 现状只在 kill 成功后删除 `state/server.pid`，不会删除整个 `session_dir`，如果 `server.pid` 缺失更是直接 `Ok(())`，导致 `content/`、`state/server-info`、`state/events` 等残留。
- 当前工作树存在与本任务无关的已有改动：`README.zh_CN.md` 已修改。执行时必须避免误回滚或误纳入。
- 已存在旧任务 `.hotpot/workspaces/HusuSama/tasks/2026-05-17-auto-cleanup-test-tempdirs.md`，其中记录了 TempDir 改造的早期计划；本任务应吸收其有效上下文，但范围更大，且新增了删除 `test_render_issues_to_markdown`、删除 `issues.md`、测试隔离、以及 `server stop` session 清理。

## Plan

### Mode

- tdd: false

### File Map

- Modify: `Cargo.toml` - 如仓库尚未登记 `tempfile` 测试依赖，则新增 `[dev-dependencies] tempfile = "3"`，为统一自动清理 helper 提供支撑。
- Modify: `src/issues.rs` - 删除 `test_render_issues_to_markdown`，并清理/重构 issues 模块中其余临时目录测试辅助逻辑。
- Modify: `src/task/mod.rs` - 隔离当前会写真实 `.hotpot/workspaces/test*` 的测试，并让临时项目根自动清理。
- Modify: `src/context.rs` - 将测试 helper 改为自动清理的临时目录语义。
- Modify: `src/lock.rs` - 将 `temp_data_path` 一类 helper 改为持有自动清理句柄的形式。
- Modify: `src/commands/update.rs` - 将 update 测试项目根切换到自动清理的隔离目录。
- Modify: `src/vuepress.rs` - 将 VuePress 测试临时根改为自动清理语义。
- Modify: `src/server.rs` - 增加/重构 stop cleanup 逻辑，并补对应测试覆盖。
- Modify: `issues.md` - 删除该已跟踪文件，作为用户显式批准的任务交付内容。
- Test: `src/issues.rs` - 验证删除测试后 issues 模块仍可通过，且不再向真实仓库根写 markdown 文件。
- Test: `src/task/mod.rs` - 验证 task 测试在隔离根运行，不再污染真实 `.hotpot/workspaces/test*`。
- Test: `src/server.rs` - 新增 stop/session 清理测试，覆盖正常停止与孤儿 session 场景。

### Implementation Tasks

#### Task 1: 删除根污染测试与已跟踪 `issues.md`

**Files:**

- Modify: `src/issues.rs`
- Modify: `issues.md`

- [x] Step 1: 在 `src/issues.rs` 中删除 `test_render_issues_to_markdown`，同时检查该测试独有的辅助导入/注释是否可一并清理，避免留下死代码或误导性说明。
- [x] Step 2: 删除仓库根 `issues.md` 文件。执行时注意这是用户显式批准的范围，而不是清未跟踪垃圾。
- [x] Step 3: 运行 `cargo test issues::tests`，预期 issues 模块测试通过，且不会再向当前仓库根写出 `issues.md`。
- [x] Step 4: 运行 `git status --short --untracked-files=all`，确认该部分变更表现为 `src/issues.rs` 修改与 `issues.md` 删除，不应出现新的根目录测试垃圾文件。

#### Task 2: 隔离会污染真实 `.hotpot/workspaces/test*` 的 task 测试

**Files:**

- Modify: `src/task/mod.rs`

- [x] Step 1: 盘点 `src/task/mod.rs` 中所有使用 `resolve_root_dir(None)` 且配合 `username = "test"` / `test_*` 的测试，把它们迁移到隔离临时项目根，而不是当前仓库根。
- [x] Step 2: 优先复用或重构已有 `make_isolated_project_dir` helper，让其返回可自动清理的临时目录句柄；调用侧必须持有句柄直到断言结束，避免目录过早 Drop。
- [x] Step 3: 对仍需要真实字面量 username（例如 `"default"` 协作者保护测试）的场景，继续保持“独立临时项目根 + 局部 username 语义”的测试设计，不退回真实仓库根。
- [x] Step 4: 运行 `cargo test task::tests`，预期 task 模块测试全绿。
- [x] Step 5: 在测试完成后检查当前仓库 `.hotpot/workspaces/` 下是否新增 `test` / `test*` 目录；预期不新增。如果工作区当前已经存在这类目录，记录来源并在实现末尾一并清理。

#### Task 3: 统一测试临时目录为自动清理语义

**Files:**

- Modify: `Cargo.toml`
- Modify: `src/lock.rs`
- Modify: `src/context.rs`
- Modify: `src/issues.rs`
- Modify: `src/task/mod.rs`
- Modify: `src/commands/update.rs`
- Modify: `src/vuepress.rs`

- [x] Step 1: 确认 `Cargo.toml` 是否已有 `tempfile` 的测试依赖；若没有，新增 `[dev-dependencies] tempfile = "3"`。
- [x] Step 2: 逐一把当前手写 `env::temp_dir().join(...)` 的测试 helper 或内联创建逻辑迁移到 `tempfile::TempDir` 或等价 RAII 语义，至少覆盖已确认的 `src/lock.rs`、`src/context.rs`、`src/issues.rs`、`src/task/mod.rs`、`src/commands/update.rs`、`src/vuepress.rs`。
- [x] Step 3: 对返回路径而不是目录句柄的 helper（例如 `temp_data_path` 一类），改成返回“句柄 + 业务路径”的组合，或让调用侧显式持有 `TempDir`，避免 `_` 立即 Drop 导致目录提前删除。
- [x] Step 4: 跑最小定向回归：`cargo test lock::tests context::tests issues::tests task::tests commands::update::tests vuepress::tests`；预期无失败。
- [x] Step 5: 如某些 helper 签名从 `&PathBuf` 收窄为 `&Path`，同步调整调用点并补中英双语 doc comment，确保意图清晰。

#### Task 4: 为 `hotpot server stop` 增加 session 目录自动清理

**Files:**

- Modify: `src/server.rs`

- [x] Step 1: 在 `src/server.rs` 设计并实现一个幂等的 session 清理路径：正常停止时，kill 成功后删除 `server.pid` 并移除整个 `session_dir`；孤儿 session（`server.pid` 缺失、空文件、无效 pid、或进程已不在）也要做 best-effort 残留清理，但不能误删仍在运行且 stop 失败的 live session。
- [x] Step 2: 保持 `stop --all` 语义为遍历 `.hotpot/brainstorm/*`，但每个目录都应复用同一套 stop + cleanup 逻辑，避免两套分支出现不一致。
- [x] Step 3: 在 `src/server.rs` 增加 `#[cfg(test)]` 测试模块或等价测试入口，至少覆盖：正常 session 清理、缺失 `server.pid` 的孤儿目录清理、重复 stop 的幂等性。
- [x] Step 4: 运行 `cargo test server` 或更精确的 server 模块测试命令，预期新增测试通过。

#### Task 5: 守住平台目录“单文件语义，不碰真实配置目录”边界

**Files:**

- Modify: `src/commands/update.rs`
- Modify: `src/assets/mod.rs`
- Modify: 相关测试文件（如需要）

- [x] Step 1: 复核现有安装策略测试，确认 `.claude/settings.json`、`.opencode/package.json` 等仍通过 merge 路径，其余 Hotpot 私有资产仍为逐文件写入；本任务不要把任何代码改成目录级复制/覆盖。
- [x] Step 2: 若需要补测试，使用临时项目根创建最小 `.claude` / `.opende` 夹具，验证 Hotpot 更新只触碰目标单文件，不会重写整目录或依赖当前仓库真实配置目录。
- [x] Step 3: 运行与该边界相关的定向测试，例如 `cargo test commands::update::tests`，预期通过。

#### Task 6: 全量验证与残留审计

**Files:**

- Validation only

- [x] Step 1: 运行 `cargo test`，预期全绿。
- [x] Step 2: 运行 `git status --short --untracked-files=all`，确认只出现本任务预期修改，不应新增新的 `.hotpot/workspaces/test*` 或根目录测试垃圾文件。
- [x] Step 3: 如当前工作区确实存在本任务范围内的历史 `.hotpot/workspaces/test*` 目录，在确认其来源后将其删除，并再次用 `git status --short --untracked-files=all` 验证无新增脏数据。
- [ ] Step 4: 若本机环境允许，额外检查系统 temp 目录中是否还新增了本轮测试留下的 `hotpot-*` 目录；预期不新增或仅存在短暂、可解释的占用残留。

### Validation

- `cargo test` - 全仓测试通过。
- `cargo test issues::tests task::tests lock::tests context::tests commands::update::tests vuepress::tests` - 相关模块定向回归通过。
- `cargo test server` - session stop cleanup 相关测试通过。
- `git status --short --untracked-files=all` - 不出现新的 `.hotpot/workspaces/test*`、根目录 `issues.md` 再生、或其它测试垃圾文件；仅保留预期代码改动与用户既有改动。

### Risks and Watchouts

- `issues.md` 是已跟踪文件：删除它会出现在 git diff 中，必须把这视为用户确认后的正式交付，而不是实现过程中的顺手清理。
- `TempDir`/RAII 句柄必须被持有到断言结束；如果写成单字符 `_` 或过早离开作用域，会让目录在测试运行前就被删除，导致假失败。
- `stop_session` 清理孤儿目录时要避免“kill 失败却仍删目录”这种误伤；只有在能够确认无 live 进程、或根本不存在有效 pid 的场景下才能进入孤儿残留清理分支。
- 当前工作树已有与本任务无关的 `README.zh_CN.md` 改动；执行时不能回滚或混淆这部分既有工作。
- 本任务涉及多个测试模块与一个运行时流程文件，改动分散；实现时应尽量按模块小步推进，并在每步后跑对应最小测试，避免最后一次性排查大面积失败。

## Execution Instructions

- 先执行 Task 1，优先去掉已废弃测试与根 `issues.md` 的污染源。
- Task 2 与 Task 3 强相关：先把 task 测试污染源隔离，再统一其余 tempdir helper，避免在真实仓库根和 `%TEMP%` 两个方向同时留残留。
- Task 4 的 `server stop` 清理逻辑必须单独补测试，不要只靠手工运行验证。
- Task 5 只做边界守护，不做架构翻新；发现现有单文件语义已经满足时，优先补测试或补说明，而不是改安装器核心模型。
- 修改过程中保持复选框进度同步更新，便于后续执行与 review。
- 遇到与现有未提交改动冲突的文件时，先读取并理解上下文，避免覆盖用户已有工作。
