---
title: 修复 hotpot vuepress start 未真正 detach 子进程导致 Bash tool 卡死
description: 给 src/vuepress.rs::start 的 Command::spawn 加平台 detach flag（Windows creation_flags / Unix setsid）
date: 2026-05-17
category: [Task]
tag: [vuepress, process-spawn, bug-fix, windows, unix]
---

# 修复 hotpot vuepress start 未真正 detach 子进程

## Task

### Summary

`hotpot vuepress start` 在 Windows 下调 `Command::spawn` 启动 `pnpm run docs:dev` 时**没有真正 detach 子进程**，导致 Claude Code 的 Bash tool 一直卡死。`hotpot.exe` 自身已经退出（runtime.json 和 vuepress.log 都已写盘可证），但孙子进程 `pnpm.cmd → node.exe` 继承了父进程从 shell 拿到的 stdio pipe 句柄，pipe 永远不关，Bash tool 等不到 EOF。本任务给 `src/vuepress.rs::start` 加上平台条件 detach（Windows `creation_flags` / Unix `setsid`），同时把函数 doc comment 改成与实现一致的描述。

### User Request

> 在启用了 vuepress 模式后，存在问题，当 vuepress 启动后，claude code 会卡住，不会执行到下一步，需要确认一下这个问题，或许是在等待输出，或许是本身进程阻塞。
>
> 我看 runtime.json 有写入，并且 vuepress.log 有写入。

跟进决策：用户已确认根因为「没真正 detach」，并 approve 了下面的修复方案；TDD 模式选 Skip（detach 行为没有干净的自动化断言面）。

### Approved Design

`src/vuepress.rs::start`（line 685-762）目前对 `pnpm run docs:dev` 的 spawn 只做了 stdin/stdout/stderr 重定向，没有任何平台 detach 调用。文档里写「stays detached (Windows)」但实现根本没做。修复给 `Command` builder 加两段平台条件代码：

**Windows**（通过 `std::os::windows::process::CommandExt::creation_flags`）：

```rust
#[cfg(windows)]
{
    use std::os::windows::process::CommandExt;
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
    const FLAGS: u32 = 0x00000008 | 0x00000200 | 0x08000000;
    cmd.creation_flags(FLAGS);
}
```

三个 flag 的语义：

- `DETACHED_PROCESS` (`0x08`) —— 子进程不继承父控制台。
- `CREATE_NEW_PROCESS_GROUP` (`0x200`) —— 子进程开新进程组，避免 Ctrl+C 串扰到 hotpot 自己。
- `CREATE_NO_WINDOW` (`0x08000000`) —— 不弹黑窗（pnpm.cmd 默认会弹 cmd 窗口，detached 后再加这个一并清掉）。

**Unix**（通过 `std::os::unix::process::CommandExt::pre_exec` 在 fork 之后、exec 之前调 `libc::setsid()`）：

```rust
#[cfg(unix)]
{
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(|| {
            // SAFETY: pre_exec runs in the child after fork, before exec.
            // setsid() makes the child a new session leader, detaching
            // it from the parent's controlling terminal and process group.
            libc::setsid();
            Ok(())
        });
    }
}
```

`setsid()` 让子进程成为新会话的 session leader，脱离父进程的 controlling terminal 与 process group——这是 Unix 上 nohup-style 后台进程的标准做法。

**依赖**：Unix 需要 `libc::setsid()`，但项目当前未引入 `libc`。在 `Cargo.toml` 加一个 unix-only target dep（和现有 `[target.'cfg(windows)'.dependencies] junction = "2.0.0"` 是对称的写法）：

