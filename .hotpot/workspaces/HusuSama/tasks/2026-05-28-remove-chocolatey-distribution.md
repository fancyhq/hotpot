# remove-chocolatey-distribution

::: info Overview
| Status | TDD | Tasks | Risk |
| ------ | --- | ----- | ---- |
| Done | false | 6 | medium |
:::

## Task

移除 Hotpot 的 Chocolatey 安装与发布渠道支持，并同步更新 README 和架构文档，确保项目只把仍受支持的安装/发布方式暴露给用户和维护者。

### Context

当前仓库仍把 Chocolatey 描述为受支持渠道：

- `README.md` / `README.zh_CN.md` 包含 `choco install hotpot` 安装说明。
- `.github/workflows/release-please.yml` 包含 `publish-chocolatey` job，并在顶部注释和 job graph 中描述发布到 Chocolatey。
- `release-please-config.json` 仍同步 `packaging/chocolatey/hotpot.nuspec` 版本。
- `packaging/chocolatey/` 包含 Chocolatey package metadata 和 install script。
- `scripts/update-release-package-manifests.sh` 只服务于 Chocolatey release package 更新。
- `docs/ARCH.md` / `docs/ARCH.zh_CN.md` 的 Multi-Channel Distribution 章节仍记录 Chocolatey 渠道、secret、job、脚本和手动重建约束。
- `docs/ROADMAP.md` 有一条已完成 Chocolatey 发布记录，需要评估是否保留为历史记录或改成移除说明，避免误导当前支持状态。

### Requirements

- 从用户文档中移除 Chocolatey 安装方式，不再出现推荐用户运行 `choco install hotpot` 的说明。
- 从 release workflow 中移除 Chocolatey 发布 job、job 依赖和相关注释，保留 GitHub Release、npm、crates.io 的发布路径。
- 从 release-please extra-files 中移除 Chocolatey nuspec 版本同步。
- 删除不再使用的 Chocolatey packaging 目录和专用 manifest 更新脚本，除非执行时发现仍有非 Chocolatey 用途。
- 同步更新 `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md`，因为该变更影响发布执行流。
- 保持代码和脚本中的用户可见输出为 English。
- 新写或保留的注释如需新增，应遵循项目 bilingual English + Chinese 风格。
- 不改变 npm、crates.io、GitHub Release 二进制资产构建和发布逻辑。

### Success Criteria

- `rg -n "Chocolatey|chocolatey|choco|CHOCO|choco install" README.md README.zh_CN.md docs .github release-please-config.json scripts packaging` 不再发现当前支持 Chocolatey 的残留说明；只允许明确历史/移除语境的文本残留，且不得误导用户。
- `.github/workflows/release-please.yml` 在 release 创建后仍会构建 release assets，并发布 npm 与 crates.io。
- `release-please-config.json` 的 extra-files 只包含仍存在且仍需要版本同步的文件。
- `docs/ARCH.md` 与 `docs/ARCH.zh_CN.md` 对多渠道分发、job graph、手动重建 workflow 的描述一致。
- 仓库中不再有无引用的 Chocolatey packaging 或更新脚本。

## Plan

### Mode

- tdd: false
- rationale: 本任务主要是文档、CI 配置和发布渠道清理，自动化验证以搜索残留和配置语法检查为主，不适合强制 Red → Green → Refactor。

### Execution Strategy

- git-worktree: true
- rationale: 任务会删除 packaging/script 文件并修改 release workflow，使用隔离 worktree 可以降低对当前 checkout 中其他工作的干扰。

### File Map

