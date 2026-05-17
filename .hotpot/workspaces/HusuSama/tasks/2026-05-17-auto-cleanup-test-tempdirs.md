---
title: 单测临时目录自动清理（tempfile + Drop）
description: 用 tempfile::TempDir 替换 5 处手写的 env::temp_dir().join 模式，让 cargo test 结束时通过 Drop 自动清理
date: 2026-05-17
category: [Task]
tag: [test, cleanup, tempfile, refactor]
---

# 单测临时目录自动清理（tempfile + Drop）

## Task

### Summary

本仓库内 5 个测试辅助函数都用 `env::temp_dir().join(format!("hotpot-{label}-{nanos}"))` 在系统临时目录下手建子目录但从不清理，导致 `%TEMP%` 内 `hotpot-*` 残留持续累积（当前已 199 个）。引入 `tempfile` crate，把这 5 处全部改造为返回 `tempfile::TempDir` 的语义，由测试作用域结束时的 Drop 自动 `rm -rf`，从根上止血新泄漏。本任务**不**清理历史遗留的 199 个目录（用户后续手动 `rm` 即可）。

### User Request

> 当前项目中，单测完成后会有很多遗留的测试数据，我希望有一个后置的操作能清理这些数据

**Brainstorm 阶段确认的关键决策**：

1. 「后置操作」采用 Drop 自动清理（不另开 CLI 子命令清理历史目录）。
2. 引入 `tempfile` crate（不自写 `TestTmp(PathBuf) impl Drop`）。
3. 跳过 TDD 模式——这次是测试 helper 重构，验证点是 Drop 副作用，Red→Green 循环不翻股。

### Approved Design

**核心思路**：

- 新增 `[dev-dependencies] tempfile = "3"`（仅测试依赖，不影响 release binary 体积）。
- 5 处测试 helper 统一改成返回 `tempfile::TempDir`（或 `(TempDir, PathBuf)` 元组，针对需要返回内部文件路径的 `lock.rs`），调用方持有 handle，作用域结束 Drop 自动清理。
- 不动生产代码，不增加 CLI 子命令，不批量清扫历史 199 个目录。

**为什么 `tempfile` 而不是自写 `Drop` 类型**：

- 标准 Rust 临时目录方案，`TempDir::drop` 已经包含 Windows 文件占用场景下的重试与降级逻辑。
- 自写版本要么忽略平台差异、要么要手动重写一遍同样的重试逻辑——多走一遍标准库已经走过的弯路。

**为什么不批量清扫历史 199 个目录**：

- 用户明确选了「Drop 自动清理」，没选「两者都要」。
- 一次性 `rm` 操作在 shell 里一行能做完（`rm -rf %TEMP%/hotpot-*`），不值得为它新加 CLI 子命令——加了之后再没人会用第二次。

### Alternatives Considered

- **自写 `TestTmp(PathBuf) impl Drop` 不引入依赖**：代码量小，但要自行处理 Windows 文件占用重试 → 不如 `tempfile` 健壮。**未采纳**。
- **新增 CLI 子命令 `hotpot internal cleanup-test-artifacts` 扫描 `%TEMP%/hotpot-*` 并删除**：保留测试代码原样，但每次 cargo test 都得记得手跑一次，体验差；且与「跨平台 binary 不暴露测试 helper」的设计原则相冲突。**未采纳**。
- **「两者都要」（Drop + 一次性 CLI 清扫历史）**：用户已明确仅取 Drop 一条路径。**未采纳**。
- **推荐方案（已批准）**：引入 `tempfile`，5 处 helper 统一返回 `TempDir`，调用方以 RAII 方式持有，作用域结束自动清理。

### Requirements

