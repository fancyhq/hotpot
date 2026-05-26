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

/**
 * Runtime guard state for the first tool call after a Hotpot Pi slash command.
 * Pi slash command 后首个工具调用的运行时护栏状态。
 *
 * The guard records the command that armed it, the exact workflow prompt path
 * that must be read first, and the user-input label where the model should
 * find the real request in the latest user message.
 * 该状态记录触发命令、首读必须命中的 workflow prompt 绝对路径，以及模型应在
 * 最近 user message 中查找真实需求的用户输入块 label。
 */
export type PendingFirstToolGuard = {
  command: string;
  workflowPromptPath: string;
  userInputLabel: string;
};

/**
 * Result returned by the first-tool guard evaluator.
 * 首轮工具调用护栏判定结果。
 *
 * `block` tells the Pi `tool_call` hook whether to stop execution. When a
 * workflow read is allowed, `disarm` tells the caller to clear the guard.
 * `block` 表示 Pi `tool_call` hook 是否应阻止执行；当 workflow read 被允许时，
 * `disarm` 表示调用方应解除护栏。
 */
export type FirstToolGuardDecision =
  | { block: false; disarm: true }
  | { block: true; reason: string; keepArmed: true };

/**
 * Build the corrective instruction returned when the first-tool guard blocks.
 * 构造首轮工具调用护栏拦截时返回给模型的纠正文案。
 *
 * The message is intentionally English and imperative because it is consumed
 * by the model as a tool-layer correction, not shown as user-facing prose.
 * 文案刻意使用英文祈使句，因为它是工具层返回给模型的纠偏指令，不是面向用户的自然语言。
 */
export function buildFirstToolGuardReason(guard: PendingFirstToolGuard): string {
  return [
    `Blocked by Hotpot first-tool guard: /${guard.command} was just invoked.`,
    `The user's real request is in the latest user message between <<< ${guard.userInputLabel} >>> and <<< END ${guard.userInputLabel} >>>.`,
    `Your next tool call MUST be Read("${guard.workflowPromptPath}").`,
    "Do not explore the project, invoke skills, read other files, run shell commands, ask what to do, or greet the user before reading that workflow prompt.",
  ].join(" ");
}

/**
 * Evaluate whether an armed Hotpot first-tool guard should allow this call.
 * 判定已 armed 的 Hotpot 首轮工具调用护栏是否允许当前调用。
 *
 * Pi's observed built-in `read` input uses `path`; the evaluator also accepts
 * `filePath` defensively because some agent tool surfaces use that field name.
 * It deliberately does not guess any other path fields.
 * 实测 Pi 内置 `read` 工具参数为 `path`；这里也防御性支持 `filePath`，因为部分
 * agent 工具表面使用该字段名。除这两者外不猜测其它路径字段。
 */
export function evaluateFirstToolGuard(
  guard: PendingFirstToolGuard,
  toolName: string,
  input: unknown,
): FirstToolGuardDecision {
  const maybeInput = input && typeof input === "object" ? input as Record<string, unknown> : {};
  const readPath = typeof maybeInput.path === "string"
    ? maybeInput.path
    : typeof maybeInput.filePath === "string"
      ? maybeInput.filePath
      : undefined;

  if (toolName === "read" && readPath === guard.workflowPromptPath) {
    return { block: false, disarm: true };
  }

  return {
    block: true,
    keepArmed: true,
    reason: buildFirstToolGuardReason(guard),
  };
}

/**
 * Inject Hotpot environment variables into a shell command.
 * 向 shell command 注入 Hotpot 环境变量。
 *
 * Kept as a helper so the `tool_call` hook remains a simple sequence:
 * first-tool guard → bash env injection → return.
 * 抽出 helper 让 `tool_call` hook 保持清晰顺序：首轮工具护栏 → bash 环境注入 → 返回。
 */
export function injectHotpotEnv(command: string, hotpot: HotpotContext): string {
  return `export ${Object.entries(hotpot)
    .map(([key, value]) => `${key}=${JSON.stringify(value)}`)
    .join(" ")}; ${command}`;
}

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

