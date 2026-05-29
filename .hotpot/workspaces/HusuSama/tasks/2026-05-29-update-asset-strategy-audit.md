# Update Asset Strategy Audit

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| Done | true | 4 | medium |
:::

## Task

### Summary

::: info
梳理并加固 `hotpot update` 的文件更新策略：明确哪些资产在 update 时走 merge，哪些走 Hotpot 私有模板覆盖，哪些只在缺失时创建；验证 `--force` 只影响应被覆盖的 Hotpot 私有文件，并评估现有策略是否需要通过更清晰的报告或文档进行优化。
:::

### Background

`hotpot update` 是协作者 day-1 入口，会自动探测已安装平台并刷新资产、安装共享 prompts、合并 `.gitignore`、创建当前用户 workspace、同步 VuePress 任务链接并运行健康检查。当前资产安装策略集中在 `src/assets/mod.rs`：

- `InstallStrategy::Owned`：Hotpot 私有文件，内容不同且未传 `--force` 时报错，传 `--force` 时整文件写入 bundled 内容。
- `InstallStrategy::MergeJson` / `InstallStrategy::MergeToml` / `InstallStrategy::MergeText`：与用户内容共存的主配置或文本块，update 每次都应幂等合并，不依赖 `--force`。
- `InstallStrategy::CreateIfMissing`：用户自有 seed，只在目标缺失时创建，已有文件即使传 `--force` 也不能覆盖。

已知资产分布：

- 共享 prompts 位于 `.hotpot/prompts/*.md`，登记在 `src/assets/shared.rs`，当前是 `Owned`。
- `.gitignore` 登记在 `src/assets/shared.rs`，当前是 `MergeText`。
- `.hotpot/config.toml` 登记在 `src/assets/shared.rs`，当前是 `CreateIfMissing`。
- Claude / OpenCode / Codex / Pi 的主配置文件分别走 `MergeJson` 或 `MergeToml`；平台下 Hotpot 私有 agent、command、skill、plugin、extension 文件走 `Owned`。
- VuePress opt-in prompts 位于 `src/assets/vuepress_opt_in.rs`，当前是 `Owned`，仅在 VuePress 启用时由 `hotpot update` 刷新。
- VuePress hub 项目位于 `src/assets/vuepress_hub.rs`，当前是 `Owned`，但不由 `hotpot update` 刷新；需要显式 `hotpot vuepress install --force` 修复差异。

### Requirements

