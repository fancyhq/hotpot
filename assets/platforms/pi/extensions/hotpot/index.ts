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
  HOTPOT_LANGUAGE: string;
  HOTPOT_ISSUE_CANDIDATES_FILE: string;
  HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT: string;
  HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT: string;
  HOTPOT_TDD_PROTOCOL_PROMPT: string;
  HOTPOT_NEW_PROMPT: string;
  HOTPOT_EXECUTE_PROMPT: string;
  HOTPOT_FINISH_WORK_PROMPT: string;
  // VuePress trio. `HOTPOT_VUEPRESS_ENABLED` is always present
  // (`"true"` / `"false"`); the other two are populated only when
  // enabled and omitted from the bootstrap JSON when disabled.
  // VuePress 三件套：ENABLED 总是有；PORT/URL 仅启用时存在。
  HOTPOT_VUEPRESS_ENABLED?: string;
  HOTPOT_VUEPRESS_PORT?: string;
  HOTPOT_VUEPRESS_URL?: string;
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
        // Per-turn output-language reassertion. Pi's `context` event fires
        // before every provider request, so this is the natural place to
        // restate the language directive — equivalent to Claude/Codex
        // `UserPromptSubmit` hooks. Keep it short to avoid context bloat;
        // the full anchor whitelist lives in
        // `$ROOT_DIR/.hotpot/prompts/output-language.md`.
        //
        // 每轮重申输出语言。Pi 的 `context` 事件在每次 provider 请求前
        // 触发，是天然的"每轮注入"切点——等价于 Claude/Codex 的
        // `UserPromptSubmit` 钩子。保持简短，完整锚点清单在
        // `$ROOT_DIR/.hotpot/prompts/output-language.md`。
        {
          role: "system",
          content: [
            `Hotpot output language for this turn: \`${hotpot.HOTPOT_LANGUAGE}\`.`,
            "Reply in that language for all user-facing prose.",
            "Structural anchors stay English: `## Task`, `## Plan`, `### Mode`, `tdd: true|false`, `ACTIVE_CONFLICT:`, kebab-case slugs.",
          ].join(" "),
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

  // VuePress 服务清理（防护第 2 层）：Pi session 关闭时调
  // `hotpot vuepress stop --if-running` 释放可能在跑的 dev server。
  // stop --if-running 是幂等的——未启用 VuePress、没在跑、或已被
  // /hotpot:execute pre-flight stop 清理过，都安全返回成功。
  //
  // VuePress server cleanup (defense layer 2). Idempotent — safe to
  // call when VuePress is disabled, not running, or already released
  // by /hotpot:execute pre-flight stop.
  pi.on("session_shutdown", async (_event, ctx) => {
    try {
      await execFileAsync(
        "hotpot",
        ["vuepress", "stop", "--if-running"],
        { cwd: ctx.cwd },
      );
    } catch (_err) {
      // 清理失败不阻塞 session 关闭流程；保持静默。
      // Cleanup failure must not block session teardown.
    }
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
