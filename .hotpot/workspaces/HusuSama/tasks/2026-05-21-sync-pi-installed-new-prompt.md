# sync-pi-installed-new-prompt

::: info Overview
| Field | Value |
| --- | --- |
| Status | In Progress |
| TDD | true |
| Tasks | 3 |
| Risk | medium |
:::

## Task

::: info Summary
修复 Pi 本地安装/项目 prompt 与源资产不同步的问题：上一轮修复已经把 `assets/platforms/pi/prompts/hotpot-new.md` 加入 `$ARGUMENTS`，但仓库中 Pi 实际读取的 `.pi/prompts/hotpot-new.md` 仍可能是旧内容，导致 `/hotpot-new ...` 的需求文本没有进入 shared workflow。
:::

::: tip Approved Design
已批准的方案是同步 Pi 项目 prompt，并补充回归验证，确保以后不会只修源资产而遗漏 Pi 实际读取的 `.pi/prompts/hotpot-new.md`。
:::

### Problem

- `assets/platforms/pi/prompts/hotpot-new.md` 已经包含 `$ARGUMENTS` 参数占位和非空/为空处理说明。
- `HEAD:.pi/prompts/hotpot-new.md` 仍是旧版 prompt，缺少 `$ARGUMENTS`，Pi 在本仓库内测试时读取的是 `.pi/prompts/hotpot-new.md`。
- 当前工作区里的 `.pi/prompts/hotpot-new.md` 已有未提交同步改动，因此执行时需要区分“工作区已修好”和“提交基线仍有缺陷”。
- 本任务只处理 Pi `hotpot-new` prompt 同步和验证，不扩展到 execute/finish-work 行为。

### Requirements

- `.pi/prompts/hotpot-new.md` 必须包含 `$ARGUMENTS` 或等价参数占位，并包含把非空命令参数作为 initial task idea 的说明。
- 新增或调整回归验证，覆盖源资产与 Pi 实际读取/安装的 prompt 不再漂移。
- 文档补充开发约束：修改平台 prompt 源资产时，如果仓库内对应平台目录被跟踪并用于本地测试，需要同步或验证实际安装 prompt。
- 验证时要明确记录 TDD Red：`HEAD:.pi/prompts/hotpot-new.md` 缺少 `$ARGUMENTS` 是本次基线缺陷。

::: details Non-Goals
- 不修改 Pi `hotpot-execute` 或 `hotpot-finish-work` prompt。
- 不改变 Pi extension 的 bootstrap 行为。
- 不改变 Claude / OpenCode / Codex 平台 prompt 行为。
- 不重构资产安装系统，除非回归测试必须做最小调整。
- 不随意纳入 `.pi/package.json` 格式化差异；只有当验证路径必须同步它时才纳入，并在结果中说明原因。
:::

## Context

- Hotpot 平台源资产在 `assets/platforms/<platform>/` 下。
- Pi 项目 prompt 安装路径为 `.pi/prompts/hotpot-new.md`，这是本仓库内 Pi 测试会读取的文件。
- `src/assets/platforms/pi.rs` 当前注册 `.pi/prompts/hotpot-new.md` 为 owned asset，并已有 `pi_new_prompt_template_passes_command_arguments` 测试覆盖源资产。
- `src/assets/platforms/mod.rs` 的 `PI_GROUPS` 包含 `pi::ASSETS` 和 `SHARED_ASSETS`。
- `src/assets/mod.rs` 的 owned asset 在目标已存在且不同步时需要 `--force` 才覆盖；dry-run 会打印将要写入的路径。
- 当前工作区在创建任务前已有 `M .pi/prompts/hotpot-new.md` 和 `M .pi/package.json`，执行阶段不要误删用户/已有改动。

## Plan

### Mode

- tdd: true

### File Map

| Path | Role | Expected Change |
| --- | --- | --- |
| `.pi/prompts/hotpot-new.md` | Pi 实际读取的项目 prompt | 同步 `$ARGUMENTS` 参数传递说明 |
| `assets/platforms/pi/prompts/hotpot-new.md` | Pi prompt 源资产 | 通常不需要改；用于对比和回归 |
| `src/assets/platforms/pi.rs` | Pi 资产注册与测试 | 添加/调整同步或安装路径回归测试 |
| `docs/platforms/pi.md` | Pi 平台文档 | 补充源资产与项目 prompt 同步注意事项 |
| `docs/ARCH.md` | 架构文档 | 仅当执行流/架构契约改变时更新 |
| `docs/ARCH.zh_CN.md` | 中文架构文档 | 仅当 `docs/ARCH.md` 需要更新时同步更新 |
| `.pi/package.json` | Pi 本地配置 | 默认不纳入；仅在验证必须同步时说明并处理 |

## Implementation Tasks

### Task 1: Add Regression Coverage

::: info Task body
**Files:**

