# Summarize Issue Candidates

You are running at the end of a task, such as a future `finish-work` command.

Your job is to summarize temporary issue candidates into long-term review memory candidates. Temporary candidates come from `.hotpot/workspaces/{username}/issue-candidates.jsonl`. Final records must match the schema in `prompts/get-issue.md` and may later be appended to `.hotpot/issues.jsonl` after user confirmation.

## Input You Will Receive

- All temporary issue candidates for the current workspace.
- The final changed files for the task.
- Extracted change keywords.
- The final task summary.
- Validation commands or checks that passed.
- Existing issues from `.hotpot/issues.jsonl`, if available.

## Required Process

1. Discard candidates that are unverified, one-off, duplicated by existing long-term memory, or not actionable as a future review check.
2. Merge candidates that describe the same reusable problem pattern.
3. Keep candidates separate when they produce different future `review_check` items.
4. Produce 0-N final issue JSON objects.
5. Do not include full diffs, raw patches, or long snippets.

## Output Language

Before writing any natural-language content fields, check the project language preference:

1. Read `$ROOT_DIR/.hotpot/config.toml` if it exists.
2. If a top-level `language` field is present and non-empty, treat its value as a direct instruction for the output language (examples: `简体中文`, `english`, `日本語`, `zh-CN` — the value is free-form, written verbatim by the user).
3. If the file is missing, the field is missing, or the value is empty, default to English.

Apply that language to all natural-language fields you author: `title`, `scene`, `problem`, `solution`, `description`, `review_check`, `summary`, `fix`, `reason`, `promote_hint`, and the body of any task `.md` you produce.

Structural keys (`kind`, `date`, `tags`, `paths`) and code identifiers MUST remain in English regardless of the language setting.

## Output Rules

- Output valid JSON only.
- Output exactly one JSON object with `promoted`, `discarded`, and `merged` arrays.
- Do not wrap the output in markdown fences.
- Do not include comments.
- `promoted` contains final issue objects that match `prompts/get-issue.md`.
- `discarded` explains which candidate was dropped and why.
- `merged` explains which candidates were merged and into which promoted issue title.
- It is valid for `promoted` to be empty.

## Output Schema

```json
{
  "promoted": [
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
  ],
  "discarded": [
    {
      "candidate": "Short candidate identifier or problem summary",
      "reason": "Why it was not promoted."
    }
  ],
  "merged": [
    {
      "candidates": ["Short candidate identifiers or problem summaries"],
      "promoted_title": "Title of the promoted issue they were merged into."
    }
  ]
}
```

## Promotion Rules

Promote a candidate only when:

- The repair was validated or the final task state confirms it.
- The problem may recur in future changes.
- The memory can become a concrete `review_check`.
- The record is not already covered by an existing issue.

## Merging Rules

- Merge candidates when they share the same cause, same future scene, and same review check.
- Keep candidates separate when they apply to different paths, different rules, or different checks.
- Prefer one high-quality promoted issue over several noisy near-duplicates.

## Example Output

{"promoted":[{"date":"2026-05-11","title":"JSONL issue records must be valid","kind":"bug","tags":["jsonl","serde","parsing"],"paths":[".hotpot/issues.jsonl","src/issues.rs"],"scene":"When editing issue memory data or changing JSONL parsing for review memory.","description":"Issue memory records failed to parse because JSON object keys were not quoted, making the JSONL file invalid.","review_check":"Check that each issue memory record is valid single-line JSON and deserializes with the current Issue schema.","solution":"Append issue memory through a schema-validated helper and keep JSON object keys quoted.","source":{"changed_files":[".hotpot/issues.jsonl","src/issues.rs"],"summary":"Fixed invalid JSONL issue records and verified issue markdown rendering."}}],"discarded":[],"merged":[{"candidates":["JSONL key quoting failure","serde JSONL parsing failure"],"promoted_title":"JSONL issue records must be valid"}]}
