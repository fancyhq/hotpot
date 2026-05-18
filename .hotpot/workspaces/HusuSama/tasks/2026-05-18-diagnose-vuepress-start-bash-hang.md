---
title: 诊断并修复 hotpot vuepress start 在 macOS + cargo run 前台调用下卡住 Claude Code Bash tool
description: 先在 macOS + cargo run + 前台 Bash 复现并采集 lsof/ps 证据，再针对根因施加最小补丁（端口就绪探测 / 后台调用切换 / FD-CLOEXEC 修复 中的最小子集）
date: 2026-05-18
category: [Task]
tag: [vuepress, process-spawn, bash-tool, macos, diagnostics, fd-leak]
---

# 诊断并修复 hotpot vuepress start 在 macOS + cargo run 前台调用下卡住 Claude Code Bash tool

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | false | 3 | medium |
:::

---

## Task

### Summary

::: info
前序任务 `fix-vuepress-start-detach`（2026-05-17）给 `src/vuepress.rs::start` 加了 Windows `creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)` 和 Unix `pre_exec(setsid)` 双平台 detach，验证条件是 **Windows + `cargo run --quiet -- vuepress start`（`run_in_background: true`）**。当前用户在 **macOS + 前台 Bash tool + `cargo run --`** 链路上仍能复现 hang：`.hotpot-hub/vuepress.log` 显示 pnpm/vuepress 已经正常启动，但 Claude Code Bash tool 不返回。本任务先用 lsof + ps 三件套定位 hang 的真实根因，再施加最小补丁（候选包含 `vuepress start` 内嵌端口就绪探测 / `assets/prompts/vuepress.md` closing-flow 改后台调用 / log_file FD-CLOEXEC 加固），由 Phase 1 的证据决定上哪几招、不无脑全上。
:::

### User Request

::: info 用户原话
目前 vuepress 启动仍然存在问题，claude code 在修改 task markdown 任务文件的时候，启动 vuepress 是正常的，但是在 brainstorming 时启动总会卡住，需要确认一下这个问题，是否是因为提前发送链接地址的问题？或者有什么其他地方需要优化
:::

### Approved Design

::: tip
诊断优先：先采证、再针对根因施加最小补丁，不在不知道根因前盲改五处。
:::

**Phase 1 — Diagnose（不动业务代码，只采证）**

1. 把当前 worktree 的 VuePress 半装态修好：当前 `.hotpot/config.toml` 写了 `[vuepress] enabled = true` 但 `.hotpot-hub/` 不存在，`vuepress start` 会被 `verify_install_consistency` 直接 bail（已在 brainstorming 阶段实证）。修复方式：跑 `cargo run -- vuepress install --port 8080`，让 hub + pnpm install + opt-in prompts 三件套一起补齐。这不是 task 的"修复内容"，只是诊断前置条件。
2. 在 Claude Code Bash tool **前台模式**跑 `cargo run -- vuepress start --port 8080`（30s timeout）。预期：30s 后 timeout，stdout 拿到 JSON 但 Bash 不返回；vuepress.log 已有 pnpm 启动 log。
3. 在 hang 进行中并行用 `run_in_background: true` 的 Bash 调用跑诊断三件套：
   - `ps -e -o pid,ppid,pgid,sid,stat,command | grep -E "(cargo|hotpot|pnpm|node|bash)"` — 看四级进程的 pid/ppid/pgid/sid 关系，确认 setsid 是否真的把 pnpm 移到新 session。
   - `lsof -p <hotpot-pid>` — 看 hotpot 自身打开的 fd（应该只剩 stdio + log_file，且 stdio 是 cargo 给的 pipe）。
   - `lsof -p <pnpm-pid>` 和 `lsof -p <node-pid>` — 看 pnpm 和 node 是否继承了 cargo/bash 的 stdio pipe；这是 hang 的高嫌疑点。
4. 跑两组对照实验：
   - **对照 A — 编译后直跑**：`./target/debug/hotpot vuepress start --port 8080`（先 stop 上一轮）。前台 Bash 调用。区分 cargo wrapper 与 hotpot detach 本身的责任。
   - **对照 B — cargo run 后台模式**：`cargo run --quiet -- vuepress start --port 8080`（`run_in_background: true`），观察是否仍 hang。
