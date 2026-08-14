# OMC-RS 平台化内化排期

状态：本轮交付基线已完成。S0--S5 的首批可用切片已经落地并通过 release、
CLI/MCP 消费和全量质量门禁；Hermes/Sentinel 私有宿主运行时与后续 director
mode 保留为需要真实消费契约后再启动的独立增量。

## 目标

把 OMC-RS 做成我们自己的、可测试、可复用的 Agent 工具平台：外部
OMC、OMX、oh-my-pi 以及后续确认的工具只贡献可验证的能力切片；OMC-RS
保留统一契约、路由、状态、记忆、团队调度、宿主适配和 CLI/MCP 消费面。

当前押注：产品边界叫 OMC-RS，用户入口叫 `omc`；Codex、Claude Code、
Hermes、Sentinel 等是宿主或消费者，不把任一宿主的模型循环变成 OMC-RS
的第二套核心。

## 当前基线

- 共享能力和契约在 `crates/omc-shared/src/agent_tool.rs`、
  `crates/omc-shared/src/operation_contract.rs`、
  `crates/omc-shared/src/code_intel.rs`。
- 现有团队调度在 `crates/omc-team/src/dispatch.rs`，已经能把内部状态投影到
  `omc.task.v1` 的 `TaskStatus`，并可附带验证后的 typed subagent result。
- MCP server library 入口是 `crates/omc-mcp/src/server.rs`，统一 CLI 通过
  `omc mcp` 启动；`crates/omc-mcp/src/main.rs` 仅保留兼容启动器。宿主工具在
  `crates/omc-mcp/src/agent_tools.rs`。
- CLI 入口是 `crates/omc-cli/src/commands/mod.rs` 和
  `crates/omc-cli/src/dispatch.rs`。
- 宿主适配已有 `crates/omc-host`，跨工具边界已有 `crates/omc-interop`。
- 当前工具契约见 `docs/agent-tool-contract.md`。

## 外部来源判断

这些判断基于截至 2026-08-13 的官方仓库页面和发布信息：

