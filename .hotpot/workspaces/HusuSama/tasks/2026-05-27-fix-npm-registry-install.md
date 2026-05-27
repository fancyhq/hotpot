---
title: fix-npm-registry-install
description: Diagnose and document npm registry resolution for the @fancyhq/hotpot package.
date: 2026-05-27
category: [Task]
tag: [npm, registry, install]
---

# fix-npm-registry-install

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| In Progress | false | 4 | medium |
:::

---

## Task

### Summary

::: info
解决 `npm i -g @fancyhq/hotpot` 在当前环境中被解析到 `http://bnpm.byted.org/@fancyhq%2fhotpot` 并返回 404 的问题。执行阶段需要先确认 package 是否已正确发布到 npmjs，再判断失败是否由本机或公司内网 npm registry 配置导致；只有发现发布配置、package metadata 或 release workflow 确有缺陷时才修改实现配置。
:::

### User Request

::: info 用户原话
重新执行 hotpot/new
:::

### Problem Statement

用户遇到的安装命令如下：

```bash
npm i -g @fancyhq/hotpot
```

失败信息显示 npm 请求了 ByteDance 内部 registry：

```text
npm ERR! 404 Not Found - GET http://bnpm.byted.org/@fancyhq%2fhotpot
```

这说明当前失败路径至少包含 npm registry 解析问题：scoped package `@fancyhq/hotpot` 没有从 npmjs 公共 registry 获取，而是被发送到了内部镜像。任务目标是给出最小、正确、可验证的修复。

### Approved Design

::: tip
执行阶段先做事实验证，再决定是否改代码。优先假设这是用户 npm registry 配置导致的安装路径错误，而不是 package 未发布或 release workflow 故障。

如果 `@fancyhq/hotpot` 在 `https://registry.npmjs.org` 可查，且当前环境 registry 指向内部镜像，则最小修复应是更新安装文档，明确说明在公司内网或自定义 registry 环境中可以使用显式 registry 参数或 scope-specific npm 配置。

如果 npmjs 不可查，或 package metadata / publish workflow 存在实际缺陷，再修改 `npm/package.json`、release workflow 或相关发布文档。
:::

### Non-Goals

::: details Non-Goals
- 不更改 CLI 命令名；安装后的命令仍为 `hotpot`。
- 不引入新的 npm 包管理器或额外运行时依赖。
- 不绕过 npm 官方 registry 发布流程。
- 不为了兼容内部 registry 添加私有发布逻辑。
- 不在未验证真实缺陷前修改 release workflow。
- 不把内部 registry 地址硬编码进项目配置。
:::

### Risks and Watchouts

::: warning
- 当前环境可能存在 shell hook 依赖 `hotpot` 命令的问题；执行阶段如果 Bash 被 hook 阻塞，需要先说明阻塞点，再使用可用方式继续验证。
- npm registry 配置可能来自多层 `.npmrc`、环境变量或公司工具链，不能只检查仓库根目录。
- 文档修复必须避免暗示所有用户都需要 `--registry`；默认 npmjs 用户应继续使用普通安装命令。
- 如果修改 `docs/ARCH.md`，必须同步修改 `docs/ARCH.zh_CN.md`。
:::

---

## Plan

### Mode

- tdd: false

### Execution Strategy

- git-worktree: false

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `README.md` | Modify | Add public npm registry guidance if registry config is confirmed as root cause. |
| `README.zh_CN.md` | Modify | Keep Chinese installation guidance aligned with English README. |
| `docs/ARCH.md` | Modify | Update npm distribution notes if execution flow or install troubleshooting guidance changes. |
| `docs/ARCH.zh_CN.md` | Modify | Keep Chinese architecture documentation aligned when `docs/ARCH.md` changes. |
| `npm/package.json` | Modify | Only if package metadata is proven to cause install or publish failure. |
| `.github/workflows/release-please.yml` | Modify | Only if npm publish workflow is proven defective. |

### Implementation Tasks

#### Task 1: Verify npm package availability and local registry configuration

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `npm/package.json` | Test | Confirm package metadata matches the published package expectation. |
| `.github/workflows/release-please.yml` | Test | Confirm publish workflow targets npmjs when relevant. |