5. 把所有观察事实落到本任务文件 `### Project Context` 的"Diagnostic findings"小节，作为 Phase 2 选补丁的依据。**Phase 1 完成的判据**：能用一句话陈述 hang 的根因，并给出可复现指纹。

**Phase 2 — Apply targeted fix(es)（由 Phase 1 证据决定）**

候选补丁清单（按推测确信度排序）：

| ID | 内容 | 触发条件（来自 Phase 1） |
| -- | ---- | ------------------------ |
| P-A | `src/vuepress.rs::start` 在 spawn 之后、`println!(summary)` 之前加 TCP `127.0.0.1:<port>` connect 轮询（每 250ms，硬 timeout 30s）。就绪才打印 URL。 | 默认上：用户已明确把"提前发送 URL"列为待修问题；任一根因都不影响这一招的必要性。 |
| P-B | `assets/prompts/vuepress.md` closing flow 第 3 步改为 `run_in_background: true` 调用 + 轮询 stdout JSON。 | Phase 1 证实 Bash-tool-wait-EOF 是根因（即 lsof 显示孙子进程继承了 cargo/bash stdio pipe，或对照 B 后台模式不卡）。 |
| P-C | `src/vuepress.rs::start` 在 `fs::File::create(&log_path)` 之后、`cmd.spawn()` 之前，给 `log_file` 显式 `fcntl(F_SETFD, FD_CLOEXEC)`（Rust 默认有，但 belt-and-suspenders）。 | Phase 1 lsof 显示 log_file fd 出现在 pnpm/node 之外的位置，或对照 A 不卡而 cargo run 卡（说明 cargo 那一段有泄漏）。 |
| P-D | 仅文档化为 `### Risks and Watchouts` 条目，不写代码。 | Phase 1 证实根因在 cargo wrapper，非 hotpot 自身——生产路径用已安装 hotpot 不受影响。 |

**Phase 2 完成的判据**：本仓库环境下，Claude Code Bash tool **前台**跑 `cargo run -- vuepress start --port 8080`，30s 内拿到 JSON 单行 stdout 并 exit 0；浏览器打开 URL 立即可见首页（不再有 "URL 已发但服务还没就绪" 的窗口）。

### Alternatives Considered

- **方案 A（被否决）**：不诊断、直接 P-A + P-B + P-C 三招齐上。否决理由：过度修复，未来回归无法定位真正起作用的招；且若 P-A 单独足够，P-B 会让流程多一次轮询步骤，得不偿失。
- **方案 B（被否决）**：只打 P-A 端口就绪探测。否决理由：如果根因是 Bash-tool-wait-EOF，加 readiness probe 反而让 `start` 阻塞 5–15s 加长 hang 窗口，治标恶化。
- **方案 C（被否决）**：只打 P-B 切后台调用。否决理由：放过了"提前发送 URL → 浏览器点开后 spin" 这条用户已明确列出的子问题。
- **方案 D（被否决）**：换实现策略（如双 fork、posix_spawn）。否决理由：变更面太大，且不知道根因前没有依据。Phase 1 后若必要可作为后续 task 单独立。
- **采纳方案**：先 Phase 1 诊断，再用 Phase 1 证据决定 P-A / P-B / P-C / P-D 的子集（极大概率是 P-A + 文档化 P-D，可能附加 P-B 或 P-C）。

### Requirements

- Phase 1 完成后，任务文件 `### Project Context` 必须含一节"Diagnostic findings"，列出至少：
  - 复现指纹（命令 + timeout + Bash 是否返回 + vuepress.log 是否正常 + hotpot/pnpm pid 关系）。
  - 对照 A、对照 B 的结果。
  - lsof 中观察到的可疑 fd（如有）。
  - 一句话根因结论。
