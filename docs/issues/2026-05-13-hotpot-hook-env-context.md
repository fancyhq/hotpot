<!-- 记录 Hotpot hook 去 Python 化实现中暴露的环境变量与上下文分层问题。 -->

# Hotpot Hook 环境变量与上下文分层问题

## 背景

原始实施方案要求去除 Claude Code 和 Codex 对 Python hook 脚本的运行时依赖，将 Hotpot 的基础上下文推导逻辑收敛到 Rust 二进制 `hotpot` 中。

方案中的核心目标包括：

- 新增统一入口 `hotpot hook bootstrap`。
- 新增平台事件入口：
  - `hotpot hook claude pre-tool-use`
  - `hotpot hook claude subagent-start`
  - `hotpot hook codex pre-tool-use`
  - `hotpot hook codex session-start`
- Claude Code 和 Codex 不再安装或调用 Python hook 文件。
- 平台脚本只作为极薄的 `sh` / `cmd` 转发层，不解析 JSON，不推导路径，不创建文件，不拼接 hook 响应。
- OpenCode 和 Pi 仍保留 TypeScript 形式，但只承担平台桥接，Hotpot 上下文推导由 Rust 负责。

本次实现按照该方向做了初步改造：

- 在 Rust 中增加了 Claude/Codex 平台 hook 子命令。
- 删除了 Claude/Codex 的 Python hook 资产。
- 新增了 `.sh` / `.cmd` 薄脚本资产。
- 将 Claude/Codex 配置改为默认直接调用 `hotpot hook ...`。
- 保留 `hotpot hook bootstrap` 作为统一上下文 JSON/shell 输出入口。

## 实现中暴露的问题

### 1. `sh` / `cmd` 不会被 AI 自动选择

项目中同时存在 `.sh` 和 `.cmd` 脚本时，AI 或 Hotpot 本身不会自动决定执行哪个脚本。

实际执行哪个入口取决于平台配置中的 `command` 字段：

- Unix/macOS 可配置为 `sh .claude/hooks/hotpot-pre-tool-use.sh`。
- Windows 可配置为 `.claude\\hooks\\hotpot-pre-tool-use.cmd`。
- 如果配置直接写 `hotpot hook claude pre-tool-use`，则完全绕过脚本。

因此，`.sh` / `.cmd` 只能作为平台兼容兜底资产，不能被理解为运行时自动选择机制。

### 2. Claude Code hook 不能把 env 永久写回后续进程

Claude Code 执行 hook 的过程大致是：

```text
Claude Code
  -> 执行 hotpot hook claude pre-tool-use
  -> 通过 stdin 传入 hook payload
  -> hotpot 解析 cwd 并生成上下文
  -> hotpot 输出 Claude Code hook JSON 响应
  -> Claude Code 将 additionalContext 注入模型上下文
```

这里存在一个关键限制：

```text
子进程不能修改父进程或未来兄弟进程的环境变量。
```

也就是说，`hotpot hook claude pre-tool-use` 即使在自己的进程内计算出了 `ROOT_DIR`、`HOTPOT_USERNAME` 等值，也不能让后续另一次独立执行的 `hotpot task ...` 自动继承这些环境变量。

环境变量继承方向只能是：

```text
父进程 env -> 子进程 env
```

不能反向传播：

```text
子进程 env -/-> 父进程 env
```

因此，Claude Code hook 更适合用于注入模型上下文，而不是作为后续 CLI 命令环境变量的可靠来源。

### 3. 业务命令仍强依赖 `ROOT_DIR` 会导致断层

当前项目中部分业务逻辑仍通过 `utils::get_root_dir()` 强制读取 `ROOT_DIR` 环境变量。

这会导致以下断层：

```text
hotpot hook claude pre-tool-use
  -> 已经计算出 ROOT_DIR
  -> 但 hotpot 进程退出

hotpot task list
  -> 新进程启动
  -> 如果外部没有 ROOT_DIR env
  -> 仍然失败
```

本次验证时运行 `cargo test` 也暴露了这个问题：测试失败集中在缺少 `ROOT_DIR` 环境变量，而不是 hook 子命令本身编译失败。

这说明业务层仍把平台 hook 注入的 env 当作唯一上下文来源，和“Rust 统一推导上下文”的目标还没有完全对齐。

### 4. 在 `sh` / `cmd` 中设置临时 env 只能解决当前子进程

可以在脚本中设置临时环境变量再执行 Hotpot，例如：

```sh
ROOT_DIR=...
HOTPOT_USERNAME=...
exec hotpot hook claude pre-tool-use
```

或 Windows：

```bat
set ROOT_DIR=...
set HOTPOT_USERNAME=...
hotpot hook claude pre-tool-use
```

这种方式可以让当前这一次 `hotpot` 子进程获得环境变量，但仍无法影响 Claude Code 主进程或后续独立启动的 `hotpot` 命令。

如果进一步把 `cwd` 解析、用户名推导、路径拼接、JSONL 文件创建、hook JSON 响应拼接都放进脚本，就会重新引入跨平台脚本复杂性。

这会带来几个问题：

