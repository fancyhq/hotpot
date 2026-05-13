//! OpenCode assets installed by `hotpot init`.

use super::Asset;

/// OpenCode asset registry.
pub(super) const ASSETS: &[Asset] = &[
    Asset {
        path: ".opencode/agents/hotpot-execution.md",
        content: include_str!("../../../assets/platforms/opencode/agents/hotpot-execution.md"),
    },
    Asset {
        path: ".opencode/agents/hotpot-review.md",
        content: include_str!("../../../assets/platforms/opencode/agents/hotpot-review.md"),
    },
    Asset {
        path: ".opencode/commands/hotpot/execute.md",
        content: include_str!("../../../assets/platforms/opencode/commands/hotpot/execute.md"),
    },
    Asset {
        path: ".opencode/commands/hotpot/new.md",
        content: include_str!("../../../assets/platforms/opencode/commands/hotpot/new.md"),
    },
    Asset {
        path: ".opencode/commands/hotpot/finish-work.md",
        content: include_str!("../../../assets/platforms/opencode/commands/hotpot/finish-work.md"),
    },
    Asset {
        path: ".opencode/plugins/bash-before.ts",
        content: include_str!("../../../assets/platforms/opencode/plugins/bash-before.ts"),
    },
    Asset {
        path: ".opencode/plugins/review-memory.ts",
        content: include_str!("../../../assets/platforms/opencode/plugins/review-memory.ts"),
    },
    Asset {
        path: ".opencode/package.json",
        content: include_str!("../../../assets/platforms/opencode/package.json"),
    },
    Asset {
        path: ".opencode/tsconfig.json",
        content: include_str!("../../../assets/platforms/opencode/tsconfig.json"),
    },
];
