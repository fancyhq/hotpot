//! Strategy-aware merge helpers for Hotpot's asset installer.
//!
//! Platform main-config files (`.claude/settings.json`, `.codex/config.toml`,
//! `.opencode/package.json`, `.pi/package.json`) coexist with user-authored
//! content and must not be overwritten wholesale. This module performs
//! idempotent merges via `serde_json` (JSON) and `toml_edit` (TOML), using
//! "hotpot anchors" to dedupe hotpot-owned array entries on re-install.
//!
//! 平台主配置文件（`.claude/settings.json`、`.codex/config.toml`、
//! `.opencode/package.json`、`.pi/package.json`）与用户内容共存，不能整文件
//! 覆盖。本模块用 `serde_json` / `toml_edit` 对 hotpot 注入段做幂等合并：
//! 识别「hotpot 段锚点」做按谓词替换 / append；用户其它键值保留不变。

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::Value;
use toml_edit::{DocumentMut, Item, Table};

/// JSON anchor table: at the given path, identify hotpot-owned array items
/// by looking for a nested `hooks[].command` starting with the substring.
///
/// JSON 数组锚点表：在指定 path 下识别 hotpot 注入的数组元素（按内层
/// `hooks[].command` 是否以子串开头判断）。
const JSON_HOOK_ANCHORS: &[(&[&str], &str)] = &[
    (&["hooks", "PreToolUse"], "hotpot hook claude"),
    (&["hooks", "SubagentStart"], "hotpot hook claude"),
];

/// TOML array-of-tables anchor table. Same shape as the JSON anchors but
/// applies to `toml_edit::ArrayOfTables` navigation.
///
/// TOML array-of-tables 锚点表。结构同 JSON 锚点，但用于 `toml_edit` 文档
/// 树的导航。
const TOML_HOOK_ANCHORS: &[(&[&str], &str)] = &[
    (&["hooks", "PreToolUse"], "hotpot hook codex"),
    (&["hooks", "SessionStart"], "hotpot hook codex"),
];

/// Begin marker for a hotpot-managed text block (used by [`merge_text`]).
///
/// Hotpot 管理的文本块起始标记（见 [`merge_text`]）。
pub(super) const TEXT_BLOCK_BEGIN: &str = "# hotpot:begin";

/// End marker for a hotpot-managed text block (used by [`merge_text`]).
///
/// Hotpot 管理的文本块结束标记（见 [`merge_text`]）。
pub(super) const TEXT_BLOCK_END: &str = "# hotpot:end";

/// Merges the hotpot JSON asset into an existing JSON file's content,
/// preserving user-authored keys and using anchors to keep hotpot-owned array
/// entries idempotent on re-install.
///
/// 把 hotpot 资产中的 JSON 段合并到用户已有 JSON 文件上，保留用户其它键；
/// hotpot 段锚点命中时按谓词替换数组项，命中失败时 append。再次安装时合并
/// 结果与现状一致 → 上层会 skip。
pub(super) fn merge_json(existing: &str, hotpot: &str, target_path: &Path) -> Result<String> {
    let mut dst: Value = serde_json::from_str(existing)
        .with_context(|| format!("failed to parse JSON at {}", target_path.display()))?;
    let src: Value = serde_json::from_str(hotpot).with_context(|| {
        format!(
            "internal: hotpot asset for {} is not valid JSON",
            target_path.display()
        )
    })?;
    let mut path: Vec<String> = Vec::new();
    merge_json_value(&mut dst, &src, &mut path);
    // 用 to_string_pretty 输出 2-space 缩进，与 hotpot 资产风格一致。
    let mut out = serde_json::to_string_pretty(&dst).with_context(|| {
        format!(
            "failed to serialize merged JSON for {}",
            target_path.display()
        )
    })?;
    out.push('\n');
    Ok(out)
}