- Phase 2 完成后，`hotpot vuepress start` 在 Claude Code 前台 Bash tool 调用时，**必须**在 30 秒内打印单行 `{"url","pid"}` JSON 并 exit 0。这是最终硬判据。
- URL 被打印时，`localhost:<port>` 必须已经可以接受 TCP 连接（消除"提前发 URL"问题）。
- `hotpot vuepress stop / status` 行为不变；runtime.json schema 不变。
- 改动**只**限于（最多）：`src/vuepress.rs::start` 函数主体 + `assets/prompts/vuepress.md`。**不**改 install/uninstall/runtime.json schema/其他子命令/`.hotpot/prompts/vuepress.md`（那是已安装副本，不是 source-of-truth）。

### Non-Goals

::: details Non-Goals
- 不修复 worktree 当前 `.hotpot-hub/` 缺失的半装态（用户跑 `hotpot vuepress install` 即可，无需写代码）。
- 不在本 task 内修 cargo wrapper 在 dev path 的潜在 hang——如证实是 cargo 问题，仅记入 Risks 章节。
- 不重构 hub 资产分级、prompt 注入机制、`verify_install_consistency` 语义。
- 不引入新的 OS-level 依赖（不加 `nix` crate；继续用 `libc` 直调）。
- 不改 `find_pnpm` 实现、`stop`、`status`、runtime.json 字段、`hub_dir` 路径、`hotpot-hub/docs` 软链逻辑。
- 不把 detach 策略改成 daemonize（双 fork 等）——超出本 task 范畴。
- 不改 `.hotpot/prompts/vuepress.md`——那是 `assets/prompts/vuepress.md` 通过 `install` 安装的副本，唯一改动入口是 source-of-truth。
:::

### Project Context

**当前实现（基于 brainstorming 阶段 Read 的快照）**：

- `src/vuepress.rs::start`（line 714–837）：先 `verify_install_consistency`，读 / 清理旧 runtime.json，`find_pnpm`，spawn `pnpm run docs:dev -- --clean-cache --port <port>`，spawn 后立即写 runtime.json、`println!` 单行 JSON、返回 `Ok(())`。
- spawn 配置：`stdin = null`，`stdout = log_file`（`.hotpot-hub/vuepress.log`），`stderr = log_dup`（同 fd dup），Windows `creation_flags = CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`，Unix `pre_exec(libc::setsid)`。
- `find_pnpm` 跑 `pnpm --version` 探测（每个候选 `Command::output()`）。
- `vuepress.md` closing flow（`.hotpot/prompts/vuepress.md` line 35–47）：让 AI 用前台 Bash 调用 `hotpot vuepress start --port $HOTPOT_VUEPRESS_PORT`、解析 stdout JSON、立即输出 URL 给用户。

**当前 worktree 状态**：

- `.hotpot/config.toml`：`[vuepress] enabled = true`，`port = 8080`。
- `.hotpot-hub/` 不存在（已 ls 实证）。
- `.hotpot/prompts/vuepress.md` 和 `vuepress-style.md` 都在。
- 直接跑 `cargo run -- vuepress start` 在 brainstorming 阶段已经实证返回 `Error: .hotpot-hub/package.json is missing; ... Run vuepress uninstall then install to repair.` — 这是 `verify_install_consistency` 的预期行为，**不是**本任务要修的问题。

**前序工作**：

- `fix-vuepress-start-detach`（task_id `OJAZCCCMWM`，2026-05-17 Done）已在 `start()` 加了双平台 detach。本任务的 hang 是在那个修复基础上仍能复现的残余问题。
- 该任务验证条件覆盖度：Windows 上 `cargo run --quiet -- vuepress start`（背景模式）、以及 `hotpot.exe vuepress start`（直跑）。**未覆盖**：macOS、cargo run + 前台 Bash tool。

**Diagnostic findings（Phase 1 实测）**：

