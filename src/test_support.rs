//! Shared test utilities for environment variable isolation and panic-safe
//! restoration across the entire crate.
//!
//! Tests that mutate process-global environment variables (`ROOT_DIR`,
//! `HOTPOT_USERNAME`, `HOTPOT_VUEPRESS_ENABLED`, etc.) must use
//! [`ScopedEnvVar`] instead of raw `std::env::set_var` / `remove_var` so
//! that:
//!
//! 1. All env-mutating tests are serialized by a **single crate-wide**
//!    [`Mutex`] — without this, `cargo test`'s parallel runner interleaves
//!    writes across `context.rs`, `task/markdown.rs`, `commands/task.rs`, etc.
//! 2. Original values are **always** restored, even when the test panics,
//!    because [`Drop`] runs during unwinding.
//!
//! # Usage (用法)
//!
//! ```ignore
//! use crate::test_support::ScopedEnvVar;
//!
//! #[test]
//! fn my_test() {
//!     let _env = ScopedEnvVar::new(&[
//!         ("ROOT_DIR", Some("/tmp/test-root")),
//!         ("HOTPOT_VUEPRESS_ENABLED", None),  // unset
//!     ]);
//!     // ... test body using the env vars ...
//!     // _env drops here, restoring everything — even on panic.
//! }
//! ```
//!
//! 测试共享工具：提供 panic-safe 环境变量守卫与跨模块全局锁。
//!
//! 所有变异进程全局环境变量的测试都应用 [`ScopedEnvVar`] 代替原始
//! `std::env::set_var` / `remove_var`，以保证：
//!
//! 1. 全部 env-mutating 测试被**单个 crate 级** `Mutex` 串行化，防止
//!    `cargo test` 并行时 `context.rs`、`task/markdown.rs`、
//!    `commands/task.rs` 等模块互相污染。
//! 2. 原始值**始终**被恢复（即使测试 panic 也会走 `Drop` 析构）。

use std::sync::{Mutex, MutexGuard};

/// Crate-wide mutex serializing ALL env-mutating tests.
///
/// 全局互斥锁，序列化 crate 内所有环境变异测试。
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Acquires the crate-wide env serialisation lock.
///
/// Every [`ScopedEnvVar`] instance calls this, so all env-mutating tests
/// share a single serialisation point regardless of which module they
/// live in.
///
/// 获取 crate 级环境序列化锁，所有 [`ScopedEnvVar`] 实例都调用此函数，
/// 保证不同模块的 env-mutating 测试共享同一个串行化点。
pub fn acquire_env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Panic-safe guard that temporarily overrides environment variables.
///
/// On construction it saves the current value (if any) of each variable,
/// then applies the requested override. On `Drop` — including unwinding
/// from a test failure — every variable is restored to its original
/// state. A single crate-wide [`Mutex`] ensures no concurrent test
/// observes an intermediate or dirty state.
///
/// 临时覆盖环境变量的 panic-safe 守卫。
///
/// 构造时保存每个变量的当前值并施加新值；析构时（含 test panic 展开）
/// 自动恢复所有变量到原始状态。通过 crate 级 `Mutex` 保证并发测试
/// 不会看到脏中间态。
pub struct ScopedEnvVar {
    /// Held for the guard's entire lifetime; released on drop.
    /// 守卫持有期间的锁，析构时自动释放。
    _lock: MutexGuard<'static, ()>,
    /// Saved original values: `(variable_name, Some(value) | None)`.
    /// `None` means the variable was not set before the override.
    /// 保存的原始值：`(变量名, Some(原值) | None)`，`None` 表示原先未设。
    saved: Vec<(String, Option<String>)>,
}

impl ScopedEnvVar {
    /// Creates a new guard, saves current values, and applies `pairs`.
    ///
    /// Each entry in `pairs` is `(env_var_name, new_value)`. When
    /// `new_value` is `Some(v)` the variable is set to `v`; when `None`
    /// it is removed from the process environment.
    ///
    /// Panics if the internal lock is poisoned (a previous test panicked
    /// while holding the lock — the lock is recovered via
    /// `into_inner()` which itself may panic if the poisoned value was
    /// held across a panic).
    ///
    /// 创建守卫，保存当前值并施加新值。
    ///
    /// `pairs` 中每项为 `(环境变量名, 新值)`。`new_value` 为 `Some(v)`
    /// 时设值，为 `None` 时删除该环境变量。
    ///
    /// 若内部锁被毒化（前一个测试 panic 时持有锁），会先尝试恢复锁
    /// （`into_inner()`），此操作亦可能 panic。
    pub fn new(pairs: &[(&str, Option<&str>)]) -> Self {
        let _lock = acquire_env_lock();
        let mut saved = Vec::with_capacity(pairs.len());
        for &(name, value) in pairs {
            let original = std::env::var(name).ok();
            // SAFETY: guarded by the crate-wide mutex (held for the full
            // lifetime of `Self`). 2024 edition marks env mutators as
            // unsafe due to global process state.
            // 安全性：全程持有 crate 级互斥锁，不会与其它 env 写入并发。
            unsafe {
                match value {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
            saved.push((name.to_string(), original));
        }
        ScopedEnvVar { _lock, saved }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        // Restore even on panic — `saved` is owned by `self` so this
        // always executes during unwinding.
        // 即使在 panic 展开路径中也恢复原始值。
        for (name, original) in self.saved.drain(..) {
            // SAFETY: same reasoning as in `new()` — no other env
            // mutation can run concurrently because the lock is still
            // held (released after drop returns).
            // 安全性：锁依然被持有，不会有其它 env 写入并发执行。
            unsafe {
                match original {
                    Some(val) => std::env::set_var(&name, &val),
                    None => std::env::remove_var(&name),
                }
            }
        }
    }
}
