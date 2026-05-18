# Roadmap

- [ ] 完善更新功能
- [ ] 增加卸载项目文件配置功能
- [ ] 增加意图判断，不要直接对代码进行优化
- [ ] 检查所有警告和错误，移除未使用冗余代码
- [ ] 增加 `npm` 、`bun` 、`cargo` 、`brew` 、`choco` `scoop` 等安装机制
- [ ] 增加 `github action` 机制，使用 `release-please` 完善发布流程，注意不能每次合并都创建新版本发布
- [ ] 在 `brainstorming` 探索阶段，增加多个并行子 `agent` 进行探索，最后汇总给主代理
- [ ] `brainstorming` 时检测哪些任务可以并行，启动多个子代理执行任务
- [ ] 增加主代理执行和子代理执行的选择，主代理执行无需重新读取 `task` 文件
- [ ] 启动子代理执行，直接启动子代理执行，主代理无需再次阅读 `task` 文件
- [ ] 如果用户上次未启动 `vuepress` ，在 `hotpot:execute` 时，无需启动 `vuepress stop`
- [x] 在自动 `commit` 并写入 `commit hash` 后，再次将这个写入，提交一个 `commit`，使用信息：`chore: record task Done`
- [ ] 使用 `hotpot init --platform` 添加其他平台时，如果 `config.toml` 已配置 `vuepress` ，将不再提示是否启用，有 `hotpot-hub` 文件夹将不再重复添加
- [ ] 在 `brainstorming` 等文件中，移除使用 `cargo run --` 的说明，全部使用 `hotpot` 正式二进制名称
- [ ] 有时候在写入 `task` 文件时，在 `config.toml` 中设置 `language = "中文简体"` 会出现中英混合写入的情况
- [ ] 在 `opencode` 中，`brainstorming` 结束未提示用户是否使用 `vuepress` 查看，写入的文件也未使用 `vuepress` 规定格式，需要验证所有平台的一致性

## 测试部署

- [ ] 移除所有的单元测试内容，尝试通过集成测试进行完整基础功能测试
