<div align="center">

# HOTPOT

可进化的 Agent 规范框架

简体中文 | [English](README.md)

</div>

## 为什么创建HOTPOT

- 我比较喜欢 `superpowers` 的 `brainstorming` 能力，但是我希望这个是能主动触发，而非通过 `skill` 进行
- 每次 `AI` 编写的代码有问题或者不规范，我都需要手动去更新 `AGENTS.md` ，甚至有时可能会忘记，我希望 `AI` 能够自己记住这些东西，以防下次又会出现同样的问题
- 普通的 `markdown` 文件浏览比较吃力，我希望能从浏览器访问更美观的任务文件，同时不影响 `AI` 的解析

## ✨特点

- 基础框架能力，包含头脑风暴、代码执行、代码审查等
- 执行过程的问题和用户提出的问题，将会自动沉淀，由用户决定是否持久化，`review` 时，将通过场景和评分，获取最接近的数据，最多5条，提升 `review` 准确率
- `vuepress` 集成能力，可以通过浏览器来查看更美观的任务文件，无需 `markdown` 阅读器等，此功能依赖 `npm` 和 `pnpm`
- 两轮自检与自修复
- 无任何外部依赖（启用 `vuepress` 会需要 `pnpm`）
- `TDD` 命中机制，当判断需要使用 `TDD` 时才会启用，但仍由你选择，`AI` 仅做判断，不做决定
- 子代理失败检测与恢复

## 🖌️使用

### 项目初始化

> [!CAUTION]
> 一般情况下，不要直接使用 `hotpot init` 命令，这会将所有 `agent` 工具配置安装上，你需要选择你目前使用的平台

每个项目至少需要操作一次，如果需要添加其他 `agent` 工具支持，你可以使用下面类似的命令，：

```bash
hotpot init --platform opencode
```

### 开始任务

启动你的 `agent` ，然后使用 `hotpot:new` 命令来创建一个任务

```bash
/hotpot:new <你的需求>
```

### 完成任务

当全部完成并人工检验后，使用 `hotpot:finish-work` 来完成此任务

```bash
/hotpot:finish-work
```

### 新增用户

已存在 `hotpot` 的项目，可以添加其他协作用户，新用户需要在项目根目录，执行如下命令：

```bash
hotpot update
```

默认使用 `git` 用户名，你也可以自己定义名称：

```bash
hotpot update --username <你的用户名>
```

### 重新拉取 Vuepress

- 如果项目已经启用了 `vuepress` （见 `.hotpot/config.toml`），但是项目中没有 `.hotpot-hub` 目录，你需要使用 `hotpot vuepress install` 重新拉取 `vuepress` 框架
- 对于初次拉取的项目，可以使用 `hotpot vuepress install` 命令来创建对应的用户目录

### 配置文件

在 `.hotpot/config.toml` 中，可以进行简单的项目配置，目前手动配置数据仅包含：

> [!CAUTION]
> 不要手动修改 `vuepress` 的配置，比如直接修改 `vuepress` 的状态为 `enabled = true`，因为 `vuepress` 还会有依赖目录，所以手动修改并不会生效

- `language` : 产出物使用的语言，你可以根据自身的需求，设置不同的语言，比如 `简体中文` / `English` / `日本語` / `Français` 等

### 发布版本

本项目使用 `release-please` 自动维护版本发布流程，基于 [Conventional Commits](https://www.conventionalcommits.org/) 规范。

**发布流程：**

1. 日常开发：向 `main` 分支提交遵循 Conventional Commits 规范的 PR（如 `feat:`、`fix:` 前缀）。
2. 自动聚合：每次 push 到 `main` 后，GitHub Actions 会自动创建或更新一个 **Release PR**，汇总所有新的 conventional commits。
3. 人工发布：当需要正式发布时，维护者手动合并 Release PR。合并后 `release-please` 会自动创建 Git tag 和 GitHub Release，并更新 `CHANGELOG.md` 和版本文件。
4. 自动构建：Release 创建后，GitHub Actions 自动为 Windows、macOS（x86_64 + aarch64）和 Linux（x86_64 + aarch64）编译 release 二进制，将压缩包和 SHA256 校验文件上传到对应的 GitHub Release。

> 普通功能分支合并到 `main` 不会立即创建 tag 或 GitHub Release，确保你可以累积多个功能后再统一发布。
>
> 二进制 release assets 仅在合并 Release PR 后自动构建上传，crates.io / Homebrew / Scoop / Chocolatey 等包管理器发布需要单独评估，不在当前发布流程覆盖范围内。

## 关于 vuepress

关于 `html` 代替 `markdown` 的言论甚嚣尘上，但是 `html` 不可避免的有很多不足，`AI` 在解析 `html` 时明显是比 `markdown` 要更复杂的，使用 `html` 的好处是能让人在阅读的时候更舒适，但我并不觉得应该为此消耗更多 `token` ，通常一个任务文件的大小是有限的，所以我想找到一个折中的办法，
既能让人类阅读更舒适，同时也不会加重 `AI` 解析的负担，于是我将目光投向了文档服务，他们通常使用 `markdown` 或者 `mdx` 来做源文件，但是在页面展示层面有更高的自由度，经过筛选，我选择了 `vuepress` ，即使不使用 `vue` 组件，也能保持较好的阅读体验，同时整体的框架，
也比较适合分类查看

你可以选择是否启用 `vuepress` ，启用后会在当前项目创建一个 `vuepress` 的目录，你可以将这个目录直接放进 `git` 中（需要剔除 `node_modules` 目录），因为他数据都是通过软链接来定位的，安装后体积不会增长，整个目录仅占用不到 `130k` （不计算 `node_modules` 情况下）

但这也带来额外的问题，因为项目是以最小依赖设计的，启用 `vuepress` ，意味着需要 `node` 环境
