# Fix Npm Bin Executable Bit

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | true | 3 | medium |
:::

---

## Task

### Summary

::: info
Fix the npm global installation path where `npm install -g @fancyhq/hotpot` creates `/Users/bytedance/.npm-global/bin/hotpot` as a symlink to `bin/hotpot.js`, but fish refuses to run it because the symlink target is not executable.
:::

### User Request

::: info 用户原始请求
通过 `/hotpot/new` 帮我创建一个修复任务，解决 npm 安装不能执行的问题。

用户现场诊断信息：

- `npm install -g` 的全局 bin 目录是 `/Users/bytedance/.npm-global/bin`。
- 同目录下其他全局命令可用，只有 `hotpot` 不可用。
- `ls /Users/bytedance/.npm-global/bin` 可以看到 `hotpot`。
- 原生 Rust 二进制 `/Users/bytedance/.npm-global/lib/node_modules/@fancyhq/hotpot/bin/hotpot --version` 可以正常执行。
- `/Users/bytedance/.npm-global/bin/hotpot` 是正常 symlink：`../lib/node_modules/@fancyhq/hotpot/bin/hotpot.js`。
- fish 执行 `hotpot --help` 报错：`fish: Unknown command. '/Users/bytedance/.npm-global/bin/hotpot' exists but is not an executable file.`
:::

### Approved Design

::: tip
修复应聚焦 npm 包发布/打包阶段的入口文件权限，而不是要求用户手动 `chmod`。

已确认的根因方向：npm 全局命令 `hotpot` 是 symlink，fish 会检查 symlink 最终目标 `npm/bin/hotpot.js` 的 executable bit。如果 npm package tarball 中 `bin/hotpot.js` 没有 executable bit，即使 shebang 正确、PATH 正确、原生 Rust 二进制可执行，fish 仍会拒绝执行。

执行阶段应做最小修复：确保仓库中的 `npm/bin/hotpot.js` 以可执行权限进入 npm tarball，并新增验证防止回归。必要时也确认 `npm/scripts/install.js` 下载出的原生二进制仍会设置 executable bit。
:::

### Alternatives Considered

- 用户本机执行 `chmod +x /Users/bytedance/.npm-global/lib/node_modules/@fancyhq/hotpot/bin/hotpot.js`：可临时解决，但重装后可能复现，不能修复发布包。
- 改 `package.json` 让 `bin.hotpot` 直接指向原生二进制：不可取，因为原生二进制由 `postinstall` 动态下载，不适合作为 npm package 静态 `bin` 入口；并且 Windows/Unix 二进制名不同。
- 在 `postinstall` 中额外 chmod wrapper：可作为兼容性兜底，但首选仍是让 npm tarball 自身保留 `bin/hotpot.js` 的 executable bit，因为 npm install 创建 bin link 时通常依赖包内入口文件权限。
- 推荐方案：确保 `npm/bin/hotpot.js` 在 git/worktree/package tarball 中是 executable，并用 `npm pack --dry-run --json` 或实际 `npm pack --json` 加测试断言 tarball entry mode 包含 executable bit；如需要，再增加 postinstall chmod wrapper 的兜底。

### Requirements

- `npm install -g @fancyhq/hotpot` 后，Unix-like shells including fish must be able to execute `hotpot` directly without manual `chmod`.
- `npm/bin/hotpot.js` must keep the shebang `#!/usr/bin/env node` and must be executable in the published npm tarball.
- The native Rust binary downloaded by `npm/scripts/install.js` must remain executable on Unix and keep the existing Windows behavior for `hotpot.exe`.
- The npm `bin` mapping must remain `{ "hotpot": "bin/hotpot.js" }`; installed command name must stay `hotpot`.
- Add regression validation that detects a non-executable npm bin wrapper before publish.
- Do not introduce third-party npm dependencies for this fix.
- Outputs in scripts/tests must be English.
- Comments in newly written code should be bilingual English first, Chinese second, following repository conventions.
- If the fix changes npm distribution behavior or release validation, update `docs/ARCH.md` and `docs/ARCH.zh_CN.md` with matching English/Chinese content.

### Non-Goals

