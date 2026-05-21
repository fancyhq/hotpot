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
            template.contains("<<< INITIAL TASK IDEA"),
            "Pi hotpot-new prompt template must open an explicit INITIAL TASK IDEA delimiter block"
        );
        assert!(
            template.contains("<<< END INITIAL TASK IDEA >>>"),
            "Pi hotpot-new prompt template must close the INITIAL TASK IDEA delimiter block"
        );
        assert!(
            template.contains("IS the user's initial task idea"),
            "Pi hotpot-new prompt template must declare the block IS the initial task idea unconditionally"
        );
        assert!(
            template.contains("MUST explicitly reference or paraphrase"),
            "Pi hotpot-new prompt template must force the first brainstorm message to reference the idea"
        );
        assert!(
            template.contains("Do NOT ask another question to obtain the initial task idea"),
            "Pi hotpot-new prompt template must forbid re-asking for the initial task idea"
        );
        assert!(
            template.contains("Exception (empty arguments)"),
            "Pi hotpot-new prompt template must mark the empty-arguments fallback as an explicit Exception override"
        );
        assert!(
            template.contains("no non-whitespace text"),
            "Pi hotpot-new prompt template must define the empty-block criterion explicitly so the model does not parse marker labels as user input"
        );
        assert!(
            template.contains("ask exactly one concise question"),
            "Pi hotpot-new prompt template must preserve the single-question empty-arguments fallback"
        );
        assert!(
            template.contains("=== USER ACTIVE REQUEST ==="),
            "Pi hotpot-new prompt template must open with the USER ACTIVE REQUEST framing block so Pi parses the body as an active request, not background docs"
        );
        assert!(
            template.contains("NOT background documentation"),
            "Pi hotpot-new prompt template must explicitly exclude background-documentation framing so competing context (AGENTS.md, skills) does not absorb the prompt"
        );
    }

    /// Ensures source and checked-in Pi prompt templates stay synchronized.
    ///
    /// 确保源资产和仓库内 Pi 实际读取的 prompt 模板保持同步。
    #[test]
    fn pi_prompt_source_and_installed_template_stay_in_sync() {
        let source_template = include_str!("../../../assets/platforms/pi/prompts/hotpot-new.md");
        let installed_template = include_str!("../../../.pi/prompts/hotpot-new.md");
        let required_fragments = [
            "$ARGUMENTS",
            "<<< INITIAL TASK IDEA",
            "<<< END INITIAL TASK IDEA >>>",
            "IS the user's initial task idea",
            "MUST explicitly reference or paraphrase",
            "Do NOT ask another question to obtain the initial task idea",
            "Exception (empty arguments)",
            "no non-whitespace text",
            "ask exactly one concise question",
            "=== USER ACTIVE REQUEST ===",
            "NOT background documentation",
        ];

        for fragment in required_fragments {
            assert!(
                source_template.contains(fragment),
                "Pi hotpot-new source prompt must contain required argument fragment: {fragment}"
            );
            assert!(
                installed_template.contains(fragment),
                "Pi hotpot-new installed prompt must contain required argument fragment: {fragment}"
            );
        }
    }
}
