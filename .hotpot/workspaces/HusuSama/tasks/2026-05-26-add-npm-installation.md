# Add npm installation

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| Done | false | 5 | medium |
:::

---

## Task

### Summary

::: info
为 Hotpot 增加 npm 全局安装方式，让用户可以通过 npm 安装一个轻量 JS wrapper，并在安装阶段按当前平台下载现有 GitHub Release 中的 Rust 二进制压缩包。该任务还要把 npm 包版本纳入 Release Please 管理，并在 GitHub Actions release 流程中发布 npm 包。
:::

### User Request

::: info 用户原始需求
增加一个 npm 的安装方式，最好在 Github Actions 中能更新对应版本，目前的二进制文件形式：

- linux: `hotpot-hotpot-v0.3.1-linux-aarch64.tar.gz` / `hotpot-hotpot-v0.3.1-linux-x86_64.tar.gz`
- macos: `hotpot-hotpot-v0.3.1-macos-aarch64.tar.gz` / `hotpot-hotpot-v0.3.1-macos-x86_64.tar.gz`
- windows: `hotpot-hotpot-v0.3.1-windows-x86_64.zip`
:::

### Approved Design

::: tip
采用轻量 npm wrapper 方案：仓库新增 `npm/` 包目录，`npm/package.json` 暴露 `hotpot` bin，`postinstall` 脚本根据 `process.platform` / `process.arch` 映射到现有 release asset label，下载 `https://github.com/fancyhq/hotpot/releases/download/<tag>/<asset>`，解压根目录中的 `hotpot` 或 `hotpot.exe` 到 npm 包本地 `bin/` 目录。运行时 JS wrapper 只负责转发 CLI 参数和 stdio 到已下载的 Rust 二进制。

版本同步由 Release Please 负责：把 `npm/package.json` 加入 `release-please-config.json` 的 `extra-files`，确保 Release PR 更新 Rust 版本时也更新 npm 包版本。发布流程由 `.github/workflows/release-please.yml` 在 release 创建后构建并上传现有二进制资产，然后新增 npm publish job，在资产上传完成后检查 npm 包版本并执行 `npm publish ./npm`。执行阶段如果发现 npm 包名 `hotpot` 不可用，应把包名冲突作为阻塞项报告，不要擅自改名。
:::

### Alternatives Considered

- 将所有平台二进制直接打进一个 npm 包：安装时不依赖网络下载 GitHub Release，但包体积大、每次发布上传重复资产，不适合当前已有 release asset 的流程。
- 使用 npm optionalDependencies 拆分 `@scope/hotpot-<platform>` 平台包：更符合部分原生 npm 工具链实践，但需要维护多个 npm 包和发布矩阵，初始复杂度过高。
- 批准方案：单 npm wrapper 包 + GitHub Release 下载。它复用当前已存在的压缩包命名和 release workflow，新增文件少，后续也可平滑演进到平台拆包。

### Requirements

- 新增 npm 全局安装入口，用户安装后可运行 `hotpot`，并把所有参数转发给 Rust CLI。
- `postinstall` 必须支持当前 release asset 命名：`hotpot-${TAG}-${asset_label}.${ext}`，其中当前 tag 形如 `hotpot-v0.3.1`，所以最终文件名形如 `hotpot-hotpot-v0.3.1-linux-x86_64.tar.gz`。
- 支持平台至少包括 Linux x86_64/aarch64、macOS x86_64/aarch64、Windows x86_64；不支持的平台要给出英文错误输出。
- npm 包版本必须由 Release Please 更新，避免 `Cargo.toml` 与 `npm/package.json` 版本漂移。
- GitHub Actions release 流程要能在 release asset 构建上传之后发布 npm 包，并明确需要 `NPM_TOKEN` 或等价 secret。
- README 英文和中文都要补充 npm 安装说明与 release 流程说明。
- 代码输出和脚本错误信息保持英文；新增 JS 文件中的注释如有必要，遵守 English-first bilingual comment 风格。