/**
 * Build the user-voice message text sent by a Hotpot Pi slash-command handler.
 *
 * Hotpot Pi slash command (`/hotpot-new` / `/hotpot-execute` /
 * `/hotpot-finish-work`) handler 拼装发给 AI 的「user 消息」文本。
 *
 * ## Three known failure modes this message body must survive
 *
 * 1. **Prompt-template absorption** (legacy): when this content was a
 *    `.pi/prompts/hotpot-*.md` thin shell, Pi projects loading
 *    `AGENTS.md` / `CLAUDE.md` / global skills lists absorbed it as
 *    system-level background documentation, so the AI would greet
 *    "What would you like me to do?" instead of starting the workflow.
 *    Fix: deliver as a real `role:user` message via `pi.sendUserMessage`.
 *
 * 2. **Skill auto-invocation hijack** (current): even with the message
 *    correctly arriving as `role:user`, weaker-instruction-following
 *    models (e.g. `kimi-k2.6` on `moonshotai-cn`) hallucinate generic
 *    "explore the project" intent and trigger the global
 *    `project-structure-explorer` skill before reading the workflow
 *    file. Symptom in real Pi sessions: model thinks "The user wants
 *    me to explore the project structure", runs `ls`/`tree`/`git log`,
 *    Reads the skill's `SKILL.md`, eventually hallucinates that the
 *    user said "嗯" and replies "你好！有什么我可以帮你的吗？".
 *    Fix: front-load this message with an unambiguous first-action
 *    directive ("YOUR FIRST TOOL CALL MUST BE `Read ${workflowPath}`")
 *    and an explicit "DO NOT" list naming the observed distractions
 *    (`ls`, `tree`, `git log`, `project-structure-explorer`).
 *
 * 3. **Attention loss on user-message body** (current, mitigated by
 *    message reordering + per-turn system injection): even with the
 *    front-loaded `FIRST TOOL CALL` directive and FORBIDDEN list,
 *    severely weak instruction-following models (observed
 *    `kimi-k2.6` on `moonshotai-cn`) lose attention on the
 *    mid-message `<<< INITIAL TASK IDEA >>>` block entirely — the
 *    chain-of-thought literally writes "user hasn't asked anything
 *    yet", runs `pwd && ls -la`, hallucinates an `agent-browser`
 *    extension, and ends with "What would you like to do?". Fix:
 *    (i) reorder this function so the user-input block is the FIRST
 *    paragraph (open with `A new Hotpot ${workflowName} request from
 *    me:` as a lead-in, then immediately quote the user input); the
 *    `FIRST TOOL CALL` / FORBIDDEN / `@path` / Platform sections
 *    follow. (ii) the extension closure also keeps a `pendingWorkflow`
 *    field — see the `hotpotExtension` factory — which causes
 *    `pi.on("context", ...)` to inject a per-turn system message
 *    restating the workflow path + user-input block markers + FORBIDDEN
 *    behaviors on the very next provider request.
 *
 * ## API location reminder
 *
 * `sendUserMessage` lives on `ExtensionAPI` (the factory's `pi`
 * parameter), NOT on `ExtensionCommandContext` — `ctx.sendUserMessage`
 * throws `is not a function` at runtime.
 *
 * ## 三类需要这条消息体抵御的失效模式
 *
 * 1. **prompt template 吸收**（历史问题）：旧版以 `.pi/prompts/hotpot-*.md`
 *    thin shell 实现时，在加载 `AGENTS.md` 等竞争上下文的 Pi 项目里整段
 *    被当作背景文档；AI 不进入 workflow 而是问"你想做什么"。修复：
 *    通过 `pi.sendUserMessage` 以 `role:user` 投递。
 *
 * 2. **skill 自动调用劫持**（当前问题）：即便消息已是 `role:user`，弱
 *    指令跟随模型（如 `moonshotai-cn` 的 `kimi-k2.6`）仍会幻觉"用户想
 *    探索项目结构"，在读取 workflow 之前就调起全局
 *    `project-structure-explorer` skill。修复：消息首行用绝对命令式
 *    给出"第一次工具调用必须 Read workflow 文件"，并列出已观测到的
 *    干扰行为（`ls`/`tree`/`git log`/`project-structure-explorer`）
 *    作为显式 "DO NOT" 列表。
 *
 * 3. **用户消息体 attention 丢失**（当前问题，由消息体重排 + 每轮
 *    system 注入双层 mitigation）：即便已有 front-loaded
 *    `FIRST TOOL CALL` 指令和 FORBIDDEN 列表，更弱的指令跟随模型
 *    （实测 `moonshotai-cn` 的 `kimi-k2.6`）仍会把消息中段的
 *    `<<< INITIAL TASK IDEA >>>` 块完全 attention 丢失：思维链直接写
 *    "user hasn't asked anything yet"、跑 `pwd && ls -la`、幻觉
 *    `agent-browser` 扩展、最后用 "你想做什么" 收尾。修复：
 *    （i）本函数把用户输入块前移到消息**第一段**——开篇用
 *    `A new Hotpot ${workflowName} request from me:` 作引导，紧跟
 *    用户输入；`FIRST TOOL CALL` / FORBIDDEN / `@path` / Platform 等
 *    段落随后。（ii）扩展闭包另有 `pendingWorkflow` 字段——见
 *    `hotpotExtension` 工厂函数——`pi.on("context", ...)` 在下一次
 *    provider 请求前会注入一条 system 消息，重述 workflow 路径、
 *    用户输入块分隔符、FORBIDDEN 行为列表。
 *
 * ## API 位置提醒
 *
 * `sendUserMessage` 在 `ExtensionAPI`（工厂函数的 `pi` 参数）上，
 * **不**在 `ExtensionCommandContext` 上——用 `ctx.sendUserMessage`
 * 会运行时抛 `is not a function`。
 */
