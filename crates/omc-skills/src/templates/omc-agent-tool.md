---
name: omc-agent-tool
description: Use the OMC-RS host-neutral agent tool contract for capability discovery and progressive task routing.
hosts: [claude, codex]
protocol_version: "1.0"
---

# OMC-RS Agent Tool

Use OMC-RS as a capability and routing layer. Do not reimplement an agent loop
inside the host.

## Progressive routing

1. Call `agent_capabilities` once when the integration starts.
2. Keep simple reads and focused edits in the current host.
3. Call `agent_route` before work that may cross files, require architecture
   decisions, or has already failed.
4. Use the returned semantic `tier` and `recommendedSurface`; never depend on
   a provider-specific model ID.
5. Use existing `state_*`, `notepad_*`, and `project_memory_*` tools for durable
   context. They are already part of the OMC-MCP surface.
6. Treat `python_repl` as an explicit-side-effect tool, never as a sandbox. It
   requires `allowSideEffects=true` for execution or session mutation; use it
   only when the host has separately approved local code execution.

## CLI fallback

When MCP is unavailable:

```text
omc tool capabilities
omc tool route --task "..." --agent-type executor --previous-failures 0
```

Both commands return the `omc.tool.v1` JSON envelope. Treat `error.code` as the
machine-readable branch key and `error.message` as display text.

The current tool surface recommends orchestration; it does not pretend to
start or cancel a runtime. Those lifecycle operations will be added only when
they are backed by the real `omc-team` runtime service.
