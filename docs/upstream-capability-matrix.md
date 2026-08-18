# 外部能力内化矩阵

这份矩阵是 agency 领取任务前的边界真相。`Keep` 表示已有实现优先复用；
`Adapt` 表示只吸收协议/方法；`Gap` 表示需要新切片；`Reject` 表示不进入
OMC-RS 核心。

## 来源锁定（2026-08-13）

这里的版本是研究和能力映射的基准，不是 OMC-RS 的运行时依赖。真正导入
代码或资产前必须重新核对上游 release、commit 和许可证；只保留可验证的
协议、边界和测试，不复制上游运行时。

| 来源 | 基准 release/tag | commit | 许可证证据 | 本仓库决定 |
|---|---|---|---|---|
| [oh-my-codex](https://github.com/Yeachan-Heo/oh-my-codex) | `v0.20.5` | [`27b3a91`](https://github.com/Yeachan-Heo/oh-my-codex/releases/tag/v0.20.5) | MIT（上游 [`package.json`](https://raw.githubusercontent.com/Yeachan-Heo/oh-my-codex/main/package.json) 的 `license` 字段；根 `LICENSE` 路径未找到） | 只吸收 workflow/skill/state 方法；不引入 npm、Codex provider 或 OMX runtime |
| [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) | `v4.15.10` | [`115ed1f`](https://github.com/Yeachan-Heo/oh-my-claudecode/releases/tag/v4.15.10) | MIT（上游 [`LICENSE`](https://raw.githubusercontent.com/Yeachan-Heo/oh-my-claudecode/main/LICENSE)） | 映射已有 `omc-team`/`omc-host`；不复制 Claude/npm/plugin runtime |
| [oh-my-pi](https://github.com/can1357/oh-my-pi) | `v17.2.15` | [`06aecdd`](https://github.com/can1357/oh-my-pi/releases/tag/v17.2.15) | MIT（上游 [`LICENSE`](https://raw.githubusercontent.com/can1357/oh-my-pi/main/LICENSE)） | 只吸收 typed result、hash edit、URI、只读 LSP 等已验证 adapter |

如果 `OMY` 不是 `can1357/oh-my-pi`，上表第三行只视为暂定来源；在用户确认
准确仓库前，不从其他项目导入代码或设计结论。

## OMX / OM Codex

来源：[Yeachan-Heo/oh-my-codex](https://github.com/Yeachan-Heo/oh-my-codex)

| 外部能力 | OMC-RS 对应面 | 判断 | 验收 |
|---|---|---|---|
| `deep-interview` | `crates/omc-skills/src/templates/deep-interview.md`、`omc-shared` state tools、`omc.workflow.v1` | Keep/Adapt | 状态可恢复，最终 spec 有明确 acceptance criteria；阶段决策不启动执行器 |
| `ralplan` | `crates/omc-skills/src/templates/ralplan.md`、`omc-team/src/agents/ralplan.rs`、`workflow_advance` | Keep/Adapt | 计划、架构、批评和 approval gate 可被宿主消费 |
| `ultrawork` | `omc-skills/src/templates/ultrawork.md`、已有 task graph/team runtime | Adapt | 只保留并行分工和验证协议，不复制执行器 |
| `ultragoal` durable ledger | `omc.goal.v1`、`.omc/state/goals/`、`GoalLedger` | Adapt/P1 slice done | CLI/MCP 能创建、恢复、阻塞、checkpoint、完成；执行仍由宿主/team 负责 |
| `setup` / `doctor` | `omc-cli`、`omc-host` | Keep | `omc setup`、`omc doctor --json` 在临时项目通过 |
| tmux/psmux runtime | `omc-team` host spawn | Adapt | 只作为可选 worker adapter，Windows 无 tmux 时基础契约仍可用 |
| Codex provider/model loop | OMC-RS routing 只输出 semantic tier | Reject | 不能出现第二套 provider 或模型调用循环 |

## OMC / OM Claude Code

来源：[Yeachan-Heo/oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode)

| 外部能力 | OMC-RS 对应面 | 判断 | 验收 |
|---|---|---|---|
| `team-plan -> team-prd -> team-exec -> team-verify -> team-fix` | `omc.workflow.v1`、`crates/omc-skills/src/templates/team.md`、`omc-team` task graph/phase controller | Keep/Adapt | 阶段状态由宿主证据推进，验证失败能回到 fix；不启动第二套执行器 |
| role/skill catalog | `omc-skills` templates、`omc-team` role router | Keep | 同一 catalog 能被 Codex/Claude host adapter 使用 |
| hooks | `crates/omc-hooks`、`omc-host` unified hooks | Keep | 事件映射有 host-specific 输出和回归测试；实际 lifecycle script/宿主入口仍需真实 Hermes/Sentinel 场景后再接 |
| Claude/Codex CLI worker | `omc-host`、`omc-team` spawn directives | Adapt | worker 是宿主进程，不成为 OMC-RS provider 核心；`omc-host/src/mcp_reg.rs` 验证两宿主消费同一 discovery/route/event/result 契约 |
| session/replay/HUD | `omc-team` observability、`omc-hud`、`team_observability` | Keep/Adapt/P1 slice | CLI/MCP 只读投影真实 sessions/top/doctor；不依赖模型自报，不创建伪 task lifecycle |
| npm/plugin marketplace runtime | Rust CLI/MCP/skills 已有入口 | Reject | 不复制市场和安装运行时；只允许受控资源导入 |

## oh-my-pi / OMP

来源：[can1357/oh-my-pi](https://github.com/can1357/oh-my-pi)

这里先按用户此前提供的仓库理解 `OMY`；若用户指另一个项目，必须重新确认
来源和许可证。

| 外部能力 | OMC-RS 对应面 | 判断 | 验收 |
|---|---|---|---|
| typed subagent result | `operation_contract.rs` 的 `TypedSubagentResult` + `omc-team` result projection + CLI/MCP validator | Adapt/P1 slice done | schema 校验失败可见，宿主不解析 prose |
| hash-anchored edit | `omc-shared::hash_edit` + CLI/MCP `hash_edit` | Adapt/P1 slice done | 过期 hash 明确拒绝，成功结果带前后文件 digest |
| URI-shaped resources/artifacts | `ArtifactRef.uri`、Code Intel/goal checkpoint normalization and bounded URI inspection | Adapt/P1 slice done | `omc://artifact/sha256/<digest>` must match digest; legacy path-only refs remain readable; provenance is preserved; no duplicate raw-byte reader |
| LSP/code intelligence | Code Intel adapter + `lsp_document_symbols` one-shot rust-analyzer adapter | Adapt/P1 slice | CLI/MCP 返回统一 `omc.tool.v1`，路径、超时、消息大小和副作用边界可验证 |
| debugger | `omc.debug.v1` + `debug_inspect` 的外部 stdio DAP adapter | Adapt/P2 slice done | 显式 launch/attach target；只读 action；5--300 秒超时、4MiB framing、反向请求拒绝、退出清理和真实 host consumer 验证；不下载或托管 debugger adapter |
| Hermes MCP consumer | `omc setup --host hermes` + existing `omc mcp` | Adapt/P1 slice | native Windows `%LOCALAPPDATA%\\hermes\\config.yaml` and POSIX `~/.hermes/config.yaml` register idempotently, preserve siblings, and reject conflicting `omc-rs`; Hermes remains the host's loop/provider |
| OMC↔OMX interop | existing `omc-interop` readers/writers + CLI/MCP `interop_snapshot`/`interop_bridge` | Adapt/P1 slice done | bounded read-only snapshot plus `omc.interop.bridge.v1` explicit task/message writes; active flags and `allowSideEffects` required; no worker start, second scheduler, or fake lifecycle |
| browser | 暂不接入 | Gap/defer | OMP 的 Puppeteer/CDP/共享 Chromium/relay 依赖独立运行时；除非有真实 Hermes/Sentinel 场景，否则不引入浏览器 broker、Chromium 下载器或 relay |
| Python | `omc-python` session-backed subprocess + `python_repl` CLI/MCP adapter | Adapt/P2 slice done | `omc.python.v1`，明确 side-effect gate、session scope、超时重启和真实跨调用状态；不引入 upstream eval/tool bridge |
| vibe/director mode | routing + task delegation | Adapt | 宿主可选择 director 模式，不能创建第二 Agent loop |
| in-process coreutils/tool wheels | `omc-shared`/`omc-mcp` 现有工具 | Reject/Reuse | 先复用现有工具，除非有基准证明需要新增 |
| provider/interactive prompt loop | OMC-RS host-neutral surface | Reject | OMC-RS 不接管宿主模型循环 |
| plugin marketplace | OMC-RS skills/host registration | Reject | 不引入重复插件市场 |

## Agency 领取规则

每个任务必须包含：

1. 来源 URL、版本/tag/commit 和许可证确认；
2. 上表中的目标模块和 `Keep/Adapt/Gap/Reject` 决定；
3. 最小实现范围与明确 non-goals；
4. 单元测试、跨进程或宿主消费测试；
5. 删除条件：如果不能证明真实 side effect、artifact provenance 或宿主消费，
   不进入主线。

## 当前优先级

1. **P0**：统一 contract、setup/doctor、CLI/MCP、Code Intel、宿主 fixture。
2. **P1**：OMX 的 durable goal/checkpoint ledger、OMC Team 阶段投影，以及
   OMP 的 typed result/hash edit/URI artifact 第一批已完成；当前已补齐
   `omc.workflow.v1`/`workflow_advance` 的 clarify-plan-execute-verify-fix
   决策边界、LSP 第一批 `lsp_document_symbols` 和 Python `python_repl`
   adapter；debug 已接入受控 DAP adapter，browser 仍暂缓。
3. **P2**：browser 只有在 Hermes/Sentinel 给出真实消费场景后再排期，
   随后做现有 LSP/Python/debug adapter 的性能优化。
   当前互操作已落地只读 `interop_snapshot` 和显式 gated `interop_bridge`；
   Hermes/Sentinel 的宿主 session/task 消费契约仍需真实场景 ask-match。
4. **明确不做**：第二 Agent loop、第二 Provider 层、第二套 tool wheels、插件
   市场、无真实 runtime 的 task lifecycle API。
