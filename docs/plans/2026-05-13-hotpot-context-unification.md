<!-- Hotpot Hook 与业务命令的运行时上下文统一化方案。 -->

# Hotpot 运行时上下文统一化方案

## 背景

`docs/issues/2026-05-13-hotpot-hook-env-context.md` 指出：去 Python 化的第一步已经完成，但暴露出"业务命令不能独立解析上下文"的更深层问题。

当前状态（已在代码中核实）：

- `src/commands/hook.rs` 内部封装了完整的 cwd / git fallback 推导链（`HookContext` + `resolve_root_dir` + `resolve_username` + `build_context`），所有 hook 子命令共享。
- 业务命令 `task` / `issues` / `server` 仍走 `src/utils.rs` 的 `get_root_dir()` / `get_username()`，这两个函数只读 env，**无任何 fallback**，env 缺失就直接报错。
- `src/server.rs:194` 还有一处裸的 `std::env::var("ROOT_DIR").unwrap_or_else(|_| ".".to_string())` 软兜底，是项目里唯一的非 hook 场景 fallback，孤例。
- 5 个测试全部依赖 ambient shell env，没有 `std::env::set_var`，导致 `cargo test` 在干净 shell 下集体 panic。

平台层无须改动：

- OpenCode (`bash-before.ts`、`review-memory.ts`) 与 Pi (`extensions/hotpot/index.ts`) 已正确委托到 `hotpot hook bootstrap --format json`，TypeScript 侧不再重复推导上下文。
- Claude `settings.json` 与 Codex `config.toml` 都直接调用 `hotpot hook ...`；`.sh` / `.cmd` 是合格的薄兜底脚本（一行 `exec hotpot hook ...`）。

### 平台注入能力不对称（本次重构的根因）

四个平台对"在 bash 子进程里看到 `ROOT_DIR` 等 env"的支持能力**不一致**：

| 平台 | hook 能否直接注入 env 到 bash 子进程 | 机制 |
|---|---|---|
| OpenCode | ✅ 能（真 env 注入） | TS 插件 `shell.env` 返回 dict，OpenCode 把它直接写进 bash 工具子进程环境 |
| Pi | ✅ 能（mutate 命令前缀） | TS 插件 `tool_call` 事件可改 bash 命令文本，等效自动加 `export` 前缀 |
| Claude Code | ❌ 不能，只能给模型文字提示 | hook 是外部子进程，返回 JSON `additionalContext` 进入模型上下文；模型需要主动在生成 bash 命令时加 `export` |
| Codex | ❌ 不能，只能给模型文字提示 | 同 Claude，机制是 `additionalContext` / `systemMessage` |

OpenCode / Pi 的插件**与 agent 同进程**，能直接控制 spawn bash 工具时传什么 env；Claude / Codex 的 hook 是**独立子进程**，子进程的 env 修改无法反向传播到父进程。这是 OS + 平台安全模型层的硬约束，hotpot 改不了。

结论：**只要要兼容 Claude / Codex，业务命令就必须能在 env 缺失时自行从 cwd + git 推导上下文。** 平台 hook 注入退化为"加速器"，cwd fallback 是"最低保障"。这是本次重构的核心动机。

**预期结果**：无论从哪个入口（OpenCode 真 env 注入 / Claude/Codex 模型 export 提示 / 裸终端 / `cargo test`）启动 `hotpot`，都能拿到一致的 `ROOT_DIR` / `HOTPOT_USERNAME` 与派生路径，不再依赖外部环境变量预设。

## 设计

### 新增 `src/context.rs`（顶层平铺模块）

`src/utils.rs` 当前只有 13 行两个函数；为它建一个 `src/utils/` 目录纯属负担。直接删除 `utils.rs`，新增同级 `src/context.rs`，与已有的 `paths.rs` 平铺。