- `Cargo.toml` 新增 `[dev-dependencies]` 段，登记 `tempfile = "3"`。
- 以下 5 处 helper 必须改为返回 `tempfile::TempDir`（或包含它的元组）：
  - `src/lock.rs::temp_data_path`
  - `src/issues.rs` 中 `test_concurrent_append_issue_does_not_lose_rows` 内联的 tmp_root 构造
  - `src/context.rs::unique_root`（其私有辅助 `write_config` 跟随改签名）
  - `src/task/mod.rs::make_isolated_project_dir`
  - `src/commands/update.rs::temp_project_dir`
- 改造后 `cargo test` 必须全绿；不允许出现 `temp_dir_handle` 未绑定本地变量导致测试一开始就被清的悬挂引用。
- 改造后跑一遍测试，`%TEMP%` 内 `hotpot-*` 计数相对改造前**不增加**（旧 199 不计）。

### Non-Goals

- **不**删除 `%TEMP%` 内已经堆积的 199 个历史 `hotpot-*` 目录。
- **不**新增任何生产代码或 CLI 子命令。
- **不**触碰 `src/worktree/mod.rs` 等生产路径的 `create_dir_all`。
- **不**改造对外暴露的 `hotpot init` / `hotpot update` 行为。

### Project Context

::: info 项目背景
该项目为 Rust CLI 工具 `hotpot`（跨平台任务编排器），测试随 `cargo test` 跑，启用 unwinding panic（`[profile.release]` 才是 `panic = abort`），测试 panic 时 Drop 依旧会触发。
:::

**5 处 helper 当前形态**（均在各自 `#[cfg(test)] mod tests` 内）：

| 文件 | 函数 | 返回 | 调用点 |
| --- | --- | --- | --- |
| `src/lock.rs` | `temp_data_path(label) -> PathBuf` | 返回 `<tmp>/data.jsonl` 路径 | 3：`lock_releases_after_op` / `concurrent_threads_serialize` / `lock_sidecar_path_is_data_path_plus_lock` |
| `src/issues.rs` | 内联在 `test_concurrent_append_issue_does_not_lose_rows` 内 | 直接构造 `tmp_root` | 1（该单测内部） |
| `src/context.rs` | `unique_root(label) -> PathBuf` | 返回 `<tmp>`，并在里面建 `.hotpot/` | 14：所有 `resolves_language_*` / `vuepress_*` 测试 |
| `src/task/mod.rs` | `make_isolated_project_dir(label) -> String` | 返回 `<tmp>` 的 String | 1：`test_concurrent_create_task_does_not_lose_rows` |
| `src/commands/update.rs` | `temp_project_dir(label) -> PathBuf` | 返回 `<tmp>` | 4：`update_bails_without_any_platform_dir` / `update_creates_workspace_skeleton_on_first_run` / `update_is_idempotent_on_second_run` / `update_warns_on_default_username_without_allow_flag` |

**关键约束**：

- `src/context.rs` 内 `write_config` 私有辅助当前签名 `fn write_config(root: &PathBuf, contents: &str)`，需要跟随改成 `fn write_config(root: &Path, contents: &str)`，因为 `TempDir::path()` 返回 `&Path`。
- `src/task/mod.rs::make_isolated_project_dir` 当前返回 `String`，调用方 `Arc::new(root_dir)` 期望 `Arc<String>`；新签名返回 `TempDir`，调用方需要保留 handle 并单独 `let root_dir = Arc::new(tmp.path().display().to_string());`。
- `src/lock.rs::temp_data_path` 当前返回 `<tmp>/data.jsonl`；改造后返回 `(TempDir, PathBuf)`，调用方 `let (_tmp, data) = temp_data_path("releases");`。`_tmp` 前缀的下划线只是约定上让 clippy 不警告，**绝不能省略 `_tmp` 绑定**——否则该 `TempDir` 立即被 Drop，测试运行时目录已不存在。

**双语注释风格**（按 `AGENTS.md`）：项目里所有新增/修改的函数与重要参数都用「英文 + 中文」双语 doc 注释，参考 `src/lock.rs` 既有风格保持一致。

## Plan