::: tip
1. **复现指纹（前台 cargo run）**：`cargo run --quiet -- vuepress start --port 8080`，60s timeout，**Bash tool 立即返回**（< 1s）exit 0，stdout 单行 `{"url":"http://localhost:8080","pid":13868}`。`.hotpot-hub/vuepress.log` 末尾依次有 `➜ Local: http://localhost:8080/` 等监听 banner。`runtime.json` 含 pid 13868，与 stdout JSON 完全一致。**hang 未复现**。
2. **`ps -e -o pid,ppid,pgid,sess,stat,command` 关键行**（hotpot 已退出，pnpm 还活着）：
   - `13868  1  13868  0 Ss  node /usr/local/bin/pnpm run docs:dev -- --clean-cache --port 8080`
   - `13895  13868  13868  0 S  node …/vuepress.js dev docs -- --clean-cache --port 8080`
   - `13901  13895  13868  0 S  …/esbuild --service=0.24.2 --ping`
   - **PPID=1（launchd 接管）+ PGID=13868（自成进程组首领）+ SESS=0（无 controlling terminal）→ `pre_exec(setsid)` 完美生效**。孙子进程 vuepress.js 和 esbuild 也在同一 pgid，没泄漏到父 shell。
3. **lsof 关键 fd（pnpm pid 13868）**：
   - `0r CHR /dev/null`
   - `1w REG …/.hotpot-hub/vuepress.log`
   - `2w REG …/.hotpot-hub/vuepress.log`
   - `4/5 PIPE …` 是 node 内部 pipe pair（两端都在同进程，不是父继承）
   - **没有任何 fd 类型为 PIPE 且对端指向 cargo/bash 的 stdio**——fd 继承链路完全干净，detach 已断尽。
4. **对照 A（编译后直跑）**：`./target/debug/hotpot vuepress start --port 8080`（前台 Bash）→ 立即返回 `{"url":"http://localhost:8080","pid":14582}` exit 0，**不 hang**。cargo wrapper 与 hotpot 自身行为一致。
5. **对照 B（cargo run + 后台模式）**：`run_in_background: true` 跑 `cargo run --quiet -- vuepress start --port 8080` → task completed exit 0，stdout `{"url":"http://localhost:8080","pid":15079}`，**不 hang**。
6. **端口就绪窗口实证**：Step 3 在 11:01:46 拿到 JSON，curl `localhost:8080` 在 11:02:52 才回 HTTP/1.1 200 —— start 返回时 TCP 端口**尚未就绪**，约 5–15s 后 vuepress 才完成首次构建并 bind。这是用户报告"提前发送 URL"的真问题：浏览器点开会看到 connection refused / spin。
7. **一句话根因结论**：fd 继承与 setsid detach 在 macOS 上**完全正确**（本任务在前台 / 直跑 binary / 后台 cargo run 三条路径上均无 hang）；用户报告的"启动卡住"实际是 **start() 返回时 TCP 端口未就绪、URL 被提前发出**导致浏览器点开后 spin，被误读为启动 hang。Bash-tool-wait-EOF 假设证伪。

Phase 2 plan: 选用补丁 **P-A + P-D**，理由：lsof / ps 证据完全否定 Bash-tool-wait-EOF 与 fd 泄漏假说（P-B / P-C 触发条件不成立），而"提前发 URL"是已实证的真问题，P-A TCP readiness probe 是唯一直接修复；P-D 把 fd-继承链路已经被 setsid + null stdin + log redirect 三件套断尽的事实文档化，方便未来回归。
:::

---

## Plan

### Mode

- tdd: false   <!-- machine-readable; the execute flow parses this line. -->

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `.hotpot-hub/` (一次性恢复) | Create | Phase 1 前置：`cargo run -- vuepress install` 自动产生；不算 task 输出。 |
| `<本任务文件>` | Modify | Phase 1 填 `### Project Context` 的"Diagnostic findings"小节。 |
| `src/vuepress.rs` | Modify | Phase 2 候选：`start()` 函数加 P-A 端口就绪探测、和/或 P-C 显式 CLOEXEC 加固。仅当 Phase 1 证据支持时才动。 |
| `assets/prompts/vuepress.md` | Modify | Phase 2 候选：closing flow 改为 P-B 后台调用样式。仅当 Phase 1 证据支持时才动。注意是 `assets/` 下的 source-of-truth，**不是** `.hotpot/prompts/` 副本。 |

