# Bash Before Username Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement session-scoped username resolution in `.opencode/plugins/bash-before.ts` so shell commands receive `HOTPOT_USERNAME` from environment, git, or interactive prompt.

**Architecture:** Keep all logic in `.opencode/plugins/bash-before.ts` with a small set of local helpers and a closure-scoped `sessionUsername` cache. Resolve the username during `session.created`, then inject it alongside `ROOT_DIR` during each `shell.env` call without mutating persistent system state.

**Tech Stack:** TypeScript, Node.js built-ins, `@opencode-ai/plugin`, git CLI

---

### File Structure

**Files:**
- Modify: `.opencode/plugins/bash-before.ts`
- Create: `docs/superpowers/plans/2026-05-10-bash-before-username-implementation.md`

`.opencode/plugins/bash-before.ts` remains the single implementation unit and will own:

- Session username cache
- Environment variable lookup
- Git work tree detection and username resolution
- Interactive fallback prompt
- Shell environment injection

### Task 1: Add Resolution Helpers

**Files:**
- Modify: `.opencode/plugins/bash-before.ts`

- [ ] **Step 1: Replace the placeholder comments with the helper imports and helper function skeletons**

```ts
import { execFile } from "node:child_process";
import { createInterface } from "node:readline/promises";
import { stdin, stdout } from "node:process";
import { promisify } from "node:util";
import { Plugin } from "@opencode-ai/plugin";

const execFileAsync = promisify(execFile);

function normalizeUsername(value: string | undefined): string | undefined {
  const normalized = value?.trim();
  return normalized ? normalized : undefined;
}

async function runGit(args: string[], cwd: string): Promise<string | undefined> {
  try {
    const { stdout } = await execFileAsync("git", args, { cwd });
    return normalizeUsername(stdout);
  } catch {
    return undefined;
  }
}

async function isGitWorkTree(cwd: string): Promise<boolean> {
  return (await runGit(["rev-parse", "--is-inside-work-tree"], cwd)) === "true";
}

async function getGitUsername(cwd: string): Promise<string | undefined> {
  if (await isGitWorkTree(cwd)) {
    const localUsername = await runGit(["config", "user.name"], cwd);
    if (localUsername) {
      return localUsername;
    }
  }

  return runGit(["config", "--global", "user.name"], cwd);
}

async function promptForUsername(): Promise<string> {
  const rl = createInterface({ input: stdin, output: stdout });

  try {
    while (true) {
      const answer = normalizeUsername(
        await rl.question(
          "请输入用户名以设置 HOTPOT_USERNAME: ",
        ),
      );

      if (answer) {
        stdout.write(
          "提示: 可通过设置环境变量 HOTPOT_USERNAME 来持久化用户名。\n",
        );
        return answer;
      }
    }
  } finally {
    rl.close();
  }
}
```

- [ ] **Step 2: Review the helper code against the spec before integrating it**

Check that the helper layer already satisfies these requirements:

- `normalizeUsername()` trims whitespace and rejects empty values.
- `runGit()` swallows git failures and returns `undefined`.
- `isGitWorkTree()` distinguishes repository vs non-repository directories.
- `getGitUsername()` prefers local git config before global config.
- `promptForUsername()` loops until it gets a usable value and prints the persistence hint.

Expected: all five requirements are visibly covered by the code above.

### Task 2: Resolve the Username Once Per Session

**Files:**
- Modify: `.opencode/plugins/bash-before.ts`

- [ ] **Step 1: Add the session cache and final resolution helper**

```ts
async function resolveSessionUsername(cwd: string): Promise<string> {
  const environmentUsername = normalizeUsername(process.env.HOTPOT_USERNAME);
  if (environmentUsername) {
    return environmentUsername;
  }

  const gitUsername = await getGitUsername(cwd);
  if (gitUsername) {
    return gitUsername;
  }

  return promptForUsername();
}

export const bashBefore: Plugin = async (ctx) => {
  let sessionUsername: string | undefined;
```

- [ ] **Step 2: Replace the current plugin body with the session initialization flow**

```ts
export const bashBefore: Plugin = async (ctx) => {
  let sessionUsername: string | undefined;

  return {
    "shell.env": async (_input, output) => {
      output.env.ROOT_DIR = ctx.directory;

      if (sessionUsername) {
        output.env.HOTPOT_USERNAME = sessionUsername;
      }
    },
    event: async ({ event }) => {
      if (event.type !== "session.created") {
        return;
      }

      sessionUsername = await resolveSessionUsername(ctx.directory);
    },
  };
};
```

- [ ] **Step 3: Verify the integration matches the approved behavior**

Check the combined file and confirm:

- `sessionUsername` exists only inside the plugin closure.
- `session.created` is the place where resolution happens.
- `shell.env` always injects `ROOT_DIR`.
- `shell.env` injects `HOTPOT_USERNAME` only when the cache is populated.
- No code writes back to `process.env` or to git config.

Expected: all five behaviors are directly visible in the file.

### Task 3: Run Lightweight Verification

**Files:**
- Modify: `.opencode/plugins/bash-before.ts`

- [ ] **Step 1: Read the final target file and verify there are no placeholder comments left**

Check that `.opencode/plugins/bash-before.ts` no longer contains the original three TODO-style comment lines and still exports `bashBefore`.

Expected: the file contains executable logic instead of the original placeholder block.

- [ ] **Step 2: Run a syntax-level TypeScript check with Node**

Run: `node --check .opencode/plugins/bash-before.ts`

Expected: exit code `0` and no syntax errors.

- [ ] **Step 3: Review the implementation against the spec verification checklist**

Confirm all of the following from the final code:

- Environment variable `HOTPOT_USERNAME` is checked before git.
- Git lookup falls back from repository-local to global config.
- Prompt fallback happens when neither environment nor git yields a username.
- Prompted values remain session-scoped and are only reinjected via `shell.env`.
- `ROOT_DIR` behavior is unchanged.

Expected: the code satisfies every item without additional file changes.
