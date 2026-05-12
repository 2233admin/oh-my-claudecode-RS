# Architecture

`oh-my-claudecode-RS` is a from-scratch Rust rewrite of [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode). No upstream source code is consumed; only external contracts are re-implemented (Claude Code stdin schema, `~/.claude/settings.json statusLine.command`, OMC state path conventions).

## Goals

1. **Sub-5ms cold-start.** Upstream TS HUD spawns Node per render and reports 390-502ms. Measured: **3.81ms median** on Windows 11 / Ryzen 9800X3D.
2. **Static binary, no npm/node.** Single executable, zero runtime dependencies.
3. **Modular crates, independent subsystems.** Each crate compiles and tests in isolation. Dropping one never breaks the others.
4. **Never crash the host process.** Statusline must not block, panic-out, or hang Claude Code.
5. **Multi-agent orchestration for Claude Code.** Team coordination, DAG-based task graphs, file-based IPC between agents.

## Non-goals

- GUI / web interface -- terminal-only by design.
- Plugin marketplace / runtime extensions -- build-time composition only.
- 1:1 TS upstream parity -- we keep what earns its cold-start cost; we drop what doesn't.
- Cross-language interop with TS upstream -- this is a clean break, not a transpile.

## Workspace layout

```
oh-my-claudecode-RS/
├── Cargo.toml             workspace root, edition 2024, opt-z + lto + strip
├── crates/
│   ├── omc-shared/        core: routing, tools, state, memory, config
│   ├── omc-host/          host abstraction: Claude + Codex adapters
│   ├── omc-team/          orchestration: DAG, lifecycle, communication, governance
│   ├── omc-hooks/         hook system: 15 events, executor, registry
│   ├── omc-skills/        skill system: 40 templates, registration, host filtering
│   ├── omc-mcp/           MCP: tool registry, protocol routing
│   ├── omc-hud/           HUD: 13 elements, i18n, cache
│   ├── omc-context/       context: AGENTS.md, rules injection
│   ├── omc-git-provider/  git: 6 providers (gh/glab/tea/az/bb)
│   ├── omc-notifications/ notify: Slack, tmux
│   ├── omc-wiki/          wiki: ingest, query, lint
│   ├── omc-autoresearch/  research: types, PRD, runtime
│   ├── omc-interop/       interop: MCP bridge, OMX state
│   ├── omc-installer/     installer: config, updater
│   ├── omc-python/        python: REPL interface
│   ├── omc-macros/        macros: #[derive(Tool)]
│   ├── omc-xcmd/          x-cmd: integration
│   └── omc-cli/           cli: 26+ command dispatch
└── tests/macro-tests/     integration tests for proc macros
```

18 crates + 1 integration test target. All under one `[workspace]` with shared `[profile.release]` settings.

## Dependency graph

```
                    omc-shared  (foundation: config, state, types)
                   /    |    \       \       \
                  /     |     \       \       \
          omc-host  omc-team  omc-hooks  omc-context  omc-hud
             |         |
             v         v
        omc-cli    omc-skills
             \       /
              v     v
           omc-mcp   omc-git-provider
              |
              v
         omc-interop  omc-autoresearch  omc-wiki
              |
              v
    omc-notifications  omc-installer  omc-python  omc-xcmd
```

`omc-shared` is the foundation crate. Currently all crates depend only on external
workspace dependencies (serde, tokio, etc.) -- inter-crate links will solidify as
the workspace matures. `omc-macros` is a proc-macro crate depended on at build time
only. `tests/macro-tests` validates the macro output.

## Key design decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Rust edition | 2024 | Latest language features, edition 2024 compatibility |
| Allocator (HUD) | mimalloc | 0.32ms saving over system allocator on Windows |
| Async runtime (HUD) | none (sync) | One-shot stdin-to-stdout path; tokio init costs ~1ms with zero benefit |
| Async runtime (other crates) | tokio | Long-running servers and concurrent I/O need it |
| Agent IPC | file-based JSONL | inbox/outbox files; no network, no shared memory, no kernel deps |
| Shared state | `Arc<RwLock>` | Thread-safe shared mutable state without async overhead |
| Serialization | serde + serde_json | Universal across all crates, JSON for IPC |
| Error handling (libs) | thiserror | Typed, structured error enums |
| Error handling (apps) | anyhow | Ergonomic error propagation in binaries |
| HUD error policy | catch_unwind per element | One element panic returns "?" placeholder; rest renders normally |
| HUD always exits 0 | `exit(0)` in main | Statusline must never block or fail visibly to Claude Code |
| Build profile | opt-level "z", lto, strip, panic=abort | Smallest possible binary, fastest cold-start |
| i18n | compile-time string tables | Zero runtime cost; adding a locale = adding a file + recompile |

## Data flow

How a mission flows through the full system:

```
User
 |
 v
CLI (omc-cli)
 |
 v
Routing (omc-shared)
 |
 v
Phase Controller (omc-team)
 |
 v
Task Graph (DAG in omc-team)
 |
 v
Dispatch (omc-team)
 |
 +---> Communication (omc-team: JSONL inbox/outbox)
 |        |
 |        v
 |     Worker agents (file-based IPC)
 |        |
 |        v
 |     Health Monitor (omc-team)
 |
 +---> Usage Tracker (omc-shared)
 |
 +---> Notifications (omc-notifications: Slack, tmux)
```

HUD hot path (separate lifecycle):

```
stdin JSON (~1KB)
  -> parse -> cache load -> sample -> render 13 elements -> stdout
  -> cache save (atomic rename) -> exit 0
```

## Patterns adopted from competitors

| Pattern | Source | Used in |
|---------|--------|---------|
| Context ETA (LSE regression) | [codachi](https://github.com/vincent-k2026/codachi) | omc-hud |
| CJK / terminal width handling | codachi via unicode-width crate | omc-hud |
| Statusline element catalog | [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) | omc-hud |
| `.omc/state/` path conventions | oh-my-claudecode | omc-shared |
| stdin schema contract | Claude Code | omc-hud, omc-interop |
| `settings.json statusLine.command` | Claude Code | omc-hud |
| File-based agent IPC (JSONL) | [SWE-agent](https://github.com/princeton-nlp/SWE-agent) | omc-team |
| DAG task orchestration | [CrewAI](https://github.com/crewAIInc/crewAI) | omc-team |
| Hook event system | [Husky](https://github.com/typicode/husky) | omc-hooks |
| Skill templates | [LangChain tools](https://github.com/langchain-ai/langchain) | omc-skills |

## Open questions

- **Cache locking:** Claude Code serializes statusline per session, but unverified across CC releases.
- **Inter-crate dependency solidification:** As crates mature, omc-shared will become the true dependency hub.
- **i18n migration trigger:** At what locale count to move from compile-time tables to build.rs + JSON. Currently 2 locales, threshold at 5+.