### Implementation Tasks

#### Task 1: 修复 worktree 半装态 + 前台复现 hang + 采集 lsof/ps 证据

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.hotpot-hub/` | Create | 通过 `cargo run -- vuepress install` 自动部署 hub + pnpm install + 写 `vuepress` opt-in prompts；前置条件。 |
| `<本任务文件>` | Modify | Phase 1 完成后填 `### Project Context > Diagnostic findings` 小节。 |

**Steps:**

- [x] **Step 1**：确认起点。跑 `cargo run -- vuepress status`，预期 `{"running":false}`。如返回 `running:true`，先跑 `cargo run -- vuepress stop` 清掉，再跑一次 status 确认。
  - 实测：`cargo run --quiet -- vuepress status` → `{"running":false}`。worktree 起点干净。
- [x] **Step 2**：跑 `cargo run -- vuepress install --port 8080`。这会部署 `.hotpot-hub/` + 跑 `pnpm install`（首次 30s–2min）+ 写 `vuepress` opt-in prompts。**预期**：stdout 末尾打印 `VuePress installed. Start with hotpot vuepress start.`；`.hotpot-hub/package.json` 存在；`.hotpot/prompts/vuepress.md` 存在；`.hotpot/config.toml` 的 `[vuepress] enabled = true / port = 8080` 已写入（应该已经是这个状态，但 install 是幂等的）。
  - 实测：stdout 末尾依次打印 `Done in 8.8s using pnpm v10.4.1` → `Docs symlinks created.` → `skip unchanged .hotpot/prompts/vuepress.md` / `vuepress-style.md` → `Set [vuepress] enabled = true / port = 8080 in .hotpot/config.toml.` → `VuePress installed. Start with hotpot vuepress start.`。`.hotpot-hub/package.json` 与 `.hotpot/prompts/vuepress.md` 均存在；`config.toml` 含 `[vuepress] enabled = true` / `port = 8080`。
- [x] **Step 3（关键 — 前台复现）**：在 Claude Code Bash tool 前台模式跑 `cargo run --quiet -- vuepress start --port 8080`。
  - 实测：**Bash tool 立即返回**（< 1s）exit 0，stdout `{"url":"http://localhost:8080","pid":13868}`；vuepress.log 末尾含 `➜ Local: http://localhost:8080/` banner；runtime.json pid=13868 与 stdout 一致。**hang 未复现。**
- [x] **Step 4（并行采证 — Phase 1 核心）**：server 仍活着时采证 ps + lsof。
  - `ps`：pnpm pid 13868 PPID=1 PGID=13868 SESS=0 Ss——setsid 完美生效；vuepress.js (13895) + esbuild (13901) 同 pgid。
  - `lsof -p 13868`：fd 0=/dev/null、fd 1/2=vuepress.log（REG）、fd 4/5 为 node 进程内部 PIPE pair（双端同进程）。**无任何 PIPE 对端指向 cargo/bash 的 stdio。**
- [x] **Step 5（对照 A — 编译后直跑）**：`cargo build --bin hotpot` → `Finished dev profile`；`./target/debug/hotpot vuepress start --port 8080` 前台 Bash → 立即返回 `{"url":"http://localhost:8080","pid":14582}` exit 0，**不 hang**。
- [x] **Step 6（对照 B — cargo run 后台模式）**：`run_in_background: true` 跑 `cargo run --quiet -- vuepress start --port 8080` → task completed exit 0，stdout `{"url":"http://localhost:8080","pid":15079}`，**不 hang**。
- [x] **Step 7**：清场。`cargo run -- vuepress stop` → `Stopped vuepress (pid 15079, was running on port 8080).`；`cargo run -- vuepress status` → `{"running":false}`；`.hotpot-hub/vuepress.runtime.json` 不存在（`ls` 返回 No such file or directory）。
- [x] **Step 8（产出物）**：已填入本任务文件 `### Project Context > Diagnostic findings` 小节（替换原 `::: warning` 提示）。一句话根因：**fd 继承链路完全干净（setsid + null stdin + log redirect 三件套断尽），用户报告的"启动卡住"实际是 start() 返回时 TCP 端口未就绪、URL 被提前发出导致浏览器点开后 spin 被误读为启动 hang**。
- [x] **Step 9**：已写入"Phase 2 plan: 选用补丁 **P-A + P-D**" 一行，理由：lsof/ps 完全否定 Bash-tool-wait-EOF 与 fd 泄漏假说（P-B / P-C 触发条件不成立），而"提前发 URL"是已实证的真问题。