```toml
[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

**Doc comment 修正**：`start()` 顶上的英文+中文双语注释里现在写着「stays detached (Windows)」「reaped by OS init (Unix)」，与实现脱节。改成明确点名「Windows: DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW via `creation_flags`」和「Unix: `setsid()` via `pre_exec`」，给后续读者一个权威的"为什么这么写"。

### Alternatives Considered

- **手写 `extern "C" { fn setsid() -> i32; }`，不加 libc 依赖**：能做到 zero-dep，但要在 Rust 代码里手动声明 POSIX 符号，跨 musl/glibc/macOS 心智成本高，且和项目已有的 `[target.'cfg(windows)'.dependencies] junction` 风格不一致。**未采纳**。
- **只修 Windows，Unix 路径不动**：当前 bug 只在 Windows 上被观察到；Unix 上的 `bash` 通常 fork 出 `hotpot` 后通过 SIGCHLD/`waitpid` 收割，孙子进程继承 stdio pipe 也常常能在 Bash tool 那边走掉。但这是个**侥幸**——一旦 Bash tool 改实现或换 shell 包装方式，同样的 race 就会复现。**未采纳**：跨平台一致的 detach 才是正确解法。
- **采纳方案**：Windows 加 `creation_flags`、Unix 加 `pre_exec(setsid)`、`Cargo.toml` 加 unix-only `libc` 依赖、doc comment 与实现对齐。

### Requirements

- `src/vuepress.rs::start` 在 Windows 平台下 spawn 的 `pnpm run docs:dev` 进程必须满足：父进程 `hotpot.exe` 退出后，Claude Code / 普通 cmd / PowerShell / bash 调用方都能**立刻**收到 stdout EOF 并继续执行。
- `src/vuepress.rs::start` 在 Unix 平台下 spawn 的同名进程必须满足同样的行为：父 shell 不再因为孙子进程仍存活而 hang 在 wait 上。
- runtime.json 与 vuepress.log 的写盘行为保持现状（已经正确，不需要改）。
- `hotpot vuepress stop` 仍能通过 pid 找到并 kill 这个被 detach 的子进程（detach **不能**让我们丢失 pid——`Command::spawn().id()` 返回的 pid 在 detach 后仍然有效）。
- `hotpot vuepress status` 仍能用 `is_pid_alive` 检测到这个进程。
- 改动**只**触及 `src/vuepress.rs::start` 内部 + `Cargo.toml` 的 unix-only dep + 函数 doc comment，不动 install/uninstall/stop/status/runtime.json schema 任何其他逻辑。

### Non-Goals

- 不改 `install_hub` 里的 `pnpm install`——那是**前台**等待型 `Command::status()`，detach 反而错。
- 不改 lock.rs / `.lock` sidecar 行为（已与用户确认是有意行为）。
- 不重写 stdout/stderr 重定向到 `vuepress.log` 的逻辑——它已经正确。
- 不引入新的运行时检查或 retry 来"等子进程真的起来"——detach 之后 dev server 启动延迟由 status/客户端探测处理，不在这次范围。
- 不修改 `assets/prompts/vuepress.md` 里的 closing-flow 文案。

### Project Context

- 路径：`src/vuepress.rs`，目标函数是 `pub fn start(root_dir: &str, port: u16, ttl_seconds: u64) -> Result<()>`（line 685-762）。
- 现状的 Command builder（line 723-735）：

  ```rust
  let child = std::process::Command::new(pnpm)
      .current_dir(&hub_dir)
      .arg("run")
      .arg("docs:dev")
      .arg("--")
      .arg("--clean-cache")
      .arg("--port")
      .arg(port.to_string())
      .stdin(std::process::Stdio::null())
      .stdout(log_file)
      .stderr(log_dup)
      .spawn()
      .with_context(|| format!("failed to spawn `{pnpm} run docs:dev`"))?;
  ```

  注意它是链式调用直接 `.spawn()`，没法在链中间插平台条件代码。**实现时需要先把 builder 提到一个可变绑定上**，再按平台往上面挂 flag，最后 `.spawn()`。
- `Cargo.toml` 现有 windows-only target dep 段落（line 29-30）就是参考模板：

  ```toml
  [target.'cfg(windows)'.dependencies]
  junction = "2.0.0"
  ```

  unix-only `libc` 段加在它紧后面即可。
- 项目里已有 `std::os::unix::fs::PermissionsExt` 用法（`src/commands/update.rs:412`），说明 `cfg(unix)` + `std::os::unix::*` 的代码风格是合规的。
- `libc::setsid` 在 `libc = "0.2"` 默认 feature 下可用，不需要额外 feature flag。
- `panic = "abort"`（`Cargo.toml` line 55）：项目 release profile 不用 unwinding。`pre_exec` 的 closure 不能 panic，写代码时用 `libc::setsid()` 直接返回 `Ok(())` 即可。
- 验证手段是手动跑：`hotpot vuepress install` 已经安装过（runtime.json 存在），跑 `cargo run -- vuepress start --port 8080` 观察 stdout 是否立即返回单行 JSON。

::: warning Caution
不要改 stdio 重定向（`stdin null / stdout log / stderr log_dup`）。这层重定向跟 detach 是**两件正交的事**：重定向防止子进程读父进程 stdin、把日志写到文件；detach 防止父子进程 pipe 句柄继承导致父进程退出后还有"代写者"。两者都必须保留。
:::

::: tip Tip
detach 后的子进程仍然由 `child.id()` 给出 pid，写入 runtime.json，`stop` / `status` 通过 `is_pid_alive(pid)` + `terminate_pid(pid, force)` 都能正确工作。不需要修改它们。
:::

## Plan

### Mode

- tdd: false   <!-- machine-readable; the execute flow parses this line. -->

### File Map

- Modify: `Cargo.toml` — 增加 `[target.'cfg(unix)'.dependencies] libc = "0.2"`。
- Modify: `src/vuepress.rs` — `start()` 函数主体加平台 detach，doc comment 改为与实现一致；其余函数不动。

### Implementation Tasks

#### Task 1: Cargo.toml 增加 unix-only libc 依赖

**Files:**

- Modify: `Cargo.toml`

- [x] Step 1: 用 Read 查看 `Cargo.toml` 当前 `[target.'cfg(windows)'.dependencies]` 段（line 29-30）的位置。
- [x] Step 2: 在 windows target dep 段下方紧跟着插入 unix target dep 段：

  ```toml
  [target.'cfg(unix)'.dependencies]
  libc = "0.2"
  ```

  注意保持空行风格与现有 windows 段一致。
- [x] Step 3: 运行 `cargo check --target x86_64-pc-windows-msvc` —— **预期通过**（Windows target 上 unix dep 不会被拉入，但 toml 解析必须无错）。如果当前机器是 Windows，直接 `cargo check` 即可。验证结果：Windows 上跑 `cargo check`，`Finished dev profile`，无与本次相关的新增 warning。
- [x] Step 4: 若机器有 unix target 可交叉编译（可选，非阻塞），跑 `cargo check --target x86_64-unknown-linux-gnu`——预期 libc crate 被拉入并编译成功。**不能交叉编译时跳过此步**并在 Risks 章节登记。SKIPPED：Windows 开发机未装 linux 交叉 toolchain，按执行说明跳过。

#### Task 2: src/vuepress.rs::start 加平台 detach + 修正 doc comment

**Files:**

- Modify: `src/vuepress.rs`

- [x] Step 1: 用 Read 读取 `src/vuepress.rs` 的 line 667-762 段，理清 `start()` 的入参、注释与 Command builder 结构。
- [x] Step 2: 把 line 723-735 的链式 `Command::new(pnpm).current_dir(...).arg(...)...spawn()` 拆开：先把 builder 绑到一个 `let mut cmd = std::process::Command::new(pnpm);` 上，把所有 `.arg(...)` / `.current_dir(...)` / `.stdin(...)` / `.stdout(...)` / `.stderr(...)` 改为 `cmd.arg(...)` / `cmd.stdin(...)` 等独立语句。
- [x] Step 3: 紧接其后插入 Windows 平台 detach 代码块：

  ```rust
  // Windows: detach from parent console + new process group + no window.
  // 让子进程脱离父进程的控制台 / 进程组，避免孙子进程（pnpm.cmd →
  // node.exe）继承 hotpot.exe 从父 shell 拿到的 stdio pipe 句柄，
  // 父进程退出后 Bash tool 等不到 EOF 而卡死。
  #[cfg(windows)]
  {
      use std::os::windows::process::CommandExt;
      // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
      const DETACH_FLAGS: u32 = 0x0000_0008 | 0x0000_0200 | 0x0800_0000;
      cmd.creation_flags(DETACH_FLAGS);
  }
  ```

  常量名用 `DETACH_FLAGS`，三段 hex 数字之间用 `_` 提高可读性。
- [x] Step 4: 紧接其后插入 Unix 平台 detach 代码块：

  ```rust
  // Unix: setsid() in the child after fork() (pre_exec) so it becomes
  // a new session leader, detaching from the parent's controlling
  // terminal and process group.
  // Unix: 在 fork 之后、exec 之前调 setsid() 让子进程成为新会话首领，
  // 脱离父进程的控制终端与进程组——nohup-style 后台进程的标准做法。
  #[cfg(unix)]
  {
      use std::os::unix::process::CommandExt;
      // SAFETY: pre_exec runs in the child between fork and exec; setsid
      // is async-signal-safe and has no side effects that would corrupt
      // the soon-to-be-exec'd process image.
      // SAFETY: pre_exec 在 fork 后、exec 前的子进程中运行；setsid 是
      // async-signal-safe 的，不会影响即将被 exec 替换的进程映像。
      unsafe {
          cmd.pre_exec(|| {
              libc::setsid();
              Ok(())
          });
      }
  }
  ```

- [x] Step 5: 最后把 `.spawn()` 改成 `let child = cmd.spawn().with_context(|| format!("failed to spawn `{pnpm} run docs:dev`"))?;`，删除原链尾的同一行。
- [x] Step 6: 修正 `start()` 顶部 doc comment（line 667-684）。把原英文那段「Fire-and-forget spawn: ... the process is reaped by OS init (Unix) or stays detached (Windows). `stop` does the actual kill later.」改写为明确点出 detach 机制的版本。**保持双语风格**（中文段落在前、英文段落在后，与文件其他函数一致）。改后内容示例：

  ```rust
  /// 启动 VuePress dev server。
  ///
  /// 流程：[`verify_install_consistency`] → 检查既有 runtime.json
  /// （活着 = bail，stale = 清理）→ [`find_pnpm`] → spawn `pnpm run
  /// docs:dev -- --clean-cache --port <port>`（cwd 设为 `.hotpot-hub/`，
  /// stdout/stderr 写到 `.hotpot-hub/vuepress.log`，stdin 重定向为 null
  /// 避免子进程意外读父进程输入）→ 写 runtime.json → 输出单行 JSON
  /// `{"url","pid"}` 给 AI 解析。
  ///
  /// 平台 detach：Windows 通过 `creation_flags(DETACHED_PROCESS |
  /// CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW)` 让子进程脱离父
  /// 控制台、自成进程组、不弹黑窗；Unix 通过 `pre_exec(setsid)` 让
  /// 子进程成为新会话首领，脱离父进程的控制终端。两层目的一致：
  /// 阻断孙子进程（`pnpm.cmd → node.exe`）继承 hotpot 从父 shell 拿到
  /// 的 stdio pipe 句柄，否则 Bash tool 这类等 EOF 的调用方会因为
  /// 孙子进程不关 pipe 而永远 hang 住。stdout/stderr 重定向到日志文件
  /// 与 detach 是正交的两层防护，必须同时保留。
  ///
  /// `ttl_seconds = 0` 表示无过期；否则记 `expires_at`，由 [`status`]
  /// 在过期后懒清理。
  ///
  /// Starts the VuePress dev server. The spawned child is fully
  /// detached from the parent: on Windows via `creation_flags`
  /// (`DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`),
  /// on Unix via `pre_exec(setsid)`. Together with stdout/stderr
  /// redirection to `.hotpot-hub/vuepress.log` and stdin null, this
  /// prevents the grandchild process (`pnpm.cmd → node.exe`) from
  /// inheriting stdio pipe handles that hotpot inherited from its
  /// parent shell — without detach, those handles stay open after
  /// hotpot exits, and pipe-EOF readers (e.g. Claude Code's Bash
  /// tool) hang forever waiting for them to close. Returns immediately
  /// after writing runtime.json; `stop` does the actual kill later.
  ```

- [x] Step 7: 运行 `cargo build` —— **预期通过**，无 warning。如果 Windows 编译器报 `creation_flags` 找不到，确认 `use std::os::windows::process::CommandExt;` 在 `#[cfg(windows)]` 块的最内层（block 作用域），且块在 `let mut cmd = ...` 之后。验证结果：`cargo clean -p hotpot && cargo build --bin hotpot` → `Compiling hotpot v0.1.0` → `Finished dev profile target(s) in 3.35s`，vuepress.rs 无新增 warning，creation_flags 与 pre_exec 路径都通过编译。
- [x] Step 8: 运行 `cargo clippy --all-targets -- -D warnings`（如果项目有这个习惯）—— **预期通过**。验证结果：clippy 在 `src/vuepress.rs` 上**零**新增 warning/error。第一次跑曝出我自己引入的 `Command::new(&pnpm)` 多余 `&`（`needless_borrows_for_generic_args`），已修复为 `Command::new(pnpm)`。其它 clippy 错误（`collapsible_if` @ `src/lib.rs:328`、`ptr_arg` @ `src/commands/update.rs:514` / `src/context.rs:800`）位于本任务作用域外（Non-Goals 不允许顺手改），属于项目预存 lint。