### Non-Goals

::: details Non-Goals
- 不发布 crates.io、Homebrew、Scoop、Chocolatey 等其他包管理器。
- 不改变现有 release asset 命名格式，除非执行阶段发现必须修复现有 workflow bug。
- 不把 Rust 二进制提交进 git 仓库或 npm 源码包。
- 不为每个平台创建独立 npm 包，除非执行阶段证明单包方案不可行。
- 不实现运行时自动更新；安装后更新仍通过 npm 升级触发。
:::

### Project Context

- Rust crate 名称和二进制名均为 `hotpot`，当前 `Cargo.toml` 版本为 `0.3.1`，仓库地址为 `https://github.com/fancyhq/hotpot`。
- `.github/workflows/release-please.yml` 当前由 `googleapis/release-please-action@v4` 创建 release，并在 `build-release-assets` job 中构建 5 个目标平台。
- release asset 由 workflow 使用 `ARCHIVE="hotpot-${TAG}-${ASSET_LABEL}${EXT}"` 生成；当 tag 是 `hotpot-v0.3.1` 时，压缩包名就是用户列出的 `hotpot-hotpot-v0.3.1-...`。
- Windows 压缩包是 `.zip`，Unix 压缩包是 `.tar.gz`，二进制都位于压缩包根目录，无嵌套路径。
- `rebuild-release-assets.yml` 是手动重建已有 tag assets 的 fallback workflow，也使用相同 asset 命名。
- `release-please-config.json` 当前只把 `Cargo.lock` 放入 `extra-files`，需要增加 `npm/package.json`。
- README 当前明确说包管理器发布不在 release 流程覆盖范围内；完成本任务后需要更新这段说明。

---

## Plan

### Mode

- tdd: false

### Execution Strategy

- git-worktree: true
- rationale: 该任务会新增 npm 包、修改 GitHub Actions 和文档，涉及发布流程；使用独立 worktree 可隔离发布配置改动，降低影响当前 checkout 中其他工作的风险。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `npm/package.json` | Create | 定义 npm 包元数据、`bin` 入口、`postinstall` 脚本和发布文件清单。 |
| `npm/bin/hotpot.js` | Create | 作为 npm 暴露的 `hotpot` 可执行 wrapper，转发参数到下载后的 Rust 二进制。 |
| `npm/scripts/install.js` | Create | 安装阶段检测平台、下载对应 GitHub Release asset、解压并设置可执行权限。 |
| `release-please-config.json` | Modify | 把 `npm/package.json` 加入 Release Please 版本同步范围。 |
| `.github/workflows/release-please.yml` | Modify | 在 release asset 上传后增加 npm publish job，并说明 secret 需求。 |
| `.github/workflows/rebuild-release-assets.yml` | Modify | 视情况补充或保持与 asset 命名相关的注释，确保 npm 安装脚本假设与重建 workflow 一致。 |
| `README.md` | Modify | 添加英文 npm 安装说明，并更新 release 流程描述。 |
| `README.zh_CN.md` | Modify | 添加中文 npm 安装说明，并更新 release 流程描述。 |
| `docs/ARCH.md` | Modify | 若 npm 发布流程被纳入项目架构，更新英文架构说明。 |
| `docs/ARCH.zh_CN.md` | Modify | 与 `docs/ARCH.md` 同步更新中文架构说明。 |

### Implementation Tasks

#### Task 1: Create npm package wrapper

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `npm/package.json` | Create | 定义 npm 包、版本、bin、postinstall 和发布文件。 |
| `npm/bin/hotpot.js` | Create | npm 全局命令入口。 |
| `npm/scripts/install.js` | Create | 下载并解压平台二进制。 |

**Steps:**

