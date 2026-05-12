# oh-my-claudecode-RS

Rust rewrite of [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) -- multi-agent orchestration, hooks, skills, MCP routing, statusline, and context injection for Claude Code.

[![CI](https://github.com/2233admin/oh-my-claudecode-RS/actions/workflows/ci.yml/badge.svg)](https://github.com/2233admin/oh-my-claudecode-RS/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](CHANGELOG.md)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-816-brightgreen.svg)](#build--test)
[![Cold-start](https://img.shields.io/badge/cold--start-3.81ms-brightgreen.svg)](#performance)

## What is OMC-RS

A single-author Rust rewrite that replaces the TypeScript oh-my-claudecode stack. 17 crates, 42K+ lines, 816 tests, and a sub-5ms HUD cold-start -- no npm, no `node_modules`, no upstream churn dependency.

| Metric | Value |
|---|---|
| Crates | 17 |
| Lines of Rust | 42,000+ |
| Tests | 816 |
| HUD cold-start median | 3.81ms (Win11, Ryzen 9800X3D) |
| Binary size (release, HUD) | 397 KB |
| Edition | Rust 2024, rustc 1.85+ |

## Quick Start

```bash
git clone https://github.com/2233admin/oh-my-claudecode-RS.git
cd oh-my-claudecode-RS
cargo build --release
```

Wire the HUD into Claude Code via `~/.claude/settings.json`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "/absolute/path/to/target/release/omc-hud"
  }
}
```

Basic CLI usage:

```bash
cargo run -p omc-cli -- --help
cargo run -p omc-team -- init
cargo run -p omc-team -- start ./task.md --team-size 3
```

## Architecture

```
omc-shared          (foundation -- types, config, routing, resilience)
  |
  +-- omc-macros    (proc macros: #[derive(Tool)])
  |
  +-- omc-hooks, omc-skills, omc-context, omc-hud, omc-mcp,
      omc-git-provider, omc-notifications, omc-wiki, omc-interop,
      omc-xcmd, omc-autoresearch, omc-python
      (mid-layer crates, depend on omc-shared)
  |
  +-- omc-team       (orchestration -- DAG, lifecycle, comms, governance)
  |
  +-- omc-cli        (top-level binary dispatcher)
  +-- omc-installer  (standalone install/update)
```

### Crate Reference

| Crate | Lines | Tests | Description |
|---|---|---|---|
| omc-shared | 7,931 | 100 | Types, config, routing, tools, state, memory, resilience (circuit breaker) |
| omc-team | 14,218 | 234 | Agent orchestration -- DAG task graph, 8-state lifecycle FSM, priority scheduler, governance, fault tolerance |
| omc-hud | 4,449 | 205 | Statusline binary -- 13 elements, context ETA, color degrade, CJK width, i18n |
| omc-mcp | 2,732 | 11 | MCP tool server -- JSON-RPC over stdio, tool registry |
| omc-interop | 2,399 | 43 | Cross-tool interoperability layer (OMC/OMX communication) |
| omc-hooks | 1,853 | 58 | Claude Code hooks integration (15 event types) |
| omc-skills | 1,729 | 31 | Skills loader and executor (38 templates) |
| omc-git-provider | 1,463 | 10 | Git hosting provider abstraction (GitHub, GitLab, Gitea, Bitbucket, Azure DevOps) |
| omc-notifications | 1,292 | 22 | Multi-platform lifecycle notification system |
| omc-autoresearch | 1,218 | 10 | Autoresearch runtime, orchestrator, and PRD management |
| omc-wiki | 1,213 | 4 | Wiki knowledge layer |
| omc-context | 1,119 | 7 | Context injection, rules injection, AGENTS.md management |
| omc-cli | 559 | 11 | CLI dispatcher (26+ commands) |
| omc-installer | 578 | 9 | Installation, update, and configuration management |
| omc-python | 335 | 2 | Persistent Python REPL execution environment |
| omc-macros | 378 | 0 | Proc macros for oh-my-claudecode-RS |
| omc-xcmd | 243 | 0 | x-cmd integration (skills, tools, status) |

## Key Features

- **3-tier model routing** -- haiku/sonnet/opus dispatch based on task complexity
- **DAG task graph + priority scheduler** -- dependency-aware task execution in omc-team
- **8-state agent lifecycle FSM** -- spawn, idle, busy, waiting, blocked, done, failed, terminated
- **3-layer fault tolerance** -- retry, circuit breaker, fallback per agent
- **File-based agent communication** -- inbox/outbox pattern under `.omc/team/`
- **Governance + sentinel gates** -- policy enforcement before agent actions
- **`#[derive(Tool)]` proc macro** -- zero-boilerplate MCP tool definitions
- **ReasoningBank learning memory** -- persistent agent experience store
- **CircuitBreaker resilience** -- configurable failure thresholds with automatic recovery

## Build & Test

```bash
cargo build --release                # optimized binary
cargo test --workspace               # all 816 tests
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

## Contributing

1. Fork the repository
2. Create a feature branch from `dev`
3. Make your changes
4. Ensure all tests pass: `cargo test --workspace`
5. Ensure clippy is clean: `cargo clippy --workspace -- -D warnings`
6. Open a pull request against `dev`

Follow [conventional commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`.

## License

MIT -- see [LICENSE](LICENSE).

Independent re-implementation, no source code copying. See [CHANGELOG.md](CHANGELOG.md) and [ARCHITECTURE.md](ARCHITECTURE.md) for attribution details.
