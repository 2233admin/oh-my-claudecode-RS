# omc-rs Decisions

## 2026-05-18

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| 1 | Project goal | Feature parity with oh-my-claudecode TS, same effect or better | Rust rewrite must be a drop-in replacement with no capability regression |
| 2 | Core auto-trigger mechanism | Path B: Hook-based keyword scanning | Hooks read UserPromptSubmit stdin, scan for magic words, output `[MAGIC KEYWORD: skill]` as system-reminder; deterministic, no LLM routing overhead |
| 3 | Build strategy | MVP first, then expand | Get `cargo build --release` green + one E2E hook keyword working before full feature build-out |
| 4 | Tooling | Codex + Claude multi-agent parallel | Codex leads implementation direction; Claude executes in parallel for efficiency |
| 5 | Upstream sync | Required — track oh-my-claudecode TS releases for new hook events / schema changes | Strategy: weekly upstream-watch.yml GitHub Action |
| 6 | First step | `cargo build --release` on this machine, then verify HUD binary | No binaries exist in target/release yet |

## 2026-05-20 — Framework Architecture (post Codex 8-round research)

### Strategic

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| 7 | Product positioning | Cross-host unified framework (Claude + Codex + OpenCode) | No competitor does all three. CShip=Claude only, OMC TS=Claude primary. First-mover in unified runtime. |
| 8 | omc-hook binary location | Add `[[bin]] name="omc-hook"` to `crates/omc-hooks/Cargo.toml` | lib/bin same crate, no new workspace member. `use omc_hooks::*` works directly. |
| 9 | Unified layer approach | C: runtime layer (omc-hook binary) + config layer (omc setup compiles to each host) | Runtime layer fixes token waste root cause; config layer lowers onboarding friction. |
| 10 | Token waste priority | Fix all three layers: CLAUDE.md injection + hook stdout + skill full-load | Three layers are independent and additive. OMC TS couldn't fix any without rewrite. |
| 11 | Framework entry point (MVP) | omc-hook runner + three-host support first | Without the binary, Context Broker and lazy loading are unusable. Binary is the load-bearing foundation. |

### Implementation Constraints

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| 12 | Command parsing | `shlex` crate, not `shellwords` | `shellwords` last release 2020-06-27 (stale). `shlex` actively maintained, lighter dep. Do NOT use `split_whitespace()` — breaks paths with spaces. |
| 13 | executor timeout | `try_wait()` polling loop, ~25-35 lines | `wait_with_output()` has no timeout. Worst case: hook subprocess hangs forever, blocking Claude. Fix in MVP, not post-MVP. |
| 14 | Hook output size | `compact_hook_output()` at 8000 chars (head 6000 + tail 2000 + artifact path) | Claude Code native limit is ~10k chars. OMC must truncate before that to protect chain transmission. Full stdout to `.omc/artifacts/<event>-<ts>.log`. |
| 15 | RTK integration order | C (omc setup --with-rtk) > B (output_policy=rtk) > A (executor pipe) | A breaks because RTK is a command proxy not an stdout filter. C gives immediate 60-90% savings. B adds per-hook semantic compression. |
| 16 | Skill search | `tantivy` for production, in-memory substring for MVP | `bm25` crate last publish ~1.8yr ago (stale). Tantivy supports incremental index, field weights, CJK tokenizer. |
| 17 | Token estimation | `tiktoken-rs` for OpenAI/Codex, conservative `char/3` for Claude, provider-based for OpenCode | No public Claude tokenizer. Conservative estimate avoids over-injection. |
| 18 | UserPromptSubmit registration | Add to `OMC_HOOK_EVENTS` const array in `installer.rs:56` | Current installer only registers PreToolUse/PostToolUse/Stop/SessionStart. Must be PascalCase. Existing tests auto-cover new events. |

### Three-Host Adapter Architecture