function buildPiCommandMessage(opts: {
  command: "hotpot-new" | "hotpot-execute" | "hotpot-finish-work";
  args: string;
  hotpot: HotpotContext;
  ideaBlockLabel: string;
  workflowName: string;
  workflowPromptPath: string;
  atPathRefs: { shortRef: string; absolutePath: string }[];
  emptyArgsBehavior: string;
}): string {
  const refLines = opts.atPathRefs
    .map((r) => `- ${r.shortRef} → ${r.absolutePath}`)
    .join("\n");

  return [
    `A new Hotpot ${opts.workflowName} request from me:`,
    ``,
    `<<< ${opts.ideaBlockLabel} >>>`,
    opts.args,
    `<<< END ${opts.ideaBlockLabel} >>>`,
    ``,
    `If the block above is empty: ${opts.emptyArgsBehavior}`,
    ``,
    `YOUR FIRST TOOL CALL MUST BE \`Read("${opts.workflowPromptPath}")\` — DO NOTHING ELSE FIRST. That file is the Hotpot ${opts.workflowName} workflow; follow it end-to-end.`,
    ``,
    `FORBIDDEN before you Read the workflow:`,
    `- Do NOT run \`ls\`, \`tree\`, \`git log\`, \`git status\`, or any exploration command.`,
    `- Do NOT invoke any skill (especially \`project-structure-explorer\` or \`skill-creator\`); the workflow file tells you what to do.`,
    `- Do NOT ask me to clarify — my full input is in the \`${opts.ideaBlockLabel}\` block above.`,
    `- Do NOT respond with a greeting; just start the workflow.`,
    ``,
    `When the workflow file references \`@.hotpot/prompts/<name>.md\` (Pi has no \`@path\` expansion), substitute these absolute paths and \`Read\`:`,
    refLines,
    ``,
    `Platform note: Pi has no dedicated subagents. Run execution and review as strictly separated phases in this same session — announce each phase explicitly ("I am now in the EXECUTION phase" / "I am now in the READ-ONLY REVIEW phase"). The review phase must never use write/edit tools.`,
  ].join("\n");
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

  // Closure-scoped per-turn reinforcement marker. A slash-command handler
  // (`hotpot-new` / `hotpot-execute` / `hotpot-finish-work`) sets this
  // immediately before calling `pi.sendUserMessage`; the next
  // `pi.on("context", ...)` invocation (which Pi fires before every
  // provider request) consumes it exactly once and resets it to
  // `undefined`. This drives the third documented Pi failure mode's
  // mitigation — see `buildPiCommandMessage`'s third failure mode entry
  // and `docs/platforms/pi.md`. Handlers overwrite unconditionally, so a
  // stale marker from a previous handler that somehow missed
  // consumption (e.g. a streaming retry that skipped `context`) is
  // naturally replaced rather than compounded.
  //
  // 闭包级单次消费标记。三个 Hotpot slash command handler 在调
  // `pi.sendUserMessage` 前赋值；`pi.on("context", ...)`（Pi 在每次
  // provider 请求前触发）消费一次后立即置 `undefined`。该字段是第三档
  // 失败模式（用户消息体 attention 丢失）的对策骨架——详见
  // `buildPiCommandMessage` 文档第 3 条与 `docs/platforms/pi.md`。
  // handler 入口无条件覆盖，确保历史残留（极少见的未被消费场景，例如
  // streaming 中断重试跳过 `context`）天然被新值覆盖而非叠加。
  let pendingWorkflow:
    | { command: string; workflowPromptPath: string; userInputLabel: string }
    | undefined;

  // Runtime first-tool-call guard. Slash-command handlers arm this immediately
  // before `pi.sendUserMessage`; the `tool_call` hook keeps it armed until the
  // model reads the exact workflow prompt path, blocking every other first
  // tool call so weak models cannot explore, invoke skills, or ask what to do.
  //
  // 运行时首轮工具调用护栏。slash command handler 在 `pi.sendUserMessage` 前
  // arm；`tool_call` hook 会保持 armed，直到模型读取精确 workflow prompt
  // 路径，并阻止其它首个工具调用，避免弱模型探索、调技能或反问需求。
  let pendingFirstToolGuard: PendingFirstToolGuard | undefined;

  const ensureContext = async (cwd: string): Promise<HotpotContext> => {
    context ??= await bootstrapHotpot(cwd);
    return context;
  };

  pi.on("context", async (_event, ctx) => {
    const hotpot = await ensureContext(ctx.cwd);
    const messages: { role: "system"; content: string }[] = [
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
    ];

    // Per-turn workflow reinforcement: if a slash-command handler just
    // dispatched a `pi.sendUserMessage`, inject a single concise system
    // message restating the workflow file, the user-input block
    // markers, and the FORBIDDEN behaviors. Consumed exactly once, then
    // cleared — keeps subsequent turns clean.
    //
    // 每轮工作流强提醒：若 slash command handler 刚 dispatch 了
    // `pi.sendUserMessage`，注入一条简洁的 system 消息，复述 workflow
    // 路径、用户输入块分隔符、FORBIDDEN 行为列表。单次消费后立即清空，
    // 不污染后续轮次。
    if (pendingWorkflow) {
      messages.push({
        role: "system",
        content: [
          `IMPORTANT: A Hotpot slash command (/${pendingWorkflow.command}) was just invoked.`,
          `The user's actual request is in the most recent user message between \`<<< ${pendingWorkflow.userInputLabel} >>>\` and \`<<< END ${pendingWorkflow.userInputLabel} >>>\` markers — read it carefully before doing anything else.`,
          `Your FIRST tool call MUST be \`Read("${pendingWorkflow.workflowPromptPath}")\`. Do NOT run \`ls\` / \`tree\` / \`git log\` / \`git status\` / any other exploration command first. Do NOT invoke any skill (especially \`project-structure-explorer\` or \`skill-creator\`). Do NOT reply with a greeting like "What would you like me to do?" or "你好" — just start the workflow.`,
        ].join(" "),
      });
      pendingWorkflow = undefined;
    }

    return { messages };
  });

  pi.on("tool_call", async (event, ctx) => {
    if (pendingFirstToolGuard) {
      const decision = evaluateFirstToolGuard(
        pendingFirstToolGuard,
        event.toolName,
        event.input,
      );
      if (decision.block) {
        return { block: true, reason: decision.reason };
      }

      pendingFirstToolGuard = undefined;
    }

    if (event.toolName !== "bash") {
      return;
    }

    const hotpot = await ensureContext(ctx.cwd);
    const command = event.input?.command;
    if (typeof command === "string") {
      event.input.command = injectHotpotEnv(command, hotpot);
    }
  });

  pi.on("user_bash", async (_event, ctx) => {
    const hotpot = await ensureContext(ctx.cwd);
    ctx.ui.notify(`Hotpot shell context prepared for ${hotpot.ROOT_DIR}`, "info");
  });

  // VuePress server cleanup (defense layer 2). Idempotent — safe to
  // call when VuePress is disabled, not running, or already released
  // by /hotpot:execute pre-flight stop.
  //
  // VuePress 服务清理（防护第 2 层）：Pi session 关闭时调
  // `hotpot vuepress stop --if-running` 释放可能在跑的 dev server。
  // stop --if-running 是幂等的——未启用 VuePress、没在跑、或已被
  // /hotpot:execute pre-flight stop 清理过，都安全返回成功。
  pi.on("session_shutdown", async (_event, ctx) => {
    try {
      await execFileAsync(
        "hotpot",
        ["vuepress", "stop", "--if-running"],
        { cwd: ctx.cwd },
      );
    } catch (_err) {
      // Cleanup failure must not block session teardown.
      // 清理失败不阻塞 session 关闭流程；保持静默。
    }
  });

  // ── Slash-command registrations ───────────────────────────────────
  // Three Hotpot slash commands routed through Pi extension commands
  // (`pi.registerCommand` + `pi.sendUserMessage`) instead of legacy
  // prompt-template thin shells. `sendUserMessage` is on `ExtensionAPI`
  // (the `pi` parameter in lexical scope), not on the handler's `ctx`
  // (`ExtensionCommandContext`). See `buildPiCommandMessage` doc.
  //
  // 三个 Hotpot slash command 改为 extension command 注册，不再使用
  // `.pi/prompts/hotpot-*.md` 模板（在加载 AGENTS.md 的 Pi 项目里失效）。
  // `sendUserMessage` 在 `ExtensionAPI`（`pi` 参数）上，不在 handler 的
  // `ctx`（`ExtensionCommandContext`）上。
  //
  // IMPORTANT: every handler MUST assign `pendingWorkflow` immediately
  // BEFORE calling `pi.sendUserMessage(text)`. The next
  // `pi.on("context", ...)` event consumes the marker once and injects
  // a per-turn system reinforcement message — the mitigation for the
  // third documented Pi failure mode (attention loss on user-message
  // body). Adding a new Hotpot Pi slash command requires extending all
  // three pieces: the handler registration, the `buildPiCommandMessage`
  // call, AND the `pendingWorkflow` assignment.
  //
  // 重要：每个 handler 都必须在调用 `pi.sendUserMessage(text)` **之前**
  // 赋值 `pendingWorkflow`；下一次 `pi.on("context", ...)` 事件会单次
  // 消费该标记并注入 per-turn system 强提醒消息——这是第三档失败模式
  // （用户消息体 attention 丢失）的对策。新增 Pi slash command 时三处
  // 都要同步：handler 注册、`buildPiCommandMessage` 调用、`pendingWorkflow`
  // 赋值。

  pi.registerCommand("hotpot-new", {
    description: "Create a Hotpot task through brainstorming",
    handler: async (args: string, ctx) => {
      const hotpot = await ensureContext(ctx.cwd);
      const text = buildPiCommandMessage({
        command: "hotpot-new",
        args,
        hotpot,
        ideaBlockLabel: "INITIAL TASK IDEA",
        workflowName: "new-task",
        workflowPromptPath: hotpot.HOTPOT_NEW_PROMPT,
        atPathRefs: [
          {
            shortRef: "@.hotpot/prompts/output-language.md",
            absolutePath: `${hotpot.ROOT_DIR}/.hotpot/prompts/output-language.md`,
          },
          {
            shortRef: "@.hotpot/prompts/tdd-protocol.md",
            absolutePath: hotpot.HOTPOT_TDD_PROTOCOL_PROMPT,
          },
          {
            shortRef: "@.hotpot/prompts/record-issue-candidate.md",
            absolutePath: hotpot.HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT,
          },
          {
            shortRef: "@.hotpot/prompts/summarize-issue-candidates.md",
            absolutePath: hotpot.HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT,
          },
          {
            shortRef: "@.hotpot/prompts/get-issue.md",
            absolutePath: `${hotpot.ROOT_DIR}/.hotpot/prompts/get-issue.md`,
          },
          {
            shortRef: "@.hotpot/prompts/hotpot-execute.md",
            absolutePath: hotpot.HOTPOT_EXECUTE_PROMPT,
          },
          {
            shortRef: "@.hotpot/prompts/hotpot-finish-work.md",
            absolutePath: hotpot.HOTPOT_FINISH_WORK_PROMPT,
          },
        ],
        emptyArgsBehavior:
          "ask me exactly one concise question to obtain the initial task idea instead of proceeding to brainstorming.",
      });
      pendingWorkflow = {
        command: "hotpot-new",
        workflowPromptPath: hotpot.HOTPOT_NEW_PROMPT,
        userInputLabel: "INITIAL TASK IDEA",
      };
      pendingFirstToolGuard = {
        command: "hotpot-new",
        workflowPromptPath: hotpot.HOTPOT_NEW_PROMPT,
        userInputLabel: "INITIAL TASK IDEA",
      };
      pi.sendUserMessage(text);
    },
  });

  pi.registerCommand("hotpot-execute", {
    description: "Execute the active Hotpot task and run the review loop",
    handler: async (args: string, ctx) => {
      const hotpot = await ensureContext(ctx.cwd);
      const text = buildPiCommandMessage({
        command: "hotpot-execute",
        args,
        hotpot,
        ideaBlockLabel: "EXECUTION NOTES",
        workflowName: "execute",
        workflowPromptPath: hotpot.HOTPOT_EXECUTE_PROMPT,
        atPathRefs: [
          {
            shortRef: "@.hotpot/prompts/output-language.md",
            absolutePath: `${hotpot.ROOT_DIR}/.hotpot/prompts/output-language.md`,
          },
          {
            shortRef: "@.hotpot/prompts/tdd-protocol.md",
            absolutePath: hotpot.HOTPOT_TDD_PROTOCOL_PROMPT,
          },
          {
            shortRef: "@.hotpot/prompts/record-issue-candidate.md",
            absolutePath: hotpot.HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT,
          },
          {
            shortRef: "@.hotpot/prompts/summarize-issue-candidates.md",
            absolutePath: hotpot.HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT,
          },
          {
            shortRef: "@.hotpot/prompts/get-issue.md",
            absolutePath: `${hotpot.ROOT_DIR}/.hotpot/prompts/get-issue.md`,
          },
        ],
        emptyArgsBehavior:
          "proceed without additional execution notes — do not ask me for any; just begin the execute workflow.",
      });
      pendingWorkflow = {
        command: "hotpot-execute",
        workflowPromptPath: hotpot.HOTPOT_EXECUTE_PROMPT,
        userInputLabel: "EXECUTION NOTES",
      };
      pendingFirstToolGuard = {
        command: "hotpot-execute",
        workflowPromptPath: hotpot.HOTPOT_EXECUTE_PROMPT,
        userInputLabel: "EXECUTION NOTES",
      };
      pi.sendUserMessage(text);
    },
  });

  pi.registerCommand("hotpot-finish-work", {
    description: "Finish the active Hotpot task: promote candidates and mark done",
    handler: async (args: string, ctx) => {
      const hotpot = await ensureContext(ctx.cwd);
      const text = buildPiCommandMessage({
        command: "hotpot-finish-work",
        args,
        hotpot,
        ideaBlockLabel: "FINISH NOTES",
        workflowName: "finish-work",
        workflowPromptPath: hotpot.HOTPOT_FINISH_WORK_PROMPT,
        atPathRefs: [
          {
            shortRef: "@.hotpot/prompts/output-language.md",
            absolutePath: `${hotpot.ROOT_DIR}/.hotpot/prompts/output-language.md`,
          },
          {
            shortRef: "@.hotpot/prompts/summarize-issue-candidates.md",
            absolutePath: hotpot.HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT,
          },
          {
            shortRef: "@.hotpot/prompts/get-issue.md",
            absolutePath: `${hotpot.ROOT_DIR}/.hotpot/prompts/get-issue.md`,
          },
          {
            shortRef: "@.hotpot/prompts/hotpot-execute.md",
            absolutePath: hotpot.HOTPOT_EXECUTE_PROMPT,
          },
        ],
        emptyArgsBehavior:
          "proceed without additional finish notes — do not ask me for any; just begin the finish-work workflow.",
      });
      pendingWorkflow = {
        command: "hotpot-finish-work",
        workflowPromptPath: hotpot.HOTPOT_FINISH_WORK_PROMPT,
        userInputLabel: "FINISH NOTES",
      };
      pendingFirstToolGuard = {
        command: "hotpot-finish-work",
        workflowPromptPath: hotpot.HOTPOT_FINISH_WORK_PROMPT,
        userInputLabel: "FINISH NOTES",
      };
      pi.sendUserMessage(text);
    },
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
