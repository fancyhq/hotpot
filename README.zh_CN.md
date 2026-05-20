<div align="center">

# HOTPOT

可进化的 Agent 规范框架

</div>

## 为什么创建HOTPOT

- 我比较喜欢 `superpowers` 的 `brainstorming` 能力，但是我希望这个是能主动触发，而非通过 `skill` 进行
- 每次 `AI` 编写的代码有问题或者不规范，我都需要手动去更新 `AGENTS.md` ，甚至有时可能会忘记，我希望 `AI` 能够自己记住这些东西，以防下次又会出现同样的问题

## ✨特点

- 基础框架能力，包含头脑风暴、代码执行、代码审查等
- 通过记录问题，形成长期的记忆，在审查代码时，通过评分机制，获取最贴合的问题，协助代码审查
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

## 执行流程
