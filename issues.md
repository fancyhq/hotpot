# Hotpot Review Memory

这里记录的是历史 review 或实现中出现过的问题。执行代码 review 时，必须先根据每条记录的 `Scene` 判断当前变更是否相关。

如果当前变更匹配某条 `Scene`，必须执行对应的 `Review Check`。如果发现同类问题，参考 `Solution` 给出修复建议，避免重复引入历史问题。

示例：

```markdown
## 创建卡片显示优化

- kind: optimization
- date: 2026-05-11
- tags: ui, card, border-radius
- paths: src/components

### Scene

当修改卡片、列表项、详情面板、弹窗内容块等 UI 容器时。

### Problem

添加的卡片没有使用圆角形式，导致视觉风格不一致。

### Review Check

检查新增或修改的卡片类 UI 是否遵循项目现有圆角规范。

### Solution

给卡片添加 `10px` 的圆角边框。
```

## Command::spawn 长命子进程必须显式平台 detach
- kind: bug
- date: 2026-05-17
- tags: process-spawn, detach, windows, unix, creation-flags, setsid, pre-exec, stdio-inheritance, bash-tool-hang
- paths: src/

### Scene
When new code calls `Command::spawn()` for a long-running child process (not awaited via `.status()` / `.output()`) — e.g. dev servers, background daemons, watchers — especially when stdout/stderr are redirected to files or null.

### Problem
默认 Command::spawn() 不 detach。父进程与子孙进程共享 console（Windows）或 process group / controlling terminal（Unix），导致孙子进程继承父 shell 的 stdio pipe 句柄。父进程退出后这些 pipe 仍开着，pipe-EOF reader（Claude Code Bash tool、cargo run wrapper 等）会无限期 hang。Windows 上还要警惕 DETACHED_PROCESS 与 CREATE_NO_WINDOW 互斥——同时设置会让后者被静默忽略，cmd.exe 弹黑窗。

### Review Check
审查任何新增 Command::spawn() 调用（非 .status()/.output() 同步等待），逐项核对：(1) Windows 路径有 creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP)，且没有 DETACHED_PROCESS（互斥）；(2) Unix 路径有 pre_exec(libc::setsid)；(3) stdin/stdout/stderr 显式重定向到 file 或 null，不留 inherit。任何一条缺失都会复现 Bash tool 卡死或 Windows 弹窗。

### Solution
Windows：use std::os::windows::process::CommandExt; cmd.creation_flags(0x0200 | 0x08000000)（CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW）。Unix：use std::os::unix::process::CommandExt; unsafe { cmd.pre_exec(|| { libc::setsid(); Ok(()) }) }，配合 Cargo.toml 的 [target.'cfg(unix)'.dependencies] libc = "0.2"。Stdio 重定向（Stdio::null() / Stdio::from(file)）与 detach 是正交的两层防护，必须同时保留。

## VuePress 2 容器内嵌 fenced 含 ::: 会触发 anchor 漏出
- kind: bug
- date: 2026-05-17
- tags: vuepress, markdown-hint, container, nested-containers, fenced-code, anchor-leak, rendering
- paths: assets/prompts/, assets/vuepress/, .hotpot/workspaces/

### Scene
审查任何 VuePress 渲染相关变更——AI 任务文件、`assets/prompts/vuepress-*.md`、`assets/vuepress/docs/.vuepress/*` 等——尤其当文档需要展示 `:::` 容器示例时。

### Problem
VuePress 2 (theme-default RC.88) 的 markdown-hint container 解析器扫描自己的闭合 `:::` 时不尊重 fenced code block 边界。如果 `:::` container body 含 fenced，且 fenced 内含 `:::`（无论 3 反引号还是 4 反引号 fence），内层 `:::` 会被贪婪 match 为外层 close，外层容器提前关闭，fenced 块之后的 markdown 漏出为真章节，ToC 出现重复 H2/H3 anchors（例如同一页面出现两个 `## Task`）。

### Review Check
扫描任何 VuePress 渲染目标文件（任务 `.md` / asset prompts / hub docs）：检查 `:::` 容器内是否嵌套含 `:::` 的 fenced code block。命中即标记为渲染破坏风险，要求重构为顶层 fenced（容器外）或 indented code（4 空格）。

### Solution
展示 `:::` 容器用法时把 fenced block 放在顶层 markdown 上下文（不被任何 `:::` 包裹）；或用 indented code 4 空格。`assets/prompts/vuepress-style.md` MUST 5 是项目内的硬规则，必须遵守。

## VuePress 2 默认主题 selector 与 CSS 变量名静默变更
- kind: bug
- date: 2026-05-17
- tags: vuepress, vuepress-2, theme-default, selector, css-variable, silent-breaking-change, scss
- paths: assets/vuepress/, src/assets/vuepress_hub.rs

### Scene
审查任何 VuePress 主题相关 CSS / SCSS 变更（`assets/vuepress/docs/.vuepress/styles/*`、theme override 等），特别是从 VuePress 1 迁移到 VuePress 2 或在 VuePress 2 RC 之间升级时。

### Problem
VuePress 1 → VuePress 2 默认主题更改了三处关键命名但无 deprecation 警告：(1) 正文容器从 class `.theme-default-content` 改为属性 selector `[vp-content]`；(2) markdown-it-container 插件被 `@vuepress/plugin-markdown-hint` 替换，CSS class `.custom-container` → `.hint-container`；(3) 边框颜色变量 `--vp-c-divider` → `--vp-c-gutter`。VuePress 1 selector 写法在 VuePress 2 里编译通过、运行时不报错，但静默失效——CSS 不匹配任何元素，整套样式不生效。

### Review Check
审任何 VuePress SCSS / CSS 变更时，grep 是否出现 `.theme-default-content` / `.custom-container` / `--vp-c-divider` 等 VuePress 1 selector。命中即标记为静默失效风险，要求改为 VuePress 2 命名（`[vp-content]` / `.hint-container` / `--vp-c-gutter`）。同时审核 `@vuepress/theme-default` 版本是否锁定在已验证的 RC 版本。

### Solution
在 SCSS 顶部注释维护 VuePress 2 RC.88 三处命名约定对照清单作为升级 alert；新增 selector 优先用 VuePress 2 命名；升级 theme-default 时 grep 整个 styles/ 目录确认所有 selector / 变量名仍存在。

