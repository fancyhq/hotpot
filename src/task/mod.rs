//! Task ledger module.
//!
//! Public API for `overview.jsonl` task rows. Splits the implementation
//! across three private submodules — `storage` (read/write primitives and
//! queries), `transitions` (one-way state-machine entrypoints), and
//! `create` (append + two-tier active guards) — and re-exports the public
//! surface here so external callers (`crate::commands::task`) keep using
//! `task::<fn>` paths unchanged.
//!
//! Cross-process safety: every public mutator wraps its body in
//! [`crate::lock::with_file_lock`] over `overview.jsonl`. The lock scope
//! must span the entire read-check-write critical section; do not move
//! work outside the lock during refactors.
//!
//! 任务台账模块。
//!
//! 对外提供 `overview.jsonl` 行级 API。实现被拆到三个私有子模块：
//! `storage`（读写原语与查询）、`transitions`（单向状态机入口）、
//! `create`（追加 + 两层 active 守卫）；公共 API 在此通过 `pub use`
//! 重新导出，使外部调用方（`crate::commands::task`）仍以
//! `task::<fn>` 调用，**零迁移成本**。
//!
//! 跨进程并发：所有对外写函数都用 [`crate::lock::with_file_lock`] 包住
//! `overview.jsonl`。锁作用域必须覆盖整个"读 → 校验 → 写"临界区，重构
//! 时不可把任何写相关步骤挪到锁外。

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

mod create;
mod markdown;
mod storage;
mod transitions;

pub use create::create_task;
// `get_task_filename` 当前没有 crate 内消费者，但原 `task.rs` 已把它作为
// `pub fn` 暴露给外部；保留 `pub use` 以稳定公共 API，并允许未使用的导入
// 警告，避免拆分本身改变可见性。
// `get_task_filename` has no in-crate consumer right now, but the previous
// monolithic `task.rs` already exposed it as a `pub fn`. Keep the re-export
// to stabilize the public surface and silence the unused-import lint that
// `pub use` triggers without an in-crate caller.
pub use markdown::sync_task_file_status;
#[allow(unused_imports)]
pub use storage::{
    get_active_task, get_active_task_count, get_active_task_filepath, get_task_by_id,
    get_task_filename, get_task_list,
};
// `OverviewSyncOutcome` and `update_overview_status` are re-exported as part
// of the module's public API even though no current in-crate consumer names
// them explicitly — `sync_task_file_status` returns `OverviewSyncOutcome`, and
// the string-level helper is available for direct use in integration tests.
// 保留公共 API 的完整可见性：`sync_task_file_status` 返回 `OverviewSyncOutcome`，
// `update_overview_status` 供集成测试直接调用。
#[allow(unused_imports)]
pub use markdown::{OverviewSyncOutcome, update_overview_status};
pub use transitions::{
    attach_worktree, detach_worktree, mark_task_cancelled, mark_task_done, mark_task_resumed,
    stop_all_active_tasks,
};

/// Task lifecycle status persisted in `overview.jsonl`.
/// 任务行在 `overview.jsonl` 中的生命周期状态。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    #[serde(rename = "Done")]
    Done,
    #[serde(rename = "In Progress")]
    InProgress,
    #[serde(rename = "Cancelled")]
    Cancelled,
}

impl TaskStatus {
    /// Returns the JSON-canonical string form of this status.
    /// 返回与 JSON 序列化一致的字符串形式。
    pub fn as_str(&self) -> &str {
        match self {
            TaskStatus::Done => "Done",
            TaskStatus::InProgress => "In Progress",
            TaskStatus::Cancelled => "Cancelled",
        }
    }
}

