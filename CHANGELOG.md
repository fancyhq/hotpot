# Changelog

## [0.3.0](https://github.com/fancyhq/hotpot/compare/hotpot-v0.2.0...hotpot-v0.3.0) (2026-05-25)


### Features

* ✨ add task utils ([767af9d](https://github.com/fancyhq/hotpot/commit/767af9dfdd011153a8bfb9dd700e404f8931889c))
* ✨ issue base ([8619f0f](https://github.com/fancyhq/hotpot/commit/8619f0fbd9a2a83981fbb964ed04dbd7a61a0f3a))
* ✨ 任务获取增加状态过滤 ([8a110f2](https://github.com/fancyhq/hotpot/commit/8a110f24f620883fee1ae45f8a8b400e1551cabe))
* ✨ 修改isuue缓存文件为全局 ([335c004](https://github.com/fancyhq/hotpot/commit/335c004b2be2406a5bbc07b2a123044cc7dc8950))
* ✨ 增加opencode配置 ([17ab614](https://github.com/fancyhq/hotpot/commit/17ab614f9dc2cd5d8f389b9275e94f30d83356b5))
* ✨ 增加vuepress ([08044d5](https://github.com/fancyhq/hotpot/commit/08044d58d468d8b1493f847c108f36c60ea15ed1))
* ✨ 增加vuepress文档基本功能 ([4d999d4](https://github.com/fancyhq/hotpot/commit/4d999d4e8a6edf26f16f4a908943cb875919d090))
* ✨ 增加对claude/codex/pi的环境兼容 ([daf12f4](https://github.com/fancyhq/hotpot/commit/daf12f432accff3f6498d76ca78c229f1e56eb33))
* ✨ 增加配置更新ROADMAP ([5b20dde](https://github.com/fancyhq/hotpot/commit/5b20ddee008dcbedfd9c5d8eb85e0c99f44feefe))
* ✨ 核心功能实现 ([a640ff6](https://github.com/fancyhq/hotpot/commit/a640ff61847f3350b13b4cbf23b9b88c5e3ca4aa))
* ✨ 移除关于 cargo run 的说明 ([29559eb](https://github.com/fancyhq/hotpot/commit/29559ebf8b2015ca213ad9d197be0222f93f1d7c))
* add release binary assets ([5f8ad31](https://github.com/fancyhq/hotpot/commit/5f8ad31ba25079eb4ab6f97badc313216db252ac))
* add release-please workflow ([d191053](https://github.com/fancyhq/hotpot/commit/d191053134699202f048e6b76192a97b1df6555a))
* add update force flag ([e1887cb](https://github.com/fancyhq/hotpot/commit/e1887cbd0a8b50efe0370efdb343b8b0e4a1f668))
* **commands:** add /new command for guided brainstorming ([667dea7](https://github.com/fancyhq/hotpot/commit/667dea7ced5a7291efff0875c49679a5c9581165))
* no change ([7dede29](https://github.com/fancyhq/hotpot/commit/7dede29256ea8527193b123fd39f9e5edbea5c38))
* polish VuePress task page rendering ([33fc631](https://github.com/fancyhq/hotpot/commit/33fc63151867ff796238afbc062426b370f7b380))
* **skills:** add writing-plan skill for /new workflow ([a699f64](https://github.com/fancyhq/hotpot/commit/a699f64c9fb3823b2ff2e2ee2183586e6f618f28))
* 增加兼容，但需修复环境变量问题 ([79d38bc](https://github.com/fancyhq/hotpot/commit/79d38bc60f9640ca3515dbcb81a72e0d078b247f))
* 增加初始化、新建任务、执行任务相关内容 ([5bfcf2e](https://github.com/fancyhq/hotpot/commit/5bfcf2e2456bd2e29222cdb29853a0fe6e43d9f8))


### Bug Fixes

* 🐛 修复创建任务文件不存在问题；修复多语言不生效问题 ([a41acaf](https://github.com/fancyhq/hotpot/commit/a41acafb1bf18e087ed52b3cf88c642dc08804ea))
* 🐛 修复多语言输出问题，每轮增加提示 ([ddd5b6c](https://github.com/fancyhq/hotpot/commit/ddd5b6cff27be0fe60c17f11e92338b6b5626536))
* 🐛 更改 Windows编译 ([1b17acc](https://github.com/fancyhq/hotpot/commit/1b17accccf43246d3ca2363ed51e6bd2191babbf))
* clean test artifacts ([e5d92b7](https://github.com/fancyhq/hotpot/commit/e5d92b7f2264d5ec8f970311f7b2d9de155cb8df))
* guard Pi first tool call ([f8fe8e1](https://github.com/fancyhq/hotpot/commit/f8fe8e1ebf6e68e2b350f13a7644873e9a1b793a))
* harden execute subagent error recovery ([cccfee4](https://github.com/fancyhq/hotpot/commit/cccfee4a6702ce46503fe7ba4c426b9ffbd5cec3))
* harden Pi extension for weak models ([af1487a](https://github.com/fancyhq/hotpot/commit/af1487a6e3c7e6d863a16e6fc28b7f3ada79e3a4))
* harden Pi hotpot-new prompt (partial) ([628f586](https://github.com/fancyhq/hotpot/commit/628f5867d85c1167939bf840b064d2082457465d))
* keep VuePress task URL stem ([ea23990](https://github.com/fancyhq/hotpot/commit/ea239902bc435cb77f782d1c23a3694d69402dab))
* move issue candidates global ([7296869](https://github.com/fancyhq/hotpot/commit/7296869aee0a23b1fc5f1922d239140de82dcf0f))
* move worktree choice to new ([332af39](https://github.com/fancyhq/hotpot/commit/332af39cab2e9c15a76b6641f43b68ed31872cd6))
* pass Pi new arguments ([51bc25b](https://github.com/fancyhq/hotpot/commit/51bc25bf8077dc3f13aa57701ed6b9ec13fc35e9))
* probe vuepress port before emitting url ([2a2938e](https://github.com/fancyhq/hotpot/commit/2a2938ef0d7c79a422836453917d1f24f006b5a9))
* sync Pi new prompt ([cb7fad6](https://github.com/fancyhq/hotpot/commit/cb7fad6a5964a1fa1b33b5d8d8f1e5990586a507))
* sync task Overview status ([519a3d6](https://github.com/fancyhq/hotpot/commit/519a3d6577c8616d9b4a33ec03fac484a466adfb))
* unify VuePress gate via file-existence ([cc0c7f6](https://github.com/fancyhq/hotpot/commit/cc0c7f662350f5f001331df72238f24df7bd0664))

## [0.2.0](https://github.com/fancyhq/hotpot/compare/hotpot-v0.1.0...hotpot-v0.2.0) (2026-05-25)


### Features

* ✨ add task utils ([767af9d](https://github.com/fancyhq/hotpot/commit/767af9dfdd011153a8bfb9dd700e404f8931889c))
* ✨ issue base ([8619f0f](https://github.com/fancyhq/hotpot/commit/8619f0fbd9a2a83981fbb964ed04dbd7a61a0f3a))
* ✨ 任务获取增加状态过滤 ([8a110f2](https://github.com/fancyhq/hotpot/commit/8a110f24f620883fee1ae45f8a8b400e1551cabe))
* ✨ 修改isuue缓存文件为全局 ([335c004](https://github.com/fancyhq/hotpot/commit/335c004b2be2406a5bbc07b2a123044cc7dc8950))
* ✨ 增加opencode配置 ([17ab614](https://github.com/fancyhq/hotpot/commit/17ab614f9dc2cd5d8f389b9275e94f30d83356b5))
* ✨ 增加vuepress ([08044d5](https://github.com/fancyhq/hotpot/commit/08044d58d468d8b1493f847c108f36c60ea15ed1))
* ✨ 增加vuepress文档基本功能 ([4d999d4](https://github.com/fancyhq/hotpot/commit/4d999d4e8a6edf26f16f4a908943cb875919d090))
* ✨ 增加对claude/codex/pi的环境兼容 ([daf12f4](https://github.com/fancyhq/hotpot/commit/daf12f432accff3f6498d76ca78c229f1e56eb33))
* ✨ 增加配置更新ROADMAP ([5b20dde](https://github.com/fancyhq/hotpot/commit/5b20ddee008dcbedfd9c5d8eb85e0c99f44feefe))
* ✨ 核心功能实现 ([a640ff6](https://github.com/fancyhq/hotpot/commit/a640ff61847f3350b13b4cbf23b9b88c5e3ca4aa))
* ✨ 移除关于 cargo run 的说明 ([29559eb](https://github.com/fancyhq/hotpot/commit/29559ebf8b2015ca213ad9d197be0222f93f1d7c))
* add release binary assets ([5f8ad31](https://github.com/fancyhq/hotpot/commit/5f8ad31ba25079eb4ab6f97badc313216db252ac))
* add release-please workflow ([d191053](https://github.com/fancyhq/hotpot/commit/d191053134699202f048e6b76192a97b1df6555a))
* add update force flag ([e1887cb](https://github.com/fancyhq/hotpot/commit/e1887cbd0a8b50efe0370efdb343b8b0e4a1f668))
* **commands:** add /new command for guided brainstorming ([667dea7](https://github.com/fancyhq/hotpot/commit/667dea7ced5a7291efff0875c49679a5c9581165))
* no change ([7dede29](https://github.com/fancyhq/hotpot/commit/7dede29256ea8527193b123fd39f9e5edbea5c38))
* polish VuePress task page rendering ([33fc631](https://github.com/fancyhq/hotpot/commit/33fc63151867ff796238afbc062426b370f7b380))
* **skills:** add writing-plan skill for /new workflow ([a699f64](https://github.com/fancyhq/hotpot/commit/a699f64c9fb3823b2ff2e2ee2183586e6f618f28))
* 增加兼容，但需修复环境变量问题 ([79d38bc](https://github.com/fancyhq/hotpot/commit/79d38bc60f9640ca3515dbcb81a72e0d078b247f))
* 增加初始化、新建任务、执行任务相关内容 ([5bfcf2e](https://github.com/fancyhq/hotpot/commit/5bfcf2e2456bd2e29222cdb29853a0fe6e43d9f8))


### Bug Fixes

* 🐛 修复创建任务文件不存在问题；修复多语言不生效问题 ([a41acaf](https://github.com/fancyhq/hotpot/commit/a41acafb1bf18e087ed52b3cf88c642dc08804ea))
* 🐛 修复多语言输出问题，每轮增加提示 ([ddd5b6c](https://github.com/fancyhq/hotpot/commit/ddd5b6cff27be0fe60c17f11e92338b6b5626536))
* 🐛 更改 Windows编译 ([1b17acc](https://github.com/fancyhq/hotpot/commit/1b17accccf43246d3ca2363ed51e6bd2191babbf))
* clean test artifacts ([e5d92b7](https://github.com/fancyhq/hotpot/commit/e5d92b7f2264d5ec8f970311f7b2d9de155cb8df))
* guard Pi first tool call ([f8fe8e1](https://github.com/fancyhq/hotpot/commit/f8fe8e1ebf6e68e2b350f13a7644873e9a1b793a))
* harden execute subagent error recovery ([cccfee4](https://github.com/fancyhq/hotpot/commit/cccfee4a6702ce46503fe7ba4c426b9ffbd5cec3))
* harden Pi extension for weak models ([af1487a](https://github.com/fancyhq/hotpot/commit/af1487a6e3c7e6d863a16e6fc28b7f3ada79e3a4))
* harden Pi hotpot-new prompt (partial) ([628f586](https://github.com/fancyhq/hotpot/commit/628f5867d85c1167939bf840b064d2082457465d))
* keep VuePress task URL stem ([ea23990](https://github.com/fancyhq/hotpot/commit/ea239902bc435cb77f782d1c23a3694d69402dab))
* move issue candidates global ([7296869](https://github.com/fancyhq/hotpot/commit/7296869aee0a23b1fc5f1922d239140de82dcf0f))
* move worktree choice to new ([332af39](https://github.com/fancyhq/hotpot/commit/332af39cab2e9c15a76b6641f43b68ed31872cd6))
* pass Pi new arguments ([51bc25b](https://github.com/fancyhq/hotpot/commit/51bc25bf8077dc3f13aa57701ed6b9ec13fc35e9))
* probe vuepress port before emitting url ([2a2938e](https://github.com/fancyhq/hotpot/commit/2a2938ef0d7c79a422836453917d1f24f006b5a9))
* sync Pi new prompt ([cb7fad6](https://github.com/fancyhq/hotpot/commit/cb7fad6a5964a1fa1b33b5d8d8f1e5990586a507))
* sync task Overview status ([519a3d6](https://github.com/fancyhq/hotpot/commit/519a3d6577c8616d9b4a33ec03fac484a466adfb))
* unify VuePress gate via file-existence ([cc0c7f6](https://github.com/fancyhq/hotpot/commit/cc0c7f662350f5f001331df72238f24df7bd0664))
