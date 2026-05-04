# Architecture

`oh-my-claudecode-RS` is a from-scratch Rust rewrite of [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode). **No upstream source code is consumed**; we re-implement against external contracts only (Claude Code stdin schema, `~/.claude/settings.json statusLine.command` interface, OMC state path conventions).

## Goals

1. **Sub-5ms cold-start.** Upstream TS HUD spawns Node per render and reports 390-502ms ([upstream issue #2843](https://github.com/Yeachan-Heo/oh-my-claudecode/issues/2843)). Rust target: <5ms. **Measured: 3.84ms median on Windows 11 / Ryzen 9800X3D.**
2. **Static binary.** No runtime, no `node_modules`, no npm. Single executable distributable.
3. **Modular.** Each subsystem is a separate Rust crate. Dropping one element never breaks the others.
4. **Never crashes the host.** Statusline must not block, panic-out, or hang the Claude Code UI.

## Non-goals

- GUI / web interface — terminal-only by design
- Plugin marketplace / runtime extensions — build-time composition only
- 1:1 feature parity with upstream TS — we keep the elements that earn their cold-start cost; we drop what doesn't
- Cross-language interop with the TS upstream — this is a clean break, not a transpile

## Workspace layout

```
oh-my-claudecode-RS/
├── Cargo.toml          workspace root, edition 2024, opt-z + lto + strip
└── crates/
    └── omc-hud/        statusline binary (current MVP)
        ├── Cargo.toml
        └── src/
            ├── main.rs         entry — sync runtime, mimalloc allocator
            ├── input.rs        stdin JSON parse
            ├── cache.rs        per-session sliding window, atomic IO
            ├── terminal.rs     color capability detection
            ├── i18n.rs         static locale string tables
            ├── render.rs       element dispatch + line composition
            └── elements/       per-element renderers (13)
                ├── mod.rs                 enum + dispatch + catch_unwind
                ├── context.rs             ctx:67%
                ├── context_eta.rs         ~15m (LSE regression)
                ├── token_usage.rs         tok:i12K/o4K
                ├── model_name.rs          Opus 4.7
                ├── git_status.rs          git:(main*) ~3 ?1
                ├── todos.rs               todos:5/8
                ├── autopilot.rs           ralph:3/10 / ulw / autopilot
                ├── rate_limits.rs         5h:32% ⇡5% ~2h
                ├── cost.rs                $0.42
                ├── prompt_time.rs         18s
                ├── color_degrade.rs       (utility, no visible output)
                └── cjk_width.rs           (utility, used during render)
```

### Planned sibling crates

| Crate | Description | Status |
|-------|-------------|--------|
| `omc-shared` | config loader, state path resolver, protocol types — extracted when first consumer arrives | ⚪ planned |
| `omc-hooks` | pre/post-tool-use, session-start/end, replaces TS hook engine | ⚪ planned |
| `omc-cli` | autopilot / ralph / ultrawork commands | ⚪ planned |
| `omc-team` | multi-agent orchestration | ⚪ planned |

### External sibling

[`omc-hub-rs`](https://github.com/2233admin/omc-hub-rs) ships independently as the MCP server replacement (v0.1.0+, 7.4MB binary replacing 663MB bun+haiku). This monorepo will reference it as a runtime/installed-binary dependency when first consumer arrives, **not** absorb it into the workspace.

## Hot path

```
stdin (JSON, ~1KB from Claude Code)
  ↓
parse to Input
  ↓
load cache (.omc/state/sessions/<session_id>/hud-cache.json)
  ↓
record current sample → sliding context window (max 36)
  ↓
detect locale + color level (env vars only, no IO)
  ↓
render 13 elements (catch_unwind each, "?" placeholder on panic)
  ↓
join with " | " separators
  ↓
write stdout + newline
  ↓
save cache (atomic: write .json.tmp, rename)
  ↓
exit 0
```

Total target: 5ms cold-start. Measured: median 3.84ms over 5 runs.

### Cold-start budget

| Stage | Estimated cost | Source |
|-------|----------------|--------|
| Windows PE loader | ~3.79ms | Codex consult, unverified by independent profile |
| mimalloc init | ~0.67ms over empty binary | Codex consult, unverified |
| stdin read (1KB) | sub-ms | sync io::stdin().read_to_string |
| serde_json parse | negligible | <1KB JSON |
| element render × 13 | sub-ms total | most return None / short string |
| stdout write | sub-ms | direct |
| cache rename | sub-ms | NTFS metadata-only |
| **Total measured median** | **3.84ms** | 5-run sample |

Budget headroom validated; individual stages **not** independently profiled — those numbers come from the Codex architecture consult and are best treated as ballpark.

## Decisions (with rationale)

### Async runtime: **none (sync)**

Hot path is one-shot: stdin once, two file IO ops, exit. Tokio runtime init costs ~1ms cold-start with **zero benefit** on a non-concurrent path. The sibling `omc-hub-rs` uses tokio because it's a long-running stdio server — entirely different lifecycle.

### Allocator: **mimalloc**

System allocator on Windows measures 4.56ms; mimalloc 4.24ms (Codex measurement). 0.32ms saving is real and statusline runs every few seconds, so it pays off. `#[global_allocator] static ALLOC: mimalloc::MiMalloc`.

### Element dispatch: **enum + match**

```rust
enum Element { Context, ContextEta, TokenUsage, ... }  // closed set of 13
fn render_element(e: Element, ctx: &RenderContext) -> Option<String> {
    match e { Element::Context => context::render(ctx), ... }
}
```

`Vec<Box<dyn Element>>` with vtable lookup is measurably slower on cold-start and gains nothing here. **Plugin extensibility is explicitly a non-goal.** A new element = a new Rust file + a new enum arm + recompile. Anyone uncomfortable with this constraint should fork.

### Error policy: **never crash, never block**

- Each element's `render()` runs inside `catch_unwind(AssertUnwindSafe(...))`. Panic → returns `"?"` placeholder; the rest of the line still renders.
- Top-level `run()` returns `Result<(), String>`. Any error is logged via `eprintln!` to stderr.
- `main` always `exit(0)`.
- Cache write failures are logged but never bubble up. Losing one cache update is acceptable; blocking the statusline is not.

### Cache: **per-session JSON, atomic write**

- Path: `<cwd>/.omc/state/sessions/<session_id>/hud-cache.json` (mirrors OMC TS conventions for compatibility / interop).
- Format: serde JSON. Parse cost is negligible at <1KB.
- Write: serialize to `<file>.json.tmp`, then `fs::rename`. Survives crash mid-write on POSIX and Windows (NTFS rename is atomic).
- **No locking.** Claude Code calls statusline serially per session by contract. Concurrent corruption is `Not-tested`.

### i18n: **static string tables (compile-time)**

Locale detected once per render from `LANG` / `LC_ALL` env vars. Strings are `&'static str` slices in compiled tables. **No JSON parse at runtime, no filesystem read for locale.** Adding a locale = adding a Rust file + recompiling.

We accept this rigidity in exchange for zero runtime cost. If locale count ever exceeds ~10 with overlap, migrating to a `build.rs` reading `locales/*.json` is low-risk.

### Color capability: **detected per-render**

`NO_COLOR` / `FORCE_COLOR` / `TERM` / `COLORTERM` env reads cost nanoseconds. No caching needed. Yields one of `ColorLevel::{None, Sixteen, TwoFiftySix, TrueColor}`. Used by every element to gate ANSI escape generation.

### CJK / emoji width: **unicode-width crate**

East Asian Wide / Fullwidth / Emoji code points occupy 2 terminal cells. We use the [`unicode-width`](https://crates.io/crates/unicode-width) crate (vetted, no_std-friendly) instead of hand-rolling Unicode tables. Codachi's `width.ts` was the inspiration; the algorithm in our binary is the crate's, not codachi's.

### Element rendering side effects

`elements::*::render()` is **read-only**. All mutation (cache updates, sample appending) happens in `cache::record_context()` *before* the render pass. This keeps the render hot path purely functional and inexpensive to test.

## Inspiration & attribution

| Source | License | Role | Code copied? |
|---|---|---|---|
| [codachi](https://github.com/vincent-k2026/codachi) | MIT | Context ETA concept, CJK width awareness, terminal degrade idea | **No** — independently re-implemented |
| [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) | Apache 2.0 | stdin schema, element catalog, `.omc/state/` path conventions | **No** — only external contracts consumed |
| Codex (OpenAI) via the `codex-rescue` agent | n/a (model output) | Initial production skeleton authored 2026-05-05 | **Yes** — see commit `d813abf` for attribution trailer |

## Open architectural questions

- **Cache locking** for concurrent statusline calls in same session: assumed not needed because Claude Code serializes per session, but unverified across CC releases. Tagged `Not-tested`.
- **`omc-hub-rs` integration shape**: git-dependency vs workspace-path vs fully-separate. Decide when first crate (likely `omc-hooks`) needs OMC state primitives.
- **i18n migration trigger**: at what locale count to move from `i18n.rs` literals to `build.rs` + `locales/*.json`. Currently 2 locales (en, zh-CN), threshold tentatively set at 5+.

## Versioning & release

`0.0.x` until per-element implementation is complete and at least 3 elements are end-to-end useful. Then `0.1.0` first usable release.

This file is the source of truth for architectural intent. Code drift away from this document = update the document or revert the code.
