//! Pi assets installed by `hotpot init`.

use super::Asset;

/// Pi asset registry.
pub(super) const ASSETS: &[Asset] = &[
    Asset {
        path: ".pi/prompts/hotpot-execute.md",
        content: include_str!("../../../assets/platforms/pi/prompts/hotpot-execute.md"),
    },
    Asset {
        path: ".pi/prompts/hotpot-new.md",
        content: include_str!("../../../assets/platforms/pi/prompts/hotpot-new.md"),
    },
    Asset {
        path: ".pi/package.json",
        content: include_str!("../../../assets/platforms/pi/package.json"),
    },
    Asset {
        path: ".pi/extensions/hotpot/index.ts",
        content: include_str!("../../../assets/platforms/pi/extensions/hotpot/index.ts"),
    },
];
