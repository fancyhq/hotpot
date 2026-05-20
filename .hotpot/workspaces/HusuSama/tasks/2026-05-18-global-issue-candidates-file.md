# Global Issue Candidates File

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | true | 5 | medium |
:::

---

## Task

### Summary

::: info
把临时 review memory 候选从 per-user 文件 `.hotpot/workspaces/<username>/issue-candidates.jsonl` 调整为项目级全局文件 `.hotpot/issue-candidates.jsonl`，并提供兼容迁移，避免已有候选因为路径变更在 `finish-work` 阶段不可见。
:::

### User Request

::: info 用户原话
`issue-condidates.jsonl` 应该直接在 `.hotpot` 下，而不是某个用户目录下，因为这是属于全局的错误记录，帮我优化一下
:::

用户后续确认：包含从旧 per-user `issue-candidates.jsonl` 到 `.hotpot/issue-candidates.jsonl` 的迁移/合并逻辑，并启用 TDD 模式。

### Approved Design

::: tip
将 issue candidate 的权威路径改为 `.hotpot/issue-candidates.jsonl`，它和 `.hotpot/issues.jsonl` 一样属于项目级 review memory 管线，而不是某个用户 workspace 的私有状态。保留公共 env-var 名称 `HOTPOT_ISSUE_CANDIDATES_FILE`，但其值改为新全局路径，以兼容 OpenCode/Pi 插件和现有 prompt 消费方式。

实现应通过 Rust 路径函数集中收敛，不在调用方拼字符串。`hotpot init`、`hotpot update`、`hotpot hook bootstrap` 和 `hotpot issues candidate {list,add,clear}` 都应使用同一条全局路径。

兼容迁移采用一次性、幂等策略：当初始化或访问候选文件时，扫描 `.hotpot/workspaces/*/issue-candidates.jsonl` 中的非空旧 JSONL 行，追加合并到 `.hotpot/issue-candidates.jsonl`，然后清空旧文件，防止后续重复迁移。迁移必须持有全局 candidates 文件锁，避免并发 add/clear 与迁移互相覆盖。旧文件如果不存在或为空应静默跳过。

提示词、平台资产说明和架构文档都要同步更新为全局候选文件语义，避免 agent 继续认为候选位于 per-user workspace。代码输出仍保持英文；自然语言文档可按现有中英文文档分别维护。
:::

### Alternatives Considered

- 只改 `issue_candidates_file_path` 返回 `.hotpot/issue-candidates.jsonl`，不迁移旧文件。实现最小，但已有用户目录中的候选会在 `finish-work` 中不可见，存在数据丢失感知。
- 保留 per-user 文件但在 `finish-work` 汇总所有用户候选。会让读取路径和写入路径语义分裂，也无法满足“应该直接在 `.hotpot` 下”的核心诉求。
- 推荐并批准：改为单一全局 candidates 文件，并加入旧 per-user 文件的幂等迁移。它让路径语义、env-var、CLI 和文档保持一致，同时保护已有数据。

### Requirements

- `paths::issue_candidates_file_path` 必须返回 `<root>/.hotpot/issue-candidates.jsonl`，不再依赖 `username` 决定文件位置。
- `HOTPOT_ISSUE_CANDIDATES_FILE` 必须指向 `.hotpot/issue-candidates.jsonl`，并继续通过 `path_to_agent_string` 输出 POSIX 形式。
- `hotpot init` / `hotpot update` / `hook bootstrap` 必须确保全局候选文件存在。
- `hotpot issues candidate list/add/clear` 必须读写同一个全局候选文件。
- 已存在的 `.hotpot/workspaces/*/issue-candidates.jsonl` 非空内容必须能迁移合并到全局文件，并清空旧文件以避免重复迁移。
- 迁移与追加、清空等写操作必须使用 `src/lock.rs::with_file_lock` 保护全局 candidates 文件。
- 更新 `docs/ARCH.md`、`docs/ARCH.zh_CN.md`，以及源 prompt/assets 中关于候选文件路径和“per-user”的描述。
- 保持结构性 token、CLI 命令、env-var 名称、JSON 字段名为英文。

### Non-Goals