| Host | Adapter Type | Config File | Capability | Notes |
|---|---|---|---|---|
| Claude Code | stdin/stdout JSON | `~/.claude/settings.json` | FullHook | Full `additionalContext`, `hookSpecificOutput.updatedInput`. exit 0/2 semantics. |
| Codex CLI | stdin/stdout JSON (容错) | `~/.codex/hooks.json` | PartialHook | `PreToolUse.additionalContext` NOT supported (issue #19385). All fields optional parse. |
| OpenCode | TypeScript in-process plugin | `.opencode/plugins/omc-rs.ts` | Plugin | NOT stdin/stdout — function calls via `@opencode-ai/plugin` SDK. `tool.execute.before/after`. Cannot reuse Claude/Codex adapter. |

**HostAdapter trait** (`crates/omc-host/src/adapter.rs:103`): add capability methods instead of expanding monolithic trait:
```rust
fn command_rewrite_support(&self) -> CommandRewriteSupport; // FullHook | Plugin | InstructionsOnly | None
fn rtk_setup_spec(&self) -> Option<RtkSetupSpec>;
```

### Token Budget Numbers

```
CLAUDE.md static budget:     min(800, 1% context window)
Single hook stdout:          min(8000 chars, 0.5% window) — OMC truncates at 8000
Single skill pack:           min(1500 tokens, 3% window)
Output reserve:              15-25%
Compact trigger:             Claude 60-70%, Codex 75-85%, OpenCode 50-60% (unknown provider)
```

### Hook Output Pipeline (in order)

```
hook stdout
    │
    ▼ opt-in (HookCommand.output_filter = "rtk:cargo-test")
rtk pipe --filter <name>     — semantic compression for known formats
    │                          fail-open if rtk not in PATH or no filter match
    ▼ always
compact_hook_output()         — 8000 chars hard cap, head+tail+artifact path
    │                          Unicode char boundary safe
    ▼
additional_context → Claude Code  — Claude's own 10k cap as final backstop
```

### RTK Reference

- **Repo**: https://github.com/rtk-ai/rtk (50.7k stars, v0.40.0 2026-05-13, Rust)
- **What it does**: CLI proxy for git/cargo/ls/pytest/grep etc, 60-90% token reduction
- **API**: `rtk rewrite "cmd"` → stdout=rewritten cmd, exit 0=rewrite/1=no match/2=deny/3=ask
- **NOT**: a generic stdout filter. `rtk pipe --filter <name>` only works for known named formats.
- **Windows**: native binary exists (`rtk-x86_64-pc-windows-msvc.zip`) but auto-rewrite hook needs Unix shell. WSL for full hook support.
- **NOT a Rust crate**: binary only, `cargo install --git https://github.com/rtk-ai/rtk rtk`

### Competitive Context

| Tool | Stars | Scope | OMC-RS gap |
|---|---|---|---|
| CShip | 346 | Claude statusline only | No hook/skill/budget |
| OMC TS | 34.3k | Claude orchestration | Node/npm/tmux deps, 3 separate host impls |
| RTK | 50.7k | Command output compression | No skill/hook framework |
| OpenCode | 163k | AI coding multi-provider | TS plugin only, skill context waste known pain point |
| Codex CLI | 83.8k | OpenAI coding agent | hooks.json but no unified framework |

**README positioning**: "CShip shows tokens. RTK saves command-output tokens. OMC TS orchestrates Claude. OMC-RS unifies Claude + Codex + OpenCode with one hook/skill/HUD/token-budget runtime."

## 2026-05-20 — Phase 1 + Phase 2 Additions

### Phase State Machine Wheel

| Decision | Choice | Rationale |
|----------|--------|-----------|
| omc-team phase state machine | Embed `statewright_engine` crate | statewright (https://github.com/statewright/statewright, Apache 2.0, Rust) — JSON-defined states, per-phase allowed_tools, transition guards. Embeddable. NOT rolling our own. |
| Phase states | 6: planning/executing/verifying/reviewing/fixing/handoff | planning+verifying+reviewing=read-only; executing+fixing=edit-enabled; handoff=read-only+sync |

### Ticket Map

**Phase 1 (OMC-73..80) — Install/Skills/Hooks closure**
- P1: OMC-73 installer deploys, OMC-74 accurate report, OMC-75 hooks wire to binary, OMC-76 conflict UI, OMC-77 setup 4-phase Rust
- P2: OMC-78 skills single truth, OMC-79 doctor actionable
- P3: OMC-80 install initializes .omc/team

**Phase 2 (OMC-81..89) — Agent framework closure**
- P1: OMC-81 /team unified entry, OMC-82 start launches agents, OMC-83 human-readable progress, OMC-84 handoff delivery
- P2: OMC-85 linear→team auto, OMC-86 stuck agent recovery, OMC-87 statewright phase machine
- P3: OMC-88 /ultragoal pipeline, OMC-89 team as organization

**Phase 3 (OMC-90..97) — Intelligence feedback layer**
- P2: OMC-90 hook utilization feedback, OMC-91 adaptive budget (depends OMC-90), OMC-92 behavioral A/B harness, OMC-93 entropy stuck detection (replaces OMC-86 timeout), OMC-94 MI skill injection (upgrades OMC-68), OMC-95 independent verifier + commit protocol (upgrades OMC-84)
- P3: OMC-96 constitutional hook (new HookKind::Constitutional), OMC-97 auditor agent (depends observability stream)

## Open questions (resolved)

- ~~Spike 1 keyword first vs full keyword-map build-out~~ → omc-hook binary first (OMC-57)
- ~~Upstream sync cadence~~ → weekly upstream-watch.yml (OMC-70)
- ~~omc-wiki integration timing~~ → deferred, not in current milestone
- ~~Phase state machine implementation~~ → embed statewright_engine (OMC-87)