- Claude/Codex/OpenCode/Pi 各平台逻辑重新分散。
- Unix shell 和 Windows cmd 行为不一致，维护成本高。
- 很容易再次依赖 `jq`、PowerShell、Python 或 Node。
- 脚本难以进行统一测试。
- Hotpot 的业务上下文不再有单一真相来源。

## 对当前方案的判断

去除 Python 运行时依赖的方向是正确的，但需要进一步明确分层。

平台 hook 不应该被设计成“设置未来所有 Hotpot 命令环境变量”的机制。它更适合做两件事：

- 把平台 payload 中的 `cwd` 交给 Hotpot。
- 将 Hotpot 计算出的上下文以平台支持的形式注入当前会话或当前工具调用。

业务命令不能假设平台 hook 已经成功为它准备好了环境变量。每一次 `hotpot task`、`hotpot issues`、`hotpot server` 等 CLI 调用都应该能独立解析自己的运行上下文。

## 建议优化方式

### 1. 抽出 Rust 内部统一上下文解析模块

建议新增独立的 Rust 上下文解析层，例如：

```text
src/context.rs
```

该模块负责统一生成 Hotpot 运行上下文，包括：

- `ROOT_DIR`
- `HOTPOT_USERNAME`
- `HOTPOT_ISSUE_CANDIDATES_FILE`
- `HOTPOT_RECORD_ISSUE_CANDIDATE_PROMPT`
- `HOTPOT_SUMMARIZE_ISSUE_CANDIDATES_PROMPT`

上下文解析优先级建议为：

```text
1. 显式参数，例如 --root-dir
2. 已存在的 ROOT_DIR / HOTPOT_USERNAME 环境变量
3. 当前工作目录 cwd
4. git repository root 或 canonical cwd
5. 默认用户名 default
```

这样可以同时支持平台注入和独立 CLI 调用。

### 2. `hook` 命令复用统一上下文模块

`hotpot hook bootstrap`、`hotpot hook claude ...`、`hotpot hook codex ...` 都应该只负责：

- 读取平台 payload 或 CLI 参数。
- 调用统一上下文解析模块。
- 按目标平台输出对应格式。

hook 命令不应该拥有一套独立的上下文推导逻辑。

### 3. 业务命令也复用统一上下文模块

`task`、`issues` 等业务命令不应该继续只依赖 `utils::get_root_dir()` 强制读取 env。

更合理的方式是：

```text
业务命令启动
  -> 调用 Rust 统一上下文解析
  -> env 存在则使用 env
  -> env 不存在则从 cwd/git 推导
  -> 正常执行命令
```

这样即使在 Claude Code 下 hook 只能注入模型上下文，后续 `hotpot task list` 仍然能从当前目录自行恢复必要上下文。

### 4. 平台脚本继续保持极薄

`.sh` / `.cmd` 建议继续只作为兜底入口：

```sh
#!/bin/sh
exec hotpot hook claude pre-tool-use
```

```bat
@echo off
hotpot hook claude pre-tool-use
exit /b %ERRORLEVEL%
```

不要在脚本中加入复杂 JSON 解析、路径推导、用户名推导或文件创建逻辑。

如需减少混淆，可以在 `init` 阶段按操作系统只安装对应脚本：

- Unix/macOS 只安装 `.sh`。
- Windows 只安装 `.cmd`。

但默认配置仍建议优先直接调用：

```text
hotpot hook claude pre-tool-use
```

### 5. 修正测试对环境变量的强依赖

当前测试失败说明测试和业务代码一样依赖外部 `ROOT_DIR`。

建议测试改为覆盖两类场景：

- 环境变量存在时，优先使用 env。
- 环境变量不存在时，能从临时目录或当前目录 fallback 推导。

这样可以验证 Hotpot 在 Claude Code、Codex、OpenCode、Pi 以及普通终端直接执行时都具备稳定行为。

## 推荐目标架构

最终推荐分层如下：

```text
平台层
  -> 只提供 cwd / stdin payload / 可选 env 注入能力

Rust context 层
  -> Hotpot 上下文单一真相来源
  -> 负责 root、username、prompt、JSONL 文件路径和文件创建

hook 层
  -> 调用 context 层
  -> 输出平台需要的 JSON 或 shell 格式

业务命令层
  -> 调用 context 层
  -> 不强依赖平台 hook 预先设置 env
```

这样可以避免以下问题：

- Python 运行时依赖回归。
- `.sh` / `.cmd` 逻辑分叉膨胀。
- Claude Code hook 计算过上下文但后续 CLI 仍缺 env。
- 不同平台对 `ROOT_DIR`、`HOTPOT_USERNAME` 等值的理解不一致。

## 结论

本次实现完成了 Python hook 资产迁移的第一步，但用户提出的问题暴露了更深层的上下文生命周期问题。

关键结论是：

- `sh` / `cmd` 可以设置当前子进程的临时 env，但不能解决后续独立 CLI 调用的上下文问题。
- Claude Code hook 能注入模型上下文，但不能可靠地为未来 Bash 或 Hotpot 命令设置环境变量。
- Hotpot 业务命令必须具备独立上下文解析能力。
- 最优方向是把上下文推导抽成 Rust 内部公共层，让 hook 和业务命令共同复用。