::: details Non-Goals
- 不重命名公共 env-var `HOTPOT_ISSUE_CANDIDATES_FILE`。
- 不改变 `.hotpot/issues.jsonl` 的长期记忆晋升语义。
- 不改变 `IssueCandidate` JSON schema。
- 不实现复杂去重或语义合并；旧文件迁移只做 JSONL 行级合并，后续去重仍由 `summarize-issue-candidates.md` 的 LLM 流程处理。
- 不删除旧 per-user `issue-candidates.jsonl` 文件；迁移后清空即可，减少破坏性操作。
:::

### Project Context

当前架构文档仍描述 `Issue candidates` 位于 `<workspace>/issue-candidates.jsonl`，目录树也把它放在 `.hotpot/workspaces/<username>/` 下。代码入口如下：

- `src/paths.rs::issue_candidates_file_path(root_dir, username)` 当前返回 `workspace_dir(root_dir, username).join("issue-candidates.jsonl")`。
- `src/issues.rs` 通过 `ensure_issue_candidates_exists(root_dir, username)`、`get_issue_candidates_list`、`append_issue_candidate`、`clear_issue_candidates` 读写候选。
- `src/context.rs::build_context` 把 `paths::issue_candidates_file_path(&root_dir, &username)` 写入 `HOTPOT_ISSUE_CANDIDATES_FILE`。
- `src/workspace.rs::ensure_workspace_skeleton` 当前会在每个用户 workspace 下创建 `issue-candidates.jsonl`。
- `src/commands/issues.rs` 的 candidate 子命令解析 username 后传给 issues 层。
- `src/commands/update.rs` 测试 `update_creates_workspace_skeleton_on_first_run` 断言 per-user candidates 文件存在。
- `.hotpot/prompts/*` 是当前项目安装后的 prompt，`assets/prompts/*` 是源资产，执行 agent 应优先改源资产并按需要同步已安装 `.hotpot/prompts/*`，因为本仓库同时包含产品源和当前测试安装资产。
- OpenCode/Pi 插件通过 `HOTPOT_ISSUE_CANDIDATES_FILE` 写入，不需要改工具 API，但注释中的旧路径需要更新。
- `docs/ROADMAP.md` 已有待办项：`issue-condidates.jsonl` 应该直接在 `.hotpot` 下。

---

## Plan

### Mode

- tdd: true

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/paths.rs` | Modify | 将 candidates 路径集中改为 `.hotpot/issue-candidates.jsonl`，并补齐双语 doc comments。 |
| `src/issues.rs` | Modify | 增加全局候选文件 ensure/迁移逻辑，调整读写函数和测试。 |
| `src/workspace.rs` | Modify | workspace skeleton 不再创建 per-user candidates，而是确保全局 candidates 文件。 |
| `src/context.rs` | Modify | `HOTPOT_ISSUE_CANDIDATES_FILE` 输出新全局路径，必要时触发 ensure/迁移。 |
| `src/commands/issues.rs` | Modify | CLI 文档和实现从“当前用户候选”调整为“项目级全局候选”。 |
| `src/commands/update.rs` | Test | 更新 `update_creates_workspace_skeleton_on_first_run` 对新路径的断言。 |
| `src/lock.rs` | Modify | 更新锁模块注释中 candidates 文件路径。 |
| `docs/ARCH.md` | Modify | 更新英文架构、目录树和注意事项。 |
| `docs/ARCH.zh_CN.md` | Modify | 更新中文架构、目录树和注意事项。 |
| `assets/prompts/record-issue-candidate.md` | Modify | 更新候选写入路径说明。 |
| `assets/prompts/summarize-issue-candidates.md` | Modify | 更新候选来源路径说明。 |
| `assets/prompts/hotpot-execute.md` | Modify | 更新执行流程中禁止提前写入的路径说明和 per-user 文案。 |
| `.hotpot/prompts/record-issue-candidate.md` | Modify | 同步当前安装 prompt，保证本仓库工作流马上使用新说明。 |
| `.hotpot/prompts/summarize-issue-candidates.md` | Modify | 同步当前安装 prompt，保证 finish-work 读取新说明。 |
| `.hotpot/prompts/hotpot-execute.md` | Modify | 同步当前安装 prompt，保证 execute 工作流路径说明正确。 |
| `assets/platforms/opencode/plugins/hotpot-review-memory.ts` | Modify | 更新插件注释中的候选路径语义。 |
| `.opencode/plugins/hotpot-review-memory.ts` | Modify | 同步当前安装 OpenCode 插件注释。 |
| `assets/platforms/pi/extensions/hotpot/index.ts` | Modify | 如有路径说明，更新为全局 candidates 文件。 |
| `.pi/extensions/hotpot/index.ts` | Modify | 同步当前安装 Pi 扩展说明。 |

### Implementation Tasks

#### Task 1: 锁定全局路径和 env-var 行为

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/paths.rs` | Modify | 修改路径函数并添加/更新双语 doc comments。 |
| `src/context.rs` | Modify | 让 `HOTPOT_ISSUE_CANDIDATES_FILE` 指向全局路径。 |
| `src/context.rs` | Test | 增加或更新 context/path 单元测试验证新 env-var 路径。 |

