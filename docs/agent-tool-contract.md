# OMC-RS Agent Tool Contract

## Scope

`omc-rs` is the host-neutral operations surface consumed by Hermes, Sentinel,
Codex, and other Agent systems. It owns capability discovery and progressive
routing. The host continues to own its model loop and native tools.

The first version deliberately does not expose fake `task.start` or
`task.cancel` operations. Those operations require a real runtime service and
must be added only when their side effects can be verified.

## Transports

- MCP stdio: `omc mcp` (preferred unified entry; `omc-mcp` remains the
  compatibility binary), tools `agent_capabilities`, `agent_route`,
  `code_intel_artifact_query`, `lsp_document_symbols`, `workflow_advance`,
  `subagent_result_validate`, `hash_edit`, and
  the durable `goal_*` tools, plus the explicitly gated `python_repl` and
  `debug_inspect` tools, read-only `interop_snapshot`, and explicitly gated
  `interop_bridge`.
- JSON CLI: `omc tool capabilities`, `omc tool route ...`,
  `omc tool code-intel-query ...`, `omc tool result-validate ...`, and
  `omc tool hash-edit ...`, `omc tool python-repl ...`,
  `omc tool debug-inspect ...`, and `omc tool interop-snapshot ...`.
  The durable bridge is `omc tool interop-bridge ...`.
- Goal CLI: `omc goal create|list|show|start|block|checkpoint|complete`.
- MCP startup: `omc mcp` owns the stdio process and reuses the same tool
  registry as the `omc-mcp` compatibility binary.
- `omc setup --host codex|claude` registers the unified host entry. Claude MCP
  uses the project-root `.mcp.json` contract; `.claude/settings.json` remains
  the settings/hooks surface. Codex MCP uses `.codex/config.toml`. `omc setup
  --host hermes [--hermes-home PATH]` registers the same `omc-rs -> omc mcp`
  stdio server in Hermes' `config.yaml` under `mcp_servers`; it does not add a
  second agent loop or provider.
  An existing matching entry is left unchanged and a
  conflicting entry fails closed.
  Without `--hermes-home`, native Windows follows Hermes' `%LOCALAPPDATA%\\hermes`
  default, while POSIX/WSL follows `~/.hermes`; `HERMES_HOME` remains an explicit
  override.
- Skill guidance: `omc-agent-tool`.

The agent capability, routing, and Code Intel surfaces use the same
`omc.tool.v1` response envelope:

```json
{
  "schema_version": "omc.tool.v1",
  "request_id": "sentinel-123",
  "ok": true,
  "data": {}
}
```

Errors use stable machine-readable codes:

```json
{
  "schema_version": "omc.tool.v1",
  "request_id": "sentinel-123",
  "ok": false,
  "error": {
    "code": "invalid_request",
    "message": "task must not be empty"
  }
}
```

## Operations

### `agent_capabilities`

Read-only discovery. No required arguments. Returns the OMC-RS capability
catalog and protocol version. Each of the 16 capability records also lists its
exact MCP tool names and runtime availability (`available`, `unavailable`, or
`conditional`). Dependency-backed capabilities report the resolved executable
path or a machine-readable reason, so hosts can avoid invoking unavailable
Python, Code Intel, rust-analyzer, or team adapters. DAP remains conditional
because its adapter command is supplied per request.

The catalog in `omc-shared::capability_catalog` is the single source of truth.
An MCP contract test requires the 32 registered tools to match it exactly.

### `agent_route`

Read-only routing. MCP arguments:

```json
{
  "task": "redesign the repository architecture",
  "agentType": "architect",
  "previousFailures": 1,
  "requestId": "sentinel-123"
}
```

The result contains a semantic tier (`LOW`, `MEDIUM`, `HIGH`), a model role,
confidence, reasons, and a recommended surface. Provider-specific model IDs
are intentionally excluded.

### `workflow_advance`

Read-only staged workflow decision. It maps the portable part of OMX/OMC's
clarify/plan/execute/verify/fix flow to one contract. The host or existing
`omc-team` runtime supplies the evidence; this tool never starts an agent,
creates a task, or persists a lifecycle record.

