//! Per-platform asset registries and the [`Platform`] selector enum.
//!
//! Each child module (`claude`, `opencode`, `codex`, `pi`) owns one
//! `ASSETS: &[Asset]` array enumerating the files Hotpot installs for that
//! platform. The `*_GROUPS` constants below combine each platform's array
//! with [`super::SHARED_ASSETS`] so callers iterate them through a single
//! pass via [`asset_groups`].
//!
//! 各平台资源数组与 [`Platform`] 枚举。子模块（`claude` / `opencode` /
//! `codex` / `pi`）各自维护本平台的 `ASSETS`；本文件把它与
//! [`super::SHARED_ASSETS`] 组合为 `*_GROUPS`，供 [`asset_groups`] 统一
//! 迭代。

mod claude;
mod codex;
mod opencode;
mod pi;

use clap::ValueEnum;

use super::{Asset, SHARED_ASSETS};

/// Supported platform targets for asset installation.
///
/// Hotpot 资产安装的平台目标。`All` 是综合选项；其余为具体平台。
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum Platform {
    Opencode,
    Claude,
    Codex,
    Pi,
    All,
}

impl Platform {
    /// Returns the project-relative directory each platform's config lives
    /// under, used by [`super::detect_installed_platforms`] to figure out
    /// which platforms an existing project has already been initialized
    /// for. [`Platform::All`] is a synthetic selector and intentionally
    /// returns `None`.
    ///
    /// 返回每个平台的项目相对目录名；`All` 是综合选项，返回 `None`。
    pub fn config_dir(&self) -> Option<&'static str> {
        match self {
            Platform::Opencode => Some(".opencode"),
            Platform::Claude => Some(".claude"),
            Platform::Codex => Some(".codex"),
            Platform::Pi => Some(".pi"),
            Platform::All => None,
        }
    }

    /// Human-readable platform identifier used in summary / warning output.
    ///
    /// 人类可读的平台标识，用于摘要 / 警告输出。
    pub fn slug(&self) -> &'static str {
        match self {
            Platform::Opencode => "opencode",
            Platform::Claude => "claude",
            Platform::Codex => "codex",
            Platform::Pi => "pi",
            Platform::All => "all",
        }
    }

    /// All concrete platforms (`All` excluded), in a stable display order.
    ///
    /// 所有具体平台（排除 `All`），按稳定顺序返回。
    pub const ALL_CONCRETE: &'static [Platform] = &[
        Platform::Claude,
        Platform::Opencode,
        Platform::Codex,
        Platform::Pi,
    ];
}

const OPENCODE_GROUPS: &[&[Asset]] = &[opencode::ASSETS, SHARED_ASSETS];
const CLAUDE_GROUPS: &[&[Asset]] = &[claude::ASSETS, SHARED_ASSETS];
const CODEX_GROUPS: &[&[Asset]] = &[codex::ASSETS, SHARED_ASSETS];
const PI_GROUPS: &[&[Asset]] = &[pi::ASSETS, SHARED_ASSETS];
const ALL_GROUPS: &[&[Asset]] = &[
    opencode::ASSETS,
    claude::ASSETS,
    codex::ASSETS,
    pi::ASSETS,
    SHARED_ASSETS,
];

/// Returns the asset groups requested by a [`Platform`] selection.
///
/// 返回 `platform` 选项对应的资产分组集合。
pub(super) fn asset_groups(platform: &Platform) -> &'static [&'static [Asset]] {
    match platform {
        Platform::Opencode => OPENCODE_GROUPS,
        Platform::Claude => CLAUDE_GROUPS,
        Platform::Codex => CODEX_GROUPS,
        Platform::Pi => PI_GROUPS,
        Platform::All => ALL_GROUPS,
    }
}
