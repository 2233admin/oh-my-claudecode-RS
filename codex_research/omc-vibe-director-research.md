# OMP Vibe / Director 研究记录

日期：2026-08-13

## 来源

- [OMP Vibe mode 官方文档](https://github.com/can1357/oh-my-pi/blob/main/docs/vibe-mode.md)
- [OMP task 官方文档](https://github.com/can1357/oh-my-pi/blob/main/docs/tools/task.md)
- [OMP session 官方文档](https://github.com/can1357/oh-my-pi/blob/main/docs/session.md)
- [OMP magic keywords 官方文档](https://github.com/can1357/oh-my-pi/blob/main/docs/magic-keywords.md)
- [OMP 官方 releases](https://github.com/can1357/oh-my-pi/releases)
- [OMP LICENSE](https://raw.githubusercontent.com/can1357/oh-my-pi/main/LICENSE)

## 结论

`/vibe` 不是一个简单的路由标签，也不是新的 agent 核心。它把顶层交互会话变成 director，把工作交给可持久化的后台 worker；director 的工具集收窄为读操作、可选的 todo 和 worker 控制，worker 继续使用搜索、编辑、执行、构建能力。模式与 worker 状态写入 session，恢复时重新载入。

因此 OMC-RS 不应直接复制 `vibe_spawn` 等工具或再造一套执行循环。当前最小映射是：

1. 继续复用 `omc-team` 的生命周期、任务图、worker health、通信和 runtime 启动能力。
2. 通过统一 `omc` CLI 暴露 `team` 入口，入口只做进程桥接，不新增调度器。
3. 等 Hermes/Sentinel 的真实消费契约明确后，再决定是否需要持久化 director/session mode；在此之前只保留 `tool route` 的渐进式路由。

## 当前来源状态

截至本记录日期，官方 release 页面显示最新版本为 `v17.2.15`，提交为 `06aecdd`；项目许可证为 MIT。release notes 仍在修复 `/vibe` 的工具集与 session mode 行为，说明该能力的生命周期细节仍应以版本锁定后的官方契约为准，不宜只抄名称。

## 对 OMC-RS 的边界

- 纳入：director/worker 的职责分离、后台任务可恢复、worker 状态可观测、单一入口消费已有 `omc-team`。
- 暂不纳入：OMP 的 provider/model 体系、`workflowz` eval kernel、`vibe_*` 独立工具集、`omp compress` 等与当前 OMC-RS 契约重复或缺少真实宿主消费者的能力。
- 验收：统一 `omc team ...` 能调用当前真实 `omc-team` runtime；release binary 在干净临时目录完成 init/session smoke；不能只靠模板输出或静态 help 证明完成。

