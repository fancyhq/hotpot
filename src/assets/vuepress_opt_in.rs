//! VuePress opt-in prompt asset catalog.
//!
//! [`VUEPRESS_OPT_IN_ASSETS`] is the slice of prompt files Hotpot
//! installs into `<project>/.hotpot/prompts/` **only when VuePress is
//! enabled** (via `hotpot vuepress install` or
//! `hotpot init --enable-vuepress`). These are intentionally separate
//! from [`super::SHARED_ASSETS`]: keeping them out of the default install
//! is how we guarantee disabled projects have a clean prompt directory
//! with no VuePress noise in the AI's context.
//!
//! Two files live here:
//!
//! - `vuepress.md` — the brainstorming closing flow (yes/no → start →
//!   URL). Loaded by AI via the env-gate in `hotpot-new.md` when
//!   `$HOTPOT_VUEPRESS_ENABLED == "true"`.
//! - `vuepress-style.md` — the VuePress markdown writing conventions
//!   skill (extensions catalog + disabled-features list). Loaded by AI
//!   from the same env-gate, **before** writing the task `.md` so the
//!   conventions are applied at write time.
//!
//! VuePress 启用时才安装的 opt-in prompt 资产。仅当用户跑
//! `hotpot vuepress install`（或 `hotpot init --enable-vuepress`）时才被
//! 装到 `<project>/.hotpot/prompts/`。`vuepress.md` 是收尾流程指令，
//! `vuepress-style.md` 是写作规范——两份都由 `hotpot-new.md` 的 env-gate
//! 引导 AI 在启用态下主动 Read 加载，禁用态完全跳过。

use super::Asset;

/// VuePress opt-in prompt assets, installed under `.hotpot/prompts/`
/// only when VuePress is enabled.
///
/// 仅启用 VuePress 时安装到 `.hotpot/prompts/` 的 opt-in prompt 资产。
pub(crate) const VUEPRESS_OPT_IN_ASSETS: &[Asset] = &[
    Asset::owned(
        ".hotpot/prompts/vuepress.md",
        include_str!("../../assets/prompts/vuepress.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/vuepress-style.md",
        include_str!("../../assets/prompts/vuepress-style.md"),
    ),
];
