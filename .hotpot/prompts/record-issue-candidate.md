# Record Issue Candidate

You are deciding whether the current repair should be recorded as a temporary review memory candidate.

Temporary candidates are written to the project-shared `.hotpot/issue-candidates.jsonl`. They are not long-term review memory yet. Do not write to `.hotpot/issues.jsonl`.

## When To Record

Record one candidate only if at least one condition is true:

- The user pointed out an AI mistake, omission, or incorrect assumption.
- The fix reveals a reusable problem pattern that may happen again.
- The problem involves project conventions, data formats, architecture constraints, UI rules, testing rules, or review rules.
- The repair can become a concrete future `review_check`.
- The same kind of issue has appeared before or is likely to recur.
- The user explicitly says to remember it, avoid it later, or add it to rules.

## When Not To Record

Do not record candidates for:

- Ordinary feature implementation.
- One-off business requirements.
- Intermediate failed attempts.
- Unverified fixes.
- Problems that cannot become an actionable future review check.
- Simple formatting, renaming, or copy changes unless they reflect a project rule.

## Output Language

Before writing any natural-language content fields, check the project language preference:

1. Read `$ROOT_DIR/.hotpot/config.toml` if it exists.
2. If a top-level `language` field is present and non-empty, treat its value as a direct instruction for the output language (examples: `简体中文`, `english`, `日本語`, `zh-CN` — the value is free-form, written verbatim by the user).
3. If the file is missing, the field is missing, or the value is empty, default to English.

Apply that language to all natural-language fields you author: `title`, `scene`, `problem`, `solution`, `description`, `review_check`, `summary`, `fix`, `reason`, `promote_hint`, and the body of any task `.md` you produce.

Structural keys (`kind`, `date`, `tags`, `paths`) and code identifiers MUST remain in English regardless of the language setting.

## Output Rules

- If the repair is worth recording, output exactly one JSON object.
- If it is not worth recording, output nothing.
- Output valid JSON only.
- Keep the JSON on one line so it can be appended to a JSONL file.
- Do not include markdown fences.
- Do not include comments.
- Do not include the full diff.
- Copy `changed_files` from the caller-provided facts. Do not invent files.
- Use caller-provided keywords when available.

## Candidate Schema

```json
{
  "created_at": "YYYY-MM-DDTHH:MM:SSZ",
  "reason": "Why this candidate may be worth promoting later.",
  "changed_files": ["actual/changed/file"],
  "keywords": ["lowercase", "reusable", "keywords"],
  "problem": "The concrete problem observed during the repair.",
  "fix": "The fix that was applied or proposed.",
  "validation": ["commands or checks that passed"],
  "promote_hint": "What long-term review memory this could become."
}
```

## Example Output

{"created_at":"2026-05-11T10:30:00Z","reason":"用户指出 JSONL 解析失败，原因是部分 key 没有双引号。","changed_files":["src/issues.rs",".hotpot/issues.jsonl"],"keywords":["jsonl","serde","parsing"],"problem":"issues.jsonl 中存在非法 JSON 行，导致反序列化失败。","fix":"将 JSON object key 改成合法双引号格式，并通过解析测试验证。","validation":["cargo test test_render_issues_to_markdown -- --nocapture"],"promote_hint":"适合沉淀为 JSONL 格式校验类 review memory。"}