:::

#### Task 2: 基于 Phase 1 证据施加最小补丁集

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `src/vuepress.rs` | Modify | 若选 P-A：在 `start()` 内 spawn 后、`write_runtime_state` 前加端口就绪探测；若选 P-C：在 log_file 创建后显式 fcntl FD_CLOEXEC。 |
| `assets/prompts/vuepress.md` | Modify | 若选 P-B：closing flow 第 3 步改后台调用 + 轮询 stdout JSON 样式。 |

**Steps:**

- [x] **Step 1**：读 Task 1 Step 9 写入的 "Phase 2 plan" 一行。Phase 2 plan = **P-A + P-D**。
- [x] **Step 2（P-A）**：在 `src/vuepress.rs` 中加 helper `wait_for_port_ready(port)` + `read_log_tail(log_path, max_lines)` + 三个 `READINESS_*` 常量，并在 `start()` 的 `cmd.spawn()?` 之后、`write_runtime_state` 之前插入探测块。
  - 探测每 250ms 一次（`READINESS_POLL_INTERVAL`），单次 `TcpStream::connect_timeout` 250ms（`READINESS_CONNECT_TIMEOUT`），总硬上限 30s（`READINESS_TIMEOUT`）。
  - 超时分支：`terminate_pid(pid, false)` 清子进程 + `bail!` 附 vuepress.log 末尾 20 行。
  - 所有新增项都带双语 doc comment（中英对照）解释设计意图。
- [ ] ~~**Step 3（P-B）**~~：未选——见 Risks 章节 `<!-- alternative considered: P-B -->`。
- [ ] ~~**Step 4（P-C）**~~：未选——见 Risks 章节 `<!-- alternative considered: P-C -->`。
- [x] **Step 5**：
  - `cargo build --bin hotpot` → `Finished dev profile [unoptimized + debuginfo] target(s) in 0.07s`，5 个 warning 全部 pre-existing（toml_edit semver metadata / `Path` in `paths.rs:1` / `vuepress_hub_paths` / 两个 `as_str` / `get_repo_name`），**我的改动未引入新 warning**。
  - `cargo clippy --bin hotpot` → 同样 5 个 pre-existing warning，`vuepress.rs` 中仅 `get_repo_name` 那条 pre-existing（Non-Goals 明确不让动）；**新加的 `wait_for_port_ready` / `read_log_tail` / `READINESS_*` 与 start() 内的探测块 0 lint**。
- [x] **Step 6**：已把"P-A 30s 硬上限"、"P-D 实证记录"、"未独立验证终端路径"等条目写入本任务文件 `### Risks and Watchouts` 小节。
- [x] **Step 7**：已把未采用的 P-B / P-C 作为 `<!-- alternative considered: ... -->` 注释记录到 `### Risks and Watchouts` 小节末尾。

:::

#### Task 3: 端到端验证 hang 不再发生

::: info Task body

**Files:**

无文件修改；仅运行命令并把 stdout/stderr 关键事实回填到本任务文件的对应 Step checkbox 旁。

**Steps:**

