# Add `--force` To `hotpot update`

## Task

为 `hotpot update` 增加 `--force` 选项，使升级 Hotpot 二进制后，用户可以显式要求 `update` 覆盖刷新 Hotpot 管理的模板文件。

当前状态：

- `hotpot init --force` 已支持覆盖 `Asset::owned(...)` 文件。
- `hotpot update` 没有暴露 `--force`，并在刷新平台资产时硬编码 `force=false`。
- 因此 `hotpot update --force` 会被 CLI 判定为未知参数，用户无法通过 update 覆盖更新 agent、command、prompt 等模板。

目标行为：

- `hotpot update --force` 成为合法命令。
- `--force` 传递到 update 的所有资产刷新路径。
- `--force` 覆盖所有 `Asset::owned(...)` 模板文件，包括 platform agent、command、plugin、shared prompt、VuePress opt-in prompt 等由 update 触达的模板。
- `MergeJson` / `MergeToml` / `MergeText` 继续按 merge 策略工作，不因 `--force` 变成整文件覆盖。
- `CreateIfMissing` 继续存在即跳过，不因 `--force` 覆盖用户配置。
- `--dry-run --force` 只报告计划写入，不修改文件。

非目标：

- 不实现结构化 agent merge。
- 不新增 manifest / three-way merge。
- 不改变 `hotpot init --force` 的既有行为。
- 不让 `hotpot update` 安装新的 platform；仍只刷新已检测到的平台目录。

## Plan

### Mode

- tdd: true

### Context

相关实现位置：

- `src/commands/update.rs`
  - `UpdateArgs` 当前没有 `force` 字段。
  - `build_report` 调用 `assets::install_for(..., /* force */ false, ...)`。
  - VuePress opt-in prompt 刷新也调用 `assets::install_vuepress_prompts(..., /* force */ false, ...)`。
- `src/commands/init.rs`
  - `InitArgs` 已有 `force` 字段，可参考帮助文案和传参方式。
- `src/assets/mod.rs`
  - `InstallStrategy::Owned` 在 `force=true` 时覆盖。
  - `Merge*` 策略本身不受 force 影响。
  - `CreateIfMissing` 明确即使 force 也不覆盖。
- `src/assets/shared.rs`
  - `.hotpot/prompts/*.md` 目前是 `Asset::owned(...)`，应被 `update --force` 覆盖。
- `src/assets/platforms/*.rs`
  - 各平台 agent / command / plugin 模板大多是 `Asset::owned(...)`，应被 `update --force` 覆盖。
- `docs/ARCH.md` / `docs/ARCH.zh_CN.md`
  - `hotpot update` 命令行为发生 CLI surface 变化，需要同步更新架构文档。

### Implementation Tasks

#### Task 1: Add failing tests for `hotpot update --force`

##### Red

- [x] R1. 在 `src/commands/update.rs` 的 `tests` 模块中新增测试：先安装一个平台，例如 Claude，然后手动修改一个 `Owned` 模板文件，例如 `.claude/agents/hotpot-execution.md` 或 `.hotpot/prompts/hotpot-new.md`。
- [x] R2. 构造 `UpdateArgs` 时需要能够设置 `force=true`；测试期望 `build_report(args)` 成功，并且被修改的模板内容恢复为内置 asset 内容。
- [x] R3. 运行 `cargo test commands::update::tests -- --nocapture`，确认测试在实现前失败，失败原因应体现 `UpdateArgs` 尚无 force 字段或 update 未覆盖 differing owned file。

##### Green

- [x] G1. 暂不修改实现，只确认 Red 测试覆盖的是 `update --force` 预期行为，而不是测试 setup 错误。

##### Refactor

- [x] F1. 如测试 helper 已有可复用 fixture，复用现有 helper，避免新建重复临时目录逻辑。

#### Task 2: Expose `--force` on `hotpot update`

##### Red

- [x] R1. 保留 Task 1 的失败测试作为驱动。

##### Green

- [x] G1. 在 `src/commands/update.rs::UpdateArgs` 增加 `force: bool` 字段。
- [x] G2. 使用与 `init --force` 一致的语义文案：覆盖 existing Hotpot-private files when their contents differ。
- [x] G3. 在 `build_report` 中保存 `args.force`，并传给每个 `assets::install_for` 调用。
- [x] G4. VuePress enabled 时，把同一个 `force` 传给 `assets::install_vuepress_prompts`。
- [x] G5. 确认 `--json`、`--dry-run` 与 `--force` 可组合，不改变现有输出 schema，除非测试表明必须新增字段。

##### Refactor

- [x] F1. 如 `force`、`dry_run`、`json` 的局部变量能减少 `args` move/borrow 复杂度，可在 `build_report` 开头局部复制布尔值。
- [x] F2. 不修改 `assets::install_one` 的策略语义，避免把 `--force` 扩散成 merge/config 覆盖。

#### Task 3: Verify CLI parsing and dry-run behavior

##### Red

- [x] R1. 新增或扩展命令解析测试，确认 `hotpot update --force` 能被 clap 接受；如果项目现有测试没有 CLI parse helper，可直接以 `UpdateArgs` 行为测试为主，不强行新增复杂 CLI 测试。

##### Green

- [x] G1. 运行 `cargo test commands::update::tests -- --nocapture`。
- [x] G2. 运行 `cargo test`，确保全量测试通过。
- [x] G3. 手动运行 `cargo run -- update --force --dry-run`，确认 CLI 接受参数且不会写盘。

##### Refactor

- [x] F1. 如果 human 输出中出现误导性文案，例如仍暗示 update 只能 skip differing owned files，则同步修正。

#### Task 4: Update architecture documentation

##### Red

- [x] R1. 检查 `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md` 中 `hotpot update` 命令描述，确认当前未说明 `--force`。

##### Green

- [x] G1. 在 `docs/ARCH.md` 的 Commands 表和相关 Notes 中补充：`hotpot update --force` 覆盖 Hotpot-private owned templates，但 merge/config/user-owned 文件仍保留对应策略。
- [x] G2. 在 `docs/ARCH.zh_CN.md` 写入等价中文内容。
- [x] G3. 确保英文版和中文版含义一致。

##### Refactor

- [x] F1. 不扩写过多实现细节；架构文档只记录用户可见 CLI 行为和资产策略边界。

### Validation

- [x] `cargo test commands::update::tests -- --nocapture`
- [x] `cargo test`
- [x] `cargo run -- update --force --dry-run`
- [ ] 如 VuePress enabled 测试环境可用，额外验证 `hotpot update --force --dry-run` 会触达 VuePress opt-in prompts，但不触碰 `.hotpot-hub/`。

## Execution Instructions

执行 agent 必须遵守：

- 先阅读 `docs/ARCH.md`，再修改命令或资产逻辑。
- 采用 TDD：先写失败测试，再实现，再重构。
- 所有新增 Rust 函数、字段或测试 helper 按项目规范写中英双语 doc 注释；普通测试函数如果现有模块风格只用简短注释，可保持一致但新增公开/结构字段文档必须双语。
- 输出字符串保持英文。
- 不要修改 `Asset::MergeJson` / `MergeToml` / `MergeText` / `CreateIfMissing` 的覆盖语义。
- 不要让 `hotpot update --force` 安装未初始化的平台。
- 如果发现现有 dirty worktree 中有无关用户改动，不要回滚，专注本任务涉及文件。
