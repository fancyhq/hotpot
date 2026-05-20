//! 跨进程文件锁助手 — Cross-process advisory file lock helper.
//!
//! Hotpot 是 CLI 工具，多个调用方（agent / hook / 用户）可能并发触碰同一份
//! 数据文件：
//!   - `.hotpot/workspaces/<u>/overview.jsonl`
//!   - `.hotpot/issue-candidates.jsonl`（项目级临时候选）
//!   - `.hotpot/issues.jsonl`（项目级共享）
//!
//! 即便单次写入是 tmp+rename 原子的，两个并发写者仍会上演 last-writer-wins。
//! 本模块用 `fs2` 在每个数据文件旁创建 `<file>.lock` sidecar，写入操作前
//! 通过 [`with_file_lock`] 获取独占锁；超时则回退到无锁路径（执行操作并
//! 在 stderr 打一行 warning），保证锁机制本身不会变成新的可用性故障源。
//!
//! 严格约束：锁持有期间**不允许** spawn 子进程（任何 `Command::new`、
//! `std::process::Command::status/output/spawn`）。原因是平台 hook（如 Claude
//! 的 PreToolUse、OpenCode 的 `tool.execute.before`）会在 agent 跑某些命令
//! 时回调 `hotpot`，hotpot 子进程一旦同时争同一把锁，就会形成无法在
//! advisory 模型下检测的嵌套死锁。
//!
//! Hotpot 是 CLI，多入口（agent / hook / 用户）会并发触碰同一份 JSONL 文件。
//! 该模块用 `fs2` advisory 锁保护写入，try-lock + 重试 + 超时回退；锁持有
//! 期间禁止 spawn 子进程，避免 hook 嵌套死锁。
//!
//! Cross-process advisory locking for Hotpot's JSONL data files. Uses
//! sibling `<file>.lock` files and `fs2::FileExt::try_lock_exclusive` with
//! short retries; on timeout, falls back to running the op without the lock
//! and prints a stderr warning so the failure mode stays observable rather
//! than fatal. **Hard rule**: do not spawn subprocesses while holding the
//! lock — platform hooks may re-enter `hotpot`, deadlocking on the same
//! sidecar.

use std::{
    fs::{File, OpenOptions},
    path::Path,
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use fs2::FileExt;

/// 尝试取锁的最大轮数。50 × 100ms = 5s 上限——给多 agent 并发与解释器
/// 启动毛刺留足空间，同时保证最坏情况下命令不会卡死。
/// Max retry rounds (100ms × 50 = 5s ceiling). Sized to absorb multi-agent
/// concurrency and interpreter startup jitter while keeping the worst-case
/// command latency bounded.
const MAX_RETRIES: u32 = 50;

/// 单轮重试的退避时长。短到不影响交互式 UX，长到避免 busy-spin。
/// Sleep between retries.
const RETRY_DELAY: Duration = Duration::from_millis(100);

/// 在 `data_path` 旁的 `<data>.lock` 上拿独占锁后运行 `op`。
///
/// 锁文件按需创建（如果父目录不存在也会一并创建）。`op` 不接收锁句柄——
/// 锁生命周期与 [`Guard`] 绑定，函数返回后自动释放。
///
/// 超时回退：连续 [`MAX_RETRIES`] 次 try-lock 都失败时，stderr 打一行
/// `lock fallback:` warning，然后在**无锁**状态下运行 `op`。这保证锁机制
/// 自身永远不会变成可用性故障源——多用户写竞争是小概率事件，让它
/// 在「能跑但 mtime 抖动」与「锁卡死整条命令」之间选前者。
///
/// **不要在 `op` 中 spawn 子进程**：当平台 hook 在 agent 命令执行路径上
/// 回调 hotpot 时，子进程会再次尝试获取同一把锁，advisory 锁模型下
/// 没法检测这种嵌套死锁。如果某天确实需要锁内子进程，请重新思考整个
/// 调用结构。
///
/// Run `op` while holding an exclusive advisory lock on `<data>.lock`
/// (created on demand alongside `data_path`). After `MAX_RETRIES`
/// unsuccessful retries (each waiting `RETRY_DELAY`), prints a stderr
/// warning and runs `op` **without** the lock as a fallback, so locking
/// can never escalate to a hard outage. **Do not spawn subprocesses
/// inside `op`** — platform hooks re-entering `hotpot` would deadlock
/// on the same sidecar.
pub fn with_file_lock<T, F>(data_path: &Path, op: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    let lock_path = lock_path_for(data_path);

    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create lock parent dir {}", parent.display()))?;
    }

    let lock_file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("failed to open lock file {}", lock_path.display()))?;

    let mut acquired = false;
    for attempt in 0..MAX_RETRIES {
        match lock_file.try_lock_exclusive() {
            Ok(()) => {
                acquired = true;
                break;
            }
            Err(err) => {
                // Last attempt — let fallback path handle.
                // 最后一次 attempt 不再 sleep，直接走 fallback。
                if attempt + 1 == MAX_RETRIES {
                    eprintln!(
                        "hotpot: lock fallback: could not acquire {} after {} retries ({err}); proceeding without lock",
                        lock_path.display(),
                        MAX_RETRIES
                    );
                    break;
                }
                thread::sleep(RETRY_DELAY);
            }
        }
    }

    // _guard ensures the lock is released regardless of how `op` exits.
    // `acquired=false` means we're in fallback; guard becomes a no-op.
    let _guard = Guard {
        file: &lock_file,
        held: acquired,
    };

    op()
}

