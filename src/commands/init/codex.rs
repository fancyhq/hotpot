//! Codex assets installed by `hotpot init`.

use super::Asset;

/// Codex asset registry.
pub(super) const ASSETS: &[Asset] = &[
    Asset {
        path: ".codex/agents/hotpot-execution.toml",
        content: include_str!("../../../assets/platforms/codex/agents/hotpot-execution.toml"),
    },
    Asset {
        path: ".codex/agents/hotpot-review.toml",
        content: include_str!("../../../assets/platforms/codex/agents/hotpot-review.toml"),
    },
    Asset {
        path: ".codex/skills/hotpot-execute/SKILL.md",
        content: include_str!("../../../assets/platforms/codex/skills/hotpot-execute/SKILL.md"),
    },
    Asset {
        path: ".codex/skills/hotpot-new/SKILL.md",
        content: include_str!("../../../assets/platforms/codex/skills/hotpot-new/SKILL.md"),
    },
    Asset {
        path: ".codex/config.toml",
        content: include_str!("../../../assets/platforms/codex/config.toml"),
    },
    Asset {
        path: ".codex/hooks/hotpot-pre-tool-use.sh",
        content: include_str!("../../../assets/platforms/codex/hooks/hotpot-pre-tool-use.sh"),
    },
    Asset {
        path: ".codex/hooks/hotpot-pre-tool-use.cmd",
        content: include_str!("../../../assets/platforms/codex/hooks/hotpot-pre-tool-use.cmd"),
    },
    Asset {
        path: ".codex/hooks/hotpot-session-start.sh",
        content: include_str!("../../../assets/platforms/codex/hooks/hotpot-session-start.sh"),
    },
    Asset {
        path: ".codex/hooks/hotpot-session-start.cmd",
        content: include_str!("../../../assets/platforms/codex/hooks/hotpot-session-start.cmd"),
    },
];