### Mode

- tdd: false

### File Map

- Modify: `Cargo.toml` - 新增 `[dev-dependencies] tempfile = "3"`。
- Modify: `src/lock.rs` - `temp_data_path` 改返回 `(TempDir, PathBuf)`，3 个调用点同步更新。
- Modify: `src/issues.rs` - 内联的 tmp_root 构造改用 `tempfile::TempDir::new()`。
- Modify: `src/context.rs` - `unique_root` 改返回 `TempDir`；`write_config` 改吃 `&Path`；14 个调用点同步更新。
- Modify: `src/task/mod.rs` - `make_isolated_project_dir` 改返回 `TempDir`；1 个调用点同步更新。
- Modify: `src/commands/update.rs` - `temp_project_dir` 改返回 `TempDir`；4 个调用点同步更新。

### Implementation Tasks

#### Task 1: 添加 tempfile dev-dependency

**Files:**

- Modify: `Cargo.toml`

- [ ] 步骤 1：在 `Cargo.toml` 现有 `[dependencies]` 段下面插入 `[dev-dependencies]` 段，加 `tempfile = "3"`。如果文件里已经有 `[dev-dependencies]` 段（目前没有），追加进去而不是重写。
- [ ] 步骤 2：跑 `cargo check --tests` 让 cargo 解锁新依赖；预期输出包含 `Compiling tempfile vX.Y.Z`，最终 `Finished` 无 error（warning 可接受）。
- [ ] 步骤 3：跑 `cargo build --tests` 确认所有现有测试代码在加入 dep 之后仍能编译（此刻还没改 helper，应继续通过）。

#### Task 2: 重构 src/lock.rs::temp_data_path

**Files:**

- Modify: `src/lock.rs`

- [ ] 步骤 1：在 `mod tests` 顶部 `use tempfile::TempDir;`。
- [ ] 步骤 2：将 `temp_data_path` 改签名为 `fn temp_data_path(label: &str) -> (TempDir, std::path::PathBuf)`。函数体改为：用 `tempfile::Builder::new().prefix(&format!("hotpot-lock-{label}-")).tempdir().expect("create tempdir")` 创建 `TempDir`，把 `tempdir.path().join("data.jsonl")` 作为 PathBuf，元组返回 `(tempdir, data_path)`。保留双语 doc 注释（英文 + 中文各一段，解释「为什么返回元组——上层必须持有 TempDir 让 Drop 在测试结束生效」）。
- [ ] 步骤 3：更新 `lock_releases_after_op`：把 `let data = temp_data_path("releases");` 改成 `let (_tmp, data) = temp_data_path("releases");`。其他逻辑不变。
- [ ] 步骤 4：更新 `concurrent_threads_serialize`：把 `let data = temp_data_path("serialize");` 改成 `let (_tmp, data) = temp_data_path("serialize");`。注意 `_tmp` 必须在 thread join 完成后才离开作用域——当前测试是 `let handles ... ; for h in handles { h.join()... }; let max = ...;`，`_tmp` 在函数末尾才 Drop，符合要求。
- [ ] 步骤 5：更新 `lock_sidecar_path_is_data_path_plus_lock`：把 `let data = temp_data_path("sidecar");` 改成 `let (_tmp, data) = temp_data_path("sidecar");`。
- [ ] 步骤 6：跑 `cargo test --lib lock::tests` 预期 3 个测试全绿。

#### Task 3: 重构 src/issues.rs 的并发测试 tmp_root

**Files:**

- Modify: `src/issues.rs`

