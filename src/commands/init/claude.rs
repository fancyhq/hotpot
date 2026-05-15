//! Claude Code assets installed by `hotpot init`.
//!
//! `.claude/settings.json` is installed via [`Asset::merge_json`] because it
//! must coexist with user-authored hooks/permissions/env. Everything else is
//! a Hotpot-private file and uses [`Asset::owned`].
//!
//! `.claude/settings.json` 走 JSON 合并（必须与用户已有 hooks/permissions/env
//! 共存），其余 Hotpot 私有文件维持「整文件写入 + `--force`」语义。

use super::Asset;

/// Claude Code asset registry.
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
    Asset::owned(
        ".claude/hooks/hotpot-pre-tool-use.sh",
        include_str!("../../../assets/platforms/claude/hooks/hotpot-pre-tool-use.sh"),
    ),
    Asset::owned(
        ".claude/hooks/hotpot-pre-tool-use.cmd",
        include_str!("../../../assets/platforms/claude/hooks/hotpot-pre-tool-use.cmd"),
    ),
    Asset::owned(
        ".claude/hooks/hotpot-subagent-start.sh",
        include_str!("../../../assets/platforms/claude/hooks/hotpot-subagent-start.sh"),
    ),
    Asset::owned(
        ".claude/hooks/hotpot-subagent-start.cmd",
        include_str!("../../../assets/platforms/claude/hooks/hotpot-subagent-start.cmd"),
    ),
];
