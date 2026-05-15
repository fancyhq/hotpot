# TDD Protocol

You are running in Hotpot TDD mode. The active task file's `## Plan > ### Mode` block declared `tdd: true`. For every Implementation Task in the task file's `## Plan > Implementation Tasks` section, follow the Red → Green → Refactor cycle strictly and capture the required evidence in your execution report.

This protocol is loaded by the orchestrator and applied to both the execution agent and the review agent. The review agent uses it as the audit checklist.

## Output Language

Apply the project language preference to the natural-language portions of your execution report and to any prose you surface to the user (task descriptions, refactor rationale, blocker explanations). The structured Reporting block below (keys: `Task`, `Red`, `Green`, `Refactor`, `command`, `failure`, `pass`, `full validation`, `action`, `rerun`) and the literal phrase `no refactor needed` MUST remain in English regardless of the configured language — the review agent grep's these tokens. Full rule:

@.hotpot/prompts/output-language.md

Codex / Pi (no `@path` expansion): use `$ROOT_DIR/.hotpot/prompts/output-language.md` via `Read`.

## Red (test first)

- Write or modify the failing test in the file listed under `Files: Test`.
- Run the exact command listed in the `R2` checkbox (typically `cargo test <exact_test_name>` or the project's test runner equivalent).
- **The run MUST fail with an assertion / behavioral error directly tied to the new test.** Compile errors, missing dependencies, command-not-found, syntax errors, or environment failures DO NOT count as a valid Red.
- Capture the failure output verbatim: failing test name, the assertion line, expected vs actual values. Include it in your execution report.
- If the test passes on the first run, the test has no discriminating power. STOP and report this back to the orchestrator. Do NOT proceed to Green.
- Tick `R1` and `R2` checkboxes only after the failure has been observed and recorded.

## Green (minimal implementation)

- Make the smallest possible change in the file(s) listed under `Files: Modify` to make the failing test pass.
- Do NOT refactor unrelated code, do NOT add unrelated features, do NOT delete unrelated code, do NOT update unrelated docs.
- Run the exact test command again. Capture the success output (key line such as `test result: ok. N passed`).
- Run the full validation command from `## Plan > Validation`. Capture the result.
- If any other test fails or any other validation breaks, STOP and report a regression. Do NOT proceed to Refactor.
- Tick `G1`, `G2`, `G3` checkboxes only after both the targeted test and the full validation have been observed green.

## Refactor (cleanup)

- Inspect the code for naming, duplication, missing abstraction, dead branches, or readability issues introduced by the Green change.
- If you refactor:
  - Describe the action concretely (e.g. "renamed `foo` to `bar`", "extracted helper `parse_x`", "removed dead branch `if y { ... }`").
  - Re-run the task's test command. Capture the result.
  - Re-run the full validation command. Capture the result.
- If no refactor is warranted, write the literal phrase `no refactor needed` in the execution report for this task.
- Tick `F1` and `F2` checkboxes only after the refactor decision is final and any required re-runs are green.

## Reporting

Your execution report MUST include, per Implementation Task, a structured block formatted exactly like:

```
Task <N>: <task title>
  Red:
    command: <exact command>
    failure: <captured failure summary, including failing test name and assertion line>
  Green:
    command: <exact command>
    pass: <captured pass summary, e.g. "test result: ok. 3 passed">
    full validation: <captured validation summary>
  Refactor:
    action: <description, or "no refactor needed">
    rerun: <captured re-run summary, or "skipped (no refactor)">
```

This block is the audit trail the review agent uses to verify TDD conformance. Missing or out-of-order blocks will be treated as a `High`-severity finding. A Red whose `failure` is a compile/dependency/environment error will be treated as `High`. A Green whose diff contains unrelated changes will be treated as `Medium`.

If the protocol cannot be followed (e.g. the project has no test runner, the task is genuinely not testable), STOP execution and report the obstacle. Do NOT silently degrade to non-TDD mode.