- [ ] 步骤 1：在 `test_concurrent_append_issue_does_not_lose_rows` 内的 `use std::time::{SystemTime, UNIX_EPOCH};` 行删除（不再需要 nanos 时间戳），改为局部 `use tempfile::TempDir;`。
- [ ] 步骤 2：把 nanos 计算 + `std::env::temp_dir().join(format!(...))` + `fs::create_dir_all(tmp_root.join(".hotpot")).unwrap();` 三行替换为：
  ```rust
  let tmp_root = TempDir::with_prefix("hotpot-issues-concurrent-")
      .expect("create tempdir");
  fs::create_dir_all(tmp_root.path().join(".hotpot")).unwrap();
  let root_dir = Arc::new(tmp_root.path().display().to_string());
  ```
  `tmp_root` 在测试函数末尾被 Drop，所有 thread 早已 join 完毕，目录可安全清理。
- [ ] 步骤 3：跑 `cargo test --lib issues::tests::test_concurrent_append_issue_does_not_lose_rows` 预期绿色。

#### Task 4: 重构 src/context.rs::unique_root

**Files:**

- Modify: `src/context.rs`

- [ ] 步骤 1：在 `mod tests`（或现有 `use std::{...}` 块）里加 `use tempfile::TempDir;`。可以删掉测试模块顶部 `SystemTime` / `UNIX_EPOCH` 的导入（确认无别处使用后再删；如果还被其它测试用就保留）。
- [ ] 步骤 2：将 `unique_root` 改签名为 `fn unique_root(label: &str) -> TempDir`。函数体改为：
  ```rust
  let tmp = tempfile::Builder::new()
      .prefix(&format!("hotpot-lang-{label}-"))
      .tempdir()
      .expect("create tempdir");
  fs::create_dir_all(tmp.path().join(".hotpot")).unwrap();
  tmp
  ```
  保留双语 doc 注释。
- [ ] 步骤 3：将 `write_config` 改签名为 `fn write_config(root: &Path, contents: &str)`，函数体 `fs::write(root.join(".hotpot/config.toml"), contents).unwrap();` 不变（`root` 类型从 `&PathBuf` 变 `&Path` 后调用方式相同）。在文件顶部测试模块导入里加 `use std::path::Path;` 若尚未存在。
- [ ] 步骤 4：批量更新 14 个调用点。模式统一为：
  - 旧：`let root = unique_root("xxx"); ... resolve_language_with_source(&root.display().to_string());`
  - 新：`let root = unique_root("xxx"); ... resolve_language_with_source(&root.path().display().to_string());`
  - `write_config(&root, ...)` 调用点旧式传 `&PathBuf`，新式因为签名改成 `&Path`，需要传 `root.path()`，即 `write_config(root.path(), ...)`。
  
  涉及测试：`resolves_language_from_env_override`、`resolves_language_from_config_toml_top_level`、`commented_out_default_yields_english`、`empty_value_yields_english`、`corrupt_toml_yields_english_no_panic`、`env_beats_config`、`whitespace_is_trimmed`、`missing_file_yields_english`、`non_string_value_yields_english`、`vuepress_enabled_from_env_override`、`vuepress_enabled_from_config_toml`、`vuepress_enabled_defaults_to_false`、`vuepress_enabled_env_case_insensitive`、`vuepress_enabled_garbage_env_falls_through`、`vuepress_port_from_env_override`、`vuepress_port_from_config_toml`、`vuepress_port_defaults_to_8080`。
- [ ] 步骤 5：跑 `cargo test --lib context::tests` 预期所有 context 模块测试全绿。

#### Task 5: 重构 src/task/mod.rs::make_isolated_project_dir

**Files:**

- Modify: `src/task/mod.rs`

- [ ] 步骤 1：在 `mod tests`（或测试函数局部）加 `use tempfile::TempDir;`。
- [ ] 步骤 2：将 `make_isolated_project_dir` 改签名为 `fn make_isolated_project_dir(label: &str) -> TempDir`，函数体改为：
  ```rust
  tempfile::Builder::new()
      .prefix(&format!("hotpot-task-{label}-"))
      .tempdir()
      .expect("create tempdir")
  ```
  保留双语 doc 注释，并把现有的「为什么需要独立 username + tmp root」注释保留下来。
