//! Cross-platform asset catalog.
//!
//! [`SHARED_ASSETS`] is installed under every `--platform` selection so the
//! four platform `ASSETS` arrays in [`super::platforms`] don't have to
//! register these entries individually.
//!
//! 跨平台共享资产目录。每次安装任意 `--platform` 都会同时安装这里登记的
//! 资产，所以各平台 `ASSETS` 数组不必单独重复登记。

use super::Asset;

/// Cross-platform assets installed alongside every `--platform` selection.
///
/// These are the LLM prompts that the four platforms reference at runtime
/// via either `@.hotpot/prompts/...` expansion (Claude/OpenCode) or the
/// `$HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT` /
/// `$HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT` / `$HOTPOT_TDD_PROTOCOL_PROMPT`
/// / `$HOTPOT_NEW_PROMPT` / `$HOTPOT_EXECUTE_PROMPT` /
/// `$HOTPOT_FINISH_WORK_PROMPT` environment variables (Codex/Pi). The
/// runtime path is resolved by `src/context.rs::prompt_path` as
/// `<ROOT_DIR>/.hotpot/prompts/<name>.md`, so the install target lives
/// under the Hotpot-owned `.hotpot/` namespace rather than the project
/// root.
///
/// 跨平台共享资产：四个平台的 new / execute / finish-work 编排都依赖
/// 这几份提示词（Claude/OpenCode 走 `@.hotpot/prompts/...`，Codex/Pi 走
/// 环境变量），而运行时路径由 `src/context.rs::prompt_path` 硬编码为
/// `<ROOT_DIR>/.hotpot/prompts/<name>.md`——和 `.hotpot/issues.jsonl`
/// 等内部文件同属 Hotpot 命名空间，不放到用户项目根。
pub(crate) const SHARED_ASSETS: &[Asset] = &[
    Asset::owned(
        ".hotpot/prompts/get-issue.md",
        include_str!("../../assets/prompts/get-issue.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/record-issue-candidate.md",
        include_str!("../../assets/prompts/record-issue-candidate.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/summarize-issue-candidates.md",
        include_str!("../../assets/prompts/summarize-issue-candidates.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/tdd-protocol.md",
        include_str!("../../assets/prompts/tdd-protocol.md"),
    ),
    // 跨工作流共用的「按 `.hotpot/config.toml::language` 决定自然语言输出」
    // 指令。四份主工作流 prompt (hotpot-new / hotpot-execute / hotpot-finish-work /
    // tdd-protocol) 都通过 `@.hotpot/prompts/output-language.md`（Claude/OpenCode）
    // 或 `$ROOT_DIR/.hotpot/prompts/output-language.md`（Codex/Pi）引用。
    //
    // Shared "respect `.hotpot/config.toml::language` for natural-language
    // output" directive. The four main workflow prompts (hotpot-new /
    // hotpot-execute / hotpot-finish-work / tdd-protocol) reference this file
    // via `@.hotpot/prompts/output-language.md` (Claude/OpenCode) or the
    // resolved `$ROOT_DIR/.hotpot/prompts/output-language.md` (Codex/Pi).
    Asset::owned(
        ".hotpot/prompts/output-language.md",
        include_str!("../../assets/prompts/output-language.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/hotpot-new.md",
        include_str!("../../assets/prompts/hotpot-new.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/hotpot-execute.md",
        include_str!("../../assets/prompts/hotpot-execute.md"),
    ),
    Asset::owned(
        ".hotpot/prompts/hotpot-finish-work.md",
        include_str!("../../assets/prompts/hotpot-finish-work.md"),
    ),
    // 项目根 .gitignore：用「# hotpot:begin / # hotpot:end」锚点行做行块合并。
    // 锚点外用户内容字节保留；锚点之间的内容由 hotpot 完全管理。
    Asset::merge_text(
        ".gitignore",
        include_str!("../../assets/templates/gitignore.hotpot"),
    ),
    // `.hotpot/config.toml`：用户自有配置文件，首次 init 时播种一份带英文
    // 注释的模板说明可用参数；之后的 init / update 都不会覆盖用户的修改。
    //
    // `.hotpot/config.toml`: user-owned config seed. Hotpot writes a fully
    // commented template on first install so the user can see every
    // available parameter; subsequent runs skip the file unconditionally so
    // edits survive re-init / update.
    Asset::create_if_missing(
        ".hotpot/config.toml",
        include_str!("../../assets/templates/hotpot-config.toml"),
    ),
];
