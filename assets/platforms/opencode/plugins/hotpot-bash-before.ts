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
  HOTPOT_LANGUAGE: string;
  HOTPOT_ISSUE_CANDIDATES_FILE: string;
  HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT: string;
  HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT: string;
  HOTPOT_TDD_PROTOCOL_PROMPT: string;
  HOTPOT_NEW_PROMPT: string;
  HOTPOT_EXECUTE_PROMPT: string;
  HOTPOT_FINISH_WORK_PROMPT: string;
  // VuePress trio. `HOTPOT_VUEPRESS_ENABLED` is always present (`"true"`
  // / `"false"`); the other two are populated only when enabled and
  // omitted from the bootstrap JSON when disabled — hence optional.
  // VuePress 三件套：ENABLED 总是有；PORT/URL 仅启用时存在（禁用时
  // bootstrap JSON 中省略），故为可选字段。
  HOTPOT_VUEPRESS_ENABLED?: string;
  HOTPOT_VUEPRESS_PORT?: string;
  HOTPOT_VUEPRESS_URL?: string;
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
      // VuePress server cleanup (defense layer 2). Idempotent — safe to
      // call when VuePress is disabled, not running, or already cleaned
      // up by /hotpot:execute pre-flight stop. Multiple event names are
      // matched defensively across OpenCode releases.
      //
      // VuePress 服务清理（防护第 2 层）：用户关闭 session 或 session
      // 主动结束时调 `hotpot vuepress stop --if-running`。stop --if-running
      // 是幂等的——VuePress 未启用、没在跑、或 runtime.json 已被 execute
      // 入口 stop 清理过，都安全返回成功。同时匹配多个可能的事件名以
      // 适配 OpenCode 版本差异；任何一个命中都触发清理。
      if (
        event.type === "session.deleted" ||
        event.type === "session.ended" ||
        event.type === "session.shutdown"
      ) {
        try {
          await execFileAsync("hotpot", [
            "vuepress",
            "stop",
            "--if-running",
          ], { cwd: ctx.directory });
        } catch (err) {
          // Cleanup failure must not block session teardown.
          // 清理失败不阻塞 session 关闭流程；保持静默。
        }
      }
    },
  };
};
