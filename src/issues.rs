use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Ok, Result};
use indoc::indoc;
use serde::{Deserialize, Serialize};

use crate::paths::{issue_candidates_file_path, issues_file_path};

#[derive(Debug, Serialize, Deserialize)]
/// Historical issue category used to separate correctness problems from quality improvements.
pub enum IssueKind {
    #[serde(rename = "bug")]
    Bug,
    #[serde(rename = "optimization")]
    Optimization,
}

impl IssueKind {
    fn as_str(&self) -> &'static str {
        match self {
            IssueKind::Bug => "bug",
            IssueKind::Optimization => "optimization",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// Source metadata for a review memory item.
pub struct IssueSource {
    /// Files changed when the original issue was fixed.
    pub changed_files: Vec<String>,
    /// Short summary of the original fix; the full diff is intentionally not stored.
    pub summary: String,
}

#[derive(Debug, Serialize, Deserialize)]
/// A reusable review memory item loaded from `.hotpot/issues.jsonl`.
pub struct Issue {
    /// Date when the issue memory was created.
    pub date: chrono::NaiveDate,
    /// Short title describing the reusable problem pattern.
    pub title: String,
    /// Category of the historical issue.
    pub kind: IssueKind,
    /// Reusable keywords used to match future changes.
    pub tags: Vec<String>,
    /// Stable file paths or prefixes where this memory is relevant.
    pub paths: Vec<String>,
    /// Natural-language condition for when reviewers should consider this memory.
    pub scene: String,
    /// Description of the historical problem.
    pub description: String,
    /// Concrete check reviewers should perform when the scene matches.
    pub review_check: String,
    /// Preferred fix or prevention pattern.
    pub solution: String,
    /// Metadata about the change that produced this memory.
    pub source: IssueSource,
}

/// Lightweight facts about the current change used to match relevant review memories.
pub struct ChangeContext {
    /// Files changed in the current review target.
    pub changed_files: Vec<String>,
    /// Extracted keywords from filenames, symbols, summaries, or small relevant snippets.
    pub keywords: Vec<String>,
}

/// Temporary issue memory candidate written during a repair session.
///
/// Candidates are not long-term review memory yet. A later finish step should
/// deduplicate, merge, discard, or promote them into [`Issue`] records.
#[derive(Debug, Serialize, Deserialize)]
pub struct IssueCandidate {
    /// Creation timestamp supplied by the caller or AI.
    pub created_at: String,
    /// Why this candidate may be worth promoting later.
    pub reason: String,
    /// Files changed when the candidate was observed.
    pub changed_files: Vec<String>,
    /// Lightweight keywords extracted from the change.
    pub keywords: Vec<String>,
    /// Problem observed during the repair.
    pub problem: String,
    /// Fix that was applied or proposed.
    pub fix: String,
    /// Validation commands or checks that passed.
    pub validation: Vec<String>,
    /// Hint describing the likely long-term review memory to promote.
    pub promote_hint: String,
}

fn ensure_issues_exists(root_dir: &str) -> Result<PathBuf> {
    let path = issues_file_path(root_dir);
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建issue目录路径失败，路径：{}", path.display()))?;
        }
        fs::write(&path, "")
            .with_context(|| format!("创建issues文件失败，路径：{}", path.display()))?;
    }

    Ok(path)
}

fn ensure_issue_candidates_exists(root_dir: &str, username: &str) -> Result<PathBuf> {
    let path = issue_candidates_file_path(root_dir, username);
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建issue候选目录路径失败，路径：{}", path.display()))?;
        }
        fs::write(&path, "")
            .with_context(|| format!("创建issue候选文件失败，路径：{}", path.display()))?;
    }

    Ok(path)
}

/// Reads `.hotpot/issues.jsonl` and deserializes each non-empty line into an [`Issue`].
pub fn get_issues_list(root_dir: &str) -> Result<Vec<Issue>> {
    let issues_file_path = ensure_issues_exists(root_dir)?;
    let issues_content = fs::read_to_string(&issues_file_path)
        .with_context(|| format!("读取issue文件失败，路径：{}", issues_file_path.display()))?;
    if issues_content.trim().is_empty() {
        return Ok(Vec::new());
    }
    issues_content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<Issue>(line).with_context(|| {
                format!(
                    "解析issues.jsonl第 {} 行失败，文件路径：{}",
                    index + 1,
                    issues_file_path.display()
                )
            })
        })
        .collect()
}