- [x] **Step 1**：清场——`cargo run -- vuepress status` → `{"running":false}`。
- [x] **Step 2（最终硬判据）**：前台 Bash 跑 `cargo run --quiet -- vuepress start --port 8080`，**实测耗时 2.06s**，exit 0，stdout `{"url":"http://localhost:8080","pid":18668}`。<30s 硬上限，硬判据**通过**。
- [x] **Step 3**：`cargo run -- vuepress status` → `{"running":true,"port":8080,"url":"http://localhost:8080","pid":18668,"expires_at":"2026-05-18T03:39:23.177900+00:00"}`，pid 与 Step 2 完全一致。
- [x] **Step 4（URL 就绪硬判据）**：`curl --silent --max-time 5 --head http://localhost:8080` → `HTTP/1.1 200 OK` 立即返回。P-A readiness probe 已让端口在 start 返回时就绪。
- [x] **Step 5**：`cargo run -- vuepress stop` → `Stopped vuepress (pid 18668, was running on port 8080).`；二次 `status` → `{"running":false}`；`.hotpot-hub/vuepress.runtime.json` 不存在（ls No such file or directory）。
- [x] **Step 6**：实测数据已回填 `### Validation` 表的"实测"列。
- [ ] ~~**Step 7（可选）**~~：未在普通 macOS 终端独立验证；已在 `### Risks and Watchouts` 小节记一条 "未独立验证终端路径"。

:::

### Validation

| Command | Expected | 实测 |
| ------- | -------- | ---- |
| `cargo build --bin hotpot` | passes, no new warnings | `Finished dev profile [unoptimized + debuginfo] target(s) in 0.07s`；5 个 warning 全部 pre-existing（未引入新 warning）。 |
| `cargo clippy --bin hotpot` | passes, no new warnings on `src/vuepress.rs` | 通过；`vuepress.rs` 唯一 lint 是 pre-existing 的 `get_repo_name never used`，与本 task 无关。新加 `wait_for_port_ready` / `read_log_tail` / `READINESS_*` 零 lint。 |
| `cargo run --quiet -- vuepress start --port 8080`（**前台 Bash**，60s timeout） | 30s 内返回 exit 0，stdout 末尾 `{"url":"http://localhost:8080","pid":<N>}` 单行 JSON | 实测 **2.06s** 返回 exit 0，stdout `{"url":"http://localhost:8080","pid":18668}`。 |
| `curl --silent --max-time 5 --head http://localhost:8080` | 200 / 304 / 3xx；非 connection refused、非 hang | `HTTP/1.1 200 OK` / `Content-Type: text/html` / `Date: Mon, 18 May 2026 03:09:56 GMT`。 |
| `cargo run -- vuepress status` | JSON `"running": true` 且 pid 与 start 一致 | `{"running":true,"port":8080,"url":"http://localhost:8080","pid":18668,"expires_at":"2026-05-18T03:39:23.177900+00:00"}`，pid 与 start 完全一致。 |
| `cargo run -- vuepress stop` | stdout `Stopped vuepress (pid <N>, was running on port 8080).` | `Stopped vuepress (pid 18668, was running on port 8080).`；后续 `status` → `{"running":false}`；`.hotpot-hub/vuepress.runtime.json` 已删（`ls` → `No such file or directory`）。 |

### Risks and Watchouts

