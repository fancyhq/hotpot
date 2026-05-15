//! Codex assets installed by `hotpot init`.
//!
//! `.codex/config.toml` is installed via [`Asset::merge_toml`] because it
//! must coexist with user-authored `[features]`, MCP servers, sandbox
//! settings, and so on. The TOML merge uses `toml_edit` to preserve user
//! comments and key order. Everything else is a Hotpot-private file.
//!
//! `.codex/config.toml` 走 TOML 合并（保留用户注释、键序、其它 `[features]`
//! 键等），其余 Hotpot 私有文件维持「整文件写入 + `--force`」语义。

use super::Asset;

/// Codex asset registry.
pub(super) const ASSETS: &[Asset] = &[
    Asset::owned(
        ".codex/agents/hotpot-execution.toml",
        include_str!("../../../assets/platforms/codex/agents/hotpot-execution.toml"),
    ),
    Asset::owned(
        ".codex/agents/hotpot-review.toml",
        include_str!("../../../assets/platforms/codex/agents/hotpot-review.toml"),
    ),
    Asset::owned(
        ".codex/skills/hotpot-execute/SKILL.md",
        include_str!("../../../assets/platforms/codex/skills/hotpot-execute/SKILL.md"),
    ),
    Asset::owned(
        ".codex/skills/hotpot-new/SKILL.md",
        include_str!("../../../assets/platforms/codex/skills/hotpot-new/SKILL.md"),
    ),
    Asset::owned(
        ".codex/skills/hotpot-finish-work/SKILL.md",
        include_str!("../../../assets/platforms/codex/skills/hotpot-finish-work/SKILL.md"),
    ),
    Asset::merge_toml(
        ".codex/config.toml",
        include_str!("../../../assets/platforms/codex/config.toml"),
    ),
    Asset::owned(
        ".codex/hooks/hotpot-pre-tool-use.sh",
        include_str!("../../../assets/platforms/codex/hooks/hotpot-pre-tool-use.sh"),
    ),
    Asset::owned(
        ".codex/hooks/hotpot-pre-tool-use.cmd",
        include_str!("../../../assets/platforms/codex/hooks/hotpot-pre-tool-use.cmd"),
    ),
    Asset::owned(
        ".codex/hooks/hotpot-session-start.sh",
        include_str!("../../../assets/platforms/codex/hooks/hotpot-session-start.sh"),
    ),
    Asset::owned(
        ".codex/hooks/hotpot-session-start.cmd",
        include_str!("../../../assets/platforms/codex/hooks/hotpot-session-start.cmd"),
    ),
    Asset::owned(
        ".codex/hooks/hotpot-user-prompt-submit.sh",
        include_str!("../../../assets/platforms/codex/hooks/hotpot-user-prompt-submit.sh"),
    ),
    Asset::owned(
        ".codex/hooks/hotpot-user-prompt-submit.cmd",
        include_str!("../../../assets/platforms/codex/hooks/hotpot-user-prompt-submit.cmd"),
    ),
];