```rust
/// 已解析的 Hotpot 运行时上下文，供 hook 与业务命令共享。
#[derive(Debug, serde::Serialize)]
pub struct Context {
    #[serde(rename = "ROOT_DIR")]
    pub root_dir: String,
    #[serde(rename = "HOTPOT_USERNAME")]
    pub username: String,
    #[serde(rename = "HOTPOT_ISSUE_CANDIDATES_FILE")]
    pub issue_candidates_file: String,
    #[serde(rename = "HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT")]
    pub record_issue_candidate_prompt: String,
    #[serde(rename = "HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT")]
    pub summarize_issue_candidates_prompt: String,
}

impl Context {
    /// 业务命令入口：env 优先 (`ROOT_DIR`)，缺失则 cwd → canonicalize。
    pub fn resolve(root_override: Option<PathBuf>) -> Result<Self>;

    /// hook 入口：从 stdin payload 的 `cwd` 字段派生（不读 env，保证反映平台 payload）。
    pub fn from_payload(payload: &serde_json::Value) -> Result<Self>;

    /// 显式创建 JSONL（opt-in 副作用）。
    pub fn ensure_issue_candidates_file(&self) -> Result<()>;
}

/// 仅解析 root_dir：env `ROOT_DIR` → cwd → canonicalize。
pub fn resolve_root_dir(root_override: Option<PathBuf>) -> Result<String>;

/// 仅解析 username：env `HOTPOT_USERNAME` → `git config --local` → `--global` → `"default"`。
pub fn resolve_username(root_dir: &str) -> Result<String>;
```

`#[serde(rename = "...")]` 必须按原字符串保留：OpenCode 的 `bash-before.ts` 与 Pi 的 `extensions/hotpot/index.ts` 都按这五个大写下划线键名直接解析 JSON，等同公开契约。

### 公共契约（执行重构时绝对不能改动）

以下是已经被外部消费者锁死的接口，任何"改个名字更易读"的诱惑都必须忽略：

**1. `hotpot hook bootstrap --format json` 输出的 5 个顶层键名**（OpenCode `bash-before.ts` 的 `shell.env` 钩子按字面量取这些键当 env dict 写回 bash 子进程）：

```json
{
  "ROOT_DIR": "...",
  "HOTPOT_USERNAME": "...",
  "HOTPOT_ISSUE_CANDIDATES_FILE": "...",
  "HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT": "...",
  "HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT": "..."
}
```

**2. `hotpot hook bootstrap --format shell` 输出格式**：每行一条 `export KEY='VALUE'`，单引号内 `'` 用 `'"'"'` 转义。

**3. Claude Code hook JSON 形状**：

```json
{
  "continue": true,
  "suppressOutput": false,
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse" | "SubagentStart",
    "additionalContext": "..."
  }
}
```

**4. Codex hook JSON 形状**：

- PreToolUse：`{ "systemMessage": "...", "hookSpecificOutput": { "permissionDecision": "allow", "additionalContext": "..." } }`
- SessionStart：`{ "systemMessage": "...", "additionalContext": "..." }`

**5. 用户名 fallback 链顺序**：env `HOTPOT_USERNAME` → `git config --local user.name`（在 root_dir 中执行）→ `git config --global user.name` → 字面量 `"default"`。**顺序不能调**，否则会改变已部署用户的会话归属目录。

**6. root_dir 入口分叉**：

- 业务命令入口（`Context::resolve` / `resolve_root_dir`）：**env 优先**（`ROOT_DIR` → cwd → canonicalize）
- hook 入口（`Context::from_payload`）：**payload.cwd 优先，不读 env**（payload.cwd → `env::current_dir()` 兜底 → canonicalize）

两条入口**不能合并成一个统一签名**，否则 hook 会被 ambient `ROOT_DIR` 污染、回不到平台传入的 cwd。

### 关键设计决策

