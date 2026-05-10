// @ts-nocheck
// # Required setup before binary execution
// hotpot uses configured environment variables to obtain the execution directory and username
//
// ## Set the agent execution directory
// 1. Before running a bash command, set the working directory in the `ROOT_DIR` environment variable so binaries can be located more easily
//
// ## Set the username
// 1. If the `HOTPOT_USERNAME` environment variable is already set, use it directly
// 2. If the current directory is inside a `git` project, use the project's `git` username; if not found, fall back to the global `git` username
// 3. If `git` is unavailable, the user must provide a username or set the `HOTPOT_USERNAME` environment variable manually

import { execFile } from "child_process";
import { stdin, stdout } from "process";
import { createInterface } from "readline/promises";
import { promisify } from "util";
import { Plugin } from "@opencode-ai/plugin";

const execFileAsync = promisify(execFile);

function normalizeUsername(value: string | undefined): string | undefined {
  const normalized = value?.trim();
  return normalized ? normalized : undefined;
}

async function runGit(
  args: string[],
  cwd: string,
): Promise<string | undefined> {
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
    const localUsername = await runGit(["config", "--local", "user.name"], cwd);
    if (localUsername) {
      return localUsername;
    }
  }

  return runGit(["config", "--global", "user.name"], cwd);
}

async function promptForUsername(): Promise<string> {
  if (!stdin.isTTY || !stdout.isTTY) {
    throw new Error(
      "Cannot prompt for a username in non-interactive mode. Set HOTPOT_USERNAME to continue.",
    );
  }

  const rl = createInterface({ input: stdin, output: stdout });

  try {
    while (true) {
      const answer = normalizeUsername(
        await rl.question(
          "Enter a username to set HOTPOT_USERNAME (persist it with an environment variable): ",
        ),
      );

      if (answer) {
        stdout.write(
          "Tip: Set the HOTPOT_USERNAME environment variable to persist your username.\n",
        );
        return answer;
      }
    }
  } finally {
    rl.close();
  }
}

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
  let pendingSessionUsername: Promise<string> | undefined;

  const ensureSessionUsername = async (): Promise<string> => {
    if (sessionUsername) {
      return sessionUsername;
    }

    if (!pendingSessionUsername) {
      pendingSessionUsername = resolveSessionUsername(ctx.directory).then(
        (username) => {
          sessionUsername = username;
          return username;
        },
      );
    }

    return pendingSessionUsername;
  };

  return {
    "shell.env": async (_input, output) => {
      output.env.ROOT_DIR = ctx.directory;

      output.env.HOTPOT_USERNAME = await ensureSessionUsername();
    },
    event: async ({ event }) => {
      if (event.type === "session.created") {
        await ensureSessionUsername();
      }
    },
  };
};