/// A single row of `overview.jsonl`.
/// `overview.jsonl` 中的一行任务记录。
///
/// `worktree_path` / `worktree_branch` / `worktree_base_branch` are
/// `#[serde(default)]` so rows written by older Hotpot versions (which
/// lack these columns) still parse cleanly. A task may have a worktree
/// attached at any point in its lifecycle; presence of `worktree_path`
/// is the source of truth that finish-work uses to switch into the
/// "commit in worktree → optionally merge back" flow.
///
/// `worktree_path` / `worktree_branch` / `worktree_base_branch` 用
/// `#[serde(default)]` 保证旧版本写入的 `overview.jsonl` 行（缺少这
/// 三列）依旧能正确解析。任务在生命周期的任意阶段都可挂上 worktree；
/// finish-work 通过 `worktree_path` 是否存在判断要不要进入"在 worktree
/// 内 commit → 可选合并回主分支"的分支流程。
#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct TaskInfo {
    pub time: NaiveDate,
    pub task_id: String,
    pub title: String,
    pub commit: Option<String>,
    pub status: TaskStatus,
    pub active: bool,
    /// Absolute path to the attached git worktree, if any.
    /// 已挂载 git worktree 的绝对路径（未挂载时为 `None`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<String>,
    /// Branch name the worktree checks out (always `hotpot/<task_id>`).
    /// 该 worktree 检出的分支名（恒为 `hotpot/<task_id>`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_branch: Option<String>,
    /// Branch the worktree was forked from (captured at create time so
    /// `finish-work`'s "merge back" choice has a deterministic target).
    /// 创建 worktree 时所基于的分支名，用于 finish-work 的"合并回主仓"选项
    /// 有确定的目标分支。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_base_branch: Option<String>,
}

