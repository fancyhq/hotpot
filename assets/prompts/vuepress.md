## VuePress Brainstorm Closing Flow

This file is opt-in: it lives in `.hotpot/prompts/vuepress.md` **only when
the user has enabled VuePress** via `hotpot vuepress install` (or
`hotpot init --enable-vuepress`). Disabled projects do not have this file
on disk and never load it. If you are reading these lines, VuePress is
enabled for the current project.

### Step 1 — Confirm the task file follows VuePress markdown conventions

Before you wrote the task `.md` file, you should already have read
`.hotpot/prompts/vuepress-style.md` (per the file-existence gate in
`.hotpot/prompts/hotpot-new.md`) and applied its conventions. If you
skipped that step, stop here and re-read `vuepress-style.md` now — the
task file must render correctly in VuePress without breaking the
plain-markdown fallback.

### Step 2 — Ask the user whether to open in browser

Output the following verbatim (translated per the `HOTPOT_LANGUAGE`
directive in `.hotpot/prompts/output-language.md`):

> Task file created. VuePress is enabled — open in browser?
> Reply yes to start the dev server and get a URL, or proceed directly to
> `/hotpot:execute`.

Then STOP your turn and wait for the user's reply. Do not pre-emptively
start the server.

### Step 3 — On agreement, start the server and emit the URL

If the user agreed (yes, "open", "view", or any clearly affirmative reply
in the project language):

1. Run `hotpot vuepress start --port $HOTPOT_VUEPRESS_PORT` via your bash
   tool. The command spawns the dev server in the background and returns
   immediately.
2. Parse its stdout — a single line of JSON like
   `{"url": "http://localhost:8080", "pid": 12345}`.
3. The browse URL for the just-created task is:
   `<url>/<HOTPOT_USERNAME>/<task-file-stem>`
   where `<task-file-stem>` is the full task filename with only the
   `.md` extension removed. Example:
   `2025-03-02-mock-task.md` → task file stem `2025-03-02-mock-task`.
4. Output to the user:
   `Browse the task at <full-url>. When you are done, run /hotpot:execute
   to continue — the server will be stopped automatically.`

If the dev server fails to start (`hotpot vuepress start` exits non-
zero), surface the stderr directly and ask the user how to proceed; do
not silently fall back to step 4.

### Step 4 — On decline, point at the file path

If the user declined (no, "skip", or any clearly negative reply in the
project language), output:

> Task file is at `<absolute-path-to-task-file>`.
> Run `/hotpot:execute` when ready.

Do not start the dev server in this branch.

### Why server lifecycle is hands-off after this point

Once `start` returns, the dev server is detached and managed by three
defensive layers:

1. `/hotpot:execute` runs `hotpot vuepress stop --if-running` at its
   entry, so the server is released the moment the user proceeds to
   execution.
2. Each platform's SessionEnd hook (Claude / OpenCode / Pi) also runs
   the same idempotent stop, catching the case where the user closes the
   session without running `/hotpot:execute`.
3. `start` also writes an `expires_at` timestamp (default 30 min) and
   the next `status` / `start` call lazily kills any expired process —
   this is the only safety net on Codex, which has no SessionEnd event.

You as the AI do **not** need to stop the server manually anywhere in
this flow.