Stages are `initializing`, `clarifying`, `planning`, `executing`, `verifying`,
`fixing`, `paused`, `completed`, and `failed`. The result returns `advance`,
`wait`, or `terminal`; `wait` means the evidence is insufficient and must not
be treated as progress.

CLI example:

```bash
omc tool workflow-advance --current-stage planning \
  --all-tasks-assigned --plan-approved
```

MCP arguments use the same fields in camelCase:

```json
{
  "currentStage": "verifying",
  "allTasksCompleted": true,
  "verificationPassed": true,
  "requestId": "hermes-workflow-1"
}
```

The result is `omc.workflow.v1` inside the normal `omc.tool.v1` envelope.

### `team_observability`

This is a read-only projection of the existing `omc-team` runtime. It does not
start, cancel, or mutate a team. The `view` is one of `sessions`, `top`, or
`doctor`; `workingDirectory` selects the project containing `.omc/team`.
The nested payload is versioned as `omc.team-observability.v1` and is returned
inside the normal `omc.tool.v1` envelope.

CLI example:

```bash
omc tool team-observability --view sessions --root .
```

MCP example:

```json
{
  "view": "top",
  "workingDirectory": "C:/work/project",
  "requestId": "sentinel-team-top-1"
}
```

The payload is an observation of existing session/usage files. Empty state is
reported as an empty snapshot rather than an invented running task.

### `interop_snapshot`

Read-only OMC/OMX interoperability observation. It reuses the existing
`omc-interop` readers for `.omc/state/interop` and `.omx/state/team`; it does
not start workers, send messages, update task status, or mark mailboxes read.
The nested payload is versioned as `omc.interop.snapshot.v1`, bounded by
`limit` (1--100), and reports `directWriteEnabled` so a host can see whether
the separately gated active bridge is enabled without invoking it.
Its `normalizedTasks` projection maps OMC and OMX native statuses to the
portable `pending`/`blocked`/`in_progress`/`completed`/`failed` superset while
retaining native IDs and source/team metadata; native task records remain
available in `sharedTasks` and `omxTeams`.

CLI example:

```bash
omc tool interop-snapshot --root . --limit 20
```

MCP example:

```json
{
  "workingDirectory": "C:/work/project",
  "limit": 20,
  "requestId": "sentinel-interop-1"
}
```

### `interop_bridge`

Send one explicit task or message between OMC and OMX through the existing
shared-state writer. The payload is versioned as
`omc.interop.bridge.v1`; `source` and `target` are required and must differ.
Every write requires both `allowSideEffects: true` and the process flags
`OMX_OMC_INTEROP_MODE=active`, `OMX_OMC_INTEROP_ENABLED=1`, and
`OMC_INTEROP_TOOLS_ENABLED=1`. It writes a durable record only; it does not
start workers, schedule tasks, or expose `task.start`/`task.cancel`.

MCP example:

```json
{
  "action": "send_message",
  "source": "omc",
  "target": "omx",
  "content": "Please inspect the failing adapter",
  "workingDirectory": "C:/work/project",
  "allowSideEffects": true,
  "requestId": "sentinel-bridge-1"
}
```

### `code_intel_artifact_query`

Read-only repository intelligence. The adapter delegates to the released
`code-intel artifact query` command and returns its result plus normalized OMC
artifact references. It never runs a scan, writes an artifact root, changes a
repository, or reimplements Code Intel's index/verification rules.

MCP arguments:

```json
{
  "repo": "code-intel-pipeline",
  "artifactRoot": "C:/Users/example/AppData/Local/code-intel/artifacts",
  "artifactType": "code_evidence.agent_slice",
  "artifactUri": "omc://artifact/sha256/<digest>",
  "limit": 10,
  "requestId": "sentinel-124"
}
```

Normalized artifact references use `omc.artifact-ref.v1`:

```json
{
  "schemaVersion": "omc.artifact-ref.v1",
  "producer": "code-intel-pipeline",
  "artifactSchema": "agent-code-slice-ranking.v1",
  "artifactType": "code_evidence.agent_slice",
  "path": "objects/sha256/<digest>",
  "sha256": "<64-hex-digest>",
  "consumedSnapshotIdentity": "<snapshot-identity>",
  "uri": "omc://artifact/sha256/<digest>"
}
```