/// Merges a hotpot text block into an existing text file by anchoring on
/// `# hotpot:begin` … `# hotpot:end` line markers.
///
/// Behavior:
/// - If `existing` contains a matching `[TEXT_BLOCK_BEGIN]` / `[TEXT_BLOCK_END]`
///   line pair, the lines between them (inclusive of the markers) are replaced
///   with the hotpot block. Content outside the markers is preserved
///   byte-for-byte (including trailing newline / EOL style).
/// - Otherwise the hotpot block is appended at the end, ensuring exactly one
///   blank line separator if the existing content does not end with a newline.
/// - The hotpot block is expected to contain both markers on their own lines;
///   if not, this function bails (defensive: the embedded template should
///   always be well-formed).
///
/// Multiple marker pairs in `existing` are not supported — only the first
/// pair is rewritten and a `bail!` fires if the document has more than one
/// `# hotpot:begin`.
///
/// 用「锚点行」策略把 hotpot 文本块合并进已有文本文件（典型场景：
/// `.gitignore`）。锚点行存在时只重写锚点之间的内容，锚点外用户行保留；
/// 锚点不存在时在文件末尾追加。多个锚点对视为异常。
pub(super) fn merge_text(existing: &str, hotpot: &str, target_path: &Path) -> Result<String> {
    // 验证 hotpot 资产自身包含合法的锚点对（防御性检查）。
    let hotpot_begin = find_marker_line(hotpot, TEXT_BLOCK_BEGIN);
    let hotpot_end = find_marker_line(hotpot, TEXT_BLOCK_END);
    let (hb, he) = match (hotpot_begin, hotpot_end) {
        (Some(b), Some(e)) if b < e => (b, e),
        _ => bail!(
            "internal: hotpot text asset for {} is missing begin/end markers",
            target_path.display()
        ),
    };

    // 在已有内容里查所有锚点：>1 对视为异常，避免重写错位。
    let begin_count = existing
        .lines()
        .filter(|l| l.trim_start().starts_with(TEXT_BLOCK_BEGIN))
        .count();
    let end_count = existing
        .lines()
        .filter(|l| l.trim_start().starts_with(TEXT_BLOCK_END))
        .count();
    if begin_count > 1 || end_count > 1 {
        bail!(
            "{} contains multiple hotpot marker blocks; please clean up manually",
            target_path.display()
        );
    }

    let existing_begin = find_marker_line(existing, TEXT_BLOCK_BEGIN);
    let existing_end = find_marker_line(existing, TEXT_BLOCK_END);

    // hotpot 资产里的整段（含锚点行），按行重组成可粘贴的字符串。
    let hotpot_lines: Vec<&str> = hotpot.lines().collect();
    let hotpot_block: Vec<&str> = hotpot_lines[hb..=he].to_vec();
    let hotpot_block_str = hotpot_block.join("\n");

    match (existing_begin, existing_end) {
        (Some(b), Some(e)) if b < e => {
            // 把锚点之间（含锚点行）替换为 hotpot 块。
            let existing_lines: Vec<&str> = existing.lines().collect();
            let mut out: Vec<String> = Vec::with_capacity(existing_lines.len());
            for (idx, line) in existing_lines.iter().enumerate() {
                if idx == b {
                    out.push(hotpot_block_str.clone());
                } else if idx > b && idx <= e {
                    // 已经在锚点块的 hotpot 输出里，跳过原行。
                    continue;
                } else {
                    out.push((*line).to_string());
                }
            }
            let mut result = out.join("\n");
            // 保持与原文件相同的尾部 newline 行为。
            if existing.ends_with('\n') {
                result.push('\n');
            }
            Ok(result)
        }
        _ => {
            // 末尾追加：保证锚点块前空一行，且文件以单一 newline 收尾。
            let mut result = String::with_capacity(existing.len() + hotpot_block_str.len() + 2);
            result.push_str(existing);
            if !existing.is_empty() && !existing.ends_with('\n') {
                result.push('\n');
            }
            if !existing.is_empty() {
                result.push('\n');
            }
            result.push_str(&hotpot_block_str);
            result.push('\n');
            Ok(result)
        }
    }
}

/// Returns the 0-based line index of the first line whose trimmed prefix
/// matches `marker`. Returns `None` if no such line exists.
///
/// 返回首个以 `marker` 为行首前缀（忽略起始空白）的行号；不存在返回 None。
fn find_marker_line(text: &str, marker: &str) -> Option<usize> {
    text.lines()
        .position(|line| line.trim_start().starts_with(marker))
}