::: details Non-Goals
- Do not rename the npm package `@fancyhq/hotpot`.
- Do not rename the installed command from `hotpot`.
- Do not change the crates.io package name `hotpot-ai`.
- Do not change GitHub Release asset naming unless execution discovers it is directly required for this bug.
- Do not require users to add aliases or manually chmod files as the primary fix.
- Do not publish a release or npm package from the task.
:::

### Project Context

- `npm/package.json` currently declares `"bin": { "hotpot": "bin/hotpot.js" }` and `"postinstall": "node scripts/install.js"`.
- `npm/bin/hotpot.js` is the Node wrapper that forwards arguments and stdio to the native binary in the same package `bin/` directory.
- `npm/scripts/install.js` downloads the platform-specific GitHub Release archive and writes the native binary to `npm/bin/hotpot` or `npm/bin/hotpot.exe`, setting executable permissions for Unix native binaries.
- The user confirmed the native binary itself works when invoked by absolute path, so the bug is specifically the npm wrapper executable permission path.
- fish reports `exists but is not an executable file` for `/Users/bytedance/.npm-global/bin/hotpot`, which is a symlink to `bin/hotpot.js`; this strongly indicates the symlink target lacks executable permission.
- `docs/ARCH.md` and `docs/ARCH.zh_CN.md` already describe npm distribution. If validation/release workflow behavior changes, update both.

---

## Plan

### Mode

- tdd: true

### Execution Strategy

- git-worktree: false
- rationale: The fix is limited to npm package metadata/file mode, validation, and possibly docs. It can be safely implemented in the current checkout without a separate worktree.

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `npm/bin/hotpot.js` | Modify metadata / possibly content | Ensure the npm CLI wrapper is executable and remains a valid shebang Node entrypoint. |
| `npm/package.json` | Modify | Add or update a test/validation script if needed. |
| `npm/scripts/install.js` | Modify | Optional fallback to chmod the wrapper during postinstall if needed. |
| `npm/scripts/install.test.js` | Modify/Create | Add regression tests for wrapper executable bit and package bin contract. |
| `scripts/` validation helper | Create if needed | Optional minimal helper if tarball mode validation is cleaner outside the test file. |
| `docs/ARCH.md` | Modify if behavior changes | Document npm wrapper executable-bit contract if release validation changes. |
| `docs/ARCH.zh_CN.md` | Modify if behavior changes | Chinese counterpart of architecture documentation. |

### Implementation Tasks

#### Task 1: Reproduce and lock the executable-bit failure

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `npm/scripts/install.test.js` | Modify/Create | Add a regression check that fails when `npm/bin/hotpot.js` is not executable in the working tree or npm tarball. |
| `npm/package.json` | Modify | Add a script such as `test:install` only if it improves discoverability. |

##### Red

- [x] R1: Add a Node built-in `node:test` test named `npm_bin_wrapper_is_executable_for_global_shells` that checks `npm/bin/hotpot.js` has executable bits on Unix (`mode & 0o111`) and keeps the `#!/usr/bin/env node` shebang.
- [x] R2: Add a test named `npm_package_exposes_hotpot_wrapper_bin` that reads `npm/package.json` and asserts `bin.hotpot === "bin/hotpot.js"`.
- [x] R3: Add tarball-level validation if feasible: run `npm pack --json ./npm` into a temporary directory or use `npm pack --dry-run --json ./npm` if it exposes mode, then assert the packed `bin/hotpot.js` entry is executable. If Node's standard library is used to inspect `.tgz`, do not add third-party deps.
- [x] R4: Run the new test command, for example `node --test npm/scripts/install.test.js`; it should fail on the current broken file mode or explicitly prove the check catches a simulated non-executable mode without leaving the simulation in place.

##### Green

- [x] G1: Set `npm/bin/hotpot.js` executable in the repository/package source so the npm tarball preserves executable mode.
- [x] G2: If tarball validation shows npm does not preserve the mode reliably, add the smallest postinstall fallback in `npm/scripts/install.js` to chmod the wrapper on Unix, with bilingual comments.
  > Tarball validation passes (mode 493/0o755 is preserved by npm pack). Fallback not needed.
