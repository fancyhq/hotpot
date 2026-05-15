// # Required setup before binary execution
//
// This plugin is intentionally thin: OpenCode supplies ctx.directory, and the
// Rust CLI owns all Hotpot context derivation.

import { execFile } from "child_process";
import { promisify } from "util";
import { Plugin } from "@opencode-ai/plugin";

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

export const bashBefore: Plugin = async (ctx) => {
  let context: HotpotContext | undefined;

  const ensureContext = async (): Promise<HotpotContext> => {
    context ??= await bootstrapHotpot(ctx.directory);
    return context;
  };

  return {
    "shell.env": async (_input, output) => {
      Object.assign(output.env, await ensureContext());
    },
    event: async ({ event }) => {
      if (event.type === "session.created") {
        await ensureContext();
      }
    },
  };
};