- [ ] 步骤 3：更新唯一调用点 `test_concurrent_create_task_does_not_lose_rows`。把：
  ```rust
  let root_dir = make_isolated_project_dir("concurrent-create");
  let root_dir = Arc::new(root_dir);
  ```
  改成：
  ```rust
  let tmp = make_isolated_project_dir("concurrent-create");
  let root_dir = Arc::new(tmp.path().display().to_string());
  ```
  注意 `tmp` 必须在测试函数末尾才 Drop，确认 `for h in handles { ... }` 与 `get_task_list` 都跑完后再退出作用域。
- [ ] 步骤 4：跑 `cargo test --lib task::tests::test_concurrent_create_task_does_not_lose_rows` 预期绿色。

#### Task 6: 重构 src/commands/update.rs::temp_project_dir

**Files:**

- Modify: `src/commands/update.rs`

- [ ] 步骤 1：在测试模块 `use` 块加 `use tempfile::TempDir;`，删除不再需要的 `SystemTime` / `UNIX_EPOCH` 导入（若无其它测试使用）。`PathBuf` 在 `build_args` 参数与 `install_claude_fixture` 内仍需要，保留。
- [ ] 步骤 2：将 `temp_project_dir` 改签名为 `fn temp_project_dir(label: &str) -> TempDir`，函数体改为：
  ```rust
  tempfile::Builder::new()
      .prefix(&format!("hotpot-update-{label}-"))
      .tempdir()
      .expect("create tempdir")
  ```
- [ ] 步骤 3：`install_claude_fixture(project_dir: &PathBuf)` 改成 `install_claude_fixture(project_dir: &Path)`，函数体不变（`assets::install_for` 第一参签名兼容 `&Path`）。同时 `use std::path::Path;`（若已 use `PathBuf` 则在同行追加）。如果 `assets::install_for` 第一参确实只接 `&Path`，本步可保。如果它要 `&PathBuf`，则维持原签名 `&PathBuf`，调用方传 `&tmp.path().to_path_buf()`。先 Read `src/assets/mod.rs::install_for` 第一参类型再决定。
- [ ] 步骤 4：批量更新 4 个调用点：
  - `update_bails_without_any_platform_dir`：`let dir = temp_project_dir("no-platform");` → 不变（仍叫 `dir`）；`build_args(dir, ...)` 改成 `build_args(dir.path().to_path_buf(), ...)`。
  - `update_creates_workspace_skeleton_on_first_run`：`let dir = temp_project_dir("first-run"); install_claude_fixture(&dir);` → `install_claude_fixture(dir.path());`（如果保留 `&PathBuf` 签名则 `install_claude_fixture(&dir.path().to_path_buf())`）。`build_args(dir.clone(), ...)` → `build_args(dir.path().to_path_buf(), ...)`。后续 `dir.join("...")` → `dir.path().join("...")`。
  - `update_is_idempotent_on_second_run`：同理，两次 `dir.clone()` 都改成 `dir.path().to_path_buf()`。
  - `update_warns_on_default_username_without_allow_flag`：`build_args(dir, ...)` → `build_args(dir.path().to_path_buf(), ...)`。
- [ ] 步骤 5：跑 `cargo test --lib commands::update::tests` 预期所有 update 测试全绿。

#### Task 7: 全量回归 + 泄漏验证

**Files:**

- Validation only

- [ ] 步骤 1：跑 `cargo test` 预期 0 失败、0 ignored 数量与改造前一致。
- [ ] 步骤 2：泄漏验证：在 `cargo test` 跑完后立即跑 `ls "$TEMP" | grep -c "^hotpot-"`（macOS/Linux 用 `ls /tmp | grep -c '^hotpot-'`）。Windows 下 git-bash 跑 `ls "$TEMP" | grep -c '^hotpot-'`。预期数值 ≤ 改造前测得的 199（具体可能比 199 多 1~2 个长时间被占用未及时清掉的，但**不应**出现新一波 `hotpot-lock-*` / `hotpot-issues-concurrent-*` / `hotpot-lang-*` / `hotpot-task-*` / `hotpot-update-*` 时间戳后缀刚写出来的新目录）。
- [ ] 步骤 3：跑两次 `cargo test`，两次之间记录 `%TEMP%` 中 `hotpot-*` 计数差值。差值应为 0（旧的不动，新的不留）。

