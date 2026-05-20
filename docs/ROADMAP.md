# Roadmap

- [ ] **feat**: 完善更新功能
- [ ] **feat**: 增加卸载项目文件配置功能
- [ ] **feat**: 增加意图判断，不要直接对代码进行优化
- [ ] **feat**: 检查所有警告和错误，移除未使用冗余代码
- [ ] **feat**: 增加 `npm` 、`bun` 、`cargo` 、`brew` 、`choco` `scoop` 等安装机制
- [ ] **feat**: 增加 `github action` 机制，使用 `release-please` 完善发布流程，注意不能每次合并都创建新版本发布
- [ ] **feat**: 在 `brainstorming` 探索阶段，增加多个并行子 `agent` 进行探索，最后汇总给主代理，然后由主代理与用户进行头脑风暴
- [ ] **feat**: `brainstorming` 时检测哪些任务可以并行，启动多个子代理执行任务
- [ ] **feat**: 增加主代理执行和子代理执行的选择，主代理执行无需重新读取 `task` 文件
- [ ] **feat**: 启动子代理执行，直接启动子代理执行，主代理无需再次阅读 `task` 文件
- [ ] **bug**: 如果用户上次未启动 `vuepress` ，在 `hotpot:execute` 时，无需启动 `vuepress stop`
- [x] **feat**: 在自动 `commit` 并写入 `commit hash` 后，再次将这个写入，提交一个 `commit`，使用信息：`chore: record task Done`
- [ ] **bug**: 使用 `hotpot init --platform` 添加其他平台时，如果 `config.toml` 已配置 `vuepress` ，将不再提示是否启用，有 `hotpot-hub` 文件夹将不再重复添加
- [ ] **feat**: 在 `brainstorming` 等文件中，移除使用 `cargo run --` 的说明，全部使用 `hotpot` 正式二进制名称
- [ ] **bug**: 有时候在写入 `task` 文件时，在 `config.toml` 中设置 `language = "中文简体"` 会出现中英混合写入的情况
- [x] **bug**: 在 `opencode` 中，`brainstorming` 结束未提示用户是否使用 `vuepress` 查看，写入的文件也未使用 `vuepress` 规定格式，需要验证所有平台的一致性
- [ ] **feat**: 安装的提示都使用英文
- [x] **bug**: 启动 `vuepress` 后，给的链接地址错误，需要使用包含日期、不包含后缀的完整文件名
- [ ] **bug**: `git-worktree` 的名称应该使用任务 `title`（不带时间），而不是任务 `task_id`
- [x] **feat**: `issue-condidates.jsonl` 应该直接在 `.hotpot` 下，而不是某个用户目录下，因为这是属于全局的错误记录
- [x] **bug**: 启动 `hotpot-execute` 时出现了报错：`Unknown agent type: hotpot-executionן codex-result-7fc55d31-78d6-4a0a-826a-4d89ca5387d3 is not a valid agent type`
- [x] **bug**: 经常会出现 `API` 错误或者 `"Concurrency limit exceeded for user, please retry later"` 类型的错误
- [ ] **feat**: `hotpot:finish-work` 流程太长，看是否能合并一些问题和操作
- [ ] **bug**: 在使用 `brainstorming` 时，如果进行了其他对话操作，对于 `vuepress` 的规范会缺失，也不会启动并提醒 `vuepress` 查看
- [ ] **bug**: 在 `pi` 中，使用 `hotpot-new` 命令，与 `pi` 的对话，会一直重复回答 `ready` ，无法使用，看是否需要在给 `pi` 的 `prompt` 中，插入 `$@` 类似的变量占位
- [ ] **bug**: 在 `TDD` 模式下，`Implementation Tasks` 中只有第一个 `task` 前面显示了圆点的图标
- [ ] **feat**: 代码优化与模块拆分
- [ ] **feat**: `subagent` 可自定义模型

## 测试部署

- [ ] 移除所有的单元测试内容，尝试通过集成测试进行完整基础功能测试