| 议题 | 决策 | 理由 |
|---|---|---|
| 模块位置 | 平铺 `src/context.rs` | 与 `paths.rs` 风格一致，免迁移目录 |
| JSONL 副作用 | 从 `build_context` 剥离，改为 `ctx.ensure_issue_candidates_file()` | hook bootstrap 显式调用；业务命令读 JSONL 不需要预创建 |
| `utils.rs` 处理 | 整体删除，无 shim 层 | issue 文档要求 clean break；调用点不多（task.rs 4 处、issues.rs 2 处、server 2 处） |
| env 优先级 | **两个入口分叉** | 业务命令 env-first（hook 注入的 env 被尊重）；hook 子命令走 `from_payload` 不读 env（payload 的 cwd 是真相） |
| `server.rs:194` 软 fallback | 统一迁移到 `context::resolve_root_dir(None)` | canonicalize 比 `"."` 更稳；行为兼容 |
| 测试迁移 | 不使用 `set_var`，改为显式参数 + cwd fallback | `set_var` 在并行 `#[test]` 中线程不安全 |
| Init 脚本按 OS 安装 | **不在本次范围**，列为 follow-up | 与本次的"env fallback"问题正交 |

## 关键文件改动

| 文件 | 操作 |
|---|---|
| `src/context.rs` | **新增**：抽出 `HookContext`（重命名为 `Context`，对外 `pub`）+ 解析函数 + doc 注释 |
| `src/main.rs` | 注册 `mod context;`，移除 `mod utils;` |
| `src/utils.rs` | **删除** |
| `src/commands/hook.rs` | 移除已迁出的私有 helper；bootstrap 改为 `Context::resolve(args.root_dir)? + ensure_issue_candidates_file()?`；claude/codex 改为 `Context::from_payload(&payload)?` |
| `src/commands/issues.rs:25, 31` | `utils::get_root_dir()?` → `context::resolve_root_dir(None)?` |
| `src/commands/server.rs:61` | 同上 |
| `src/server.rs:194` | 替换裸 `env::var`，统一走 `context::resolve_root_dir(None)?` |
| `src/commands/task.rs:45-46, 65-66, 91-92, 106-107` | `get_root_dir` / `get_username` → `context::resolve_root_dir(None)?` / `context::resolve_username(&root)?` |
| `src/task.rs` 测试（181-182, 193-194） | 用 `context::resolve_root_dir(None).unwrap()` + 字面量 `"test"` |
| `src/issues.rs` 测试（407, 415, 439） | 同上 |

可直接复用：`src/paths.rs` 全部函数（已经 env-free，零改动）。

## 执行顺序（每步独立可编译）

1. **新建 `src/context.rs`**：从 `hook.rs` 迁出 `HookContext`（重命名为 `Context`）/ `resolve_root_dir`（增加 env-first 层）/ `resolve_username` / `git_username` / `normalize_username` / `prompt_path` / `ensure_issue_candidates_file`。
   - `Context` 结构体、`Context::resolve`、`Context::from_payload`、`Context::ensure_issue_candidates_file`、`resolve_root_dir`、`resolve_username` 设为 `pub`；其余（`git_username` / `normalize_username` / `prompt_path` / `build_context`）保持 `pub(crate)` 或私有，避免泄露内部细节。
   - 每个函数与结构体加 doc 注释（遵循 AGENTS.md）。
   - 在 `main.rs` 注册 `mod context;`。
   - 此时 `hook.rs` 仍保留旧实现，整个仓库可编译，旧用例不变。
2. **改造 `src/commands/hook.rs`**：删除已迁出的私有 helper，改用 `context::Context::from_payload` / `Context::resolve`。
   - `BootstrapArgs` / `BootstrapFormat` / `print_shell_exports` / `print_json` / `shell_context_message` / `codex_shell_context_message` / `review_memory_message` / `shell_export_assignments` 等"输出形态"相关函数**留在 hook.rs**，它们是平台输出层。
   - bootstrap：`Context::resolve(args.root_dir)?` 然后调用 `ctx.ensure_issue_candidates_file()?`。
   - claude / codex：`Context::from_payload(&payload)?`，不要走 env-first 入口。
   - 运行验证小节中的 hook 命令，确认 JSON 形状与公共契约小节列出的形状一致。
