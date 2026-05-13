// @ts-nocheck
// # Review memory candidate hooks
//
// This plugin provides tools and environment variables for the temporary issue
// candidate workflow. During a repair, the agent can write reusable repair
// memories to `.hotpot/workspaces/{username}/issue-candidates.jsonl`. A later
// finish-work command can read, summarize, promote, and clear these candidates.

import { execFile } from "child_process";
import { mkdir, readFile, writeFile } from "fs/promises";
import { dirname } from "path";
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

type HotpotContext = {
  ROOT_DIR: string;
  HOTPOT_USERNAME: string;
  HOTPOT_ISSUE_CANDIDATES_FILE: string;
  HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT: string;
  HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT: string;
};

async function bootstrapHotpot(rootDir: string): Promise<HotpotContext> {
  const { stdout } = await execFileAsync("hotpot", [
    "hook",
    "bootstrap",
    "--format",
    "json",
    "--root-dir",
    rootDir,
  ]);

  return JSON.parse(stdout) as HotpotContext;
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
  let context: HotpotContext | undefined;

  const ensureContext = async (): Promise<HotpotContext> => {
    context ??= await bootstrapHotpot(ctx.directory);
    return context;
  };

  const ensureCandidatesFile = async (): Promise<string> => {
    const filePath = (await ensureContext()).HOTPOT_ISSUE_CANDIDATES_FILE;
    await ensureJsonlFile(filePath);
    return filePath;
  };

  return {
    "shell.env": async (_input, output) => {
      Object.assign(output.env, await ensureContext());
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