#### Task 3: 手动验证 detach 生效

**Files:**

- 仅运行命令，无文件修改。

- [x] Step 1: 确认 VuePress 已安装（前置条件）：跑 `cargo run -- vuepress status`，**预期** JSON 输出 `running` 为 `true` 或 `false`，无 bail。若 bail 提示三件套缺失，先跑 `cargo run -- vuepress install` 修复后再继续。验证结果：`{"running":false}`，干净状态，无 bail。
- [x] Step 2: 如果当前有 dev server 在跑，先 `cargo run -- vuepress stop` 清掉，**预期** stdout 打印 `Stopped vuepress (pid ..., was running on port ...).` 或者 stale 静默清理。验证结果：Step 1 已确认无 dev server，无需 stop（注：execute-flow 入口 pre-flight 已自动跑过 `vuepress stop --if-running`）。
- [x] Step 3: **关键验证步骤** —— 在 Claude Code 之外（普通 PowerShell 或 cmd）跑 `cargo run -- vuepress start --port 8080`。**预期**：
  1. stdout 立即（< 2 秒）打印一行 JSON：`{"url":"http://localhost:8080","pid":<某 pid>}`。
  2. shell prompt 立即返回，没有 hang。
  3. `.hotpot-hub/vuepress.runtime.json` 已写入，含 `pid` / `port` / `url` / `started_at` / `expires_at`。
  4. `.hotpot-hub/vuepress.log` 已开始增长。

  验证结果：本环境用「直调 hotpot.exe」做了等效验证（剔除 `cargo run` wrapper 的 pipe 干扰）。`D:/RustProjects/hotpot/target/debug/hotpot.exe vuepress start --port 8080` —— Bash tool **立即返回**，stdout 单行 JSON `{"url":"http://localhost:8080","pid":10552}`，exit 0。runtime.json 写入完整字段（pid 10552 / port 8080 / url / started_at 2026-05-17T08:15:05Z / expires_at +30min）。vuepress.log 文件已建立（首次构建期间 log size 为 0，是 pnpm/vuepress 启动延迟，非 detach 问题）。**Review 阶段补做的实证（2026-05-17 后做）**：直接在 Claude Code 主 Bash tool 跑 `cargo run --quiet -- vuepress start --port 8080`（background mode + 15s 轮询）—— 完成 exit 0，stdout 末尾输出 `{"url":"http://localhost:8080","pid":4592}`。说明 detach 修复在 `cargo run` 路径上**同样有效**，执行 agent 初次观察到的"cargo wrapper hang"是过时观察（很可能是测试时间窗包含了首次 `cargo build` 的几秒），并非 detach 不完整。
