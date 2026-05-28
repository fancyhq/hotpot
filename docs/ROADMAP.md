# Roadmap

## 增加功能

- [ ] **feat**: 增加意图判断，不要直接对代码进行优化
- [ ] **feat**: 检查所有警告和错误，移除未使用冗余代码
- [ ] **feat**: 在 `brainstorming` 探索阶段，增加多个并行子 `agent` 进行探索，最后汇总给主代理，然后由主代理与用户进行头脑风暴
- [ ] **feat**: `brainstorming` 时检测哪些任务可以并行，启动多个子代理执行任务
- [ ] **feat**: 增加主代理执行和子代理执行的选择，主代理执行无需重新读取 `task` 文件
- [ ] **feat**: 启动子代理执行，直接启动子代理执行，主代理无需再次阅读 `task` 文件
- [ ] **feat**: `hotpot:finish-work` 流程太长，看是否能合并一些问题和操作
- [ ] **feat**: 代码优化与模块拆分
- [ ] **feat**: `subagent` 可自定义模型
- [ ] **feat**: 检查 `vuepress` 是否启用通过 `hotpot` 命令校验，不让 `AI` 自行搜索是否有文件夹之类的

## 问题修复

### vuepress OR markdown

- [ ] **bug**: 如果用户上次未启动 `vuepress` ，在 `hotpot:execute` 时，无需启动 `vuepress stop`
- [x] **bug**: 在 `task` 文档中，有些状态，但是整体完成后，并没有修改这个 `In Progress` 的状态，要么移除这个状态显示，要么在整体完成后，再次更新这个状态，但是需要注意有 `vuepress` 和没有 `vuepress` 的文档内容同步
- [ ] **bug**: `claude code` 在有 `vuepress` 开启的情况下，没有使用 `vuepress` 的格式编写文件，也没有打开 `vuepress` 服务
- [ ] **bug**: 在使用 `brainstorming` 时，如果进行了其他对话操作，对于 `vuepress` 的规范会缺失，也不会启动并提醒 `vuepress` 查看
- [ ] **bug**: 使用 `hotpot init --platform` 添加其他平台时，如果 `config.toml` 已配置 `vuepress` ，将不再提示是否启用，有 `hotpot-hub` 文件夹将不再重复添加
- [ ] **bug**: 使用 `hotpot update` ，提示可以使用 `--force` 强制覆盖，但是执行发现没有这个命令

### Agent

- [ ] **bug**: 有时候在写入 `task` 文件时，在 `config.toml` 中设置 `language = "中文简体"` 会出现中英混合写入的情况
- [ ] **bug**: `git-worktree` 的名称应该使用任务 `title`（不带时间），而不是任务 `task_id`

#### PI

- [ ] **bug**: 在 `pi` 中，使用 `hotpot-new` 命令，与 `pi` 的对话，会一直重复回答 `ready` ，无法使用，看是否需要在给 `pi` 的 `prompt` 中，插入 `$@` 类似的变量占位

## 测试部署与安装

- [ ] **feat**: 移除所有的单元测试内容，尝试通过集成测试进行完整基础功能测试
- [ ] **feat**: 完善更新功能
- [ ] **feat**: 增加卸载项目文件配置功能
- [x] **feat**: 增加 GitHub Release 跨平台二进制自动构建与上传（Windows、macOS x86_64/aarch64、Linux x86_64/aarch64 + SHA256 校验）
- [x] **feat**: 增加 npm 自动发布（`@fancyhq/hotpot`）
- [x] **feat**: 增加 crates.io 自动发布（`cargo install hotpot-ai`，二进制命令仍为 `hotpot`）
- [ ] **feat**: 增加可直接安装的 Homebrew 发布渠道
- [ ] **feat**: 增加可直接安装的 Scoop 发布渠道
- [ ] **feat**: 增加可直接安装的 winget 发布渠道
- [x] **feat**: 增加 `github action` 机制，使用 `release-please` 完善发布流程，注意不能每次合并都创建新版本发布
- [ ] **feat**: 安装的提示都使用英文