- [x] **Step 1**: 创建 `npm/package.json`，版本初始值与 `Cargo.toml` 当前版本 `0.3.1` 一致，`bin.hotpot` 指向 `bin/hotpot.js`，`postinstall` 指向 `node scripts/install.js`。
- [x] **Step 2**: 创建 `npm/bin/hotpot.js`，用 Node 标准库解析包内 `bin/hotpot` 或 `bin/hotpot.exe`，使用 `spawnSync` 转发 `process.argv.slice(2)` 和 `stdio: "inherit"`，错误输出使用英文。
- [x] **Step 3**: 创建 `npm/scripts/install.js`，用 Node 标准库完成平台映射、HTTPS 下载、`.tar.gz` / `.zip` 解压、二进制权限设置，不引入 npm 运行时依赖，除非执行阶段证明标准库无法可靠处理 zip。
- [x] **Step 4**: 平台映射覆盖 `linux-x86_64`、`linux-aarch64`、`macos-x86_64`、`macos-aarch64`、`windows-x86_64`，构造 asset 文件名 `hotpot-${tag}-${label}${ext}`。
- [x] **Step 5**: 文件已创建，将在 Task 4 中通过 `node --check` 和 `npm pack --dry-run` 验证。

:::

#### Task 2: Wire version management and npm publish workflow

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `release-please-config.json` | Modify | 同步 npm 包版本。 |
| `.github/workflows/release-please.yml` | Modify | 在 release 创建后发布 npm 包。 |
| `.github/workflows/rebuild-release-assets.yml` | Modify | 确认手动重建 asset 与 npm 安装脚本兼容。 |

**Steps:**

- [x] **Step 1**: 修改 `release-please-config.json`，在现有 `extra-files` 中加入 `npm/package.json`。
- [x] **Step 2**: 在 `.github/workflows/release-please.yml` 中新增 `publish-npm` job，依赖 `release-please` 和 `build-release-assets`，条件为 `release_created == 'true'`。
- [x] **Step 3**: npm publish job 使用 checkout release tag、setup-node（registry-url npmjs.org）、dry-run 后 `npm publish ./npm`，用 `NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}`。
- [x] **Step 4**: workflow 注释中已说明 npm 发布依赖 `NPM_TOKEN`；缺少 token 时 npm publish 会因认证失败而报错，不会静默跳过。
- [x] **Step 5**: `rebuild-release-assets.yml` 已添加注释说明不 publish npm，与 README 说明一致。

:::

#### Task 3: Document npm installation and release behavior

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `README.md` | Modify | 英文用户安装与发布说明。 |
| `README.zh_CN.md` | Modify | 中文用户安装与发布说明。 |
| `docs/ARCH.md` | Modify | 英文架构记录 npm 分发流程。 |
| `docs/ARCH.zh_CN.md` | Modify | 中文架构同步说明。 |

**Steps:**