1. 产出代码层面的策略盘点能力或测试断言，覆盖 `hotpot update` 触达的主要资产策略。
2. 验证 `--force` 只会覆盖内容不同的 `Owned` 资产，不会把 `Merge*` 或 `CreateIfMissing` 变成整文件覆盖。
3. 明确 `hotpot update` 是否需要优化现有策略。如果优化，应优先选择低风险改动，例如更清晰的 `--json` / human summary / 文档说明，而不是改变用户数据保护语义。
4. 如果最终改变 `hotpot update` 的执行流、报告字段或资产策略，必须同步更新 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md`。
5. 保持代码输出为英文；新增 Rust 函数、模块、测试辅助函数都必须有中英双语 doc comments。

### Non-Goals

::: details Non-Goals
- 不把 `MergeJson` / `MergeToml` / `MergeText` 改成整文件覆盖。
- 不让 `--force` 覆盖 `.hotpot/config.toml` 这类用户自有 seed。
- 不让 `hotpot update` 自动刷新 `.hotpot-hub/` VuePress hub 项目。
- 不新增非 Rust 运行时依赖。
- 不拆分超过 1000 行的文件；如果执行中发现确需拆分，先停止并向用户确认。
:::

### Approved Design

::: tip
采用测试先行的方式加固现有语义，并在实现阶段基于测试结果决定优化点。推荐优化方向是“可见性优先”：让用户和维护者更容易从测试、文档或报告字段判断每个 update 触达文件的策略，而不是改变保护用户文件的核心行为。

执行代理应先写失败测试来证明当前缺口，再最小化实现。若代码检查发现现有行为已经正确，仍需要补齐覆盖策略矩阵的测试和文档，使“哪些 merge、哪些覆盖、哪些需要 `--force`”成为可回归验证的事实。
:::

### Acceptance Criteria

- `cargo test commands::update::tests` 通过。
- `cargo test assets::` 或覆盖资产安装策略的等价精确测试通过。
- `cargo test` 通过。
- 任务最终说明中列出 update 触达文件的策略矩阵，并明确 `--force` 的触发范围。
- 文档与代码对 `hotpot update`、`hotpot init`、VuePress opt-in prompts、VuePress hub 的边界描述一致。

---

## Plan

### Mode

- tdd: true

### Execution Strategy

- git-worktree: true

建议在隔离 worktree 中执行，因为任务会触碰资产安装核心路径、update 报告和架构文档，且需要反复运行测试验证现有保护语义。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/assets/mod.rs` | Modify | 必要时暴露或细化策略元数据，并保证安装语义仍集中在资产引擎。 |
| `src/assets/shared.rs` | Modify | 必要时补充共享资产策略说明或配合策略盘点测试。 |
| `src/assets/platforms/claude.rs` | Modify | 必要时补充 Claude 资产策略说明或测试可见性。 |
| `src/assets/platforms/opencode.rs` | Modify | 必要时补充 OpenCode 资产策略说明或测试可见性。 |
| `src/assets/platforms/codex.rs` | Modify | 必要时补充 Codex 资产策略说明或测试可见性。 |
| `src/assets/platforms/pi.rs` | Modify | 必要时补充 Pi 资产策略说明或测试可见性。 |
| `src/assets/vuepress_opt_in.rs` | Modify | 必要时说明 VuePress opt-in prompts 与 update 的 `--force` 关系。 |
| `src/assets/vuepress_hub.rs` | Modify | 必要时说明 hub 不属于 update 刷新范围。 |
| `src/commands/update.rs` | Modify | 若选择报告优化，在 update report 或 human summary 中体现策略信息。 |
| `docs/ARCH.md` | Modify | 如果策略说明或 update 流程有变化，同步英文架构文档。 |
| `docs/ARCH.zh_CN.md` | Modify | 与 `docs/ARCH.md` 同步的简体中文架构文档。 |
| `src/commands/update.rs` | Test | 扩展现有 update 集成测试，验证 `--force` 与 merge / seed 语义。 |
| `src/assets/mod.rs` | Test | 添加或扩展资产引擎测试，覆盖策略矩阵和 dry-run / force 行为。 |

### Implementation Tasks

#### Task 1: 建立 update 策略矩阵测试

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/commands/update.rs` | Test | 在现有 `commands::update::tests` 中补充 update 触达资产策略的回归测试。 |
| `src/assets/mod.rs` | Test | 如 update 集成测试无法覆盖底层策略，添加资产引擎单元测试。 |

##### Red

- [ ] R1: 在 `src/commands/update.rs` 的测试模块中新增测试 `update_force_does_not_overwrite_merge_or_seed_assets`，构造含 Claude 或 Codex 平台的临时项目，手写用户自定义 `.claude/settings.json` 或 `.codex/config.toml`、`.gitignore` 用户行、`.hotpot/config.toml` 用户配置，并运行 `build_report(build_force_args(...))`。
- [ ] R2: 断言 `Merge*` 文件保留用户内容且包含 Hotpot 注入段，`.hotpot/config.toml` 保留用户内容不被 seed 覆盖，同时 `Owned` 模板差异在 `--force` 下被恢复。
- [ ] R3: 运行 `cargo test update_force_does_not_overwrite_merge_or_seed_assets`; **expect failure**，失败点应体现测试尚未实现、断言缺少辅助能力，或当前报告/策略覆盖不足。记录失败测试名和断言位置。

##### Green

- [ ] G1: 只做让 `update_force_does_not_overwrite_merge_or_seed_assets` 通过的最小修改；优先补测试 fixture 和断言，只有在测试暴露真实语义缺陷时才改 `src/assets/mod.rs` 或 `src/commands/update.rs`。
- [ ] G2: 运行 `cargo test update_force_does_not_overwrite_merge_or_seed_assets`; **expect pass**。
- [ ] G3: 运行 `cargo test commands::update::tests`; **expect no other regressions**。

##### Refactor

- [ ] F1: 检查测试 fixture 是否重复过多；如需要，抽出带中英双语 doc comments 的小型测试辅助函数，否则写 `no refactor needed`。
- [ ] F2: 如果发生 refactor，重新运行 `cargo test update_force_does_not_overwrite_merge_or_seed_assets` 和 `cargo test commands::update::tests`; **expect pass**。否则标记 `skipped (no refactor)`。

#### Task 2: 验证非 force 情况下的差异 Owned 行为和 merge 自愈

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/commands/update.rs` | Test | 补齐默认 update 在 `Owned` 差异时应拒绝、但 merge 文件仍可自愈的行为。 |
| `src/assets/mod.rs` | Modify | 如错误信息或策略分支不够清晰，最小化调整。 |

