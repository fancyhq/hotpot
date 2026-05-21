//! Pi assets installed by Hotpot.
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

use crate::assets::Asset;

/// Pi asset registry.
///
/// Pi 平台资产清单。
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

#[cfg(test)]
mod tests {
    //! Pi asset content regression tests.
    //!
    //! Pi 平台资产内容回归测试，防止 prompt template 丢失命令参数传递。

    /// Ensures the Pi new-task template passes slash-command arguments.
    ///
    /// 确保 Pi 新任务模板会把 slash command 参数传给共享 workflow。
    #[test]
    fn pi_new_prompt_template_passes_command_arguments() {
        let template = include_str!("../../../assets/platforms/pi/prompts/hotpot-new.md");

        assert!(
            template.contains("$ARGUMENTS"),
            "Pi hotpot-new prompt template must expose command arguments with $ARGUMENTS"
        );
        assert!(
            template.contains("treat it as the user's initial task idea"),
            "Pi hotpot-new prompt template must use non-empty command arguments as the initial task idea"
        );
        assert!(
            template.contains("If it is empty"),
            "Pi hotpot-new prompt template must preserve the empty-arguments fallback"
        );
        assert!(
            template.contains("ask one concise question"),
            "Pi hotpot-new prompt template must ask only when command arguments are empty"
        );
    }
}
