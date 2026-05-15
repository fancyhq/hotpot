//! Pi assets installed by `hotpot init`.
//!
//! `.pi/package.json` is installed via [`Asset::merge_json`] so any extra
//! top-level fields a user has added (e.g. `scripts`, `version`,
//! `devDependencies`) survive re-init. Hotpot-owned fields (`name`,
//! `keywords`, `pi`, `dependencies`) take precedence over user-modified
//! values for those specific keys.
//!
//! `.pi/package.json` 走 JSON 合并：用户在该文件加的额外顶层字段（`scripts`
//! 等）会保留；hotpot 拥有的 `name` / `keywords` / `pi` / `dependencies`
//! 字段以资产为准。其余 Hotpot 私有文件维持原写入语义。

use super::Asset;

/// Pi asset registry.
pub(super) const ASSETS: &[Asset] = &[
    Asset::owned(
        ".pi/prompts/hotpot-execute.md",
        include_str!("../../../assets/platforms/pi/prompts/hotpot-execute.md"),
    ),
    Asset::owned(
        ".pi/prompts/hotpot-new.md",
        include_str!("../../../assets/platforms/pi/prompts/hotpot-new.md"),
    ),
    Asset::owned(
        ".pi/prompts/hotpot-finish-work.md",
        include_str!("../../../assets/platforms/pi/prompts/hotpot-finish-work.md"),
    ),
    Asset::merge_json(
        ".pi/package.json",
        include_str!("../../../assets/platforms/pi/package.json"),
    ),
    Asset::owned(
        ".pi/extensions/hotpot/index.ts",
        include_str!("../../../assets/platforms/pi/extensions/hotpot/index.ts"),
    ),
];
