//! Pi assets installed by Hotpot.
//!
//! `.pi/package.json` is installed via [`Asset::merge_json`] so any extra
//! top-level fields a user has added (e.g. `scripts`, `version`,
//! `devDependencies`) survive re-init. Hotpot-owned fields (`name`,
//! `keywords`, `pi`, `dependencies`) take precedence over user-modified
//! values for those specific keys.
//!
//! Hotpot no longer ships Pi prompt-template thin shells under
//! `.pi/prompts/hotpot-*.md`. Those templates were absorbed as system-level
//! background documentation by Pi projects that load `AGENTS.md` /
//! `CLAUDE.md` / global skills, causing the AI to fall back to generic
//! greetings instead of starting the Hotpot workflow. The three slash
//! commands (`/hotpot-new`, `/hotpot-execute`, `/hotpot-finish-work`) are
//! now registered through the Pi extension's `pi.registerCommand`, which
//! delivers the workflow prompt as an actual user message via
//! `pi.sendUserMessage` (the method is on `ExtensionAPI` — the factory's
//! `pi` parameter — not on the handler's `ctx: ExtensionCommandContext`).
//! See [`cleanup_deprecated_pi_prompts`] for the one-shot removal of the
//! legacy thin-shell files.
//!
//! `.pi/package.json` 走 JSON 合并：用户在该文件加的额外顶层字段（`scripts`
//! 等）会保留；hotpot 拥有的 `name` / `keywords` / `pi` / `dependencies`
//! 字段以资产为准。Hotpot 不再发布 `.pi/prompts/hotpot-*.md` thin shell；
//! 三个 slash command 通过 Pi extension 的 `pi.registerCommand` 注册，并以
//! `pi.sendUserMessage` 发送 workflow 提示（`sendUserMessage` 在
//! `ExtensionAPI`（factory 的 `pi` 参数）上，**不**在 handler 的
//! `ctx: ExtensionCommandContext` 上）。废弃 thin shell 由
//! [`cleanup_deprecated_pi_prompts`] 在 install 末尾一次性清理。

use std::io;
use std::path::Path;

use crate::assets::Asset;

/// Pi asset registry.
///
/// Pi 平台资产清单。
pub(super) const ASSETS: &[Asset] = &[
    Asset::merge_json(
        ".pi/package.json",
        include_str!("../../../assets/platforms/pi/package.json"),
    ),
    Asset::owned(
        ".pi/extensions/hotpot/index.ts",
        include_str!("../../../assets/platforms/pi/extensions/hotpot/index.ts"),
    ),
];

/// Project-relative paths of the deprecated Pi prompt-template thin shells.
///
/// 历史遗留的 Pi prompt template thin shell 路径列表。新增需清理的废弃
/// Pi 资产时把项目相对路径加入本列表即可。
const DEPRECATED_PI_PROMPT_PATHS: &[&str] = &[
    ".pi/prompts/hotpot-new.md",
    ".pi/prompts/hotpot-execute.md",
    ".pi/prompts/hotpot-finish-work.md",
];

/// Remove deprecated Pi prompt-template thin shells.
///
/// Called at the end of the Pi platform install path so re-running
/// `hotpot init --platform pi` (or `hotpot update`) on a project that
/// still has the legacy `.pi/prompts/hotpot-{new,execute,finish-work}.md`
/// thin shells erases them. Missing paths are skipped silently; any real
/// IO error during `remove_file` propagates up so `hotpot init` surfaces
/// it instead of swallowing it. Future deprecations can extend
/// [`DEPRECATED_PI_PROMPT_PATHS`].
///
/// 在 Pi platform install 流程末尾调用。已迁移到 extension command 的旧
/// thin shell 路径如果还存在就删掉；不存在则跳过；真实 IO 失败会向上
/// 传播。未来再有废弃路径，把它加进 [`DEPRECATED_PI_PROMPT_PATHS`]。
pub(crate) fn cleanup_deprecated_pi_prompts(root: &Path) -> io::Result<()> {
    for rel in DEPRECATED_PI_PROMPT_PATHS {
        let abs = root.join(rel);
        if abs.exists() {
            std::fs::remove_file(&abs)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tests for the Pi deprecated-prompt cleanup routine.
    //!
    //! Pi 废弃 prompt 清理逻辑的单元测试。

    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Allocate a fresh tempdir without depending on the `tempfile` crate.
    ///
    /// 不引入 `tempfile` 依赖的最小 tempdir：用 `nanoid` 生成唯一后缀。
    struct ScopedTempDir(PathBuf);

    impl ScopedTempDir {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("hotpot-pi-cleanup-{}", nanoid::nanoid!(12),));
            fs::create_dir_all(&path).expect("create tempdir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for ScopedTempDir {
        fn drop(&mut self) {
            // Best-effort cleanup; tests should not panic on teardown.
            // 尽力删除；测试 teardown 不抛错。
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Cleanup removes every legacy thin-shell when all three exist.
    ///
    /// 三个废弃 thin shell 全部存在时应被全部删除。
    #[test]
    fn cleanup_deprecated_pi_prompts_removes_all_three() {
        let dir = ScopedTempDir::new();
        let prompts_dir = dir.path().join(".pi/prompts");
        fs::create_dir_all(&prompts_dir).expect("mkdir");
        for name in [
            "hotpot-new.md",
            "hotpot-execute.md",
            "hotpot-finish-work.md",
        ] {
            fs::write(prompts_dir.join(name), "legacy").expect("write");
        }

        cleanup_deprecated_pi_prompts(dir.path()).expect("cleanup ok");

        for name in [
            "hotpot-new.md",
            "hotpot-execute.md",
            "hotpot-finish-work.md",
        ] {
            assert!(
                !prompts_dir.join(name).exists(),
                "Legacy Pi thin shell {name} should have been removed"
            );
        }
    }

    /// Cleanup is a no-op when no legacy files exist.
    ///
    /// 没有废弃文件时调用应返回 Ok 且不报错。
    #[test]
    fn cleanup_deprecated_pi_prompts_is_noop_when_absent() {
        let dir = ScopedTempDir::new();
        cleanup_deprecated_pi_prompts(dir.path()).expect("cleanup ok when absent");
    }

    /// Cleanup handles a partially-existing legacy set without erroring.
    ///
    /// 仅部分 thin shell 残留时也应正确清理已存在的，跳过缺失的。
    #[test]
    fn cleanup_deprecated_pi_prompts_handles_partial_existence() {
        let dir = ScopedTempDir::new();
        let prompts_dir = dir.path().join(".pi/prompts");
        fs::create_dir_all(&prompts_dir).expect("mkdir");
        fs::write(prompts_dir.join("hotpot-new.md"), "legacy").expect("write");

        cleanup_deprecated_pi_prompts(dir.path()).expect("cleanup ok partial");

        assert!(!prompts_dir.join("hotpot-new.md").exists());
    }
}