/// Reads the current user's temporary issue candidates from JSONL.
pub fn get_issue_candidates_list(root_dir: &str, username: &str) -> Result<Vec<IssueCandidate>> {
    let candidates_file_path = ensure_issue_candidates_exists(root_dir, username)?;
    let candidates_content = fs::read_to_string(&candidates_file_path).with_context(|| {
        format!(
            "读取issue候选文件失败，路径：{}",
            candidates_file_path.display()
        )
    })?;
    if candidates_content.trim().is_empty() {
        return Ok(Vec::new());
    }

    candidates_content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<IssueCandidate>(line).with_context(|| {
                format!(
                    "解析issue-candidates.jsonl第 {} 行失败，文件路径：{}",
                    index + 1,
                    candidates_file_path.display()
                )
            })
        })
        .collect()
}

/// Appends one temporary issue candidate to the current user's JSONL file.
pub fn append_issue_candidate(
    root_dir: &str,
    username: &str,
    candidate: &IssueCandidate,
) -> Result<()> {
    let candidates_file_path = ensure_issue_candidates_exists(root_dir, username)?;
    let mut line = serde_json::to_string(candidate).context("序列化issue候选失败")?;
    line.push('\n');

    fs::OpenOptions::new()
        .append(true)
        .open(&candidates_file_path)
        .with_context(|| {
            format!(
                "打开issue候选文件失败，路径：{}",
                candidates_file_path.display()
            )
        })?
        .write_all(line.as_bytes())
        .with_context(|| {
            format!(
                "写入issue候选文件失败，路径：{}",
                candidates_file_path.display()
            )
        })
}

/// Clears temporary issue candidates after they are promoted or deliberately discarded.
pub fn clear_issue_candidates(root_dir: &str, username: &str) -> Result<()> {
    let candidates_file_path = ensure_issue_candidates_exists(root_dir, username)?;
    fs::write(&candidates_file_path, "").with_context(|| {
        format!(
            "清空issue候选文件失败，路径：{}",
            candidates_file_path.display()
        )
    })
}