- [x] **Step 1**: 在 `README.md` 增加安装小节，说明 `npm install -g hotpot` 安装方式和 GitHub Release 依赖。
- [x] **Step 2**: 在 `README.zh_CN.md` 增加对应中文安装小节。
- [x] **Step 3**: 更新两个 README 的 release 流程，改写旧描述为 npm 发布已纳入、其他包管理器仍未覆盖。
- [x] **Step 4**: 更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`，新增 npm 分发章节，记录 wrapper 架构、版本同步、发布流程和 `NPM_TOKEN` 前提。
- [x] **Step 5**: 中英文文档内容一致，结构锚点和命令保持英文。

:::

#### Task 4: Validate package scripts and release configuration

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `npm/package.json` | Test | 验证 npm 包清单。 |
| `npm/scripts/install.js` | Test | 验证安装脚本语法和平台映射。 |
| `.github/workflows/release-please.yml` | Test | 验证 workflow 语法和依赖关系。 |
| `release-please-config.json` | Test | 验证 JSON 结构。 |

**Steps:**

- [x] **Step 1**: `node --check npm/bin/hotpot.js` — 语法检查通过（无输出）。
- [x] **Step 2**: `node --check npm/scripts/install.js` — 语法检查通过（无输出）。
- [x] **Step 3**: `npm pack --dry-run ./npm` — 列出 3 个文件（bin/hotpot.js, package.json, scripts/install.js），无二进制。
- [x] **Step 4**: JSON 验证 — 两个文件均解析正常（无异常）。
- [x] **Step 5**: 人工检查 release-please.yml — `publish-npm` job 的 `needs: [release-please, build-release-assets]`、`if: release_created == 'true'`、checkout tag、`NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}` 均正确。
- [x] **Step 6**: `cargo test` — 101 个测试全部通过。

:::

#### Task 5: Handle publish-name and secret risks explicitly

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `npm/package.json` | Modify | 包名和发布访问策略可能需要调整。 |
| `.github/workflows/release-please.yml` | Modify | 发布权限和 secret 失败模式需要清晰。 |
| `README.md` | Modify | 记录使用者和维护者可见的限制。 |
| `README.zh_CN.md` | Modify | 同步中文限制说明。 |

**Steps:**

- [x] **Step 1**: 已联网检查 npm registry — `hotpot` 包名**已被占用**（由 `benitogf` 发布的 Cordova livereload server，最后更新于 2019 年）。这是确认的冲突。
- [x] **Step 2**: 用户已决定使用 `@fancyhq/hotpot`。已更新 `npm/package.json` name、`publishConfig.access: public`、workflow 的 `npm publish --access public`、以及所有文档中的安装命令。二进制命令保持 `hotpot` 不变。
- [x] **Step 3**: 已确认 workflow 中 `NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}` 仅在 publish step 的 `env` 块中使用，不会输出到日志。
- [x] **Step 4**: 文档（README.md/README.zh_CN.md 的安装说明、ARCH.md/ARCH.zh_CN.md 的 npm 章节）已说明 npm 安装依赖 GitHub Release asset 可访问，离线环境或 GitHub 被阻断时安装会失败。

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `node --check npm/bin/hotpot.js` | JS wrapper 语法检查通过。 |
| `node --check npm/scripts/install.js` | 安装脚本语法检查通过。 |
| `npm pack --dry-run ./npm` | npm 包文件清单合理，不包含下载后的二进制或无关文件。 |
| `node -e "JSON.parse(require('fs').readFileSync('release-please-config.json', 'utf8')); JSON.parse(require('fs').readFileSync('npm/package.json', 'utf8'))"` | JSON 文件解析无异常。 |
| `cargo test` | Rust 测试通过。 |

### Risks and Watchouts

::: warning
- npm 包名 `hotpot` 可能已被占用；如果确认不可发布，需要用户决定新包名或 scoped 包名。
- `postinstall` 从 GitHub Release 下载二进制，用户网络、代理、GitHub 可用性都会影响安装体验。
- GitHub Actions 发布 npm 需要仓库 secret `NPM_TOKEN`；缺失时 release workflow 会在 npm publish 阶段失败。
- 当前 release tag 形如 `hotpot-v0.3.1`，asset 文件名包含双 `hotpot` 是现有命名结果，不要在安装脚本里误改成 `hotpot-v0.3.1-...`。
- Windows zip 解压如果只用 Node 标准库不可行，需要谨慎选择最小依赖，避免给运行时 wrapper 引入不必要依赖。
:::

---

## Execution Instructions

Give the execution sub-agent the full contents of this task file. The execution agent must:

- Read the full file before editing.
- Follow the `Plan` section step by step.
- Preserve all `Task` requirements, non-goals, constraints, and approved design decisions.
- Update checkbox steps in this file as work is completed, if the execution environment allows it.
- Do not expand scope beyond the approved task.
- Stop and report a blocker if a required file, command, API, or assumption differs from this handoff.
- Run the validation commands before reporting completion.

## Open Questions

- npm 包名 `hotpot` 是否在 npm registry 可发布，需要执行阶段联网检查或 release 前人工确认。
- 仓库是否已配置 `NPM_TOKEN` secret；如果没有，需要维护者在 GitHub repository secrets 中添加。