/// Merges the hotpot TOML asset into an existing TOML file's content via
/// `toml_edit`, which preserves comments, blank lines, and key order.
///
/// 用 `toml_edit` 把 hotpot 资产中的 TOML 段合并到用户已有 TOML 文件，保留
/// 用户原文档的注释、空行与键序；array-of-tables 锚点命中时按谓词替换、
/// 失败时 push。
pub(super) fn merge_toml(existing: &str, hotpot: &str, target_path: &Path) -> Result<String> {
    let mut dst: DocumentMut = existing
        .parse()
        .with_context(|| format!("failed to parse TOML at {}", target_path.display()))?;
    let src: DocumentMut = hotpot.parse().with_context(|| {
        format!(
            "internal: hotpot asset for {} is not valid TOML",
            target_path.display()
        )
    })?;
    let mut path: Vec<String> = Vec::new();
    merge_toml_table(dst.as_table_mut(), src.as_table(), &mut path);
    Ok(dst.to_string())
}

/// Recursively merges a `src` JSON value into `dst`.
///
/// - Objects: merge by key, recursing into values; missing keys are inserted.
/// - Arrays on a known anchor path: dedupe by the hotpot predicate.
/// - Everything else: replace leaf with `src`.
///
/// 递归合并 src 到 dst。对象按 key 合并；命中锚点的数组按 hotpot 谓词去重
/// append；其它叶子用 src 直接覆盖。
fn merge_json_value(dst: &mut Value, src: &Value, path: &mut Vec<String>) {
    match (dst, src) {
        (Value::Object(d), Value::Object(s)) => {
            for (k, v) in s.iter() {
                path.push(k.clone());
                if let Some(existing) = d.get_mut(k) {
                    merge_json_value(existing, v, path);
                } else {
                    d.insert(k.clone(), v.clone());
                }
                path.pop();
            }
        }
        (Value::Array(d), Value::Array(s)) if json_anchor_for(path).is_some() => {
            let substr = json_anchor_for(path).expect("anchor presence guarded by match guard");
            for src_item in s.iter() {
                // 资产里 hotpot 段都带锚点；如果不带（防御），按整体 append。
                if !json_hook_matches_hotpot(src_item, substr) {
                    d.push(src_item.clone());
                    continue;
                }
                let pos = d
                    .iter()
                    .position(|dst_item| json_hook_matches_hotpot(dst_item, substr));
                if let Some(idx) = pos {
                    d[idx] = src_item.clone();
                } else {
                    d.push(src_item.clone());
                }
            }
        }
        (dst_slot, src_val) => {
            *dst_slot = src_val.clone();
        }
    }
}

/// Returns the hotpot command substring associated with a JSON array path,
/// if that path matches one of the known anchors.
fn json_anchor_for(path: &[String]) -> Option<&'static str> {
    JSON_HOOK_ANCHORS.iter().find_map(|(anchor_path, substr)| {
        if path.len() == anchor_path.len()
            && path
                .iter()
                .zip(anchor_path.iter())
                .all(|(seg, expected)| seg == *expected)
        {
            Some(*substr)
        } else {
            None
        }
    })
}