### Validation

- `cargo test` - 0 失败；测试总数应与改造前一致。
- `cargo check --tests` - 无 error。
- `ls "$TEMP" | grep -c "^hotpot-"` 前后差值 = 0（旧 199 个不动，新增 0）。

### Risks and Watchouts

::: warning 必读
**`TempDir` handle 必须绑定本地变量**：`temp_data_path` 等返回 `(TempDir, PathBuf)` 的 helper 调用方写成 `let (_, data) = temp_data_path(...);`（用 `_` 而非 `_tmp`）会立刻把 `TempDir` Drop 掉，目录在测试还没用上之前就被删了。必须写 `let (_tmp, data) = ...;`——`_` 前缀只是给 clippy 看的，**整个标识符不能就是单字符 `_`**。
:::

- **Windows 文件占用**：`TempDir::drop` 在 Windows 上偶尔会因为 antivirus / Indexer 临时占用文件而失败一次。`tempfile` 内部有 retry，但极端情况下可能仍有少量残留（Linux/macOS 不会）。这不算回归，属于平台限制。
- **`make_isolated_project_dir` 调用方 ownership**：原签名返回 `String`（owned），新签名返回 `TempDir`，`tmp.path().display().to_string()` 才是 owned String。**不能**直接把 `TempDir` 丢进 `Arc::new()` ——`Arc<TempDir>` 让 Drop 在最后一个 Arc clone 释放时才跑，依然安全但语义比预期更迟。当前测试只在主线程持有 `tmp`，子线程持有 `Arc<String>`（路径字符串），无问题。
- **`write_config` 签名变更**：从 `&PathBuf` 变 `&Path` 是收窄类型约束。所有调用都从 `&root`（其中 `root: &PathBuf`）改成 `root.path()` 或 `&*root`（取决于 `root` 的新类型）。
- **`install_claude_fixture` 第一参类型**：若 `assets::install_for` 第一参实际是 `&PathBuf` 而非 `&Path`，需要 `dir.path().to_path_buf()` 显式转一次。先 Read `src/assets/mod.rs` 中 `install_for` 签名再下手，避免反复改。
- **不删历史 199 个目录**：本任务不动它们，用户后续在 shell 里一行 `rm -rf %TEMP%/hotpot-*`（Windows 下 cmd 是 `del /q /s %TEMP%\hotpot-*`）即可。这部分不写进代码。

## Execution Instructions

::: tip 给执行 sub-agent
执行 sub-agent 必须读完整份任务文件，然后按 `Plan` 的 Task 1 → Task 7 顺序串行执行：
:::

- Task 1（Cargo.toml）跑通后再进 Task 2，因为后续任务都 `use tempfile`。
- Task 2–6 各自独立，可在 Task 1 完成后按顺序逐一处理；不要一次 multi-edit 改 5 个文件——会让定位失败位置困难。
- Task 7（全量回归）必须最后跑；中途单模块测试只是早期防护网，不能替代它。
- 每个 `Task N` 的复选框必须在执行过程中改成 `- [x]` 标记进度——便于 review agent 审核。
- 遇到任意一处 helper 因 `assets::install_for` 等 API 签名细节卡住，先 Read 真实签名再决定，**不要**猜签名。
- 不要扩大范围：本任务**不**涉及生产代码、不删历史 199 个目录、不加 CLI 子命令。

跑完整体验证后，按 `Validation` 区列举的 3 条逐一跑一遍并把数据贴回给 review agent。
