/** Hotpot Pi extension for hook-equivalent context preparation. */
import { execFile } from "child_process";
import { mkdir, readFile, writeFile } from "fs/promises";
import { dirname } from "path";
import { promisify } from "util";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";

const execFileAsync = promisify(execFile);

type HotpotContext = {
  ROOT_DIR: string;
  HOTPOT_USERNAME: string;
  HOTPOT_ISSUE_CANDIDATES_FILE: string;
  HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT: string;
  HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT: string;
  HOTPOT_TDD_PROTOCOL_PROMPT: string;
  HOTPOT_NEW_PROMPT: string;
  HOTPOT_EXECUTE_PROMPT: string;
  HOTPOT_FINISH_WORK_PROMPT: string;
};

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

/** Bootstrap Hotpot context through the Rust CLI. */
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

/** Create the issue candidate JSONL file if it does not already exist. */
async function ensureJsonlFile(filePath: string): Promise<void> {
  await mkdir(dirname(filePath), { recursive: true });

  try {
    await readFile(filePath, "utf8");
  } catch {
    await writeFile(filePath, "", "utf8");
  }
}

/** Append one JSON value as a JSONL line. */
async function appendJsonLine(filePath: string, value: unknown): Promise<void> {
  await ensureJsonlFile(filePath);
  const existing = await readFile(filePath, "utf8");
  const line = JSON.stringify(value);
  const prefix = existing.length > 0 && !existing.endsWith("\n") ? "\n" : "";
  await writeFile(filePath, `${existing}${prefix}${line}\n`, "utf8");
}

/** Read a JSONL file into typed values. */
async function readJsonLines<T>(filePath: string): Promise<T[]> {
  await ensureJsonlFile(filePath);
  const content = await readFile(filePath, "utf8");

  return content
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => JSON.parse(line) as T);
}

/** Register Hotpot context hooks and review-memory tools for Pi. */
export default function hotpotExtension(pi: ExtensionAPI) {
  let context: HotpotContext | undefined;

  const ensureContext = async (cwd: string): Promise<HotpotContext> => {
    context ??= await bootstrapHotpot(cwd);
    return context;
  };

  pi.on("context", async (_event, ctx) => {
    const hotpot = await ensureContext(ctx.cwd);
    return {
      messages: [
        {
          role: "system",
          content: [
            "Hotpot context was resolved from the Pi extension cwd.",
            "Use these values for Hotpot-related Bash commands:",
            ...Object.entries(hotpot).map(([key, value]) => `- ${key}: ${value}`),
          ].join("\n"),
        },
      ],
    };
  });

  pi.on("tool_call", async (event, ctx) => {
    if (event.toolName !== "bash") {
      return;
    }

    const hotpot = await ensureContext(ctx.cwd);
    const command = event.input?.command;
    if (typeof command === "string") {
      event.input.command = `export ${Object.entries(hotpot)
        .map(([key, value]) => `${key}=${JSON.stringify(value)}`)
        .join(" ")}; ${command}`;
    }
  });

  pi.on("user_bash", async (_event, ctx) => {
    const hotpot = await ensureContext(ctx.cwd);
    ctx.ui.notify(`Hotpot shell context prepared for ${hotpot.ROOT_DIR}`, "info");
  });

  pi.registerTool({
    name: "record_issue_candidate",
    label: "Record Issue Candidate",
    description:
      "Append one temporary review-memory candidate after a validated repair.",
    parameters: Type.Object({
      created_at: Type.String(),
      reason: Type.String(),
      changed_files: Type.Array(Type.String()),
      keywords: Type.Array(Type.String()),
      problem: Type.String(),
      fix: Type.String(),
      validation: Type.Array(Type.String()),
      promote_hint: Type.String(),
    }),
    async execute(_toolCallId, params, _signal, _onUpdate, ctx) {
      const hotpot = await ensureContext(ctx.cwd);
      await appendJsonLine(hotpot.HOTPOT_ISSUE_CANDIDATES_FILE, params as IssueCandidate);

      return {
        content: [
          {
            type: "text",
            text: `Recorded issue candidate in ${hotpot.HOTPOT_ISSUE_CANDIDATES_FILE}`,
          },
        ],
        details: {},
      };
    },
  });

  pi.registerTool({
    name: "read_issue_candidates",
    label: "Read Issue Candidates",
    description:
      "Read temporary review-memory candidates for summarization, promotion, or discard decisions.",
    parameters: Type.Object({}),
    async execute(_toolCallId, _params, _signal, _onUpdate, ctx) {
      const hotpot = await ensureContext(ctx.cwd);
      const candidates = await readJsonLines<IssueCandidate>(hotpot.HOTPOT_ISSUE_CANDIDATES_FILE);

      return {
        content: [
          {
            type: "text",
            text: JSON.stringify({ filePath: hotpot.HOTPOT_ISSUE_CANDIDATES_FILE, candidates }),
          },
        ],
        details: {},
      };
    },
  });

  pi.registerTool({
    name: "clear_issue_candidates",
    label: "Clear Issue Candidates",
    description:
      "Clear temporary review-memory candidates after promotion or deliberate discard.",
    parameters: Type.Object({}),
    async execute(_toolCallId, _params, _signal, _onUpdate, ctx) {
      const hotpot = await ensureContext(ctx.cwd);
      await ensureJsonlFile(hotpot.HOTPOT_ISSUE_CANDIDATES_FILE);
      await writeFile(hotpot.HOTPOT_ISSUE_CANDIDATES_FILE, "", "utf8");

      return {
        content: [
          {
            type: "text",
            text: `Cleared issue candidates in ${hotpot.HOTPOT_ISSUE_CANDIDATES_FILE}`,
          },
        ],
        details: {},
      };
    },
  });
}