fn render_markdown_header(title: &str) -> String {
    format!(
        indoc! {r#"
            # {title}

            这里记录的是历史 review 或实现中出现过的问题。执行代码 review 时，必须先根据每条记录的 `Scene` 判断当前变更是否相关。

            如果当前变更匹配某条 `Scene`，必须执行对应的 `Review Check`。如果发现同类问题，参考 `Solution` 给出修复建议，避免重复引入历史问题。

            示例：

            ```markdown
            ## 创建卡片显示优化

            - kind: optimization
            - date: 2026-05-11
            - tags: ui, card, border-radius
            - paths: src/components

            ### Scene

            当修改卡片、列表项、详情面板、弹窗内容块等 UI 容器时。

            ### Problem

            添加的卡片没有使用圆角形式，导致视觉风格不一致。

            ### Review Check

            检查新增或修改的卡片类 UI 是否遵循项目现有圆角规范。

            ### Solution

            给卡片添加 `10px` 的圆角边框。
            ```

        "#},
        title = title,
    )
}

fn push_issue_markdown(markdown: &mut String, issue: &Issue) {
    markdown.push_str(&format!(
        indoc! {"
            ## {title}
            - kind: {kind}
            - date: {date}
            - tags: {tags}
            - paths: {paths}

            ### Scene
            {scene}

            ### Problem
            {description}

            ### Review Check
            {review_check}

            ### Solution
            {solution}

        "},
        title = &issue.title,
        kind = issue.kind.as_str(),
        date = issue.date,
        tags = issue.tags.join(", "),
        paths = issue.paths.join(", "),
        scene = &issue.scene,
        description = &issue.description,
        review_check = &issue.review_check,
        solution = &issue.solution,
    ));
}

/// Renders already-selected issue references into markdown for AI review context.
pub fn render_issue_refs_to_markdown(title: &str, issues: &[&Issue]) -> String {
    let mut markdown = format!(
        indoc! {r#"
            # {title}

            以下历史问题与当前变更可能相关。请在 review 时逐条检查。

        "#},
        title = title,
    );

    if issues.is_empty() {
        markdown.push_str("No related historical issues matched this change.\n");
        return markdown;
    }

    for issue in issues {
        push_issue_markdown(&mut markdown, issue);
    }

    markdown
}

/// Renders all stored issue memories into markdown.
pub fn render_issues_to_markdown(root_dir: &str) -> Result<String> {
    let issues_list = get_issues_list(root_dir)?;
    let mut markdown = render_markdown_header("Hotpot Review Memory");

    for issue in &issues_list {
        push_issue_markdown(&mut markdown, issue);
    }

    Ok(markdown)
}

/// Scores how relevant an issue memory is to the current change context.
///
/// Path matches are weighted higher than tag matches because changed files are
/// usually a stronger signal than extracted keywords.
pub fn score_issue(issue: &Issue, context: &ChangeContext) -> usize {
    let keywords: Vec<String> = context
        .keywords
        .iter()
        .map(|keyword| keyword.to_lowercase())
        .collect();

    let path_score = issue
        .paths
        .iter()
        .filter(|issue_path| {
            context
                .changed_files
                .iter()
                .any(|changed_file| changed_file.starts_with(*issue_path))
        })
        .count()
        * 3;

    let tag_score = issue
        .tags
        .iter()
        .filter(|tag| {
            let tag = tag.to_lowercase();
            keywords.iter().any(|keyword| keyword == &tag)
        })
        .count()
        * 2;

    path_score + tag_score
}

/// Returns the top matching issue memories for a change context.
///
/// Issues with a zero score are excluded. The remaining issues are sorted by
/// descending relevance score and truncated to `limit`.
pub fn filter_relevant_issues<'a>(
    issues: &'a [Issue],
    context: &ChangeContext,
    limit: usize,
) -> Vec<&'a Issue> {
    let mut scored_issues: Vec<_> = issues
        .iter()
        .map(|issue| (score_issue(issue, context), issue))
        .filter(|(score, _)| *score > 0)
        .collect();

    scored_issues.sort_by(|(left_score, _), (right_score, _)| right_score.cmp(left_score));

    scored_issues
        .into_iter()
        .take(limit)
        .map(|(_, issue)| issue)
        .collect()
}

/// Loads issue memories, filters the most relevant entries, and renders them as markdown.
pub fn render_relevant_issues_to_markdown(
    root_dir: &str,
    context: &ChangeContext,
    limit: usize,
) -> Result<String> {
    let issues = get_issues_list(root_dir)?;
    let relevant_issues = filter_relevant_issues(&issues, context, limit);

    Ok(render_issue_refs_to_markdown(
        "Relevant Review Memory",
        &relevant_issues,
    ))
}

#[cfg(test)]
mod tests {
    use crate::utils::get_root_dir;

    use super::*;

    #[test]
    fn test_render_issues_to_markdown() {
        let root_dir = get_root_dir().unwrap();
        let markdown = render_issues_to_markdown(&root_dir).unwrap();
        let markdown_path = format!("{}/issues.md", root_dir);
        fs::write(markdown_path, markdown).unwrap();
    }

    #[test]
    fn test_filter_relevant_issues() {
        let root_dir = get_root_dir().unwrap();
        let issues = get_issues_list(&root_dir).unwrap();
        let context = ChangeContext {
            changed_files: vec![String::from("src/issues.rs")],
            keywords: vec![
                String::from("serde"),
                String::from("jsonl"),
                String::from("markdown"),
                String::from("rendering"),
            ],
        };

        let relevant_issues = filter_relevant_issues(&issues, &context, 2);

        assert_eq!(relevant_issues.len(), 2);
        assert!(
            relevant_issues
                .iter()
                .all(|issue| issue.paths.contains(&String::from("src/issues.rs")))
        );
    }

    #[test]
    fn test_issue_candidates_jsonl() {
        let root_dir = get_root_dir().unwrap();
        let username = "test_issue_candidates_jsonl";
        let candidate = IssueCandidate {
            created_at: String::from("2026-05-11T10:30:00Z"),
            reason: String::from("用户指出 JSONL 解析失败"),
            changed_files: vec![String::from("src/issues.rs")],
            keywords: vec![String::from("jsonl"), String::from("serde")],
            problem: String::from("issue 数据无法反序列化"),
            fix: String::from("写入合法 JSONL 数据"),
            validation: vec![String::from("cargo test issues::tests -- --nocapture")],
            promote_hint: String::from("适合沉淀为 JSONL 格式校验类 review memory"),
        };

        clear_issue_candidates(&root_dir, username).unwrap();
        append_issue_candidate(&root_dir, username, &candidate).unwrap();

        let candidates = get_issue_candidates_list(&root_dir, username).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].keywords,
            vec![String::from("jsonl"), String::from("serde")]
        );

        clear_issue_candidates(&root_dir, username).unwrap();
        let candidates = get_issue_candidates_list(&root_dir, username).unwrap();
        assert!(candidates.is_empty());
    }
}