3. **迁移 `issues.rs` 2 处调用点**。
4. **迁移 `server.rs:61` 与 `src/server.rs:194`**（裸 env::var 一并替换）。
5. **迁移 `task.rs` 4 处函数 × 2 调用**。
6. **更新 `src/task.rs` 和 `src/issues.rs` 测试**，让它们在无 ambient env 时也通过。
7. **删除 `src/utils.rs`** 并从 `main.rs` 移除 `mod utils;`。

## 验证

执行任何一项验证前先 `unset ROOT_DIR HOTPOT_USERNAME`，确保 fallback 真的被走到。

**基础检查**：

- 每步后 `cargo build`。
- 步骤 6 后 `cargo test`，要求**不依赖任何 ambient env**（验证测试已经摆脱 env 耦合）。
- 完整流程后 `cargo clippy --all-targets -- -D warnings`。

**业务命令 fallback 行为**：

```sh
cd /path/to/git/repo
cargo run -- task list      # 应输出任务列表（或空列表），不报 "ROOT_DIR 未设置"
cargo run -- issues list    # 同上
```

**Hook 边界（契约对照）**：

`cargo run -- hook bootstrap --format json` 输出必须是单行 JSON，`jq 'keys'` 结果**严格等于**：

```json
[
  "HOTPOT_ISSUE_CANDIDATES_FILE",
  "HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT",
  "HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT",
  "HOTPOT_USERNAME",
  "ROOT_DIR"
]
```

少一个、多一个、改一个字符都意味着 OpenCode 插件的 `shell.env` 会断（差一个字符就崩）。

Claude hook：

```sh
echo '{"cwd":"/tmp/foo","hook_event_name":"PreToolUse"}' | cargo run -- hook claude pre-tool-use
```

输出顶层 JSON 必须有 `continue` / `suppressOutput` / `hookSpecificOutput`，`hookSpecificOutput` 下必须有 `hookEventName` 和 `additionalContext` 两个字段。

Codex hook：

```sh
echo '{"cwd":"/tmp/foo"}' | cargo run -- hook codex pre-tool-use
```

输出顶层 JSON 必须有 `systemMessage` 和 `hookSpecificOutput`，后者下必须有 `permissionDecision: "allow"` 和 `additionalContext`。

**Hook 不读 env 验证**（关键回归用例）：

```sh
ROOT_DIR=/this/should/be/ignored cargo run -- hook claude pre-tool-use <<<'{"cwd":"/tmp"}'
```

输出的 `additionalContext` 里出现的应该是 `/tmp`（或其 canonicalize 结果），**不能**是 `/this/should/be/ignored`。否则说明 hook 路径错误地读了 env。

## 非显然 Gotcha

- **`resolve_root_dir` 的 env-first 顺序在业务命令中正确，在 hook 中错误**。hook 必须反映 payload 中的 `cwd`，不能被 ambient `ROOT_DIR` 污染。典型反例：用户在 shell 里有 `export ROOT_DIR=/path/to/projectB`（之前调试遗留），现在在 projectA 里启动 Claude Code；PreToolUse hook 被 spawn 时继承了 `ROOT_DIR=projectB` 但 payload.cwd 是 projectA——env-first 会让 hook 输出 projectB 的上下文，把 issue 候选写到错误项目里。所以 hook 子命令必须走 `Context::from_payload`，**不要**调用 env-first 的 `resolve_root_dir(None)`。两条入口绝不能合并。
- **`canonicalize` 对不存在路径返回 `Err`**：当前 `hook.rs:166` 用 `.unwrap_or(path)` 兜底，迁移时必须原样保留这个保护。
- **Windows `git config`**：`Command::new("git")` 跨平台可用（PATH 解析），无需 `.exe` 后缀；`current_dir(root_dir)` 必须保留，让 `--local` 找到正确仓库。
- **测试并发**：选择"显式参数 + cwd fallback"而不是 `set_var`，正是为了避免 `#[test]` 并行执行时的 env race。不要被"测试里设个 env 就好"的诱惑误导。
- **`serde rename` 是公共契约**：5 个 `#[serde(rename = "...")]` 字符串直接被 OpenCode / Pi 插件按字面量消费。结构体字段名可以变，但 rename 字符串必须原样不动。