##### Red

- [ ] R1: 新增测试 `update_without_force_requires_force_only_for_differing_owned_assets`，让 Hotpot 私有模板内容变 stale，运行 `build_report(build_args(...))`，期望错误消息包含 `rerun with --force to overwrite`。
- [ ] R2: 在同一测试或相邻测试中证明 merge 资产在没有 `--force` 时仍会合并 Hotpot 段并保留用户内容；若 `Owned` bail 会阻断此断言，使用只含 merge 差异的 fixture。
- [ ] R3: 运行 `cargo test update_without_force_requires_force_only_for_differing_owned_assets`; **expect failure**，失败应来自新测试缺口或错误信息/fixture 未满足预期。

##### Green

- [ ] G1: 最小化实现或测试 fixture 调整，使默认 update 的 `Owned` bail 与 merge 自愈行为被准确验证。
- [ ] G2: 运行 `cargo test update_without_force_requires_force_only_for_differing_owned_assets`; **expect pass**。
- [ ] G3: 运行 `cargo test commands::update::tests`; **expect pass**。

##### Refactor

- [ ] F1: 检查 `build_args` / `build_force_args` / 平台 fixture 是否可以更清晰地表达 `force` 语义；如抽 helper，添加中英双语 doc comments，否则写 `no refactor needed`。
- [ ] F2: 如果发生 refactor，重新运行 `cargo test update_without_force_requires_force_only_for_differing_owned_assets` 和 `cargo test commands::update::tests`; **expect pass**。否则标记 `skipped (no refactor)`。

