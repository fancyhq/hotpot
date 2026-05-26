//! Codex assets installed by Hotpot.
//!
//! `.codex/config.toml` is installed via [`Asset::merge_toml`] because it
//! must coexist with user-authored `[features]`, MCP servers, sandbox
//! settings, and so on. The TOML merge uses `toml_edit` to preserve user
//! comments and key order. Everything else is a Hotpot-private file.
//!
//! Hook entries in `config.toml` invoke the `hotpot hook codex <event>` CLI
//! directly, so no on-disk `.sh` / `.cmd` shim scripts are required or
//! installed.
//!
//! `.codex/config.toml` 走 TOML 合并（保留用户注释、键序、其它 `[features]`
//! 键等），其余 Hotpot 私有文件维持「整文件写入 + `--force`」语义。
//!
//! `config.toml` 中的 hook 直接调用 `hotpot hook codex <event>` CLI，
//! 不再安装任何磁盘上的 `.sh` / `.cmd` 透传脚本。

use crate::assets::Asset;

/// Codex asset registry.
///
/// Codex 平台资产清单。
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
];