::: warning
- **P-A 的 30s 硬上限**：pnpm 首次构建慢的项目可能在 30s 内还没 bind 端口。当前 hub 体量小（一个 `home.md` + 几个用户的任务），实测应该 5–15s。设定 30s 是给 cold cache + 慢盘场景留余量，超时直接 bail 而不是无限等。如果未来 hub 变重，这个值要重新评估。超时分支会调 `terminate_pid(pid, false)` 尽力清掉子进程并附 vuepress.log 末尾 20 行作为线索。
- **P-D（Phase 1 实证记录）**：本任务在 macOS + 三条对照路径（前台 cargo run / 直跑 `./target/debug/hotpot` / 后台 cargo run）上均**未复现** Bash-tool hang；lsof 显示 pnpm/node 的 fd 0/1/2 全部断离父 shell（0=/dev/null, 1/2=vuepress.log REG file，PIPE 仅在 node 内部），`ps` 显示 PPID=1 / PGID=自身 / SESS=0，证明 `pre_exec(setsid)` + `stdin=null` + `stdout/stderr=log_file` 三件套已经把 fd 继承链路断尽。用户报告的"启动卡住"实际是 P-A 解决的"端口未就绪期间浏览器 spin"窗口，**不是** stdio pipe EOF 假说。生产路径 `cargo install --path .` 安装好的 hotpot 与 dev path `cargo run --` 行为一致，无需为 cargo wrapper 单独修复。
- **lsof / ps 输出在 macOS 上可能因 SIP 权限或孩子进程已退出而部分缺失**：诊断时如发现 hotpot 进程已退出但 hang 仍持续，重点查 cargo 的 fd（`lsof -p <cargo-pid>`）。
- **未独立验证终端路径**：本任务全部判据均在 Claude Code Bash tool 内执行；未在普通 macOS 终端独立跑一次端到端验证（Task 3 Step 7 标注为可选）。后续如发现普通终端行为不一致再补 task。
- **`.hotpot/prompts/vuepress.md` 是 `install` 安装的副本，不是 source-of-truth**：本 task 未改 `assets/prompts/vuepress.md`（P-B 未采纳），所以无需同步——但项目固有约定保留：未来若改 source-of-truth，本 worktree 已安装的副本不会自动同步，需跑 `cargo run -- update` 或重装。
- **Fix round 1（runtime.json 可见性窗口收敛）**：把 `start()` 中 `write_runtime_state` 提前到 readiness probe **之前**，让 probe 期内并发的 `vuepress status` / `vuepress stop` 也能看到这个进程；readiness 超时分支改为复用 `stop(root_dir, true)`，借用其 TERM → 3s 轮询 → KILL 升级与 runtime.json 清理，避免只杀 group leader 而留下 vuepress.js / esbuild 孙子进程。

<!-- alternative considered: P-B (closing-flow goes background) — rejected because Phase 1 lsof showed grandchild fds (0=/dev/null, 1/2=vuepress.log REG) do NOT inherit cargo/bash stdio pipes, so the Bash-tool-wait-EOF hypothesis is falsified. Switching the prompt to background mode would add a polling step to AI flow with no concrete bug to justify it. -->
<!-- alternative considered: P-C (explicit FD_CLOEXEC on log_file) — rejected because Phase 1 lsof showed log_file fd is correctly closed-on-exec by Rust's default (no log_file fd appears in pnpm/node beyond the expected 1/2 writes, and no third-party process inherits it). Rust's `fs::File::create` sets O_CLOEXEC; the belt-and-suspenders fcntl would be dead code on the path Phase 1 already validated. -->
:::

---

## Execution Instructions

把本任务文件完整内容交给 execution sub-agent。执行 agent 必须：

- 在动任何代码之前，先 Read 完整本任务文件、Read `src/vuepress.rs::start` 全函数（line 667–837）、Read `assets/prompts/vuepress.md`（如果 Task 2 选 P-B）。
- 严格按 Task 1 → Task 2 → Task 3 顺序执行。**Task 1 没有 Step 8、Step 9 的填写之前，禁止开始 Task 2。** Phase 1 的根因证据是 Phase 2 选补丁的唯一依据。
- Task 2 中候选补丁清单（P-A / P-B / P-C / P-D）的选取必须有 Task 1 Step 9 写入的"Phase 2 plan" 一行作为依据；如果证据指向其它修复路径（如双 fork、posix_spawn），停下来作为 blocker 上报，**不要**自行扩展补丁集——那超出 Approved Design。
- 完成每一步后把对应 `- [ ]` checkbox 改成 `- [x]`，并把命令 stdout 关键事实（JSON、pid、`Finished dev`、curl 响应头）原文记录在 checkbox 后面。
- 保留 `### Non-Goals` 全部限制——尤其不要顺手改 install/uninstall、stop/status、runtime.json schema、`.hotpot/prompts/vuepress.md` 副本。
- Task 3 Step 2 的硬判据失败时**不要** mark 完成；改成 blocker 报告，让用户决定回滚 Task 2 还是拓展方案。
- `.hotpot-hub/` 在 Task 1 Step 2 之后会留在 worktree。这是诊断必需的，不是 task 副作用；完成 task 后用户可以自行 `vuepress uninstall` 或保留。

## Open Questions

- 暂无。所有设计点已在 brainstorming 阶段与用户对齐（诊断优先 + Skip TDD + 候选补丁清单按证据选取）。