##### Red

- [x] R1: 在 `src/context.rs` 的 `#[cfg(test)] mod tests` 中新增测试 `context_uses_global_issue_candidates_file`，创建临时 root，调用 `build_context` 可访问的路径解析入口或 `Context::resolve(Some(root))`，断言 `context.issue_candidates_file` 以 `/.hotpot/issue-candidates.jsonl` 结尾，且不包含 `/workspaces/`。
- [x] R2: 运行 `cargo test context::tests::context_uses_global_issue_candidates_file`; **expect failure**，失败应显示当前仍指向 `.hotpot/workspaces/<username>/issue-candidates.jsonl`。

##### Green

- [x] G1: 在 `src/paths.rs` 中把 `issue_candidates_file_path` 改为返回 `hotpot_dir(root_dir).join("issue-candidates.jsonl")`；如果保留 `username` 参数用于最小改动，应将参数命名为 `_username` 并在双语 doc comment 中说明该参数仅为兼容调用签名，路径已是项目级全局。
- [x] G2: 在 `src/context.rs::build_context` 保持通过 `path_to_agent_string` 输出该路径，确保 `HOTPOT_ISSUE_CANDIDATES_FILE` 不含 Windows 反斜杠。
- [x] G3: 运行 `cargo test context::tests::context_uses_global_issue_candidates_file`; **expect pass**。
- [x] G4: 运行 `cargo test paths`; **expect pass** 或没有匹配测试但不报编译错误。

##### Refactor

- [x] F1: 检查 `src/paths.rs` 是否仍有未使用的 `Path` import；若未使用则移除，并补齐所有新增/修改函数的中英双语 doc comments。
- [x] F2: 运行 `cargo test context::tests::context_uses_global_issue_candidates_file`; **expect pass**。

:::