/// Returns true if a JSON hook item has a nested `hooks[].command` starting
/// with the hotpot command substring.
fn json_hook_matches_hotpot(item: &Value, command_substr: &str) -> bool {
    item.get("hooks")
        .and_then(|h| h.as_array())
        .map(|arr| {
            arr.iter().any(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .map(|s| s.starts_with(command_substr))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Recursively merges a `src` TOML table into `dst`. Missing keys are
/// inserted with their original `Item` (preserving hotpot-asset comments and
/// formatting attached to that key).
fn merge_toml_table(dst: &mut Table, src: &Table, path: &mut Vec<String>) {
    for (key, src_item) in src.iter() {
        path.push(key.to_string());
        if dst.contains_key(key) {
            let dst_item = dst.get_mut(key).expect("contains_key guarantees presence");
            merge_toml_item(dst_item, src_item, path);
        } else {
            dst.insert(key, src_item.clone());
        }
        path.pop();
    }
}

/// Merges a single TOML `Item`. Tables recurse; array-of-tables on an anchor
/// path dedupe by the hotpot predicate; everything else replaces.
fn merge_toml_item(dst: &mut Item, src: &Item, path: &mut Vec<String>) {
    match (dst, src) {
        (Item::Table(d), Item::Table(s)) => {
            merge_toml_table(d, s, path);
        }
        (Item::ArrayOfTables(d), Item::ArrayOfTables(s)) if toml_anchor_for(path).is_some() => {
            let substr = toml_anchor_for(path).expect("anchor presence guarded by match guard");
            for src_table in s.iter() {
                if !toml_hook_matches_hotpot(src_table, substr) {
                    d.push(src_table.clone());
                    continue;
                }
                let pos = (0..d.len()).find(|&i| {
                    d.get(i)
                        .map(|t| toml_hook_matches_hotpot(t, substr))
                        .unwrap_or(false)
                });
                if let Some(idx) = pos {
                    if let Some(slot) = d.get_mut(idx) {
                        *slot = src_table.clone();
                    }
                } else {
                    d.push(src_table.clone());
                }
            }
        }
        (dst_slot, src_val) => {
            *dst_slot = src_val.clone();
        }
    }
}

/// Returns the hotpot command substring for the current TOML path (only
/// array-of-tables anchors are tracked).
fn toml_anchor_for(path: &[String]) -> Option<&'static str> {
    TOML_HOOK_ANCHORS.iter().find_map(|(anchor_path, substr)| {
        if path.len() == anchor_path.len()
            && path
                .iter()
                .zip(anchor_path.iter())
                .all(|(seg, expected)| seg == *expected)
        {
            Some(*substr)
        } else {
            None
        }
    })
}

/// Returns true if a TOML hook table has a nested `hooks` AoT whose any entry
/// has `command` starting with the hotpot command substring.
fn toml_hook_matches_hotpot(t: &Table, command_substr: &str) -> bool {
    let Some(item) = t.get("hooks") else {
        return false;
    };
    let Some(arr) = item.as_array_of_tables() else {
        return false;
    };
    arr.iter().any(|inner| {
        inner
            .get("command")
            .and_then(|c| c.as_str())
            .map(|s| s.starts_with(command_substr))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn p() -> &'static Path {
        Path::new("/tmp/hotpot-merge-test")
    }

    // ===== claude/settings.json shape =====

    const CLAUDE_HOTPOT: &str = r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "hotpot hook claude pre-tool-use",
            "timeout": 5
          }
        ]
      }
    ],
    "SubagentStart": [
      {
        "matcher": "hotpot-execution|hotpot-review",
        "hooks": [
          {
            "type": "command",
            "command": "hotpot hook claude subagent-start"
          }
        ]
      }
    ]
  }
}
"#;

    #[test]
    fn json_claude_appends_alongside_user_hooks() {
        let existing = r#"{
  "hooks": {
    "PreToolUse": [
      {"matcher":"Edit","hooks":[{"type":"command","command":"echo user-edit"}]}
    ]
  }
}"#;
        let out = merge_json(existing, CLAUDE_HOTPOT, p()).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2);
        assert!(pre.iter().any(|i| i["matcher"] == "Edit"));
        assert!(pre.iter().any(|i| i["matcher"] == "Bash"));
        let sub = v["hooks"]["SubagentStart"].as_array().unwrap();
        assert_eq!(sub.len(), 1);
    }

    #[test]
    fn json_claude_idempotent_with_repeat_merge() {
        let out = merge_json(CLAUDE_HOTPOT, CLAUDE_HOTPOT, p()).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        // hotpot 段在每个数组中正好出现一次
        assert_eq!(v["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(v["hooks"]["SubagentStart"].as_array().unwrap().len(), 1);
        // 结构与单次解析等价
        let v_in: Value = serde_json::from_str(CLAUDE_HOTPOT).unwrap();
        assert_eq!(v_in, v);
    }

    #[test]
    fn json_claude_preserves_top_level_user_keys() {
        let existing = r#"{"env":{"MY_KEY":"1"},"hooks":{"PreToolUse":[]}}"#;
        let out = merge_json(existing, CLAUDE_HOTPOT, p()).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["env"]["MY_KEY"], "1");
        let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 1);
        assert_eq!(pre[0]["matcher"], "Bash");
    }

    // ===== codex/config.toml shape =====

    const CODEX_HOTPOT: &str = "# Hotpot Codex project hooks.\n\n\
        [features]\n\
        hooks = true\n\n\
        [[hooks.PreToolUse]]\n\
        matcher = \"Bash|shell|exec\"\n\n\
        [[hooks.PreToolUse.hooks]]\n\
        type = \"command\"\n\
        command = \"hotpot hook codex pre-tool-use\"\n\n\
        [[hooks.SessionStart]]\n\
        matcher = \"startup|resume|clear\"\n\n\
        [[hooks.SessionStart.hooks]]\n\
        type = \"command\"\n\
        command = \"hotpot hook codex session-start\"\n";

    #[test]
    fn toml_codex_preserves_user_comments_and_tables() {
        let existing = "# user-added section\n[model]\ndefault = \"gpt-5\"\n";
        let out = merge_toml(existing, CODEX_HOTPOT, p()).unwrap();
        assert!(
            out.contains("# user-added section"),
            "lost user comment: {out}"
        );
        assert!(out.contains("[model]"), "lost user [model] table: {out}");
        assert!(out.contains("default = \"gpt-5\""));
        assert!(out.contains("hooks = true"));
        assert!(out.contains("hotpot hook codex pre-tool-use"));
        assert!(out.contains("hotpot hook codex session-start"));
    }

    #[test]
    fn toml_codex_idempotent_with_repeat_merge() {
        let out = merge_toml(CODEX_HOTPOT, CODEX_HOTPOT, p()).unwrap();
        assert_eq!(out.matches("hotpot hook codex pre-tool-use").count(), 1);
        assert_eq!(out.matches("hotpot hook codex session-start").count(), 1);
        assert_eq!(out.matches("hooks = true").count(), 1);
    }

    #[test]
    fn toml_codex_features_key_level_merge() {
        let existing = "[features]\nmy_feature = true\n";
        let out = merge_toml(existing, CODEX_HOTPOT, p()).unwrap();
        assert!(
            out.contains("my_feature = true"),
            "lost user feature: {out}"
        );
        assert!(
            out.contains("hooks = true"),
            "missing hotpot feature: {out}"
        );
    }

    // ===== opencode/package.json shape =====

    const OPENCODE_HOTPOT: &str = r#"{
  "dependencies": {
    "@opencode-ai/plugin": "1.14.41"
  }
}
"#;

    #[test]
    fn json_opencode_upgrades_plugin_version() {
        let existing = r#"{"dependencies":{"@opencode-ai/plugin":"1.0.0"}}"#;
        let out = merge_json(existing, OPENCODE_HOTPOT, p()).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["dependencies"]["@opencode-ai/plugin"], "1.14.41");
    }

    #[test]
    fn json_opencode_preserves_user_dev_deps_and_extra_deps() {
        let existing = r#"{
            "devDependencies": {"typescript": "^5.0.0"},
            "dependencies": {"lodash": "^4.17.0"}
        }"#;
        let out = merge_json(existing, OPENCODE_HOTPOT, p()).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["devDependencies"]["typescript"], "^5.0.0");
        assert_eq!(v["dependencies"]["lodash"], "^4.17.0");
        assert_eq!(v["dependencies"]["@opencode-ai/plugin"], "1.14.41");
    }

    #[test]
    fn json_opencode_idempotent_with_repeat_merge() {
        let out = merge_json(OPENCODE_HOTPOT, OPENCODE_HOTPOT, p()).unwrap();
        let v_in: Value = serde_json::from_str(OPENCODE_HOTPOT).unwrap();
        let v_out: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v_in, v_out);
    }

    // ===== pi/package.json shape =====

    const PI_HOTPOT: &str = r#"{
  "name": "hotpot-pi-package",
  "private": true,
  "keywords": ["pi-package"],
  "pi": {
    "extensions": ["./extensions"],
    "prompts": ["./prompts"]
  },
  "dependencies": {
    "@earendil-works/pi-coding-agent": "latest",
    "typebox": "latest"
  }
}
"#;

    #[test]
    fn json_pi_preserves_user_scripts_and_extra_fields() {
        let existing = r#"{"scripts":{"start":"pi run"},"version":"0.1.0"}"#;
        let out = merge_json(existing, PI_HOTPOT, p()).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["scripts"]["start"], "pi run");
        assert_eq!(v["version"], "0.1.0");
        assert_eq!(v["name"], "hotpot-pi-package");
        assert_eq!(v["pi"]["extensions"][0], "./extensions");
    }

    #[test]
    fn json_pi_replaces_keywords_array_with_hotpot_owned_value() {
        // keywords 是 hotpot 拥有的叶子，用户在此字段加的值会被覆盖。
        let existing = r#"{"keywords":["user-tag"]}"#;
        let out = merge_json(existing, PI_HOTPOT, p()).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let kws = v["keywords"].as_array().unwrap();
        assert_eq!(kws.len(), 1);
        assert_eq!(kws[0], "pi-package");
    }

    #[test]
    fn json_pi_idempotent_with_repeat_merge() {
        let out = merge_json(PI_HOTPOT, PI_HOTPOT, p()).unwrap();
        let v_in: Value = serde_json::from_str(PI_HOTPOT).unwrap();
        let v_out: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v_in, v_out);
    }

    // ===== text-block (.gitignore) shape =====
    //
    // The fixture mirrors the real `assets/templates/gitignore.hotpot` shape:
    // only ephemeral artifacts (brainstorm, worktrees, lock sidecars) are
    // listed — user-owned content under `.hotpot/` stays tracked. Tests
    // assert on the function behavior (anchors, idempotency, byte-stable
    // outside markers), not on which paths are inside the block.
    const GITIGNORE_HOTPOT: &str = "# hotpot:begin (managed by `hotpot update`; do not edit between these markers)\n\
        /.hotpot/brainstorm/\n\
        /.hotpot/worktrees/\n\
        /.hotpot/**/*.lock\n\
        # hotpot:end\n";

    #[test]
    fn text_appends_when_no_existing_markers() {
        // 用户已有内容但无锚点：在末尾追加 hotpot 块，原内容字节不变。
        let existing = "/target\nnode_modules/\n";
        let out = merge_text(existing, GITIGNORE_HOTPOT, p()).unwrap();
        assert!(
            out.starts_with("/target\nnode_modules/\n"),
            "lost user content: {out}"
        );
        assert!(out.contains(TEXT_BLOCK_BEGIN));
        assert!(out.contains(TEXT_BLOCK_END));
        assert!(out.contains("/.hotpot/worktrees/"));
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn text_creates_block_when_file_empty() {
        let out = merge_text("", GITIGNORE_HOTPOT, p()).unwrap();
        assert!(out.contains(TEXT_BLOCK_BEGIN));
        assert!(out.contains("/.hotpot/brainstorm/"));
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn text_rewrites_only_between_markers() {
        // 用户在锚点之间偷偷加了一行（不该这么用，但我们要确保仍然能整段重写）；
        // 锚点之外的用户内容必须 byte-for-byte 保留。
        let existing = "/target\n\
            # hotpot:begin (old)\n\
            stale_entry_that_should_be_removed\n\
            # hotpot:end\n\
            /node_modules\n";
        let out = merge_text(existing, GITIGNORE_HOTPOT, p()).unwrap();
        assert!(out.starts_with("/target\n"), "user prefix lost: {out}");
        assert!(out.ends_with("/node_modules\n"), "user suffix lost: {out}");
        assert!(
            !out.contains("stale_entry_that_should_be_removed"),
            "stale line not pruned: {out}"
        );
        assert!(
            out.contains("/.hotpot/brainstorm/"),
            "hotpot block missing: {out}"
        );
    }

    #[test]
    fn text_idempotent_with_repeat_merge() {
        let first = merge_text("", GITIGNORE_HOTPOT, p()).unwrap();
        let second = merge_text(&first, GITIGNORE_HOTPOT, p()).unwrap();
        assert_eq!(first, second, "merge not idempotent: {second}");
    }

    #[test]
    fn text_bails_on_multiple_marker_pairs() {
        // 防御：用户手工复制了两份锚点块，必须 bail 而不是错位重写。
        let existing = format!("{}{}", GITIGNORE_HOTPOT, GITIGNORE_HOTPOT);
        let err = merge_text(&existing, GITIGNORE_HOTPOT, p()).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("multiple hotpot marker"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn text_preserves_no_trailing_newline_when_absent() {
        // 锚点出现在已有文件中（带 newline 收尾）→ 输出保留 newline 收尾。
        let existing = "# hotpot:begin\nold\n# hotpot:end\n";
        let out = merge_text(existing, GITIGNORE_HOTPOT, p()).unwrap();
        assert!(out.ends_with('\n'));
    }
}
