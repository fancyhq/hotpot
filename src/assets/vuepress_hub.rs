//! VuePress hub project asset catalog.
//!
//! [`VUEPRESS_HUB_ASSETS`] is the per-file blueprint of the VuePress
//! project Hotpot deploys under `<project>/.hotpot-hub/` when the user
//! runs `hotpot vuepress install` (or selects opt-in during
//! `hotpot init`). Unlike [`super::SHARED_ASSETS`], these files are
//! **never** installed by default — they live behind the
//! `[vuepress] enabled = true` switch and are removed by
//! `hotpot vuepress uninstall`.
//!
//! Each entry is a regular [`super::Asset::owned`] declaration; the
//! existing install engine handles them through
//! [`super::install_vuepress_hub`] (a thin wrapper around
//! [`super::install_one`]). Adding a new file to the VuePress project
//! template means dropping it under `assets/vuepress/` and registering
//! one more entry here — the install / uninstall paths pick it up
//! automatically.
//!
//! VuePress hub 项目资产清单。`hotpot vuepress install` 时（或
//! `hotpot init` 选择启用 VuePress 时）会按此清单把 vuepress 模板部署到
//! `<project>/.hotpot-hub/`。这些资产**不会**被默认安装，归属 opt-in
//! 体系；`hotpot vuepress uninstall` 反向移除。新增模板文件时往
//! `assets/vuepress/` 放一份并在此登记一行即可，install/uninstall
//! 自动覆盖。

use super::Asset;

/// VuePress hub project template, deployed under `.hotpot-hub/` only
/// when VuePress is enabled. Mirrors `assets/vuepress/` byte-for-byte.
///
/// The four `.vuepress/` files (`config.js` / `client.js` / `sidebar.js`
/// / `components/TaskIndex.vue`) are tightly coupled — `config.js`
/// invokes the `scanWorkspaces` / `generateSidebar` exports from
/// `sidebar.js` and injects `__HOTPOT_TASK_INDEX__` as a Vite compile-
/// time constant, which `TaskIndex.vue` (registered globally by
/// `client.js`) reads on render. Editing any one of them in isolation
/// breaks the home-page index. All four are `Asset::owned` so
/// `hotpot vuepress install --force` restores the bundled copies.
///
/// VuePress 模板项目，仅启用 VuePress 时部署到 `.hotpot-hub/`，与
/// `assets/vuepress/` 内容逐字节对应。`.vuepress/` 下四个文件相互耦合
/// （`config.js` 调 `sidebar.js` 的 `scanWorkspaces` / `generateSidebar`、
/// 通过 Vite define 注入 `__HOTPOT_TASK_INDEX__` 给 `client.js` 注册的
/// `TaskIndex.vue` 在渲染时读取），单独改其一会破坏首页索引。统一
/// `Asset::owned`，`hotpot vuepress install --force` 可一键还原。
pub(crate) const VUEPRESS_HUB_ASSETS: &[Asset] = &[
    Asset::owned(
        ".hotpot-hub/package.json",
        include_str!("../../assets/vuepress/package.json"),
    ),
    Asset::owned(
        ".hotpot-hub/pnpm-lock.yaml",
        include_str!("../../assets/vuepress/pnpm-lock.yaml"),
    ),
    Asset::owned(
        ".hotpot-hub/docs/README.md",
        include_str!("../../assets/vuepress/docs/README.md"),
    ),
    Asset::owned(
        ".hotpot-hub/docs/.vuepress/config.js",
        include_str!("../../assets/vuepress/docs/.vuepress/config.js"),
    ),
    Asset::owned(
        ".hotpot-hub/docs/.vuepress/client.js",
        include_str!("../../assets/vuepress/docs/.vuepress/client.js"),
    ),
    Asset::owned(
        ".hotpot-hub/docs/.vuepress/sidebar.js",
        include_str!("../../assets/vuepress/docs/.vuepress/sidebar.js"),
    ),
    Asset::owned(
        ".hotpot-hub/docs/.vuepress/components/TaskIndex.vue",
        include_str!("../../assets/vuepress/docs/.vuepress/components/TaskIndex.vue"),
    ),
];
