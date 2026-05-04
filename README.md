# oh-my-claudecode-RS

Rust rewrite of [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode). Performance and maintainability over the TS upstream.

> **Sub-5ms cold-start, statically-linked Rust binary, single-author.**
> No npm, no node_modules, no upstream churn dependency.

## Status

**Pre-alpha — skeleton ready.**

| Metric | Value |
|---|---|
| Cold-start (median, 5 runs, Win11/9800X3D) | **3.84ms** ✓ (target <5ms; TS upstream issue #2843 reports 390-502ms) |
| Binary size (release, opt-z + lto + strip) | 330 KB |
| Render pipeline | end-to-end wired (stdin → cache → render → stdout → save) |
| Elements implemented | 1 / 13 (Context ETA — full LSE regression) |
| Elements stubbed | 12 / 13 (skeletons in place, returning `None`) |
| Tests | 0 (TDD lands per-element) |
| GitHub remote | not yet pushed |

See [ARCHITECTURE.md](ARCHITECTURE.md) for design rationale, hot-path budget, and decision log.

## Why

- **Performance**: TS HUD spawn-per-render hits 390-502ms cold-start. Rust target: <5ms. **Achieved: 3.84ms.**
- **Memory**: OMC's bun + haiku MCP stack is 663MB. Sibling `omc-hub-rs` already proved 7.4MB Rust replacement; same playbook for the rest.
- **Maintenance**: Single-author Rust fork = full control, no upstream churn dependency, no npm dep hell.
- **Decoupling**: Upstream's design and maintainer style are immaterial here.

## Architecture (one paragraph)

Cargo workspace, edition 2024, mimalloc allocator, sync hot path (no tokio in MVP). Single binary today: `omc-hud` (statusline, replaces upstream HUD). 13 elements rendered via enum-match dispatch with `catch_unwind` per element so a single panic returns `"?"` instead of crashing the line. Cache lives in `.omc/state/sessions/<id>/hud-cache.json`, atomic-rename writes. See [ARCHITECTURE.md](ARCHITECTURE.md).

## Sibling project

- **[omc-hub-rs](https://github.com/2233admin/omc-hub-rs)** — independent MCP server replacement (already shipped, v0.1.0+, 7.4MB binary). Lives separately. **Will not be absorbed.** This monorepo will reference it as a runtime dependency when needed.

## Roadmap

| Phase | Crate | Description | Status |
|-------|-------|-------------|--------|
| 0 | `omc-hud` | Statusline (Context ETA / color degrade / CJK width / i18n / stats) | 🚧 in progress |
| 1 | `omc-shared` | Common config / state-path / protocol utilities | ⚪ planned |
| 2 | `omc-hooks` | Hook engine (PostToolUse, SessionStart, etc.) | ⚪ planned |
| 3 | `omc-cli` | Top-level commands (autopilot / ralph / ultrawork) | ⚪ planned |
| 4 | `omc-team` | Multi-agent orchestration | ⚪ planned |

## Build (when implementation lands)

```bash
git clone https://github.com/2233admin/oh-my-claudecode-RS.git
cd oh-my-claudecode-RS
cargo build --release
# binary at target/release/omc-hud (.exe on Windows)
```

## Inspiration credits

Independent re-implementation, no code copying. See [ARCHITECTURE.md § Inspiration](ARCHITECTURE.md#inspiration--attribution) for full attribution table.

- [codachi](https://github.com/vincent-k2026/codachi) (MIT, vincent-k2026) — Context ETA concept, CJK width awareness, terminal-degrade idea
- [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) (Apache 2.0, Yeachan-Heo) — stdin schema, element catalog, OMC state path conventions
- Codex via `codex-rescue` agent — initial production skeleton (commit `d813abf`)

## License

MIT — see [LICENSE](LICENSE).