| File | Action | Reason |
| ---- | ------ | ------ |
| `README.md` | Modify | 删除 English Chocolatey 安装小节，并确认安装章节顺序仍自然。 |
| `README.zh_CN.md` | Modify | 删除中文 Chocolatey 安装小节，并确认安装章节顺序仍自然。 |
| `.github/workflows/release-please.yml` | Modify | 移除 `publish-chocolatey` job、Chocolatey 相关注释、job graph/行为说明中的 Chocolatey 文案。 |
| `release-please-config.json` | Modify | 从 `extra-files` 移除 `packaging/chocolatey/hotpot.nuspec`。 |
| `packaging/chocolatey/hotpot.nuspec` | Delete | 删除不再发布的 Chocolatey package metadata。 |
| `packaging/chocolatey/tools/chocolateyInstall.ps1` | Delete | 删除不再使用的 Chocolatey install script。 |
| `packaging/chocolatey/` | Delete | 删除空目录。 |
| `scripts/update-release-package-manifests.sh` | Delete | 删除 Chocolatey 专用 manifest 更新脚本，除非执行时发现仍被非 Chocolatey 路径引用。 |
| `docs/ARCH.md` | Modify | 移除 Chocolatey 渠道描述，更新 channel overview、job graph、version synchronization、manual rebuild workflow 等执行流说明。 |
| `docs/ARCH.zh_CN.md` | Modify | 与 `docs/ARCH.md` 等价更新为简体中文。 |
| `docs/ROADMAP.md` | Modify | 检查 Chocolatey 已完成条目，改为移除/历史语境或删除，避免当前支持状态误导。 |

### Implementation Tasks

#### Task 1: 清点并确认 Chocolatey 引用范围

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `README.md` | Test | 搜索英文用户文档中的 Chocolatey 引用。 |
| `README.zh_CN.md` | Test | 搜索中文用户文档中的 Chocolatey 引用。 |
| `docs/` | Test | 搜索架构、路线图和其他文档中的 Chocolatey 引用。 |
| `.github/workflows/release-please.yml` | Test | 搜索 release workflow 中的 Chocolatey 发布逻辑。 |
| `release-please-config.json` | Test | 搜索 Chocolatey 版本同步配置。 |
| `scripts/` | Test | 搜索 Chocolatey 专用脚本引用。 |
| `packaging/` | Test | 搜索 Chocolatey package assets。 |

- [ ] 运行 `rg -n "Chocolatey|chocolatey|choco|CHOCO|choco install" README.md README.zh_CN.md docs .github release-please-config.json scripts packaging`。
- [ ] 将匹配项分类为用户安装说明、release workflow、release-please 配置、packaging 文件、架构文档和历史路线图。
- [ ] 确认 `scripts/update-release-package-manifests.sh` 只服务于 Chocolatey；如果发现其他用途，先调整计划再删除。

#### Task 2: 更新用户安装文档

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `README.md` | Modify | 删除 English Chocolatey 安装方式。 |
| `README.zh_CN.md` | Modify | 删除中文 Chocolatey 安装方式。 |

- [ ] 从 `README.md` 删除 `### Installation via Chocolatey` 小节及 `choco install hotpot` 示例。
- [ ] 从 `README.zh_CN.md` 删除 `### 通过 Chocolatey 安装` 小节及 `choco install hotpot` 示例。
- [ ] 检查安装章节仍只包含 direct installation、npm、crates.io，并且中英文文档内容一致。

#### Task 3: 移除 Chocolatey 发布流程

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `.github/workflows/release-please.yml` | Modify | 删除 Chocolatey 发布 job 和相关说明。 |
| `release-please-config.json` | Modify | 删除 Chocolatey nuspec 版本同步。 |

- [ ] 在 `.github/workflows/release-please.yml` 顶部说明中移除 “publishes to ... Chocolatey” 相关描述，保留 npm 和 crates.io。
- [ ] 删除 `publish-chocolatey` job 及其 checksum download、manifest update、`choco pack`、`choco push` 步骤。
- [ ] 确认 `publish-npm` 仍依赖 `release-please` 和 `build-release-assets`，`publish-crates-io` 仍保持原有发布逻辑。
- [ ] 从 `release-please-config.json` 的 `extra-files` 移除 `packaging/chocolatey/hotpot.nuspec`，并保持 JSON 格式有效。

#### Task 4: 删除 Chocolatey 专用资产

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `packaging/chocolatey/hotpot.nuspec` | Delete | 删除 Chocolatey package metadata。 |
| `packaging/chocolatey/tools/chocolateyInstall.ps1` | Delete | 删除 Chocolatey install script。 |
| `packaging/chocolatey/` | Delete | 删除空目录。 |
| `scripts/update-release-package-manifests.sh` | Delete | 删除 Chocolatey 专用 manifest 更新脚本。 |

