# oh-my-claudecode-RS

[![CI](https://github.com/2233admin/oh-my-claudecode-RS/actions/workflows/ci.yml/badge.svg)](https://github.com/2233admin/oh-my-claudecode-RS/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](CHANGELOG.md)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Cold-start](https://img.shields.io/badge/cold--start-3.81ms-brightgreen.svg)](#performance)
[![Tests](https://img.shields.io/badge/tests-204-brightgreen.svg)](#testing)
[![Edition](https://img.shields.io/badge/rust-2024-orange.svg)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0.html)

Rust rewrite of [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode). Performance and maintainability over the TS upstream.

> **Sub-5ms cold-start, statically-linked Rust binary, single-author.**
> No npm, no `node_modules`, no upstream churn dependency.

## Status

**0.1.0 — first usable release.** All 13 HUD elements implemented; cold-start under 5ms target.

| Metric | Value |
|---|---|
| Cold-start median (10 idle runs, Win11 / Ryzen 9800X3D) | **3.81ms** ✓ (target `<5ms`; TS upstream issue [#2843](https://github.com/Yeachan-Heo/oh-my-claudecode/issues/2843) reports 390-502ms) |
| Cold-start in git repo (2× git CLI spawns) | ~20ms |
| Binary size (release, `opt-z` + `lto` + `strip` + `panic=abort`) | **397 KB** |
| Render pipeline | end-to-end wired (stdin → cache → render → stdout → save) |
| Elements visible | **10 / 13** (context, context_eta, model_name, prompt_time, todos, token_usage, cost, autopilot, git_status, rate_limits) |
| Elements utility (no visible output) | **3 / 13** (color_degrade, cjk_width, i18n) |
| Tests (inline `#[cfg(test)]`) | **204 passing** |
| Lint / fmt / clippy `-D warnings` | clean |
| GitHub remote | [public](https://github.com/2233admin/oh-my-claudecode-RS), CI matrix Linux + macOS + Windows |

See [ARCHITECTURE.md](ARCHITECTURE.md) for design rationale, hot-path budget, and decision log. See [CHANGELOG.md](CHANGELOG.md) for the full 0.1.0 deliverables and known limitations.

## Why

- **Performance.** TS HUD spawn-per-render hits 390-502ms cold-start ([upstream #2843](https://github.com/Yeachan-Heo/oh-my-claudecode/issues/2843)). Rust target: `<5ms`. **Achieved: 3.81ms median.**
- **Memory.** OMC's bun + haiku MCP stack is 663MB. Sibling [`omc-hub-rs`](https://github.com/2233admin/omc-hub-rs) already proved a 7.4MB Rust replacement for that layer; same playbook applied to the rest of OMC.
- **Maintenance.** Single-author Rust fork = full control, no upstream churn dependency, no npm dep hell.
- **Decoupling.** Upstream's design and maintainer style are immaterial here.

## Architecture (one paragraph)

Cargo workspace, edition 2024, [mimalloc](https://crates.io/crates/mimalloc) global allocator, sync hot path (no tokio in MVP — tokio runtime init costs ~1ms cold-start with zero benefit on a non-concurrent path). Single binary today: `omc-hud` (statusline, replaces upstream HUD). 13 elements rendered via `enum` + `match` dispatch with `catch_unwind` per element so a single panic returns `"?"` instead of crashing the line. Cache lives at `.omc/state/sessions/<id>/hud-cache.json` with atomic `.tmp + rename` writes. See [ARCHITECTURE.md](ARCHITECTURE.md).

## Sibling project

- **[omc-hub-rs](https://github.com/2233admin/omc-hub-rs)** — independent MCP server replacement (already shipped, v0.1.0+, 7.4MB binary replacing 663MB bun + haiku). Lives separately. **Will not be absorbed.** This monorepo will reference it as a runtime dependency when needed.

## Roadmap

| Phase | Crate | Description | Status |
|-------|-------|-------------|--------|
| 0 | `omc-hud` | Statusline (Context ETA / color degrade / CJK width / i18n / 13 elements) | ✅ **0.1.0 released** |
| 1 | `omc-shared` | Common config / state-path / protocol utilities | ⚪ planned |
| 2 | `omc-hooks` | Hook engine (PostToolUse, SessionStart, etc.) | ⚪ planned |
| 3 | `omc-cli` | Top-level commands (autopilot / ralph / ultrawork / team) | ⚪ planned |
| 4 | `omc-team` | Multi-agent orchestration | ⚪ planned |

## Build

```bash
git clone https://github.com/2233admin/oh-my-claudecode-RS.git
cd oh-my-claudecode-RS
cargo build --release
# binary at target/release/omc-hud (.exe on Windows)
```

Wire into Claude Code via `~/.claude/settings.json`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "/absolute/path/to/oh-my-claudecode-RS/target/release/omc-hud"
  }
}
```

## Testing

```bash
cargo test --workspace          # 204 tests, all green on master
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

## Inspiration credits

Independent re-implementation, no source code copying. See [CHANGELOG.md § Inspiration](CHANGELOG.md#inspiration--attribution) and [ARCHITECTURE.md § Inspiration](ARCHITECTURE.md#inspiration--attribution) for the full attribution table.

- [codachi](https://github.com/vincent-k2026/codachi) (MIT, vincent-k2026) — Context ETA concept, CJK width awareness, terminal-degrade idea
- [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) (Apache 2.0, Yeachan-Heo) — stdin schema, element catalog, OMC state path conventions
- Codex via `codex-rescue` agent — initial production skeleton (commit [`d813abf`](https://github.com/2233admin/oh-my-claudecode-RS/commit/d813abf))

## License

MIT — see [LICENSE](LICENSE).