/// 返回 `<data>.lock` sidecar 路径。
///
/// Computes the sidecar lock path: `foo.jsonl` → `foo.jsonl.lock`. We append
/// rather than replace the extension so the lock and data file stay obvious
/// neighbors when listing the directory.
fn lock_path_for(data_path: &Path) -> std::path::PathBuf {
    let mut buf = data_path.as_os_str().to_owned();
    buf.push(".lock");
    std::path::PathBuf::from(buf)
}

/// RAII handle that releases the advisory lock on drop. Holding the `File`
/// reference (not owning the value) keeps the lock alive for `op`'s
/// duration; the `held` bit distinguishes the genuine-lock path from the
/// fallback so we don't try to unlock something we never locked.
///
/// 锁的 RAII 守卫；`op` 结束自动释放。`held=false` 表示走了 fallback，
/// drop 不做事。
struct Guard<'a> {
    file: &'a File,
    held: bool,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        if !self.held {
            return;
        }
        // unlock 极少失败（fs2 文档：仅在 fd 已关闭等极端情况下报错）；
        // 一旦失败也只能记录，不能 panic。
        // unlock rarely fails (only on edge cases like a closed fd); log
        // but never panic since we're in Drop.
        if let Err(err) = FileExt::unlock(self.file) {
            eprintln!("hotpot: warning: failed to release advisory lock: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, Mutex},
        thread,
    };

    fn temp_data_path(label: &str) -> std::path::PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("hotpot-lock-{label}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("data.jsonl")
    }

    /// 锁释放后再取应当立刻成功。
    #[test]
    fn lock_releases_after_op() {
        let data = temp_data_path("releases");
        let result = with_file_lock(&data, || Ok(42)).unwrap();
        assert_eq!(result, 42);
        // 二次取锁不应等待。
        let result2 = with_file_lock(&data, || Ok(7)).unwrap();
        assert_eq!(result2, 7);
    }

    /// 两个线程并发跑 `with_file_lock`：两次操作都应执行，且操作之间
    /// 不重叠（用 Mutex 计数 in-flight 操作验证）。
    #[test]
    fn concurrent_threads_serialize() {
        let data = temp_data_path("serialize");
        let in_flight = Arc::new(Mutex::new(0u32));
        let observed_max = Arc::new(Mutex::new(0u32));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let data = data.clone();
                let in_flight = Arc::clone(&in_flight);
                let observed_max = Arc::clone(&observed_max);
                thread::spawn(move || {
                    with_file_lock(&data, || {
                        {
                            let mut n = in_flight.lock().unwrap();
                            *n += 1;
                            let mut max = observed_max.lock().unwrap();
                            if *n > *max {
                                *max = *n;
                            }
                        }
                        thread::sleep(Duration::from_millis(20));
                        {
                            let mut n = in_flight.lock().unwrap();
                            *n -= 1;
                        }
                        Ok(())
                    })
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap().unwrap();
        }

        let max = *observed_max.lock().unwrap();
        assert_eq!(
            max, 1,
            "expected lock to serialize; observed {max} in-flight at once"
        );
    }

    /// 锁文件创建在 data_path 旁，文件名带 `.lock` 后缀。
    #[test]
    fn lock_sidecar_path_is_data_path_plus_lock() {
        let data = temp_data_path("sidecar");
        with_file_lock(&data, || Ok(())).unwrap();
        let expected = std::path::PathBuf::from(format!("{}.lock", data.display()));
        assert!(
            expected.exists(),
            "lock sidecar not at {}",
            expected.display()
        );
    }
}
