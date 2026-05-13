//! Claude Code assets installed by `hotpot init`.

use super::Asset;

/// Claude Code asset registry.
pub(super) const ASSETS: &[Asset] = &[
    Asset {
        path: ".claude/agents/hotpot-execution.md",
        content: include_str!("../../../assets/platforms/claude/agents/hotpot-execution.md"),
    },
    Asset {
        path: ".claude/agents/hotpot-review.md",
        content: include_str!("../../../assets/platforms/claude/agents/hotpot-review.md"),
    },
    Asset {
        path: ".claude/commands/hotpot/execute.md",
        content: include_str!("../../../assets/platforms/claude/commands/hotpot/execute.md"),
    },
    Asset {
        path: ".claude/commands/hotpot/new.md",
        content: include_str!("../../../assets/platforms/claude/commands/hotpot/new.md"),
    },
    Asset {
        path: ".claude/settings.json",
        content: include_str!("../../../assets/platforms/claude/settings.json"),
    },
    Asset {
        path: ".claude/hooks/hotpot-pre-tool-use.sh",
        content: include_str!("../../../assets/platforms/claude/hooks/hotpot-pre-tool-use.sh"),
    },
    Asset {
        path: ".claude/hooks/hotpot-pre-tool-use.cmd",
        content: include_str!("../../../assets/platforms/claude/hooks/hotpot-pre-tool-use.cmd"),
    },
    Asset {
        path: ".claude/hooks/hotpot-subagent-start.sh",
        content: include_str!("../../../assets/platforms/claude/hooks/hotpot-subagent-start.sh"),
    },
    Asset {
        path: ".claude/hooks/hotpot-subagent-start.cmd",
        content: include_str!("../../../assets/platforms/claude/hooks/hotpot-subagent-start.cmd"),
    },
];