- [ ] 删除 `packaging/chocolatey/hotpot.nuspec`。
- [ ] 删除 `packaging/chocolatey/tools/chocolateyInstall.ps1`。
- [ ] 删除空的 `packaging/chocolatey/tools/` 和 `packaging/chocolatey/` 目录。
- [ ] 删除 `scripts/update-release-package-manifests.sh`。
- [ ] 删除前后运行搜索，确认没有 workflow 或文档继续引用被删除脚本/文件。

#### Task 5: 同步架构文档

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `docs/ARCH.md` | Modify | 更新英文架构文档中的分发执行流。 |
| `docs/ARCH.zh_CN.md` | Modify | 更新中文架构文档中的分发执行流。 |
| `docs/ROADMAP.md` | Modify | 避免路线图继续暗示 Chocolatey 是当前支持渠道。 |

- [ ] 更新 `docs/ARCH.md` 的 Multi-Channel Distribution，Channel Overview 只列出 GitHub Release、npm、crates.io。
- [ ] 更新 `docs/ARCH.md` 的 Release Workflow Job Graph，移除 `publish-chocolatey` 分支。
- [ ] 删除 `docs/ARCH.md` 的 `### Chocolatey Channel` 章节和 Chocolatey 相关 prerequisites、version synchronization、manual rebuild 说明。
- [ ] 更新 `docs/ARCH.zh_CN.md`，保持与英文架构文档同义、同结构。
- [ ] 检查 `docs/ROADMAP.md` 的 Chocolatey 条目；若保留，应明确这是历史已移除事项，不能表现为当前支持渠道。

#### Task 6: 验证和收尾

**Files:**

| File | Action | Reason |
| ---- | ------ | ------ |
| `release-please-config.json` | Test | 校验 JSON 格式。 |
| `.github/workflows/release-please.yml` | Test | 人工或工具检查 workflow 结构。 |
| `README.md` | Test | 确认无误导性 Chocolatey 残留。 |
| `README.zh_CN.md` | Test | 确认无误导性 Chocolatey 残留。 |
| `docs/` | Test | 确认架构和路线图描述一致。 |

- [ ] 运行 `rg -n "Chocolatey|chocolatey|choco|CHOCO|choco install" README.md README.zh_CN.md docs .github release-please-config.json scripts packaging`，确认无误导性残留。
- [ ] 运行 `python -m json.tool release-please-config.json >/dev/null` 或等价 JSON 校验命令。
- [ ] 如果环境支持，运行 `git diff --check` 检查 whitespace 错误。
- [ ] 如果仓库有 YAML 校验工具且无需新增依赖，可对 `.github/workflows/release-please.yml` 做语法检查；否则人工检查缩进和 job 依赖。

### Validation

- `rg -n "Chocolatey|chocolatey|choco|CHOCO|choco install" README.md README.zh_CN.md docs .github release-please-config.json scripts packaging`
  - Expected: 不出现当前支持 Chocolatey 安装/发布的残留；如有历史语境，必须明确不代表当前支持。
- `python -m json.tool release-please-config.json >/dev/null`
  - Expected: exit code 0。
- `git diff --check`
  - Expected: exit code 0。

### Risks and Watchouts

::: warning
- 不要删除 Windows release binary 构建。移除的是 Chocolatey package 发布，不是 Windows `.zip` release asset。
- 不要移除 npm postinstall 对 Windows binary asset 的下载支持。
- 不要把 crates.io package name `hotpot-ai`、npm package `@fancyhq/hotpot` 或 installed CLI command `hotpot` 改名。
- `docs/ARCH.md` 和 `docs/ARCH.zh_CN.md` 必须一起更新；该变更影响发布执行流，不能只改 README。
- 删除脚本和 packaging 文件后，必须搜索所有引用，避免 workflow 指向不存在路径。
:::

## Execution Instructions

执行代理应先读取本任务文件和 `docs/ARCH.md`，然后按 `## Plan` 顺序执行。保持变更聚焦在移除 Chocolatey 安装/发布渠道，不做额外分发渠道重构。所有新增或修改的用户可见命令输出保持 English；文档语言按目标文件现有语言分别维护 English 和简体中文。

## Open Questions

- 无。