1. [OMX / oh-my-codex](https://github.com/Yeachan-Heo/oh-my-codex) 是 Codex
   的工作流层，明确保留 Codex 作为执行引擎，重点是
   `deep-interview -> ralplan -> ultragoal`、`.omx/` 计划/日志/状态以及
   skills/agents/hooks。它对原生 Windows 和 Codex App 的支持不是默认路径，
   因此在 OMC-RS 中先吸收协议和工作流，不直接依赖其 tmux/npm runtime。
2. [OMC / oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode)
   是 Claude Code 的 teams-first 编排层；官方文档把 Team 的主流程定义为
   `team-plan -> team-prd -> team-exec -> team-verify -> team-fix`，并区分
   Claude 原生 Team 和 CLI/tmux workers。OMC-RS 应映射到已有 `omc-team` 和
   `omc-host`，而不是再造一套调度器。
3. [oh-my-pi](https://github.com/can1357/oh-my-pi) 更接近完整 coding-agent
   harness：typed subagent 结果、hash-anchored edits、URI 资源、LSP、
   debugger、Python、browser、vibe mode 和插件扩展。它是能力来源，不是
   OMC-RS 的运行时依赖；先拿可跨宿主的 schema、验证和工具边界。
4. `OMY` 当前不是唯一可识别的官方仓库名。暂按用户此前给出的
   `can1357/oh-my-pi` 处理；如果指的是另一个项目，进入 ask-match 后再纳入。

本轮边界审查补充：OMP 的 debugger 是带 action 级权限边界的 DAP session，
browser 则依赖 Puppeteer/CDP、共享 Chromium 和 relay。本轮先实现 debug 的
受控 DAP adapter；browser 暂不搬入，除非 Hermes/Sentinel 提供真实消费场景。
两者都不引入第二 Agent loop、adapter 下载器、浏览器 broker 或上游运行时。

## 分阶段排期

### S0：可正常测试和使用的基线（P0，当前阶段，1--3 个工作日）

状态：已完成。CLI/MCP、setup/doctor、Code Intel 查询、宿主消费 fixture、
release 构建和全量测试均已有证据。

目标：让当前 OMC-RS agent-tool/Code Intel 改动从“代码已接入”变成“开发者
能安装、启动、测试、消费”。

交付：

- release/debug 构建入口和 `omc doctor`/环境诊断，明确 Rust、Code Intel、
  MCP、CLI 的缺失项。
- 一条 CLI smoke：`omc tool capabilities`、`omc tool route`、
  `omc tool code-intel-query`。
- 一条 MCP stdio smoke：initialize、tools/list、tools/call。
- 一个不依赖真实 Hermes/Sentinel 私有代码的宿主消费 fixture：只按
  `omc.tool.v1` 解析成功/失败和 artifact 引用，证明消费者不需要知道 Rust
  内部类型。
- 最小开发文档：安装、运行、示例、失败码、只读边界、如何接入宿主。

退出标准：新机器或干净工作树能按文档完成构建；CLI 和 MCP 都能返回稳定
JSON；Code Intel 查询能返回真实 committed artifact；所有质量门禁通过。

### S1：统一能力面和真实生命周期投影（P0，3--5 个工作日）

目标：把 capabilities、task、event、error、artifact-ref 变成唯一共享面。

交付：

- 所有新工具复用 `omc.tool.v1`，错误码不能靠字符串判断。
- `omc-team` 的真实 dispatch 状态继续投影到 `omc.task.v1`；不新增没有真实
  runtime side effect 的 `task.start`/`task.cancel`。
- 只有已有 state、memory、notepad、routing、team 能力可被宿主发现；避免
  再造 parallel executor、provider registry 或第二套 memory。
- 加入跨 transport contract tests，CLI/MCP 输出对同一 fixture 等价。

退出标准：Hermes/Sentinel 类宿主仅凭 capability discovery 和 JSON schema
即可选择工具；不存在“成功但没有真实状态/产物”的假生命周期。

### S2：OMX / OM Codex 能力内化（P1，约 1 周）

目标：把 Codex 工作流方法变成 OMC-RS 的 skill/plan/goal 资产。

优先吸收：

- `deep-interview` 的问题收敛规则；
- `ralplan` 的计划、架构、批评门禁；
- `ultragoal` 的 durable checkpoint/ledger 思路；
- `ultrawork`/team 的角色分工和验证顺序；
- `.omx/` 下 plans、logs、memory、state 的可迁移布局原则；
- setup/doctor/host guidance 的可验证安装流程。

已完成的第一批：`omc.goal.v1`、项目级 `.omc/state/goals/` durable ledger、
CLI/MCP 的 goal lifecycle 工具，以及 CLI↔MCP 临时项目黑盒恢复测试。目标
状态只负责控制面；真正的 agent/task 执行仍由宿主或现有 `omc-team` 负责。

当前进度：`omc-cli/tests/agent_tool_contract.rs` 已增加真实 Codex 项目黑盒（Windows
由内嵌 PowerShell smoke 驱动）：
临时项目执行 `omc setup --host codex`，经历 clarify/plan/execute/verify，
通过 `hash-edit` 修改源码，用 `rustc` 编译并运行，再以 result validation 和
goal checkpoints 记录证据；测试不把 OMC-RS 伪装成 Agent executor。

不吸收：Codex CLI 的模型调用、tmux 运行器、npm 安装器、插件市场实现。

退出标准：在 Codex 宿主上可以从“澄清 -> 计划 -> 执行 -> 验证”跑通一个真实
小项目；计划和 checkpoint 能被 OMC-RS 读取，且执行仍由宿主/现有 team runtime
负责。

### S3：OMC / OM Claude Code 能力内化（P1，约 1 周）

目标：把 Claude Code 侧成熟的团队流程和宿主配置映射到 OMC-RS。

优先吸收：

- Team staged pipeline 的状态/验收语义；
- role/skill catalog 和自动路由的描述格式；
- hooks、setup、doctor、session/replay 的边界与诊断方法；
- Claude/Codex 宿主配置生成和互操作测试。

不吸收：Claude Code 专有模型循环、npm/plugin marketplace runtime、与 OMC-RS
已有 `omc-team`/`omc-host` 重复的执行器。

当前进度：`omc-host/src/mcp_reg.rs` 已用两个真实 host adapter 注册同一个
`omc-mcp` server；Claude MCP 写项目根 `.mcp.json`，settings/hooks 仍写
`.claude/settings.json`，并以真实 Claude CLI `mcp get`、Codex CLI `mcp list`
和 consumer-owned JSON views 验证
discovery、route、执行前后 event、typed result 和 `omc.task.v1` status；测试
不启动假的 executor/provider runtime。

退出标准：同一个 OMC-RS task contract 能在 Codex 与 Claude Code 两个宿主上
完成 discovery、route、执行前后事件和结果消费；差异只留在 host adapter。

### S4：oh-my-pi / OMP 工具 harness 能力内化（P1，1--2 周）

目标：提取最能提升工程可靠性的工具边界，不搬完整 Agent。

优先顺序：

1. typed subagent result 与 schema validation；
2. hash-anchored edit / patch verification；
3. URI-shaped artifact/resource reference；
4. LSP、Python 和 debug 的 read-only/explicit-side-effect adapter 规范；
   debug 先做 DAP 边界，browser 延后到有真实宿主场景；
5. vibe/director 模式的宿主消费协议。

暂缓：provider abstraction、prompt loop、完整插件市场、与 OMC-RS tool wheels
重复的 in-process coreutils。

当前进度：typed result 已有共享合约、任务投影、CLI/MCP 验证入口和宿主
fixture；hash edit 已有共享原子写入、CLI/MCP 入口、陈旧锚点拒绝和前后
digest 返回；`ArtifactRef` 已有 canonical URI，Code Intel 输出和 goal
checkpoint 会标准化 URI，同时保留 producer/path/snapshot provenance；现有
Code Intel 查询支持 bounded exact URI inspection，命中返回已验证 preview，
不命中边界 fail-closed，不新增通用 raw-byte reader；LSP 第一批以
`lsp_document_symbols` 接入 one-shot `rust-analyzer` 只读 adapter，带路径
越界、超时、响应大小和进程清理边界；阶段化内化第一批以共享
`omc.workflow.v1`/`workflow_advance` 对齐 OMX 的 clarify/plan/goal 和 OMC
的 team-plan/team-verify/team-fix，只返回基于证据的 advance/wait/terminal，
由已有 `omc-team`/宿主执行，不新增 Agent loop。

当前新增：按 OMP `eval` 的边界接入 `omc.python.v1` 的 `python_repl`：MCP
进程内 session-backed 本地 Python kernel、CLI 单进程 fallback、显式
`allowSideEffects` 门控、代码/超时/输出/session 数量上限，以及超时后的
kernel 重启。CLI/MCP consumer 和 Windows 项目 smoke 均验证真实 Python
执行与跨调用变量保留；不引入 Python 包管理、tool bridge、provider 或
第二个 Agent loop。

当前新增：按 OMP debugger 的 DAP 边界接入 `omc.debug.v1` 的 `debug_inspect`：
共享层启动外部 stdio adapter，执行 initialize 与显式 launch/attach，再只允许
threads/stackTrace/scopes/variables/modules/loadedSources/output 只读 action；
4MiB framing、5--300 秒超时、配置完成事件、反向请求拒绝和 launch/attach 清理
均有边界。CLI/MCP 使用同一请求模型，外部 fixture 启动实际测试进程后由
consumer-owned JSON view 消费结果；不负责下载/选择 adapter，不在 OMC-RS 内实现
调试器，不把 breakpoint/continue/evaluate/memory write 伪装成只读工具。

退出标准：至少一个真实 code-intel 或 LSP 能力通过统一 artifact/result schema
被 CLI 和 MCP 消费；编辑类能力必须能拒绝过期 hash，不能只靠模型自报成功。

### S5：打包、发行和项目化使用（P0/P1，贯穿，首轮完成后 2--3 天）

目标：把能力变成团队每天可以使用的工具，而不是研究分支。

交付：

- 单一 `omc` CLI：setup、doctor、capabilities、route、team、MCP 启动。
- Windows 原生路径优先验证；tmux/psmux 作为可选 runtime，不作为基础契约。
- release 构建、版本化 schema、变更日志和最小升级/回滚说明。
- 建立 `examples/host-consumer` 或等价 fixture，供 Hermes、Sentinel 以及
  其他 Agent 做协议回归。
- 任何外部能力进入前先记录来源版本、许可证、映射文件、测试和删除条件。

当前进度：统一 `omc mcp` 已复用 `omc-mcp` library 的同一 stdio server；独立
`omc-mcp` 入口继续保留。CLI integration test 会启动真实进程，验证
`initialize`、`tools/list` 及既有工具注册；release smoke、全量质量门禁和
Sentrux gate 已通过。

本轮补齐：omc setup --host codex|claude 现在把 omc-rs 注册为统一的
omc mcp server；重复运行幂等，冲突配置拒绝覆盖，Claude/Codex 均有 host
adapter 回归，Windows Codex 项目 smoke 会检查真实 config.toml。

本轮 Hermes 接入采用 MCP consumer 适配，而不是新增 HostAdapter、Agent loop
或 Provider：官方 `hermes-agent` 的 `~/.hermes/config.yaml` 使用
`mcp_servers` 映射；`omc setup --host hermes [--hermes-home PATH]` 复用同一
`omc_server_definition()` 写入 `omc-rs -> omc mcp`。临时 Hermes 配置已通过
命令级注册、重复运行幂等、保留 provider/model sibling 和冲突拒绝覆盖验证；
真实 Hermes/Sentinel 私有运行时仍需 ask-match，不在本轮猜测其内部 API。

发行审计结论：`crates/omc-installer` 暂不作为可用安装入口。它目前没有接入
`omc` CLI，agent 定义加载仍返回空集合，写入的 `omc-hook <event>` 也没有对应
的发行入口；把它直接接到用户环境会制造“安装成功但宿主不可用”的假链路。当前
可用路径保持为 release `omc`/`omc-mcp` 二进制 + `omc setup` + `omc doctor`；
只有补齐真实资源打包、hook 可执行入口和干净环境 smoke 后，才重新评估是否保留
该 crate。

下一条 release-qa gate：在干净临时目录用 release `omc` 完成 Claude/Codex
setup、MCP discovery 和一个 tool call，并检查配置中的 `omc mcp` 能被实际找到；
若 PATH/安装位置仍没有稳定契约，先补发行说明或可回收的本地 launcher，不引入
第二套安装器。

该 gate 已在 Windows release binary 上通过：临时目录完成两种 host setup，
`doctor --host codex --json` 为 ready，PATH 解析到当前 release `omc.exe`，
MCP initialize/tools/list/tools/call 均成功，且消费到 `omc.tool.v1`。

本轮新增统一入口：`omc team ...` 只定位同目录或 PATH 中的现有
`omc-team` binary 并透传参数，不复制 team runtime、worker lifecycle 或
调度器。release smoke 已在临时目录通过 `omc team init` 和
`omc team session list`；因此当前单一 CLI 入口的 team 消费链路是真实可用的。

本轮 OMP 研究结论：官方 `/vibe` 是 director/后台 worker/session 持久化
语义，不是一个可直接复制的路由标签。当前不新增 `vibe_*` 工具或第二套
执行循环；先复用 `omc-team`，等 Hermes/Sentinel 给出真实 session 消费契约
后，再决定是否落地 director mode。

本轮补齐外部消费回归：新增 `tests/host-consumer/consumer.py`，只用标准库
通过 release `omc` 验证 CLI capabilities、稳定失败码，以及 MCP
initialize/tools-list/tools-call。它不导入 OMC-RS Rust 类型，已在 Windows
release binary 上实际通过；Hermes/Sentinel 私有宿主接入仍保留为 ask-match。

本轮再补 `omc.team-observability.v1`：复用 `omc-team` 已有 sessions/top/doctor
读取逻辑，统一暴露 CLI `team-observability` 和 MCP `team_observability`；
Windows release fixture 已验证 15 项 capability、真实空 session/top 快照和
嵌套 schema。该切片只观察已有状态，不新增 start/cancel 或第二个 runtime。
质量信号：shared 的 Sentrux gate 通过；team/mcp/cli 出现小幅 coupling drift
（cycles=0、unresolved imports=0），属于引入共享 team 观察依赖后的 advisory
结构变化，不影响 workspace test、clippy 或 release smoke。

本轮再补发布 bundle：`scripts/package-omc.ps1` 复用已构建的 `omc.exe`、
`omc-mcp.exe`、`omc-team.exe`，生成 `omc.release-bundle.v1` manifest 和
SHA-256 校验，并对包内 `omc --version`、capability discovery、`omc team`
透传做 smoke。包级 host-consumer fixture 已通过；当前选择相邻二进制 bundle，
不把既有 `omc-team` runtime 重写进 CLI，也不重新启用尚未具备真实资源打包和
hook 入口的 installer crate。

本轮补齐 Code Intel 的权威消费验证：已确认机器上的 v0.7.1 release 因
manifest reconciliation 的 33 个 stale registry findings 不能作为 authority，
没有绕过该失败；改用源码仓库 release/v0.7.x-rc 构建的 v0.7.2 binary，先通过
`orchestrate Validate`，再以 `omc-rs-src-v081` 发布完整 committed run。原生
`artifact query`、`change impact` 均返回 `runOutcome=completed`、当前 snapshot；
Windows JSON-only host consumer 同时通过 release `omc` CLI 和 `omc-mcp` 消费
同一 `code_evidence.agent_slice`。这样 Code Intel 是真实外部 authority，OMC-RS
只做 adapter 和契约校验，不复制 scanner、index 或 artifact verifier。

本轮补齐真实 Hermes 宿主消费：固定官方 `NousResearch/hermes-agent`
`v2026.8.3`（源码版本 0.20.0）并在临时 uv 环境运行；`omc setup --host hermes`
生成 `mcp_servers.omc-rs` 后，官方 `hermes mcp list` 和 `hermes mcp test omc-rs`
均成功，历史 release stdio 握手发现 30 个 OMC 工具。测试未写入用户 Hermes 目录、未使用
模型凭据；同时把 Windows fallback 对齐官方 `%LOCALAPPDATA%\\hermes`，保留
`HERMES_HOME` 和显式 `--hermes-home` 覆盖。Hermes 仍只是 MCP consumer，不引入
第二套 Agent loop 或 Provider。

本轮也修正 Claude MCP 的真实配置边界：官方 Claude Code 项目级 MCP 配置是
根目录 `.mcp.json`，而 `.claude/settings.json` 继续只承载 settings/hooks。
`omc setup --host claude` 的临时项目 smoke 已由 Claude CLI `mcp get omc-rs`
识别为 Project config；Codex CLI 的 `.codex/config.toml` 注册保持不变。

本轮再接入 `omc-interop` 的只读观察切片：统一 CLI `interop-snapshot` 和 MCP
`interop_snapshot` 共用 `omc.interop.snapshot.v1`，读取 OMC shared tasks/messages
以及 OMX team config/tasks，带记录上限并报告当前 interop mode；不发送任务/消息、
不更新状态、不创建第二套调度器。Windows JSON-only host consumer 已同时验证
CLI 与 MCP 的 envelope、schema 和 `readOnly` 标记。本轮再补 `normalizedTasks`，把
OMC/OMX 原生状态映射为统一的 pending/blocked/in_progress/completed/failed 超集，
同时保留 native ID、source/team 元数据。主动 bridge 仍等待真实 Hermes/
Sentinel session/task 契约与 ask-match；本轮已把已有 shared-state task/message
writer 接入 `omc.interop.bridge.v1`，要求 `source`/`target`、`allowSideEffects` 和
三项 active 环境门，CLI/MCP 均已用临时目录验证真实文件写入，且不启动 worker。
当前 release bundle 的 MCP catalog 为 32 个工具；JSON-only host consumer 已同时
验证 bridge 的 denied 与 active-written 两条路径。

本轮 hooks 审计结论：上游 OMC/OMX 的 native hook 入口只是 lifecycle script
dispatch，不是一个可单独复制的空 CLI；真实行为还依赖各宿主 stdin schema、脚本目录
和失败/超时策略。因此本轮只修正 `HookRegistry` 对 Claude 项目配置的路径错误
（`.claude/settings.json`），保留现有 unified event mapping；不新增一个没有真实
宿主输入输出契约的 `omc hook` runner。依据：[OMC hooks reference](https://github.com/Yeachan-Heo/oh-my-claudecode/blob/main/docs/REFERENCE.md#hooks-system)、
[OMX native hook mapping](https://github.com/Yeachan-Heo/oh-my-codex/blob/main/docs/codex-native-hooks.md)。

## Agency / 子任务分配

每个 agency 只领取一个边界，主线由 OMC-RS 维护者整合：

| Lane | 负责内容 | 必须交回的证据 |
|---|---|---|
| upstream-research | 官方仓库、版本、许可证、能力清单 | 来源链接、commit/tag、可迁移/不可迁移结论 |
| contract-core | shared schema、错误码、事件、artifact、compat tests | Rust 单测、JSON fixtures、破坏性变更说明 |
| host-integration | Codex/Claude/Hermes/Sentinel 消费适配 | CLI/MCP e2e、宿主输入输出样例 |
| runtime-mapping | omc-team/state/memory/host 映射 | 状态投影测试、无重复核心的边界说明 |
| release-qa | build、doctor、安装、Windows、回归和文档 | 干净工作树命令记录、失败诊断、发布清单 |

禁止一个 agency 同时改契约、runtime、宿主配置和发布脚本；每个 lane 的
结果先回到 contract-core/release-qa 做整合验证。

当前派发锚点：所有 agency 任务先创建一个 `omc.goal.v1` goal，领取时关联
`dispatchTaskId`，每个阶段只提交 checkpoint；阻塞必须写入 blocker，未有
测试/消费证据不得标记 completed。

## Ask-match 决策点

这些不阻塞 S0，但进入对应阶段前必须确认：

1. `OMY` 是否就是 `can1357/oh-my-pi`/`omp`；如果不是，提供准确仓库地址。
2. 产品公开名称是否继续是 `OMC-RS`，CLI 是否固定为 `omc`；当前不引入
   `omx`/`omy` 兼容别名。
3. Hermes 与 Sentinel 的第一批真实宿主优先级；当前先做宿主中立 fixture，
   再接真实仓库，避免猜内部 API。
4. 是否允许引入外部运行时依赖；当前默认“不引入 npm runtime”，优先 Rust
   原生和受控外部 CLI adapter。

## 总体验收

- `cargo fmt --check`、`cargo clippy --workspace -- -D warnings`、
  `cargo test --workspace --no-fail-fast` 通过。
- release binary 可启动，doctor 能明确报告依赖与配置状态。
- CLI/MCP 的 capabilities、route、Code Intel query 有真实成功和失败样例。
- 至少两个宿主类型消费同一份 `omc.tool.v1` fixture；宿主不依赖 OMC-RS
  内部 Rust 类型。
- 每个内化能力都有来源、边界、测试、回滚/删除条件。
- OMC-RS 内没有第二套 Agent loop、Provider、tool wheel、plugin marketplace，
  也没有无法验证副作用的生命周期 API。

## Stop / escalate 条件

- 外部项目许可证或依赖边界不清：暂停导入实现，只保留研究记录。
- 需要真实宿主私有仓库、凭据或生产环境才能验证：提交 ask-match，不猜测。
- 新能力要求修改统一契约：先做版本/兼容性评审，不在 agency 分支直接扩展。
- Windows 与 tmux 语义无法同时满足：保留宿主无关 JSON/CLI/MCP 契约，runtime
  差异下沉到 adapter，不牺牲基础可用性。