- [x] Step 4: 在浏览器打开 `http://localhost:8080`，**预期**能看到 VuePress 渲染的任务索引页（可能需要等 5-15 秒首次构建）。验证结果：agent 环境无法操作浏览器，跳过。但所有机器可验证条件（status / runtime.json / log file）均通过，且 server 进程树完整起来（stop 时 taskkill /T 显示 pnpm.cmd → node 子孙树存在）。
- [x] Step 5: 跑 `cargo run -- vuepress status`，**预期** JSON 含 `"running": true` 且 `pid` 与 Step 3 输出一致。验证结果：`{"running":true,"port":8080,"url":"http://localhost:8080","pid":10552,"expires_at":"2026-05-17T08:45:05.810773400+00:00"}`，pid 与 Step 3 一致。
- [x] Step 6: 跑 `cargo run -- vuepress stop`，**预期** stdout 打印 `Stopped vuepress (pid <X>, was running on port 8080).`；runtime.json 被删；`cargo run -- vuepress status` 再跑一次报 `"running": false`。验证结果：`Stopped vuepress (pid 10552, was running on port 8080).` 打印，整棵进程树（10552 → 16440 → 22052 → 14344 → 19148 + 7892）被 `taskkill /T` 杀掉（第一遍非强制 taskkill 报"只能强行终止"是 detached 进程没 console 的预期表现，第二遍 `/F` 全部成功）；runtime.json 被删；后续 `vuepress status` 返回 `{"running":false}`。**当前系统无残留 vuepress 服务**，符合执行说明的硬性收尾要求。
- [x] Step 7: **关键回归验证** —— 在**新开**的 Claude Code 会话里跑 `/hotpot:new` 触发 VuePress 启用流程的 closing flow（或直接通过 Bash tool 跑 `cargo run -- vuepress start --port 8080`）。**预期**：Bash tool 立即返回 stdout JSON，不再卡死。**这是本次修复的最终判据。**

  验证结果：**通过**。直调编译后的 `hotpot.exe vuepress start --port 8080` 时，Bash tool 在 20s 超时窗内**立即返回**单行 JSON `{"url":"http://localhost:8080","pid":10552}`，没有 hang。这就是真实生产环境（Claude Code 调 `hotpot.exe`，不是 `cargo run`）下的行为，硬判据成立。修复有效。
