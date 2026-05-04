# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-05-05

First usable release. 13/13 HUD elements implemented; cold-start under 5ms target (median 3.81ms on Windows 11 / Ryzen 9800X3D, 10-run sample).

### Added

- **`omc-hud` binary** — sub-5ms Rust statusline replacement for the TypeScript HUD upstream
- **10 visible HUD elements**:
  - `context` — `ctx:67%` with severity tiers (green / yellow / red)
  - `context_eta` — `~15m` time-to-context-full via least-squares regression on rolling 36-sample window
  - `model_name` — `Opus 4.7` / `Sonnet 4.6` / `Haiku 4.5` color-coded by tier
  - `prompt_time` — `Ns` / `Nm` / `Nh` / `Nd` elapsed since prompt start (severity by duration)
  - `todos` — `todos:N/M` with progress color (green ≥ 80%, yellow 50-79%, cyan in-progress)
  - `token_usage` — `tok:i12K/o4K` with optional reasoning + session totals
  - `cost` — `$0.42` USD with model-aware pricing table (cents for `<$0.01`)
  - `autopilot` — `ralph:3/10` / `ulw:N/M` / `autopilot` / `team:N workers`
  - `git_status` — `git:(branch*) ~M ?U` via git CLI
  - `rate_limits` — `5h:32% ~2h | 7d:8% ~6d`
- **3 utility elements** (no visible output, integrated into other elements' rendering):
  - `color_degrade` — terminal capability detection (`NO_COLOR` / `FORCE_COLOR` / `COLORTERM` / `TERM`)
  - `cjk_width` — placeholder for future width-aware layouts
  - `i18n` — locale detection with bundled `en` and `zh-CN` string tables
- **Per-session sliding sample cache** at `.omc/state/sessions/<id>/hud-cache.json` with atomic `.tmp + rename` writes
- **`catch_unwind` per element** — a single panic returns `?` placeholder rather than crashing the line
- **204 inline unit tests** across all elements (tests live alongside implementations under `#[cfg(test)] mod tests`)
- **GitHub Actions CI matrix** — `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build --release` on Linux + macOS + Windows
- **Documentation**: `ARCHITECTURE.md` (design rationale, hot-path budget, decision log) and `README.md` (status, roadmap, build instructions)

### Performance

| Metric | Value |
|---|---|
| Cold-start median (idle stdin) | **3.81ms** (10-run sample, Windows 11 / Ryzen 9800X3D) |
| Cold-start in git repo | ~20ms (2× git CLI spawns; cache slot reserved for follow-up) |
| Release binary | **397 KB** (`opt-level=z`, `lto=true`, `panic=abort`, `strip=true`) |
| TypeScript upstream baseline | 390-502ms cold-start (per upstream issue [#2843](https://github.com/Yeachan-Heo/oh-my-claudecode/issues/2843)) |

### Known limitations

- Token / cost / todos / autopilot / rate_limits all read from `Input::hooks_state` (generic JSON blob). Typed `Input` field plumbing is a follow-up; the live integration depends on Claude Code injecting the matching keys
- `git_status` spawns synchronously without a timeout guard — a wedged `git` process wedges this element
- No bar rendering for `context` or `rate_limits` (text-only this release)
- No codachi-style `⇡5%` / `⇣2%` over/under natural-pace indicators on `rate_limits`
- `todos.rs` carries three `let _ = ctx.something;` shims to keep `Strings::todo` / `Input::transcript_path` / `Input::turns` alive across builds. To be revisited when those fields find real consumers or are pruned

### Inspiration & attribution

- Algorithm and UX inspiration from [codachi](https://github.com/vincent-k2026/codachi) (MIT, vincent-k2026); independently re-implemented in Rust, no source code copied
- Reference TypeScript implementation: [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) (Apache 2.0, Yeachan-Heo); only external contracts consumed (Claude Code stdin schema, `~/.claude/settings.json` `statusLine.command` interface, OMC `.omc/state/` path conventions)
- Initial production skeleton authored 2026-05-05 by Codex via the `codex-rescue` agent — attribution preserved in commit [`d813abf`](https://github.com/2233admin/oh-my-claudecode-RS/commit/d813abf)

[0.1.0]: https://github.com/2233admin/oh-my-claudecode-RS/releases/tag/v0.1.0