**Steps:**

- [x] **Step 1**: Run `npm view @fancyhq/hotpot version --registry https://registry.npmjs.org` and record whether npmjs contains the package.
- [x] **Step 2**: Inspect effective npm registry settings with `npm config get registry`, `npm config get @fancyhq:registry`, and relevant npm config listing commands.
- [x] **Step 3**: Check whether repository-local `.npmrc` exists; if not, note that registry routing likely comes from user/global/company configuration.
- [x] **Step 4**: Inspect `npm/package.json` and release workflow only enough to confirm whether package metadata or publish target is implicated.

:::

#### Task 2: Choose the smallest correct fix

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `README.md` | Modify | Primary place for install troubleshooting guidance. |
| `README.zh_CN.md` | Modify | Chinese installation guidance must stay synchronized. |
| `npm/package.json` | Modify | Only if metadata is the verified root cause. |
| `.github/workflows/release-please.yml` | Modify | Only if publish workflow is the verified root cause. |

**Steps:**

- [x] **Step 1**: If npmjs lookup succeeds and local registry points to `bnpm.byted.org` or another mirror, update installation docs with explicit public registry examples.
- [x] **Step 2**: Include both one-off install guidance, such as `npm install -g @fancyhq/hotpot --registry https://registry.npmjs.org`, and persistent scope guidance, such as `npm config set @fancyhq:registry https://registry.npmjs.org/`.
- [x] **Step 3**: If npmjs lookup fails, investigate release status and publish workflow before changing docs.
- [x] **Step 4**: Avoid changing package metadata or workflow unless the verification step proves they are defective.

:::

#### Task 3: Keep architecture documentation aligned when behavior docs change

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify | Architecture docs describe npm wrapper distribution and install flow. |
| `docs/ARCH.zh_CN.md` | Modify | Chinese architecture doc must match the English version. |

**Steps:**

- [x] **Step 1**: If README gains npm registry troubleshooting guidance, decide whether architecture docs need a short note under npm distribution.
- [x] **Step 2**: If architecture docs are updated, keep English and Simplified Chinese content equivalent.
- [x] **Step 3**: Do not expand architecture docs with user-specific internal registry details beyond generic custom registry troubleshooting.

:::

#### Task 4: Validate installation guidance and repository health

::: info Task body

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `README.md` | Test | Ensure commands are accurate and copy-pasteable. |
| `README.zh_CN.md` | Test | Ensure Chinese instructions match English instructions. |
| `docs/ARCH.md` | Test | Ensure architecture update is consistent if changed. |
| `docs/ARCH.zh_CN.md` | Test | Ensure Chinese architecture update is consistent if changed. |

**Steps:**

- [x] **Step 1**: Re-run the npmjs lookup command after changes to confirm the public package remains discoverable.
- [x] **Step 2**: If feasible, run `npm pack --dry-run ./npm` to confirm npm wrapper package contents remain valid.
- [x] **Step 3**: Run relevant formatting, linting, or documentation checks available in this repository if touched files require them.
- [x] **Step 4**: Summarize root cause, changed files, and exact install command users should run in internal registry environments.

:::

### Validation

| Command | Expected |
| ------- | -------- |
| `npm view @fancyhq/hotpot version --registry https://registry.npmjs.org` | Returns a published version, or clearly identifies a publish-side issue. |
| `npm config get registry` | Shows whether the default registry is npmjs or an internal mirror. |
| `npm config get @fancyhq:registry` | Shows whether the `@fancyhq` scope overrides the default registry. |
| `npm pack --dry-run ./npm` | Package contents include `bin/` and `scripts/` if package files are touched. |

---

## Execution Instructions

1. Start by verifying npmjs availability and the effective npm registry configuration; do not assume the package is unpublished.
2. Prefer documentation-only changes if the root cause is custom or internal registry routing.
3. Modify npm metadata or release workflow only after proving those files are responsible for the failure.
4. Keep all user-facing command output and code/config output in English.
5. If `docs/ARCH.md` changes, update `docs/ARCH.zh_CN.md` in the same execution.
6. Preserve unrelated worktree changes and do not revert user edits.