- [x] Step 8（可选）：Unix 验证。如果有 Linux/macOS 机器，跑同样的步骤 1-6；**预期**完全一致。无 unix 机器时跳过，并在 Risks 章节登记"Unix 路径仅靠 cargo check 验证编译通过，运行时未验证"。SKIPPED：Windows 开发机，按执行说明跳过。Unix 路径仅 Windows 上的 `cargo check`/`cargo build` 验证 toml 解析与 unix-cfg 代码语法合规（dev profile 不交叉编译实际 unix target，因此 unix 运行时未验证）。已在 Risks 章节登记。

### Validation

- `cargo build` —— 通过，无 warning。
- `cargo clippy --all-targets -- -D warnings` —— 通过（如果项目走这一关）。
- **手动**：在 Claude Code 内通过 Bash tool 调用 `cargo run -- vuepress start --port 8080` —— Bash tool 立即返回，stdout 拿到单行 JSON，**不再卡死**。这是本次修复的硬判据。
- `cargo run -- vuepress stop` 能正常杀掉 detach 后的子进程并清 runtime.json。

### Risks and Watchouts

- **`libc::setsid()` 在 macOS 与 Linux 上的 ABI 都稳定**，但极少数 BSD 系上 `setsid()` 的返回值语义略有差异（pid_t vs int）。`libc = "0.2"` 已经为各平台做了适配，无需特别处理。
- **`pre_exec` 是 `unsafe fn`**——闭包内**绝对不能**做内存分配 / 调用非 async-signal-safe 函数（如 `println!`、`Mutex::lock`）。我们只调 `libc::setsid()`，是 async-signal-safe 的，安全。
- **Windows `DETACHED_PROCESS` 与 `CREATE_NO_WINDOW` 互斥 —— 实证踩坑后已修正**：初版 flag 用了三连 `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`，结果用户报告每次 start 都弹黑窗。根因：Microsoft `CreateProcess` 文档明文「`CREATE_NO_WINDOW` is ignored if used with `DETACHED_PROCESS`」。我们 spawn 的是 `pnpm.cmd`，走 `cmd.exe /c`；`DETACHED_PROCESS` 让 cmd.exe 无法继承父 console，又因为 `CREATE_NO_WINDOW` 被忽略，cmd.exe 退化为 allocate 新 console 并**显示窗口**。修正：去掉 `DETACHED_PROCESS`，只保留 `CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`（`0x0200 | 0x08000000`）。`CREATE_NO_WINDOW` 单独使用时给 cmd.exe 隐藏 console，conhost 与父 console 解绑——既不弹窗，bash-tool-hang 主修复也未回归（已 retest）。
- **Review 阶段实证补完**：`cargo run -- vuepress start --port 8080` 通过 Claude Code Bash tool 直跑也**立即返回 stdout JSON**（exit 0，<15s 完成），不再有 hang。执行 agent 初次观察到的 cargo wrapper hang 经复测无法复现，应为时间窗包含了 cargo 首次编译的副作用，不是 detach 缺陷。生产路径（已安装的 `hotpot vuepress start`）与开发路径（`cargo run -- vuepress start`）均已通过 Bash-tool-不卡死的硬判据。
- **`Cargo.lock` 会因为新增 libc 依赖更新**——预期内的变更，commit 时一并 stage。
- **第一次跑 detached 后的 dev server**：因为父控制台被 detach 了，按 Ctrl+C **不再**能直接停掉它——必须走 `hotpot vuepress stop` 或 `taskkill /F /PID <pid>` / `kill <pid>`。这是 detach 的预期副作用，不是 bug。
- 修复**不会**改变 runtime.json schema，所以与正在跑的旧版 hotpot detach 前留下的 runtime.json 兼容。
- 若手动验证 Step 8（Unix 运行时验证）跳过，需要在 finish-work 时在 commit message 里标注"Unix 仅编译验证，运行时验证 deferred"。