#### Task 2: 实现旧 per-user candidates 的幂等迁移

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/issues.rs` | Modify | 增加迁移旧 candidates 文件到全局文件的逻辑。 |
| `src/issues.rs` | Test | 新增迁移行为测试和现有 candidates JSONL 测试更新。 |

##### Red

- [x] R1: 在 `src/issues.rs` 测试模块新增 `migrates_legacy_workspace_candidates_to_global_file`，使用临时 root 创建 `.hotpot/workspaces/alice/issue-candidates.jsonl` 和 `.hotpot/workspaces/bob/issue-candidates.jsonl`，各写入一条合法 `IssueCandidate` JSONL。
- [x] R2: 测试调用 `get_issue_candidates_list(&root_dir, "alice")`，断言返回 2 条候选，`.hotpot/issue-candidates.jsonl` 包含 2 条非空行，两个旧文件内容被清空；再次调用 `get_issue_candidates_list` 后全局文件行数仍为 2，证明不会重复迁移。
- [x] R3: 运行 `cargo test issues::tests::migrates_legacy_workspace_candidates_to_global_file`; **expect failure**，失败应来自当前实现不扫描旧 workspace 文件。

##### Green

- [x] G1: 在 `src/issues.rs` 中实现 `ensure_issue_candidates_exists(root_dir, username)` 的全局语义：确保 `.hotpot/issue-candidates.jsonl` 父目录和文件存在，然后在全局 candidates 文件锁内迁移旧 `.hotpot/workspaces/*/issue-candidates.jsonl` 的非空行。
- [x] G2: 迁移逻辑应跳过全局文件本身、跳过不存在/空旧文件；对每个旧文件读取非空行并追加到全局文件末尾，然后用空字符串清空旧文件。不要删除旧文件。
- [x] G3: 迁移过程中不要 spawn 子进程；所有文件操作在 Rust 内完成，符合 `src/lock.rs::with_file_lock` 约束。
- [x] G4: 保持 `get_issue_candidates_list`、`append_issue_candidate`、`clear_issue_candidates` 的函数签名不变以减少调用面改动，但其内部都应使用全局 candidates 文件。
- [x] G5: 运行 `cargo test issues::tests::migrates_legacy_workspace_candidates_to_global_file`; **expect pass**。
- [x] G6: 运行 `cargo test issues::tests::test_issue_candidates_jsonl`; **expect pass**，并确认测试不污染真实仓库 `.hotpot/issue-candidates.jsonl`。

##### Refactor

- [x] F1: 将迁移相关 helper 保持在 `src/issues.rs` 内部私有函数，例如 `migrate_legacy_issue_candidates`，并添加中英双语 doc comments；避免新增公开 API，除非测试无法覆盖。
- [x] F2: 运行 `cargo test issues::tests`; **expect pass**。

:::

#### Task 3: 更新 init/update/workspace 与 candidate CLI 语义

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/workspace.rs` | Modify | skeleton 创建全局 candidates 文件，不再创建 per-user candidates 文件。 |
| `src/commands/update.rs` | Test | 更新 update skeleton 测试期望。 |
| `src/commands/issues.rs` | Modify | CLI 文档和注释改为项目级全局候选。 |
| `src/lock.rs` | Modify | 锁注释路径同步。 |

##### Red

- [x] R1: 修改 `src/commands/update.rs::tests::update_creates_workspace_skeleton_on_first_run`，断言 `dir.join(".hotpot/issue-candidates.jsonl").is_file()`，并断言 `dir.join(".hotpot/workspaces/alice/issue-candidates.jsonl")` 不存在或不是文件。
- [x] R2: 运行 `! rg --color never 'current user|当前用户|per-user candidates|<workspace>/issue-candidates\.jsonl|workspaces/<u>/issue-candidates\.jsonl' src/workspace.rs src/commands/issues.rs src/lock.rs`; **expect failure**，失败应来自 Task 3 剩余范围中仍存在 per-user candidates / current-user candidates 的陈旧代码文案。此 Red 替代原路径行为 Red：Task 1 已把 `paths::issue_candidates_file_path` 改为全局路径，导致原 Red 首次通过，无法再区分 Task 3 剩余缺陷。

##### Green

- [x] G1: 更新 `src/workspace.rs::ensure_workspace_skeleton` 的双语模块/函数注释，说明 workspace skeleton 只包含 `overview.jsonl` 和 `tasks/`，项目级 candidates 文件在 `.hotpot/issue-candidates.jsonl`。
- [x] G2: 在 `src/workspace.rs` 中改为确保 `paths::issue_candidates_file_path(root_dir, username)` 返回的全局文件存在；不要在 workspace 下写 candidates 文件。
- [x] G3: 更新 `src/commands/issues.rs` 中 `Candidate`、`CandidateCommand`、`add_candidates`、`list_candidates`、`clear_candidates` 的中英 doc comments，将“current user/per-user”改为“project-shared/global temporary candidates”。实现可继续解析 username 并传入现有函数签名，但注释必须说明 username 不再决定 candidates 文件路径。
- [x] G4: 更新 `src/lock.rs` 文件头中 candidates 路径为 `.hotpot/issue-candidates.jsonl`。
- [x] G5: 运行 `cargo test commands::update::tests::update_creates_workspace_skeleton_on_first_run`; **expect pass**。
- [x] G6: 运行 `cargo test commands::update::tests::update_is_idempotent_on_second_run`; **expect pass**。

##### Refactor

- [x] F1: 检查 `src/workspace.rs` 是否仍存在误导性 per-user candidates 文案；全部改为中英一致。
- [x] F2: 运行 `cargo test commands::update::tests`; **expect pass**。

:::

#### Task 4: 同步 prompt、平台资产和架构文档

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify | 英文架构同步全局 candidates 文件。 |
| `docs/ARCH.zh_CN.md` | Modify | 中文架构同步全局 candidates 文件。 |
| `assets/prompts/record-issue-candidate.md` | Modify | 源 prompt 路径说明改为 `.hotpot/issue-candidates.jsonl`。 |
| `assets/prompts/summarize-issue-candidates.md` | Modify | 源 prompt 候选来源改为全局文件。 |
| `assets/prompts/hotpot-execute.md` | Modify | 源 execute prompt 写入约束和报告文案改为全局候选。 |
| `.hotpot/prompts/record-issue-candidate.md` | Modify | 同步安装 prompt。 |
| `.hotpot/prompts/summarize-issue-candidates.md` | Modify | 同步安装 prompt。 |
| `.hotpot/prompts/hotpot-execute.md` | Modify | 同步安装 prompt。 |
| `assets/platforms/opencode/plugins/hotpot-review-memory.ts` | Modify | 更新注释旧路径。 |
| `.opencode/plugins/hotpot-review-memory.ts` | Modify | 同步安装插件注释。 |

##### Red

- [x] R1: 运行 `rg "workspaces/.+issue-candidates|per-user review-memory drafts|当前用户.*issue 候选|per-user memory log" docs assets .hotpot .opencode .pi`; **expect matches**，记录需要更新的路径和文案。
- [x] R2: 运行 `rg "issue-condidates|issue-candidates" docs/ROADMAP.md`; **expect match**，确认已有 roadmap 待办可以在实现后标记完成或调整描述。

##### Green

- [x] G1: 更新 `docs/ARCH.md`：核心概念中 `Issue candidates` 改为 `.hotpot/issue-candidates.jsonl` 项目级临时候选；目录树把它放在 `.hotpot/` 下而不是 `workspaces/<username>/` 下；执行流程/注意事项中锁路径改为全局 candidates 文件。
- [x] G2: 更新 `docs/ARCH.zh_CN.md`，内容与英文架构等价，使用简体中文。
- [x] G3: 更新 `assets/prompts/record-issue-candidate.md` 和 `.hotpot/prompts/record-issue-candidate.md`，说明临时候选写入 `.hotpot/issue-candidates.jsonl`，仍不是长期记忆。
- [x] G4: 更新 `assets/prompts/summarize-issue-candidates.md` 和 `.hotpot/prompts/summarize-issue-candidates.md`，说明临时候选来自 `.hotpot/issue-candidates.jsonl`。
- [x] G5: 更新 `assets/prompts/hotpot-execute.md` 和 `.hotpot/prompts/hotpot-execute.md` 中关于“不要提前写入”的路径，从 `.hotpot/workspaces/<user>/issue-candidates.jsonl` 改为 `.hotpot/issue-candidates.jsonl`，并把“per-user memory log”改成全局候选日志语义。
- [x] G6: 更新 `assets/platforms/opencode/plugins/hotpot-review-memory.ts` 与 `.opencode/plugins/hotpot-review-memory.ts` 的文件头注释旧路径。
- [x] G7: 如果 `assets/platforms/pi/extensions/hotpot/index.ts` 或 `.pi/extensions/hotpot/index.ts` 中只有 env-var 使用且无旧路径注释，可不改；若存在旧路径说明则同步更新。
- [x] G8: 在 `docs/ROADMAP.md` 将对应待办标记为完成或删除该已完成待办，避免重复任务。
- [x] G9: 运行 `rg "workspaces/.+issue-candidates|per-user review-memory drafts|per-user memory log|当前用户工作区下的临时 issue 候选" docs assets .hotpot .opencode .pi`; **expect no stale matches**，除非是明确描述“legacy migration”的文案。

##### Refactor

- [x] F1: 检查中英文架构文档中的目录树缩进和术语是否保持一致。
- [x] F2: `cargo test --doc` 不适用于当前 binary-only package（Cargo 返回 `error: no library targets found in package hotpot`）；改为运行 `cargo test` 作为适用于当前 crate 的等价 Rust 验证；**expect pass**。

:::

#### Task 5: 全量验证 CLI 行为和测试套件

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/issues.rs` | Test | 验证 candidate add/list/clear 全局行为。 |
| `src/context.rs` | Test | 验证 hook/context 输出路径。 |
| `src/commands/update.rs` | Test | 验证 init/update skeleton 行为。 |
| `docs/ARCH.md` | Modify | 若验证发现流程描述不完整，补齐。 |
| `docs/ARCH.zh_CN.md` | Modify | 同步中文描述。 |

##### Red

- [x] R1: 在实现完成前运行 `cargo test`; **expect existing failures from newly added Red tests only**。如果出现无关失败，先记录失败名和原因，不要扩大改动范围。
- [x] R2: 运行 `cargo run -- hook bootstrap --json` 或项目中等价 bootstrap 命令（先查看 `cargo run -- hook --help` 确认实际参数）；**expect before final implementation it may still show old path**，以该行为作为最终验证目标。

##### Green

- [x] G1: 运行 `cargo test`; **expect pass**。若现有测试因为真实仓库 `.hotpot/issues.jsonl` 数据不满足断言而失败，只修正测试隔离问题，不降低本任务断言强度。
- [x] G2: 运行 `cargo run -- task list --json`; **expect pass**，且不创建 per-user candidates 文件。
- [x] G3: 运行 `cargo run -- hook bootstrap --json` 或确认后的等价命令；**expect** 输出中 `HOTPOT_ISSUE_CANDIDATES_FILE` 指向 `/Users/bytedance/RustProjects/hotpot/.hotpot/issue-candidates.jsonl`，不包含 `/workspaces/`。
- [x] G4: 手动 CLI 验证：向 `cargo run -- issues candidate add` 输入一条合法 `IssueCandidate` JSONL，运行 `cargo run -- issues candidate list` 应看到该条，运行 `cargo run -- issues candidate clear` 应输出 `{"cleared":1}` 或对应数量，再次 list 应为空。验证后不要留下测试候选污染真实 `.hotpot/issue-candidates.jsonl`。

##### Refactor

- [x] F1: 运行 `cargo fmt`; **expect success**。
- [x] F2: 运行 `cargo test`; **expect pass**。
- [x] F3: 运行 `git diff --check`; **expect no whitespace errors**。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `cargo test context::tests::context_uses_global_issue_candidates_file` | 新增 context 路径测试通过。 |
| `cargo test issues::tests::migrates_legacy_workspace_candidates_to_global_file` | 旧 per-user candidates 迁移到全局文件且不重复迁移。 |
| `cargo test issues::tests` | issues 模块候选读写、清空、迁移相关测试通过。 |
| `cargo test commands::update::tests` | update skeleton 行为测试通过。 |
| `cargo test` | 全量 Rust 测试通过。 |
| `cargo run -- hook bootstrap --json` | `HOTPOT_ISSUE_CANDIDATES_FILE` 指向 `.hotpot/issue-candidates.jsonl`，不包含 `/workspaces/`。 |
| `cargo run -- issues candidate add/list/clear` | add/list/clear 操作同一个全局 candidates 文件，clear 后 list 为空。 |
| `rg "workspaces/.+issue-candidates|per-user memory log" docs assets .hotpot .opencode .pi` | 不再出现陈旧路径/语义，除非文本明确描述 legacy migration。 |
| `cargo fmt` | 格式化成功。 |
| `git diff --check` | 无空白错误。 |

### Risks and Watchouts

::: warning
- 这是持久化路径变更，必须迁移旧 per-user 文件，否则 finish-work 可能看不到已有候选。
- `HOTPOT_ISSUE_CANDIDATES_FILE` 是跨平台公共契约，不能重命名；只改变它的值。
- 锁内禁止 spawn 子进程；迁移只能使用 Rust 文件 IO。
- 本仓库同时包含源资产 `assets/` 和已安装资产 `.hotpot/`、`.opencode/`、`.pi/`，执行时需要同步会影响当前工作流的安装副本。
- 不要把长期 `.hotpot/issues.jsonl` 与临时 `.hotpot/issue-candidates.jsonl` 混淆；晋升仍需用户确认。
:::

---

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Follow TDD mode exactly: each `#### Task N` must run Red before Green before Refactor.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task.
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff.
- Run the validation commands before reporting completion.
