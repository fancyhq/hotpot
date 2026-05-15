//! OpenCode assets installed by `hotpot init`.
//!
//! `.opencode/package.json` is installed via [`Asset::merge_json`] because it
//! must coexist with user-authored dependencies, devDependencies, scripts,
//! and the rest of an npm package descriptor. Everything else (agents,
//! commands, plugins) is a Hotpot-private file under the `hotpot/` namespace
//! or `hotpot-` prefix.
//!
//! `.opencode/package.json` 走 JSON 合并以保留用户的 dependencies /
//! devDependencies / scripts 等其它字段；其余 Hotpot 私有文件维持原写入语义。
//! 注：插件文件改为 `hotpot-bash-before.ts` / `hotpot-review-memory.ts`，与
//! 用户可能存在的同名插件区分。

use super::Asset;

/// OpenCode asset registry.
pub(super) const ASSETS: &[Asset] = &[
    Asset::owned(
        ".opencode/agents/hotpot-execution.md",
        include_str!("../../../assets/platforms/opencode/agents/hotpot-execution.md"),
    ),
    Asset::owned(
        ".opencode/agents/hotpot-review.md",
        include_str!("../../../assets/platforms/opencode/agents/hotpot-review.md"),
    ),
    Asset::owned(
        ".opencode/commands/hotpot/execute.md",
        include_str!("../../../assets/platforms/opencode/commands/hotpot/execute.md"),
    ),
    Asset::owned(
        ".opencode/commands/hotpot/new.md",
        include_str!("../../../assets/platforms/opencode/commands/hotpot/new.md"),
    ),
    Asset::owned(
        ".opencode/commands/hotpot/finish-work.md",
        include_str!("../../../assets/platforms/opencode/commands/hotpot/finish-work.md"),
    ),
    Asset::owned(
        ".opencode/plugins/hotpot-bash-before.ts",
        include_str!("../../../assets/platforms/opencode/plugins/hotpot-bash-before.ts"),
    ),
    Asset::owned(
        ".opencode/plugins/hotpot-review-memory.ts",
        include_str!("../../../assets/platforms/opencode/plugins/hotpot-review-memory.ts"),
    ),
    Asset::merge_json(
        ".opencode/package.json",
        include_str!("../../../assets/platforms/opencode/package.json"),
    ),
];
