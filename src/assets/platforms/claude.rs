//! Claude Code assets installed by Hotpot.
//!
//! `.claude/settings.json` is installed via [`Asset::merge_json`] because it
//! must coexist with user-authored hooks/permissions/env. Everything else is
//! a Hotpot-private file and uses [`Asset::owned`].
//!
//! Hook entries in `settings.json` invoke the `hotpot hook claude <event>`
//! CLI directly, so no on-disk `.sh` / `.cmd` shim scripts are required or
//! installed.
//!
//! `.claude/settings.json` 走 JSON 合并（必须与用户已有 hooks/permissions/env
//! 共存），其余 Hotpot 私有文件维持「整文件写入 + `--force`」语义。
//!
//! `settings.json` 中的 hook 直接调用 `hotpot hook claude <event>` CLI，
//! 不再安装任何磁盘上的 `.sh` / `.cmd` 透传脚本。

use crate::assets::Asset;

/// Claude Code asset registry.
///
/// Claude Code 平台资产清单。
pub(super) const ASSETS: &[Asset] = &[
    Asset::owned(
        ".claude/agents/hotpot-execution.md",
        include_str!("../../../assets/platforms/claude/agents/hotpot-execution.md"),
    ),
    Asset::owned(
        ".claude/agents/hotpot-review.md",
        include_str!("../../../assets/platforms/claude/agents/hotpot-review.md"),
    ),
    Asset::owned(
        ".claude/commands/hotpot/execute.md",
        include_str!("../../../assets/platforms/claude/commands/hotpot/execute.md"),
    ),
    Asset::owned(
        ".claude/commands/hotpot/new.md",
        include_str!("../../../assets/platforms/claude/commands/hotpot/new.md"),
    ),
    Asset::owned(
        ".claude/commands/hotpot/finish-work.md",
        include_str!("../../../assets/platforms/claude/commands/hotpot/finish-work.md"),
    ),
    Asset::merge_json(
        ".claude/settings.json",
        include_str!("../../../assets/platforms/claude/settings.json"),
    ),
];