- [x] G3: Ensure `npm/bin/hotpot.js` still has the correct shebang and wrapper behavior.
- [x] G4: Re-run the test command; it must pass.

##### Refactor

- [x] F1: Keep tests minimal and deterministic; avoid real network or GitHub Release downloads.
- [x] F2: Re-run tests after any cleanup.

:::

#### Task 2: Validate npm packaging behavior end-to-end

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `npm/package.json` | Verify/Modify | Ensure package `files` includes `bin/` and scripts needed by install. |
| `npm/scripts/install.js` | Verify/Modify | Ensure native binary chmod remains correct. |
| `npm/scripts/install.test.js` | Modify | Cover native binary chmod assumptions if helper functions are testable. |

##### Red

- [x] R1: Add or run validation proving the npm package includes `bin/hotpot.js`, `scripts/install.js`, and no missing wrapper files.
- [x] R2: Add or run validation that `npm/scripts/install.js` still sets executable permissions for the downloaded Unix native binary.
- [x] R3: If existing code cannot be tested without performing network I/O, refactor only pure helper boundaries needed for deterministic tests, guarded by `if (require.main === module)` to avoid downloads during import.

##### Green

- [x] G1: Fix any missing package files or permission handling discovered by Red.
- [x] G2: Run `npm pack --dry-run ./npm` and confirm the package contents include the wrapper and install script.
- [x] G3: Run `npm pack --json ./npm` in a temporary directory if needed to inspect the actual tarball metadata; delete the generated tarball or keep it outside the repository.
  > Done via `npm pack --dry-run --json ./npm` which shows tarball metadata without creating files.

##### Refactor

- [x] F1: Avoid introducing a broad npm test framework or dependencies.
- [x] F2: Keep install script output in English and comments bilingual if new comments are needed.

:::

#### Task 3: Document and final validation

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify if needed | Document the npm wrapper executable-bit contract and validation. |
| `docs/ARCH.zh_CN.md` | Modify if needed | Keep Chinese architecture docs synchronized. |
| Task file | Modify | Mark completed checkboxes if execution workflow updates task progress. |

##### Red

- [x] R1: Check whether `docs/ARCH.md` / `docs/ARCH.zh_CN.md` mention enough about npm wrapper packaging. If the fix adds new validation or postinstall chmod behavior, docs are currently incomplete.
  > Docs did not mention executable bit contract or regression tests. Updated.

##### Green

- [x] G1: Update both architecture documents only if distribution behavior or validation contract changes.
- [x] G2: Run final validation commands listed below.

##### Refactor

- [x] F1: Review for accidental chmod-only local fix with no regression coverage; that is not acceptable.
- [x] F2: Review `git diff --stat` and `git diff` to ensure no generated tarballs or temp files are included.

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `node --test npm/scripts/install.test.js` | Passes executable-bit, bin mapping, and packaging contract tests without network access. |
| `npm pack --dry-run ./npm` | Shows a valid package containing `bin/hotpot.js`, `bin/` and `scripts/`; no missing npm wrapper files. |
| `ls -l npm/bin/hotpot.js` | Shows executable bits on Unix, e.g. `-rwxr-xr-x`. |
| `cargo test` | Existing Rust tests pass; if skipped due time/environment, report the reason. |

### Risks and Watchouts

::: warning
- File mode changes can be easy to miss in text diffs. Use `git diff --summary` or equivalent to verify `npm/bin/hotpot.js` mode changed to executable if needed.
- `apply_patch` may not be sufficient to change file mode. Use a non-destructive chmod command for mode changes when necessary, then verify with git diff summary.
- Do not confuse the wrapper `npm/bin/hotpot.js` with the downloaded native binary `npm/bin/hotpot`. The user already confirmed the native binary works.
- Do not rely on shell cache fixes, aliases, or local chmod instructions as the product fix.
- Avoid real network downloads in tests. Keep postinstall tests pure or inspect source/package metadata.
:::

---

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Because `tdd: true`, follow Red → Green → Refactor for every `#### Task N` and capture failing/passing validation summaries.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task.
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff.
- Run the validation commands before reporting completion.