| Path | Role | Expected Change |
| --- | --- | --- |
| `src/assets/platforms/pi.rs` | 测试位置 | 新增或调整 Pi prompt 同步/安装回归测试 |
| `.pi/prompts/hotpot-new.md` | 对比对象 | 被测试覆盖为实际读取 prompt |
| `assets/platforms/pi/prompts/hotpot-new.md` | 对比对象 | 被测试覆盖为源资产 |

**Red**

- [x] R1: 运行 `git show HEAD:.pi/prompts/hotpot-new.md | rg --color never -F '$ARGUMENTS'` 并记录预期失败，证明提交基线缺陷。
- [x] R2: 确认现有 `pi_new_prompt_template_passes_command_arguments` 只覆盖源资产，不能捕获 `.pi/prompts/hotpot-new.md` 漂移。

- 先运行基线缺陷验证：`git show HEAD:.pi/prompts/hotpot-new.md | rg --color never -F '$ARGUMENTS'`，预期失败，证明提交基线中的 Pi 实际 prompt 缺少参数占位。
- 新增回归测试前，记录当前已有 `pi_new_prompt_template_passes_command_arguments` 只覆盖源资产，不足以发现 `.pi/prompts/hotpot-new.md` 漂移。
- 如果工作区 `.pi/prompts/hotpot-new.md` 已经包含修复导致新测试立即通过，不要回滚用户改动；在执行记录中明确 Red 由 `git show HEAD` 基线验证提供。

**Green**

- [x] G1: 添加最小回归测试，覆盖源资产与 `.pi/prompts/hotpot-new.md` 的参数段落同步。
- [x] G2: 运行新增测试命令并记录通过结果。
- [x] G3: 运行 `cargo test pi_new_prompt_template_passes_command_arguments` 并记录通过结果。

- 添加一个最小回归测试，优先命名为 `pi_prompt_source_and_installed_template_stay_in_sync` 或等价名称。
- 测试应断言源资产与 `.pi/prompts/hotpot-new.md` 都包含 `$ARGUMENTS`，并包含非空参数作为 initial task idea、为空时询问一次的说明。
- 如更合适，也可添加安装路径测试，验证 Pi asset 安装生成的 `.pi/prompts/hotpot-new.md` 包含这些片段。

**Refactor**

- [x] F1: 检查新增测试是否存在不必要抽象或重复，必要时做最小清理。
- [x] F2: 重新运行相关测试，或记录 `no refactor needed`。

- 避免过度抽象；只在测试中提取必要的常量或小 helper。
- 新增 Rust 模块/函数必须有中英双语 doc comment。
:::

### Task 2: Sync Pi Installed Prompt

::: info Task body
**Files:**

| Path | Role | Expected Change |
| --- | --- | --- |
| `.pi/prompts/hotpot-new.md` | Pi 实际读取 prompt | 同步源资产中的 `$ARGUMENTS` 段落 |
| `assets/platforms/pi/prompts/hotpot-new.md` | 源资产 | 仅用于对照，除非发现源资产仍有缺陷 |
| `.pi/package.json` | 现有工作区差异 | 默认不纳入任务补丁 |

**Red**

- [x] R1: 使用 `git show HEAD:.pi/prompts/hotpot-new.md` 证明提交基线缺少 `$ARGUMENTS`。
- [x] R2: 使用 `git show HEAD:assets/platforms/pi/prompts/hotpot-new.md` 证明源资产已包含 `$ARGUMENTS`。

- 使用 `git show HEAD:.pi/prompts/hotpot-new.md` 证明提交基线缺少 `$ARGUMENTS`。
- 使用 `git show HEAD:assets/platforms/pi/prompts/hotpot-new.md` 证明源资产已包含 `$ARGUMENTS`，确认问题是同步漂移。

**Green**

- [x] G1: 将 `.pi/prompts/hotpot-new.md` 与源资产中的参数传递段落同步。
- [x] G2: 运行 `rg --color never -F '$ARGUMENTS' .pi/prompts/hotpot-new.md` 并记录通过结果。
- [x] G3: 确认没有修改 Pi execute/finish-work prompt。

- 将 `.pi/prompts/hotpot-new.md` 与源资产中的 `hotpot-new` 参数传递段落同步。
- 保留 shared workflow 引用和现有 prompt 结构，不改变 execute/finish-work。
- 不主动修改 `.pi/package.json`；若验证命令持续因它产生噪音，只在确认该格式同步属于安装一致性需求后最小纳入，并在任务结果说明。

**Refactor**

- [x] F1: 对比 `.pi/prompts/hotpot-new.md` 和源资产，确认关键参数段落一致。
- [x] F2: 运行 `cargo run -- init --platform pi --force --dry-run` 并记录输出，不实际写入。