The URI is content-addressed and must match sha256 exactly. Existing persisted
references without uri remain valid on input; Code Intel output and goal
checkpoints normalize them to the canonical URI while preserving producer,
path, and snapshot provenance. OMC-RS does not provide a general URI resolver
or copy the upstream agent runtime.

When `artifactUri` is supplied, the same read-only query surface performs a
bounded exact inspection: it asks Code Intel for its maximum 100-match page,
filters by the verified artifact digest, and returns only matching previews and
normalized refs. A miss at the page boundary is reported as
`upstream_query_truncated` instead of being presented as a false not-found.
This is an inspection/preview contract, not a raw-byte reader.

### `debug_inspect`

This is the first OMP debugger-derived adapter. The protocol boundary follows
the [Debug Adapter Protocol base protocol](https://microsoft.github.io/debug-adapter-protocol/overview):
OMC-RS starts one externally supplied stdio adapter, sends `initialize`, then an
explicit `launch` or `attach` request, and finally one read-only inspection
request. Adapter-specific launch/attach arguments remain opaque JSON owned by
the caller; OMC-RS does not install, select, or implement a debugger.

`allowSideEffects: true` is mandatory because launch/attach starts or connects
to a real debug session. Supported actions are `threads`, `stackTrace`,
`scopes`, `variables`, `modules`, `loadedSources`, and `output`. Breakpoints,
continue/step, evaluate, memory writes, and reverse requests are outside this
first contract. The adapter is disconnected before returning; launch sessions
request debuggee termination, while attach sessions leave the debuggee running.

The transport bounds each DAP message to 4 MiB, adapter argument count to 128,
and request timeouts to 5--300 seconds. Timeout and adapter failures are
returned as machine-readable `debug_timeout`, `adapter_unavailable`, or
`upstream_failed` errors inside `omc.tool.v1`; successful data is tagged
`omc.debug.v1` and includes observed events and declared side effects.

MCP example:

```json
{
  "adapterCommand": "codelldb",
  "adapterArgs": ["--stdio"],
  "workingDirectory": "C:/work/project",
  "mode": "launch",
  "action": "threads",
  "launchArguments": {"program": "C:/work/project/target/debug/app"},
  "allowSideEffects": true,
  "requestId": "sentinel-debug-1"
}
```

CLI JSON arguments use the same fields:

```bash
omc tool debug-inspect --adapter-command codelldb \
  --adapter-args-json '["--stdio"]' --mode launch --action threads \
  --root . --launch-arguments '{"program":"target/debug/app"}' \
  --allow-side-effects
```

The first real regression uses an external stdio DAP fixture that launches an
actual test process, then a consumer-owned JSON view. It does not claim that a
fixture provides debugger semantics; those remain the supplied adapter's
responsibility.

### `lsp_document_symbols`

Read-only Rust document symbols through `rust-analyzer`. The direct CLI adapter
is one-shot; the long-running MCP process keeps a bounded project-scoped pool
(maximum 4 projects, 10-minute idle TTL) and evicts a session after a transport
or request failure.
The adapter accepts a project-relative file, canonicalizes both the project
root and file, rejects traversal/symlink escapes, bounds each JSON-RPC message
to 4 MiB, and enforces a 5--60 second timeout. It sends no edit, rename,
formatting, or persistent-server operation and returns `sideEffects: []`.

CLI:

```bash
omc tool lsp-document-symbols --root . --file crates/omc-shared/src/lib.rs
```

MCP arguments:

```json
{
  "workingDirectory": "C:/work/omc-rs",
  "file": "crates/omc-shared/src/lib.rs",
  "timeoutMs": 20000,
  "requestId": "sentinel-lsp-1"
}
```

The response includes `serverProcessId` and `sessionReused` so an MCP consumer
can verify reuse without depending on Rust types. Server discovery,
configuration, background indexing management, and write-capable LSP
operations remain outside the contract.

### `python_repl`

This is the first executable OMP-derived adapter. The source boundary is the
upstream [eval tool contract](https://github.com/can1357/oh-my-pi/blob/main/docs/tools/eval.md):
language selection is explicit, cells run in order, and Python state persists
inside a kernel session. OMC-RS keeps only the local subprocess/session
boundary; it does not import the upstream agent loop, tool bridge, or provider
runtime.

The MCP server keeps sessions for its process lifetime. The CLI keeps a session
only for the current invocation, so it is a fallback for one cell rather than a
cross-invocation kernel. `projectDir` must already exist. Python execution is
not a sandbox: `execute`, `reset`, and `interrupt` require
`allowSideEffects: true`; code may read/write files, spawn processes, or use
the network according to the local Python environment. `get_state` is
read-only but only works for an existing session.

MCP example:

```json
{
  "action": "execute",
  "sessionId": "sentinel-python-1",
  "projectDir": "C:/work/project",
  "code": "value = 41",
  "allowSideEffects": true,
  "requestId": "sentinel-python-1"
}
```

CLI example:

```bash
omc tool python-repl --action execute --session-id cli-python \
  --root . --code "print(6 * 7)" --allow-side-effects
```

The adapter uses `python`/`python3` from `PATH`, or `OMC_PYTHON_COMMAND` when
set. Code is capped at 256 KiB, execution at 300 seconds, output at 4 MiB,
and the process-local session store at 16 sessions. A timeout restarts the
kernel before returning `execution_timeout`; it never leaves a possibly busy
interpreter attached to the next call. Memory fields are best-effort and are
`0` when the platform does not expose a measurement.

### `goal_*`

The goal ledger is the cross-host control-plane anchor for long-running work.
It persists project goals under `.omc/state/goals/`, supports planned/active/
blocked/completed states, and records checkpoints with optional normalized
artifact references. It does not start agents or claim task execution.

The MCP tools are:

- `goal_create`
- `goal_list`
- `goal_get`
- `goal_start`
- `goal_block`
- `goal_checkpoint`
- `goal_complete`

The CLI emits the same goal JSON records:

```bash
omc goal create --id internalize-omx --objective "absorb portable workflow capabilities"
omc goal start --id internalize-omx
omc goal checkpoint --id internalize-omx --checkpoint-id s0 --summary "setup and host doctor verified"
omc goal show --id internalize-omx
```

### `subagent_result_validate`

Validates a structured result envelope without starting an agent or provider.
The payload must be a JSON object, `resultType` is the named schema ID, and
`requiredFields` checks top-level fields:

```bash
omc tool result-validate --result-type omc.agent.findings.v1 \
  --payload '{"summary":"done"}' --required-fields summary
```

The result envelope is `omc.subagent-result.v1`; prose output is not treated as
a typed result.

### `hash_edit`

Applies one contiguous, project-relative line replacement only after every
provided line SHA-256 anchor (and optional whole-file digest) matches. A stale
anchor returns `stale_edit` and leaves the file untouched. Successful writes
use the existing temporary-file-plus-rename atomic-write pattern and return
before/after file digests.

MCP calls provide `workingDirectory`, `path`, `startLine`, `endLine`, an
`anchors` array, and `replacement`. The CLI equivalent is:

```bash
omc tool hash-edit --root . --path src/lib.rs --start-line 1 --end-line 1 \
  --anchors-json '[{"line":1,"sha256":"<64-hex-digest>"}]' \
  --replacement 'replacement line'
```

### Task, event, and error contracts

The shared Rust types freeze `omc.task.v1`, `omc.event.v1`, and the error
shape inside `omc.tool.v1`. Task and event types carry correlation IDs and
observed timestamps. A task status may carry verified artifact references and a
typed `omc.subagent-result.v1` result, and a machine-readable error. They are
control-plane contracts; they do not imply that a host runtime has been started.

`omc-team` projects its internal dispatch states into `TaskStatus`. It does
not claim that a task is externally observable or cancellable unless a real
runtime adapter reports that state.

## Consumer rule

Consumers should call native host tools first. Use OMC-RS routing when the task
crosses files, needs a role decision, or has failed. A `HIGH` result recommends
the `omc-team` surface; it does not itself start a runtime.

## External consumer fixture

[`tests/host-consumer/consumer.py`](../tests/host-consumer/consumer.py) is a
JSON-only consumer smoke. It imports no OMC-RS Rust types and verifies the CLI
capabilities/failure envelope, team observation, and MCP `initialize`,
`tools/list`, and `agent_capabilities`. Supplying `--artifact-root` additionally
verifies the real `code_intel_artifact_query` through both CLI and MCP, including
committed/current Code Intel evidence:

```bash
python tests/host-consumer/consumer.py \
  --omc target/release/omc \
  --artifact-root "$CODE_INTEL_ARTIFACT_ROOT" \
  --repo omc-rs-src \
  --repo-path .
```

The fixture proves transport and contract consumption only; it is not a
replacement for Hermes/Sentinel private-host integration or their agent loop.

The current Windows validation uses the Code Intel source release v0.7.2 and
the indexed publication `omc-rs-src-v081`. The installed v0.7.1 binary was not
used as an authority because its manifest reconciliation reported stale
registry findings; OMC-RS does not bypass that check. With the v0.7.2
publication, both CLI and MCP return `authority.status=committed`,
`runOutcome=completed`, and `freshness.status=current` for the same artifact
snapshot.

### Release bundle

The supported Windows release shape is a small adjacent-binary bundle. Build and
verify it with:

```powershell
.\scripts\package-omc.ps1 -Build
python tests/host-consumer/consumer.py --omc target/omc-package/omc.exe
```

The script writes `target/omc-package/omc-bundle.json` with
`omc.release-bundle.v1`, entrypoints, and SHA-256 checksums. The bundle contains
`omc.exe` as the unified CLI/MCP entrypoint, `omc-team.exe` for the existing
`omc team` process bridge, and `omc-mcp.exe` as the compatibility MCP entrypoint.
The adjacent `omc-team.exe` is intentional: it reuses the existing team runtime
and avoids embedding a second scheduler in `omc.exe`.

Run the release discovery performance budget with:

```powershell
python tests/host-consumer/benchmark.py --omc target/omc-package/omc.exe
```

It measures cold CLI discovery and warm discovery through a persistent MCP
process, while also enforcing the 16-capability/32-tool catalog size.

Measure the project-scoped LSP cold start and warm reuse path through a real
MCP process with:

```powershell
python tests/host-consumer/lsp_benchmark.py --omc target/omc-package/omc.exe
```

The benchmark verifies that warm calls retain the same `rust-analyzer` process,
report `sessionReused=true`, and remain inside the configured warm P95 budget.

The host-side envelope fixtures live in
`crates/omc-shared/tests/agent_tool_consumer.rs`. They deliberately deserialize
into consumer-owned views, so a Hermes/Sentinel adapter only needs the JSON
contract. Run them with:

```bash
cargo test -p omc-shared --test agent_tool_consumer
```

CLI/MCP 的同请求 envelope 回归测试位于
`crates/omc-cli/tests/agent_tool_contract.rs`，运行：

```bash
cargo test -p omc-cli --test agent_tool_contract
```

Windows 上的测试会自动调用内嵌 PowerShell 黑盒，创建并清理临时项目。
其中也会验证 Python 的副作用门控和真实 CLI 执行；MCP 的跨调用 session
状态由同一测试进程中的 `python_repl` consumer fixture 验证；DAP 的 stdio
和 host-consumer 回归位于 `crates/omc-shared/tests/dap_debug_contract.rs`。

Claude/Codex 宿主注册和 discovery → route → event → result 消费回归测试位于
`crates/omc-host/src/mcp_reg.rs`，运行：

```bash
cargo test -p omc-host --lib mcp_reg
```

Codex 临时项目黑盒测试位于
`crates/omc-cli/tests/agent_tool_contract.rs` 的
`codex_project_smoke_runs_clarify_plan_execute_verify`。它会启动真实 `omc`
进程，执行 setup、goal/checkpoint、workflow、hash-edit 和 result validation，
并用 `rustc` 编译运行修改后的项目：

```bash
cargo test -p omc-cli --test agent_tool_contract
```