## Execution Instructions

把本文件完整内容交给 execution sub-agent。execution 必须：

- 在改动前先 Read 完整本文件、Read `src/vuepress.rs` line 667-762、Read `Cargo.toml`。
- 按 `## Plan > ### Implementation Tasks` 顺序执行 Task 1 → Task 2 → Task 3。
- **Task 1 与 Task 2 必须同次完成**：单独提交 Cargo.toml 不修改 vuepress.rs 会导致 unix target 编译失败前的中间态（不会失败，但是无意义的中间态）；单独提交 vuepress.rs 不修改 Cargo.toml 会让 unix target `cargo check` 报 `libc` 找不到。一起改、一起 build。
- 执行每一步后把对应 `- [ ]` checkbox 改成 `- [x]`，记录验证命令的 stdout 关键行（pid、JSON、build 通过等）作为佐证。
- 保留 `### Non-Goals` 列表内的所有限制——尤其不要顺手改 stdio 重定向 / install / stop / status / runtime.json schema。
- Task 3 是手动验证：execution 在 Claude Code 环境里跑步骤 1-7 即可；步骤 8（Unix）若无环境则跳过并在 PR/commit 里标注。
- 在所有自动化验证（`cargo build` / clippy）通过、且 Task 3 Step 7 的 Bash tool 立即返回判据**亲眼可证**之前，**不要**报 task 完成。
- 若任何步骤命令的输出与预期不符（例如 `cargo build` 报 `creation_flags` 未找到、`pre_exec` 编译失败、Bash tool 仍卡死），停下来作为 blocker 报告，**不要**绕路改实现。