- 对比 `.pi/prompts/hotpot-new.md` 和 `assets/platforms/pi/prompts/hotpot-new.md`，确认关键参数段落一致。
- 确认 `cargo run -- init --platform pi --force --dry-run` 输出符合预期且不实际写入。
:::

### Task 3: Document And Validate

::: info Task body
**Files:**

| Path | Role | Expected Change |
| --- | --- | --- |
| `docs/platforms/pi.md` | Pi 平台文档 | 增加源资产和项目 prompt 同步说明 |
| `docs/ARCH.md` | 架构文档 | 仅在架构/执行流变化时更新 |
| `docs/ARCH.zh_CN.md` | 中文架构文档 | 仅在英文架构文档更新时同步 |

**Red**

- [x] R1: 检查 `docs/platforms/pi.md` 中缺少 tracked `.pi/prompts/` 与源资产同步提醒的位置。

- 找到现有文档中只说明 Pi 模板必须显式使用 `$ARGUMENTS`，但没有提醒本仓库 tracked `.pi/prompts/` 与源资产要同步的缺口。

**Green**

- [x] G1: 在 `docs/platforms/pi.md` 添加源资产与实际项目 prompt 同步注意事项。
- [x] G2: 判断并记录是否需要更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`。
- [x] G3: 运行完整验证命令并记录结果。

- 在 `docs/platforms/pi.md` 添加简短开发注意事项：修改 `assets/platforms/pi/prompts/*.md` 后，如果 `.pi/prompts/*.md` 被跟踪或用于本地 Pi 测试，也要同步或验证实际 prompt。
- 判断是否需要更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`；如果只是平台文档和资产同步约束，不改变执行流，则不更新 ARCH，并在结果中说明。

**Refactor**

- [x] F1: 检查文档是否简洁且没有重复大段变量说明。
- [x] F2: 重新运行受影响测试或记录 `no refactor needed`。

- 保持文档简洁，不重复大段变量说明。
- 确保文档命令示例仍符合项目约定：开发测试用 `cargo run --`，写入资产或用户文档中提到全局命令时使用 `hotpot`。
:::

## Validation

- `git show HEAD:.pi/prompts/hotpot-new.md | rg --color never -F '$ARGUMENTS'` should fail before the sync and be recorded as the Red baseline.
- `rg --color never -F '$ARGUMENTS' .pi/prompts/hotpot-new.md` should pass after the sync.
- `cargo test pi_new_prompt_template_passes_command_arguments`
- `cargo test pi_prompt_source_and_installed_template_stay_in_sync` or the chosen new regression test name.
- `cargo test`
- `cargo run -- init --platform pi --dry-run` and/or `cargo run -- init --platform pi --force --dry-run`, with any `.pi/package.json` noise explicitly explained.

::: warning Risks and Watchouts
- 当前工作区已经有 `.pi/prompts/hotpot-new.md` 未提交改动，执行时不能通过回滚来制造 Red；应使用 `git show HEAD` 记录提交基线缺陷。
- `.pi/package.json` 当前也有格式化/同步差异，但不属于已批准核心范围；除非验证必须，否则不要把它混入本任务。
- `hotpot update` 不会无条件覆盖不同步 owned asset；不要把它误认为会自动修复已有项目 prompt。
- 如果新增安装路径测试写临时工作区，必须清理测试产物，避免再次留下 `.hotpot/workspaces/test*` 或根目录 `issues.md` 噪音。
:::

## Done Criteria

- `.pi/prompts/hotpot-new.md` 已包含 `$ARGUMENTS` 参数传递说明。
- 回归测试能覆盖源资产和 Pi 实际读取/安装 prompt 的参数段落。
- 文档记录源资产与项目 prompt 同步注意事项。
- TDD Red/Green/Refactor 记录完整，且验证命令通过或有明确、可接受的解释。

## Execution Instructions

- 严格按 TDD Red → Green → Refactor 顺序执行每个 Implementation Task，并在执行报告中保留 `Task <N>:` 证据块。
- 不要通过回滚当前工作区已有 `.pi/prompts/hotpot-new.md` 改动来制造 Red；使用 `git show HEAD` 作为提交基线缺陷证据。
- 默认不要纳入 `.pi/package.json`；如果验证必须处理它，先确认原因并在报告中明确说明。
- 本任务交付/提交范围仅包含 `.hotpot/workspaces/HusuSama/overview.jsonl`、`.hotpot/workspaces/HusuSama/tasks/2026-05-21-sync-pi-installed-new-prompt.md`、`.pi/prompts/hotpot-new.md`、`docs/platforms/pi.md` 和 `src/assets/platforms/pi.rs`；当前可见的 `.pi/package.json` 与 `docs/ROADMAP.md` diff 属于任务范围外既有工作区改动，必须显式排除在本任务提交之外。
- 完成后更新本任务文件 checkbox，只勾选已经完成且有验证证据的步骤。
- 不修改与 Pi `hotpot-new` 同步无关的文件。
