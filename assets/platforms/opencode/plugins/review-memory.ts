// @ts-nocheck
// # Review memory candidate hooks
//
// This plugin provides tools and environment variables for the temporary issue
// candidate workflow. During a repair, the agent can write reusable repair
// memories to `.hotpot/workspaces/{username}/issue-candidates.jsonl`. A later
// finish-work command can read, summarize, promote, and clear these candidates.

import { execFile } from "child_process";
import { mkdir, readFile, writeFile } from "fs/promises";
import { dirname, join } from "path";
import { promisify } from "util";
import { Plugin, tool } from "@opencode-ai/plugin";

const execFileAsync = promisify(execFile);

type IssueCandidate = {
  created_at: string;
  reason: string;
  changed_files: string[];
  keywords: string[];
  problem: string;
  fix: string;
  validation: string[];
  promote_hint: string;
};

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

async function resolveSessionUsername(cwd: string): Promise<string> {
  const environmentUsername = normalizeUsername(process.env.HOTPOT_USERNAME);
  if (environmentUsername) {
    return environmentUsername;
  }

  const gitUsername = await getGitUsername(cwd);
  if (gitUsername) {
    return gitUsername;
  }

  return "default";
}

function issueCandidatesFilePath(rootDir: string, username: string): string {
  return join(rootDir, ".hotpot", "workspaces", username, "issue-candidates.jsonl");
}

function promptPath(rootDir: string, name: string): string {
  return join(rootDir, "prompts", name);
}

async function ensureJsonlFile(filePath: string): Promise<void> {
  await mkdir(dirname(filePath), { recursive: true });

  try {
    await readFile(filePath, "utf8");
  } catch {
    await writeFile(filePath, "", "utf8");
  }
}

async function appendJsonLine(filePath: string, value: unknown): Promise<void> {
  await ensureJsonlFile(filePath);
  const existing = await readFile(filePath, "utf8");
  const line = JSON.stringify(value);
  const prefix = existing.length > 0 && !existing.endsWith("\n") ? "\n" : "";
  await writeFile(filePath, `${existing}${prefix}${line}\n`, "utf8");
}

async function readJsonLines<T>(filePath: string): Promise<T[]> {
  await ensureJsonlFile(filePath);
  const content = await readFile(filePath, "utf8");

  return content
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => JSON.parse(line) as T);
}

export const reviewMemory: Plugin = async (ctx) => {
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

  const ensureCandidatesFile = async (): Promise<string> => {
    const username = await ensureSessionUsername();
    const filePath = issueCandidatesFilePath(ctx.directory, username);
    await ensureJsonlFile(filePath);
    return filePath;
  };

  return {
    "shell.env": async (_input, output) => {
      const username = await ensureSessionUsername();
      output.env.HOTPOT_ISSUE_CANDIDATES_FILE = issueCandidatesFilePath(
        ctx.directory,
        username,
      );
      output.env.HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT = promptPath(
        ctx.directory,
        "record-issue-candidate.md",
      );
      output.env.HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT = promptPath(
        ctx.directory,
        "summarize-issue-candidates.md",
      );
    },
    event: async ({ event }) => {
      if (event.type === "session.created") {
        await ensureCandidatesFile();
      }
    },
    tool: {
      record_issue_candidate: tool({
        description:
          "Append one temporary review-memory candidate after a validated repair. Use only when the repair reveals a reusable future review check. Do not use for ordinary implementation steps, one-off requirements, failed attempts, or unverified fixes.",
        args: {
          created_at: tool.schema.string(),
          reason: tool.schema.string(),
          changed_files: tool.schema.array(tool.schema.string()),
          keywords: tool.schema.array(tool.schema.string()),
          problem: tool.schema.string(),
          fix: tool.schema.string(),
          validation: tool.schema.array(tool.schema.string()),
          promote_hint: tool.schema.string(),
        },
        async execute(args) {
          const filePath = await ensureCandidatesFile();
          const candidate: IssueCandidate = {
            created_at: args.created_at,
            reason: args.reason,
            changed_files: args.changed_files,
            keywords: args.keywords,
            problem: args.problem,
            fix: args.fix,
            validation: args.validation,
            promote_hint: args.promote_hint,
          };

          await appendJsonLine(filePath, candidate);

          return `Recorded issue candidate in ${filePath}`;
        },
      }),
      read_issue_candidates: tool({
        description:
          "Read temporary review-memory candidates for finish-work summarization, merging, promotion, or discard decisions.",
        args: {},
        async execute() {
          const filePath = await ensureCandidatesFile();
          const candidates = await readJsonLines<IssueCandidate>(filePath);

          return JSON.stringify({ filePath, candidates });
        },
      }),
      clear_issue_candidates: tool({
        description:
          "Clear temporary review-memory candidates after finish-work has promoted or deliberately discarded them.",
        args: {},
        async execute() {
          const filePath = await ensureCandidatesFile();
          await writeFile(filePath, "", "utf8");

          return `Cleared issue candidates in ${filePath}`;
        },
      }),
    },
  };
};
