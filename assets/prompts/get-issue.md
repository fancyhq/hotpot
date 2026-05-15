# Generate Review Memory Issue

You are updating this project's review memory after fixing a problem.

Use the provided context to create one JSON object that can be appended as a single line to `.hotpot/issues.jsonl`.

## Input You Will Receive

- The problem that was fixed.
- The changed files.
- The change keywords extracted by the caller.
- The diff summary or implementation summary.
- The relevant snippets, if needed.
- The final fix explanation.
- The commands or checks that were run, if available.

The caller should not provide a large full diff. If the change is large, use changed files, diffstat, extracted keywords, and only the small snippets needed to understand the fix.

## Output Language

Before writing any natural-language content fields, check the project language preference:

1. Read `$ROOT_DIR/.hotpot/config.toml` if it exists.
2. If a top-level `language` field is present and non-empty, treat its value as a direct instruction for the output language (examples: `简体中文`, `english`, `日本語`, `zh-CN` — the value is free-form, written verbatim by the user).
3. If the file is missing, the field is missing, or the value is empty, default to English.

Apply that language to all natural-language fields you author: `title`, `scene`, `problem`, `solution`, `description`, `review_check`, `summary`, `fix`, `reason`, `promote_hint`, and the body of any task `.md` you produce.

Structural keys (`kind`, `date`, `tags`, `paths`) and code identifiers MUST remain in English regardless of the language setting.

## Output Rules

- Output valid JSON only.
- Output exactly one JSON object.
- Do not wrap the JSON in markdown fences.
- Do not include comments.
- Do not include the full diff.
- Do not infer changed files that were not provided by the caller.
- Do not include fields outside the schema.
- Keep the JSON on one line so it can be appended to a JSONL file.
- Use today's date for `date` unless the caller provides another date.

## Schema

```json
{
  "date": "YYYY-MM-DD",
  "title": "Short reusable title",
  "kind": "bug | optimization",
  "tags": ["lowercase", "reusable", "keywords"],
  "paths": ["stable/path/or/prefix"],
  "scene": "When future code changes should consider this memory.",
  "description": "The historical problem that happened.",
  "review_check": "The concrete check a reviewer must perform when the scene matches.",
  "solution": "The preferred fix pattern or prevention strategy.",
  "source": {
    "changed_files": ["actual/file/that/changed"],
    "summary": "Short summary of the fix source. Do not include the full diff."
  }
}
```

## Field Guidance

- `title`: 4-12 words. Describe the reusable issue, not the one-off task.
- `kind`: Use `bug` for incorrect behavior. Use `optimization` for quality, maintainability, UX, or consistency improvements.
- `tags`: Generate 3-8 lowercase tags. Prefer reusable concepts over one-off names.
- `tags`: Prefer tags that can be matched by future extracted change keywords.
- `paths`: Use stable path prefixes when the issue applies broadly. Use exact file paths when the issue is file-specific.
- `scene`: Describe when future AI review should activate this memory.
- `description`: Describe what went wrong and why it matters.
- `review_check`: Make this actionable and review-oriented.
- `solution`: Describe the fix pattern that should be reused.
- `source.changed_files`: Copy the actual changed files from the input.
- `source.summary`: Summarize the fix source in one sentence.

## Large Change Handling

When the provided change is large:

- Focus on the specific fixed problem, not every file in the change.
- Use caller-provided keywords instead of reading the entire diff.
- Prefer stable reusable tags that future keyword extraction can match.
- Keep `source.summary` concise.
- Do not include raw patches, hunks, or long code snippets in the JSON.

## Tag Examples

- `rust`
- `serde`
- `jsonl`
- `markdown-rendering`
- `error-handling`
- `path-matching`
- `ui`
- `card`
- `border-radius`
- `async`
- `state`
- `test`

## Path Examples

- Use `src/issues.rs` when the memory only applies to issue parsing or rendering.
- Use `src/task.rs` when the memory applies to task state logic.
- Use `src/components` when the memory applies broadly to UI components.
- Use `site` when the memory applies broadly to the website.

## Example Output

{"date":"2026-05-11","title":"JSONL issue keys must be quoted","kind":"bug","tags":["jsonl","serde","parsing","rust"],"paths":[".hotpot/issues.jsonl","src/issues.rs"],"scene":"When editing issue memory data or changing JSONL parsing for review memory.","description":"Issue records failed to parse because some JSON object keys were not quoted, making the JSONL file invalid.","review_check":"Check that every appended issue record is valid single-line JSON and can be deserialized by the Issue schema.","solution":"Write issue memory through a schema-validated helper and append only valid compact JSON objects.","source":{"changed_files":[".hotpot/issues.jsonl","src/issues.rs"],"summary":"Fixed invalid JSONL issue records and tightened the review memory schema."}}