#### Task 3: 优化 update 策略可见性

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/commands/update.rs` | Modify | 根据前两步结论优化用户可见报告或 JSON report。 |
| `src/assets/mod.rs` | Modify | 如果 report 需要策略分类，提供集中、可测试的资产策略元数据。 |
| `docs/ARCH.md` | Modify | 如 update 报告或流程发生变化，同步英文说明。 |
| `docs/ARCH.zh_CN.md` | Modify | 同步中文说明。 |

##### Red

- [ ] R1: 选择一个低风险可见性优化并先写测试。推荐测试名：`update_report_explains_force_sensitive_asset_strategy`。优先验证 `UpdateReport` 新字段或 human summary 新文案能区分 `Owned`、`Merge*`、`CreateIfMissing` 的 `--force` 行为。
- [ ] R2: 如果决定不新增报告字段，则写文档或测试断言来固定现有策略说明，例如测试资产策略分类 helper 的输出矩阵。
- [ ] R3: 运行 `cargo test update_report_explains_force_sensitive_asset_strategy`; **expect failure**，失败应证明当前实现还没有该可见性能力。

##### Green

- [ ] G1: 在 `src/commands/update.rs` 或 `src/assets/mod.rs` 中实现最小可见性优化。代码输出必须为英文；新增函数和结构体字段要有中英双语 doc comments。
- [ ] G2: 若 `UpdateReport` schema 变更，确保 `hotpot update --json` 仍输出合法 JSON，字段命名保持英文结构键。
- [ ] G3: 运行 `cargo test update_report_explains_force_sensitive_asset_strategy`; **expect pass**。
- [ ] G4: 运行 `cargo test commands::update::tests`; **expect pass**。

##### Refactor

- [ ] F1: 检查策略分类是否与 asset catalog 发生重复清单漂移；如果存在重复，改为从 `Asset` / registry 派生，或明确保持测试级 fixture，避免运行时代码重复维护。
- [ ] F2: 如果发生 refactor，重新运行 `cargo test update_report_explains_force_sensitive_asset_strategy` 和 `cargo test commands::update::tests`; **expect pass**。否则标记 `skipped (no refactor)`。

#### Task 4: 同步文档并运行完整验证

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify | 记录最终 update 策略矩阵和 `--force` 边界。 |
| `docs/ARCH.zh_CN.md` | Modify | 与英文架构文档保持同等内容。 |
| `src/assets/mod.rs` | Test | 运行完整资产相关测试。 |
| `src/commands/update.rs` | Test | 运行 update 测试与全量测试。 |

##### Red

- [ ] R1: 新增或更新文档一致性检查的测试（若项目已有合适模式）或先运行 `cargo test commands::update::tests`，确认文档更新前仍缺少最终说明对应的代码/测试覆盖。
- [ ] R2: 运行 `cargo test commands::update::tests`; **expect failure only if Task 3 的文档/报告约束尚未实现**。若已经通过，记录当前通过状态作为 Red 阶段的基线并继续文档修改。

##### Green

- [ ] G1: 更新 `docs/ARCH.md`，用英文说明 update 触达资产的策略矩阵、`--force` 只影响 `Owned`、merge 与 seed 不受 `--force` 变成覆盖、VuePress hub 不由 update 刷新。
- [ ] G2: 更新 `docs/ARCH.zh_CN.md`，用简体中文同步同等内容。
- [ ] G3: 运行 `cargo test commands::update::tests`; **expect pass**。
- [ ] G4: 运行 `cargo test assets::`; **expect pass**。如果该过滤器没有匹配测试，改运行覆盖资产模块的精确测试名并在最终报告中说明。

##### Refactor

- [ ] F1: 通读修改后的文档和测试命名，确保没有英文输出被误写成中文，且所有新增函数/模块有中英双语 doc comments；如无需代码整理，写 `no refactor needed`。
- [ ] F2: 运行 `cargo test`; **expect pass**。

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo test update_force_does_not_overwrite_merge_or_seed_assets` | 新增 force 边界测试通过。 |
| `cargo test update_without_force_requires_force_only_for_differing_owned_assets` | 新增默认 update 边界测试通过。 |
| `cargo test update_report_explains_force_sensitive_asset_strategy` | 新增策略可见性测试通过，或执行代理说明替代测试名和原因。 |
| `cargo test commands::update::tests` | update 集成测试全部通过。 |
| `cargo test assets::` | 资产模块相关测试通过；若过滤器无匹配，执行代理需运行等价精确测试并说明。 |
| `cargo test` | 全量测试通过。 |

### Risks and Watchouts

::: warning
- `hotpot update` 当前在第一个平台刷新失败时会提前返回错误；设计测试时不要误以为后续 merge 资产一定会继续执行。
- `--force` 是用户显式覆盖 Hotpot 私有模板的逃生口，不能扩展到用户自有配置或 merge 文件。
- `.hotpot/prompts/*.md` 虽然位于项目内，但属于 Hotpot 共享 prompt 资产，当前按 `Owned` 处理；这与 `.hotpot/config.toml` 的用户自有 seed 不同。
- VuePress opt-in prompts 会在 VuePress 启用时由 `hotpot update` 刷新；`.hotpot-hub/` hub 项目不会由 `hotpot update` 刷新。
- 如果新增 `UpdateReport` 字段，注意 `--json` 输出是外部可见 schema，字段应稳定、英文命名且测试覆盖。
- 不要用 ad hoc string 拼接解析 JSON/TOML；已有 merge helper 使用 `serde_json` 和 `toml_edit`，应继续沿用。
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