/// Mode controlling how [`create_task`] reconciles existing `active` rows
/// and what flag the new row carries. There is no fourth mode and no
/// "force-clear stale" toggle — stale active rows are always cleared as a
/// fixed side effect of every create call.
///
/// State machine summary (combined with the create logic):
/// - [`CreateMode::Default`] — new row is `active=true`; bails with
///   `ACTIVE_CONFLICT:` prefix if any existing row has
///   `active=true && status=InProgress`. Stale `active=true` rows
///   (status `Done` / `Cancelled`) are silently cleared regardless.
/// - [`CreateMode::Switch`] — atomically clears EVERY existing
///   `active=true` row (both In-Progress and stale), then appends the
///   new row with `active=true`. Caller has already asked the user.
/// - [`CreateMode::Inactive`] — leaves In-Progress active rows untouched;
///   still clears stale active rows; appends the new row with
///   `active=false`.
///
/// 控制 [`create_task`] 如何处理已有 `active` 行以及新行的 active 字段。
/// 不接受第四种模式、也不接受可调"是否清陈旧 active"开关——陈旧 active
/// 始终静默清理，这是 create 的固定副作用。
///
/// 三态对照（与 create 实现合并）：
/// - [`CreateMode::Default`]：新行 `active=true`；若存在
///   `active=true && status=InProgress` 行则 bail（错误以
///   `ACTIVE_CONFLICT:` 开头），陈旧 active 行（status 为 `Done` /
///   `Cancelled`）始终静默清理。
/// - [`CreateMode::Switch`]：原子地把**所有** `active=true` 行（含
///   In-Progress 与陈旧）清零，然后追加新行 `active=true`。调用方应
///   已经问过用户。
/// - [`CreateMode::Inactive`]：保留 In-Progress active 行；仍清理陈旧
///   active 行；追加新行 `active=false`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateMode {
    /// New row is `active=true`; bail on conflict with In-Progress active.
    /// 新行 active=true；遇 In-Progress active 冲突即 bail。
    Default,
    /// New row is `active=true`; atomically clear every existing active.
    /// 新行 active=true；原子清掉所有现有 active 行。
    Switch,
    /// New row is `active=false`; only clear stale active rows.
    /// 新行 active=false；仅清陈旧 active 行。
    Inactive,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::storage::rewrite_overview_with;
    use super::*;
    use crate::paths::overview_file_path;
    use tempfile::{Builder, TempDir};

    /// Wipes the per-username overview ledger so each test starts from a
    /// clean slate. Cargo runs tests in parallel by default, so every test
    /// must use a UNIQUE username — sharing one would race on the same
    /// `overview.jsonl`.
    ///
    /// 清空指定用户的 overview 台账，让每个测试从空状态开始。`cargo test`
    /// 默认并行跑用例，因此每个测试**必须**使用独立 username，否则会在
    /// 同一份 `overview.jsonl` 上互相覆盖。
    fn reset_workspace(root_dir: &str, username: &str) {
        let path = overview_file_path(root_dir, username);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_get_task_list() {
        let root = make_isolated_project_dir("get-task-list");
        let root_dir = root.path().display().to_string();
        let username = "test";
        let task_list = get_task_list(&root_dir, username).unwrap();
        for info in &task_list {
            let result = info.status == TaskStatus::InProgress;
            println!("任务状态：{result}");
        }
        println!("获取任务列表：{task_list:#?}")
    }

    #[test]
    fn test_create_task() {
        let root = make_isolated_project_dir("create-task");
        let root_dir = root.path().display().to_string();
        let username = "test";
        // 用 Switch 模式避免与既有 active 行冲突；只验证创建本身能跑通。
        // Use Switch to avoid colliding with any existing active row; this
        // test only validates that the create path works.
        let new_row = create_task(
            &root_dir,
            username,
            "我的新任务",
            Some("deadbeef"),
            CreateMode::Switch,
            false,
        )
        .unwrap();
        let list = get_task_list(&root_dir, username).unwrap();
        println!("现在有 {} 条任务", list.len());
        println!("新任务返回值：{new_row:#?}");
        println!("最后一条：{:#?}", list.last().unwrap());
    }

    /// 空 overview + Default → 新行唯一 active=true。
    #[test]
    fn test_create_default_on_empty_makes_new_row_active() {
        let root = make_isolated_project_dir("create-default-empty");
        let root_dir = root.path().display().to_string();
        let username = "test_create_default_empty";
        reset_workspace(&root_dir, username);

        let new_row =
            create_task(&root_dir, username, "A", None, CreateMode::Default, false).unwrap();

        assert!(new_row.active);
        assert_eq!(new_row.status, TaskStatus::InProgress);

        let list = get_task_list(&root_dir, username).unwrap();
        let actives: Vec<&TaskInfo> = list.iter().filter(|t| t.active).collect();
        assert_eq!(actives.len(), 1, "应只有 1 条 active 行");
        assert_eq!(actives[0].task_id, new_row.task_id);
    }

    /// Default 遇 In-Progress active 必须 bail，且不改动台账。
    #[test]
    fn test_create_default_bails_on_in_progress_conflict() {
        let root = make_isolated_project_dir("create-default-conflict");
        let root_dir = root.path().display().to_string();
        let username = "test_create_default_conflict";
        reset_workspace(&root_dir, username);

        // 先建一个 In-Progress active 任务。
        let first =
            create_task(&root_dir, username, "A", None, CreateMode::Default, false).unwrap();

        let before = fs::read_to_string(overview_file_path(&root_dir, username)).unwrap();

        let err = create_task(&root_dir, username, "B", None, CreateMode::Default, false)
            .expect_err("Default 模式应该 bail");
        let msg = format!("{err}");
        assert!(
            msg.starts_with("ACTIVE_CONFLICT:"),
            "错误消息应以 ACTIVE_CONFLICT: 开头，实际：{msg}"
        );
        assert!(msg.contains(&first.task_id), "错误消息应列出冲突 id");

        let after = fs::read_to_string(overview_file_path(&root_dir, username)).unwrap();
        assert_eq!(before, after, "bail 不应改动 overview.jsonl");
    }

    /// Switch 模式抢占：原 In-Progress active 被清零，新行成为唯一 active。
    #[test]
    fn test_create_switch_preempts_existing_active() {
        let root = make_isolated_project_dir("create-switch");
        let root_dir = root.path().display().to_string();
        let username = "test_create_switch";
        reset_workspace(&root_dir, username);

        let first =
            create_task(&root_dir, username, "A", None, CreateMode::Default, false).unwrap();
        let second =
            create_task(&root_dir, username, "B", None, CreateMode::Switch, false).unwrap();

        assert!(second.active);
        let list = get_task_list(&root_dir, username).unwrap();
        let actives: Vec<&TaskInfo> = list.iter().filter(|t| t.active).collect();
        assert_eq!(actives.len(), 1, "Switch 后应只有 1 条 active");
        assert_eq!(actives[0].task_id, second.task_id);

        let first_after = list.iter().find(|t| t.task_id == first.task_id).unwrap();
        assert!(!first_after.active, "原 In-Progress active 应被清零");
        assert_eq!(
            first_after.status,
            TaskStatus::InProgress,
            "Switch 不改 status，只清 active"
        );
    }

    /// Inactive 模式旁路：原 In-Progress active 保持，新行 active=false。
    #[test]
    fn test_create_inactive_keeps_existing_active() {
        let root = make_isolated_project_dir("create-inactive");
        let root_dir = root.path().display().to_string();
        let username = "test_create_inactive";
        reset_workspace(&root_dir, username);

        let first =
            create_task(&root_dir, username, "A", None, CreateMode::Default, false).unwrap();
        let second =
            create_task(&root_dir, username, "B", None, CreateMode::Inactive, false).unwrap();

        assert!(!second.active, "Inactive 模式新行应 active=false");
        let list = get_task_list(&root_dir, username).unwrap();
        let actives: Vec<&TaskInfo> = list.iter().filter(|t| t.active).collect();
        assert_eq!(actives.len(), 1, "原 In-Progress active 应保持");
        assert_eq!(actives[0].task_id, first.task_id);
    }

    /// 陈旧 active（Done 行 active=true）应被 Default 创建静默清理；
    /// 新行成为唯一 active。
    #[test]
    fn test_create_default_silently_clears_stale_active() {
        let root = make_isolated_project_dir("clear-stale");
        let root_dir = root.path().display().to_string();
        let username = "test_create_clear_stale";
        reset_workspace(&root_dir, username);

        let stale = create_task(
            &root_dir,
            username,
            "stale",
            None,
            CreateMode::Default,
            false,
        )
        .unwrap();
        // 把 stale 标 Done（active 会被清零），再手工把 active 改回 true 模拟陈旧。
        let _ = mark_task_done(&root_dir, username, Some(&stale.task_id), None).unwrap();
        rewrite_overview_with(&root_dir, username, |t| {
            if t.task_id == stale.task_id {
                t.active = true; // 重新制造陈旧 active=true && status=Done
            }
        })
        .unwrap();

        // 此时 stale 是 active=true && status=Done；Default 应静默清掉它。
        let new_row = create_task(
            &root_dir,
            username,
            "fresh",
            None,
            CreateMode::Default,
            false,
        )
        .unwrap();

        assert!(new_row.active);
        let list = get_task_list(&root_dir, username).unwrap();
        let actives: Vec<&TaskInfo> = list.iter().filter(|t| t.active).collect();
        assert_eq!(actives.len(), 1, "陈旧 active 应被清，新行成为唯一 active");
        assert_eq!(actives[0].task_id, new_row.task_id);

        let stale_after = list.iter().find(|t| t.task_id == stale.task_id).unwrap();
        assert!(!stale_after.active);
        assert_eq!(stale_after.status, TaskStatus::Done);
    }

    /// 陈旧 + In-Progress 同时存在 + Inactive → 陈旧被清，In-Progress 保持，
    /// 新行 active=false。
    #[test]
    fn test_create_inactive_clears_stale_keeps_in_progress() {
        let root = make_isolated_project_dir("inactive-mixed");
        let root_dir = root.path().display().to_string();
        let username = "test_create_inactive_mixed";
        reset_workspace(&root_dir, username);

        // 第一条：Default 建后 done，再手工制造陈旧 active=true && status=Done。
        let stale = create_task(
            &root_dir,
            username,
            "stale",
            None,
            CreateMode::Default,
            false,
        )
        .unwrap();
        let _ = mark_task_done(&root_dir, username, Some(&stale.task_id), None).unwrap();
        rewrite_overview_with(&root_dir, username, |t| {
            if t.task_id == stale.task_id {
                t.active = true;
            }
        })
        .unwrap();
        // 第二条：Default 建一个真 In-Progress active。
        let live = create_task(
            &root_dir,
            username,
            "live",
            None,
            CreateMode::Default,
            false,
        )
        .unwrap();
        // 第二条创建时已经把 stale 清了；为模拟方案中的"同时存在"场景，再次手工注入陈旧。
        rewrite_overview_with(&root_dir, username, |t| {
            if t.task_id == stale.task_id {
                t.active = true;
            }
        })
        .unwrap();

        let new_row = create_task(
            &root_dir,
            username,
            "side",
            None,
            CreateMode::Inactive,
            false,
        )
        .unwrap();

        assert!(!new_row.active);
        let list = get_task_list(&root_dir, username).unwrap();
        let actives: Vec<&TaskInfo> = list.iter().filter(|t| t.active).collect();
        assert_eq!(
            actives.len(),
            1,
            "Inactive 应保留 In-Progress active，陈旧应被清"
        );
        assert_eq!(actives[0].task_id, live.task_id);

        let stale_after = list.iter().find(|t| t.task_id == stale.task_id).unwrap();
        assert!(!stale_after.active, "陈旧 active 应被静默清理");
    }

    /// 连续两次 Default 在 In-Progress 存在时 → 两次都 bail，台账未损坏。
    #[test]
    fn test_create_default_repeat_bail_is_idempotent() {
        let root = make_isolated_project_dir("default-repeat");
        let root_dir = root.path().display().to_string();
        let username = "test_create_default_repeat";
        reset_workspace(&root_dir, username);

        let _ = create_task(&root_dir, username, "A", None, CreateMode::Default, false).unwrap();
        let snapshot = fs::read_to_string(overview_file_path(&root_dir, username)).unwrap();

        for _ in 0..2 {
            let err = create_task(&root_dir, username, "B", None, CreateMode::Default, false)
                .expect_err("应 bail");
            assert!(format!("{err}").starts_with("ACTIVE_CONFLICT:"));
        }

        let after = fs::read_to_string(overview_file_path(&root_dir, username)).unwrap();
        assert_eq!(snapshot, after, "重复 bail 不应损坏 overview");
    }

    /// Default workspace 已有 In-Progress 任务时，第二个协作者用 `default`
    /// username 直接 `task create` 应 bail，且消息含「协作者」提示与 `--allow-default`
    /// 旁路说明。
    ///
    /// When the shared "default" workspace already holds an In-Progress task,
    /// a second collaborator hitting it with username == "default" must bail
    /// with a message that names the collaborator hazard and lists bypass
    /// options.
    #[test]
    fn test_create_default_username_bails_with_collaborator_hint() {
        // 该测试**必须**使用字面量 username "default"（守卫匹配的是字面量），
        // 并在自有 tmp project_dir 中运行避免与其他测试污染同一 workspace。
        // 同时持有 env_lock() 序列化所有触碰 HOTPOT_ALLOW_DEFAULT_USERNAME
        // 的测试，防止并发 set/unset 互相干扰。
        let _guard = env_lock();
        let root_dir = make_isolated_project_dir("collab-hint");
        // SAFETY: env mutation is safe through std::env; 2024 edition wants
        // an unsafe block. The mutex above serializes us with the env-bypass
        // test, so no parallel reader sees a half-written state.
        unsafe {
            std::env::remove_var("HOTPOT_ALLOW_DEFAULT_USERNAME");
        }

        let root_dir = root_dir.path().display().to_string();
        let _ = create_task(&root_dir, "default", "A", None, CreateMode::Default, false)
            .expect("first default create should succeed");

        let err = create_task(&root_dir, "default", "B", None, CreateMode::Default, false)
            .expect_err("second default create must bail on collaborator guard");
        let msg = format!("{err}");
        assert!(
            msg.starts_with("ACTIVE_CONFLICT:"),
            "missing ACTIVE_CONFLICT prefix: {msg}"
        );
        assert!(
            msg.contains("default") && msg.contains("workspace"),
            "missing collaborator hint: {msg}"
        );
        assert!(
            msg.contains("--allow-default") || msg.contains("HOTPOT_ALLOW_DEFAULT_USERNAME"),
            "missing bypass options: {msg}"
        );
    }

    /// `allow_default=true` flag 旁路掉协作者守卫；落到 Switch 模式可创建成功。
    ///
    /// `allow_default=true` bypasses the collaborator guard; with Switch mode
    /// the create succeeds (the second-tier mode-aware Default gate would
    /// still block, hence the Switch mode here).
    #[test]
    fn test_create_default_username_allow_flag_bypasses_guard() {
        let _guard = env_lock();
        let root_dir = make_isolated_project_dir("collab-flag-bypass");
        // Mutex 已经把 env-bypass 测试和我们串行化；这里再 unset 以保证
        // 没有遗留：CI 上有可能这个测试是首个跑、env-bypass 测试上一次跑
        // 后没正常清理（panic 等）也能正确恢复。
        unsafe {
            std::env::remove_var("HOTPOT_ALLOW_DEFAULT_USERNAME");
        }

        let root_dir = root_dir.path().display().to_string();
        let _ = create_task(&root_dir, "default", "A", None, CreateMode::Default, false).unwrap();

        // 用 Switch 模式 + allow_default=true：协作者守卫与 mode 守卫都让路。
        let second = create_task(
            &root_dir,
            "default",
            "B",
            None,
            CreateMode::Switch,
            /* allow_default */ true,
        )
        .expect("Switch + allow_default should bypass both guards");

        assert!(second.active, "Switch should set new row active");
        // Default 模式 + allow_default=true 仍应被 Tier-2 mode 守卫拦下，
        // 验证两条守卫互不耦合。
        // Default mode + allow_default=true still hits the tier-2 mode guard.
        let mode_err = create_task(&root_dir, "default", "C", None, CreateMode::Default, true)
            .expect_err("tier-2 guard still applies");
        assert!(
            format!("{mode_err}").starts_with("ACTIVE_CONFLICT:"),
            "tier-2 guard should still bail: {mode_err}"
        );
    }

    /// `HOTPOT_ALLOW_DEFAULT_USERNAME=1` env 与 `--allow-default` 等价。
    ///
    /// Setting `HOTPOT_ALLOW_DEFAULT_USERNAME=1` is equivalent to passing
    /// `--allow-default`. We test this with a serialized helper because env
    /// is process-global and cargo runs tests in parallel; this test
    /// briefly sets the var, runs assertions, then unsets it.
    #[test]
    fn test_create_default_username_env_bypass_equivalent_to_flag() {
        let _guard = env_lock();
        let root_dir = make_isolated_project_dir("collab-env-bypass");
        let root_dir = root_dir.path().display().to_string();
        let _ = create_task(&root_dir, "default", "A", None, CreateMode::Default, false).unwrap();

        // SAFETY: env mutation guarded by env_lock(); 2024 edition flags
        // std::env::set_var/remove_var as unsafe due to global state.
        unsafe {
            std::env::set_var("HOTPOT_ALLOW_DEFAULT_USERNAME", "1");
        }
        let result = create_task(
            &root_dir,
            "default",
            "B",
            None,
            CreateMode::Switch,
            /* allow_default flag */ false,
        );
        unsafe {
            std::env::remove_var("HOTPOT_ALLOW_DEFAULT_USERNAME");
        }
        let second = result.expect("env bypass should match flag bypass");
        assert!(second.active);
    }

    /// 在 cargo 临时目录下创建独立 project 根，让需要使用字面量 username
    /// `"default"` 的测试互不污染（每个测试一份独立 `.hotpot/`）。
    ///
    /// Allocates a unique tmp project root under `env::temp_dir()` so each
    /// test that must operate on the literal `"default"` workspace runs
    /// in isolation.
    fn make_isolated_project_dir(label: &str) -> TempDir {
        Builder::new()
            .prefix(&format!("hotpot-task-{label}-"))
            .tempdir()
            .unwrap()
    }

    /// Serializes any test that reads or writes `HOTPOT_ALLOW_DEFAULT_USERNAME`.
    /// Env vars are process-global and cargo's default parallel runner would
    /// otherwise cause flaky interleaving (one test sets, another asserts
    /// "unset"). Lock once at the top of each env-sensitive test.
    ///
    /// 序列化所有读写 `HOTPOT_ALLOW_DEFAULT_USERNAME` 的测试——env 是进程
    /// 全局，cargo 默认并发会让它们互相干扰。每个 env-敏感测试在最开始锁
    /// 一次，结束自动释放。
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::test_support::acquire_env_lock()
    }

    /// 多线程并发 `create_task` 不丢更新：N 个线程各创建一条任务，最终
    /// `overview.jsonl` 应该恰好有 N 行（用 Inactive 模式避免 active 冲突，
    /// 这样真正测的是写入串行化，而不是模式守卫）。
    ///
    /// Concurrent `create_task` from N threads must result in exactly N rows
    /// in `overview.jsonl` — without the cross-process lock this is the
    /// last-writer-wins scenario (FLOW.md known-gap #8) that drops rows.
    /// We use Inactive mode so the test focuses on serialization, not on
    /// mode guards.
    #[test]
    fn test_concurrent_create_task_does_not_lose_rows() {
        use std::sync::Arc;
        use std::thread;

        // 独立 username + 临时 root_dir，避免与其他并发测试争抢同一台账。
        let root_dir = make_isolated_project_dir("concurrent-create");
        let root_dir = Arc::new(root_dir.path().display().to_string());
        let username = "concurrent_user";
        // 一条 Default seed 行让后续 Inactive 创建有意义（Inactive 必须
        // 看到一条 In-Progress active 才不退化为 Default）。
        // Seed an active In-Progress row so Inactive's "leave live active
        // alone" branch is exercised.
        let _ = create_task(
            &root_dir,
            username,
            "seed",
            None,
            CreateMode::Default,
            false,
        )
        .unwrap();

        const THREADS: u32 = 6;
        let handles: Vec<_> = (0..THREADS)
            .map(|i| {
                let root_dir = Arc::clone(&root_dir);
                let username = username.to_string();
                thread::spawn(move || {
                    create_task(
                        &root_dir,
                        &username,
                        &format!("concurrent-{i}"),
                        None,
                        CreateMode::Inactive,
                        false,
                    )
                    .unwrap_or_else(|e| panic!("thread {i} failed: {e}"))
                })
            })
            .collect();

        let mut titles: Vec<String> = handles
            .into_iter()
            .map(|h| h.join().expect("thread panicked").title)
            .collect();
        titles.sort();

        let final_list = get_task_list(&root_dir, username).unwrap();
        assert_eq!(
            final_list.len() as u32,
            THREADS + 1,
            "expected seed + {THREADS} concurrent rows in overview, got {}: {final_list:?}",
            final_list.len()
        );
        let mut final_titles: Vec<String> = final_list.iter().map(|t| t.title.clone()).collect();
        final_titles.sort();
        let mut expected: Vec<String> = (0..THREADS).map(|i| format!("concurrent-{i}")).collect();
        expected.push("seed".to_string());
        expected.sort();
        assert_eq!(final_titles, expected, "row set mismatch under concurrency");
    }

    /// Build a minimal `TaskInfo` fixture for filename tests.
    /// 构造一个仅用于文件名测试的最小 `TaskInfo` 夹具。
    fn fixture_task(title: &str) -> TaskInfo {
        TaskInfo {
            time: chrono::NaiveDate::from_ymd_opt(2026, 5, 15).unwrap(),
            task_id: "fixture-id".to_string(),
            title: title.to_string(),
            commit: None,
            status: TaskStatus::InProgress,
            active: true,
            worktree_path: None,
            worktree_branch: None,
            worktree_base_branch: None,
        }
    }

    /// kebab-case 标题不应被改动，逐字段保留。
    /// kebab-case titles pass through `get_task_filename` unchanged.
    #[test]
    fn test_get_task_filename_keeps_kebab_case_intact() {
        let task = fixture_task("add-login-retry");
        assert_eq!(
            get_task_filename(&task),
            "2026-05-15-add-login-retry",
            "kebab-case 标题应原样保留"
        );
    }

    /// 含空格的旧标题（兜底路径）：单空格折成 `-`。
    /// Legacy space-separated titles get collapsed to a single `-`.
    #[test]
    fn test_get_task_filename_collapses_single_spaces() {
        let task = fixture_task("add login retry");
        assert_eq!(
            get_task_filename(&task),
            "2026-05-15-add-login-retry",
            "单空格应被折成 `-`"
        );
    }

    /// 多空格 / 首尾空白 / Tab 混合：`split_whitespace` 一次性兜底。
    /// Mixed whitespace (runs, leading/trailing, tabs) all collapse via
    /// `split_whitespace`.
    #[test]
    fn test_get_task_filename_collapses_runs_and_trims() {
        let task = fixture_task("  add   login\tretry  ");
        assert_eq!(
            get_task_filename(&task),
            "2026-05-15-add-login-retry",
            "连续空白、首尾空白、tab 应一并折叠"
        );
    }

    /// `get_active_task_filepath` 遇 >1 条 active 应 bail 列出 id。
    #[test]
    fn test_get_active_task_filepath_rejects_multi_active() {
        let root = make_isolated_project_dir("active-multi-guard");
        let root_dir = root.path().display().to_string();
        let username = "test_active_multi_guard";
        reset_workspace(&root_dir, username);

        let _ = create_task(&root_dir, username, "A", None, CreateMode::Default, false).unwrap();
        let _ = create_task(&root_dir, username, "B", None, CreateMode::Inactive, false).unwrap();
        // 手工把 B 也设成 active=true，制造多 active 状态。
        rewrite_overview_with(&root_dir, username, |t| {
            if t.title == "B" {
                t.active = true;
            }
        })
        .unwrap();

        let err = get_active_task_filepath(&root_dir, username)
            .expect_err("多 active 应 bail，而非静默取首条");
        let msg = format!("{err}");
        assert!(msg.contains("发现 2 条 active=true 的任务"), "实际：{msg}");
    }
}
