<div align="center">

# HOTPOT

Evolvable Agent Specification Framework

[简体中文](README.zh_CN.md) | English

</div>

## Why HOTPOT Was Created

- I like the `brainstorming` capability in `superpowers`, but I want it to be triggered proactively instead of through a `skill`.
- Whenever code written by `AI` has issues or does not follow conventions, I have to manually update `AGENTS.md`, and sometimes I may even forget. I want `AI` to remember these things by itself so the same issues do not happen again next time.
- Plain `markdown` files can be tiring to browse. I want more polished task files that can be accessed from a browser without affecting how `AI` parses them.

## ✨ Features

- Basic framework capabilities, including brainstorming, code execution, code review, and more.
- Issues from execution and questions raised by users are automatically accumulated. Users decide whether to persist them. During `review`, Hotpot retrieves the closest data through scenarios and scoring, up to 5 records, to improve review accuracy.
- `vuepress` integration, allowing more polished task files to be viewed in a browser without a `markdown` reader or similar tools. This feature depends on `npm` and `pnpm`.
- Two rounds of self-checking and self-repair.
- No external dependencies, unless `vuepress` is enabled, which requires `pnpm`.
- `TDD` hit mechanism: `TDD` is enabled only when Hotpot determines it is needed, but the choice is still yours. `AI` only judges, and does not decide.
- Subagent failure detection and recovery.

## 🖌️ Usage

### Project Initialization

> [!CAUTION]
> In general, do not use `hotpot init` directly. This installs configurations for all `agent` tools. You should choose the platform you are currently using.

Each project needs to be initialized at least once. If you need to add support for another `agent` tool, you can use a command like this:

```bash
hotpot init --platform opencode
```

### Start A Task

Start your `agent`, then use the `hotpot:new` command to create a task.

```bash
/hotpot:new <your request>
```

### Finish A Task

After everything is complete and manually verified, use `hotpot:finish-work` to finish the task.

```bash
/hotpot:finish-work
```

### Add A New User

For a project where `hotpot` already exists, other collaborators can be added. New users need to run the following command in the project root:

```bash
hotpot update
```

The `git` username is used by default. You can also define your own name:

```bash
hotpot update --username <your username>
```

### Re-fetch VuePress

- If `vuepress` is already enabled for the project (see `.hotpot/config.toml`) but the `.hotpot-hub` directory is missing, use `hotpot vuepress install` to re-fetch the `vuepress` framework.
- For a newly pulled project, you can use `hotpot vuepress install` to create the corresponding user directory.

### Configuration File

Simple project configuration can be done in `.hotpot/config.toml`. Currently, manually configurable data only includes:

> [!CAUTION]
> Do not manually modify the `vuepress` configuration, such as directly changing the `vuepress` status to `enabled = true`, because `vuepress` also has dependent directories. Manual changes will not take effect.

- `language`: the language used for generated artifacts. You can set different languages according to your needs, such as `简体中文` / `English` / `日本語` / `Français`, and so on.

### Installation via npm

hotpot can be installed globally via npm. The npm package (`@fancyhq/hotpot`) is a lightweight wrapper that downloads the platform-specific Rust binary from the corresponding GitHub Release version during installation. Even though the package is scoped under `@fancyhq`, the installed CLI command remains `hotpot`.

```bash
npm install -g @fancyhq/hotpot
```

After installation, the `hotpot` CLI command is available on your PATH.

> **Note:** The npm installation requires network access to GitHub Releases. If GitHub is not accessible from your network, the installation will fail. In that case, you can download the binary directly from the [Releases](https://github.com/fancyhq/hotpot/releases) page.

### Release Versions

This project uses `release-please` to automatically maintain the version release process, based on the [Conventional Commits](https://www.conventionalcommits.org/) specification.

**Release process:**

1. Daily development: submit PRs to the `main` branch that follow Conventional Commits, such as the `feat:` and `fix:` prefixes.
2. Automatic aggregation: after each push to `main`, GitHub Actions automatically creates or updates a **Release PR** that summarizes all new conventional commits.
3. Manual release: when an official release is needed, maintainers manually merge the Release PR. After it is merged, `release-please` automatically creates the Git tag and GitHub Release, and updates `CHANGELOG.md` and version files. The version in `npm/package.json` is also updated automatically alongside the Rust crate version.
4. Automatic build: after the Release is created, GitHub Actions automatically compiles release binaries for Windows, macOS (x86_64 + aarch64), and Linux (x86_64 + aarch64), then uploads archives and SHA256 checksum files to the corresponding GitHub Release.
5. Automatic npm publish: after the binary assets are built and uploaded, GitHub Actions automatically publishes the npm wrapper package to the npm registry with the matching version. This requires the `NPM_TOKEN` repository secret to be configured.

> Merging regular feature branches into `main` does not immediately create a tag or GitHub Release, so multiple features can be accumulated and released together later.
>
> Binary release assets are built and uploaded automatically only after the Release PR is merged. Publishing to package managers such as crates.io, Homebrew, Scoop, and Chocolatey requires separate evaluation and is not covered by the current release process.

## About VuePress

There is a lot of discussion about replacing `markdown` with `html`, but `html` inevitably has many shortcomings. It is clearly more complex for `AI` to parse `html` than `markdown`. The benefit of using `html` is that it is more comfortable for humans to read, but I do not think it should consume more `token`s for that. A task file is usually limited in size, so I wanted to find a compromise that makes reading more comfortable for humans without increasing the burden on `AI` parsing. That led me to documentation services. They usually use `markdown` or `mdx` as source files, while offering greater freedom in the page presentation layer. After evaluating options, I chose `vuepress`. Even without using `vue` components, it still provides a good reading experience, and the overall framework is also well suited for categorized browsing.

You can choose whether to enable `vuepress`. After it is enabled, a `vuepress` directory is created in the current project. You can put this directory directly into `git`, excluding the `node_modules` directory, because its data is located through symlinks. The installed size does not grow much: the whole directory is less than `130k`, excluding `node_modules`.

However, this also introduces an extra issue. Since the project is designed with minimal dependencies, enabling `vuepress` means a `node` environment is required.
