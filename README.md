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
| 4 | `omc-team` | Claude Code experimental agent team orchestration shell | 🧪 v0.4 session + runtime adapters |

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

## Experimental Team Orchestration

`omc-team` is an aggressive v0.4 orchestration shell. Claude Code experimental agent teams remain the default runtime, while GitHub/Linear stay as visibility adapters. FSC and KohakuTerrarium can be selected as local runtime adapters for users who want swarm execution or creature/terrarium execution without turning those projects into trackers. v0.4 adds the local Agent Memory & Observability Kernel: every team launch writes durable session records, invocation records, usage ledgers, run briefings, and whiteboard files under `.omc/team/`.

```bash
cargo run -p omc-team -- init
set CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1
cargo run -p omc-team -- linear doctor --team OMC-RS --fix
cargo run -p omc-team -- github doctor --repo 2233admin/oh-my-claudecode-RS --fix
cargo run -p omc-team -- runtime doctor kohaku
cargo run -p omc-team -- runtime doctor fsc
cargo run -p omc-team -- start XAR-123 --tracker linear --team-size 3
cargo run -p omc-team -- start '#123' --tracker github --team-size 3
cargo run -p omc-team -- start ./task.md --runtime kohaku --team-size 4
cargo run -p omc-team -- start ./task.md --runtime fsc --team-size 5
cargo run -p omc-team -- start ./task.md --team-size 16
cargo run -p omc-team -- session list
cargo run -p omc-team -- session resume <agent-id-or-run-id>
cargo run -p omc-team -- usage report --by agent
cargo run -p omc-team -- top
cargo run -p omc-team -- doctor observability
cargo run -p omc-team -- handoff 2233admin-oh-my-claudecode-rs-123 --github
cargo run -p omc-team -- handoff <run-id> --runtime kohaku
cargo run -p omc-team -- research "investigate parser architecture"
cargo run -p omc-team -- review PR-142 --security --tests
```

Claude Code v2.1.32+ is required for the default runtime. Generated missions tell Claude to create the official agent team and use its shared task list, mailbox, hooks, and worktree isolation. Linear and GitHub Issues are visibility adapters only: `agent-ready` is the intake gate, OMC writes lease/start/handoff comments, and the selected runtime remains the execution source of truth.

Runtime adapters are local-only in v0.3:

- `--runtime claude` is the default and preserves the official Claude Code team model.
- `--runtime kohaku` generates a temporary Kohaku package under `.omc/team/kohaku/<run_id>/`, with an OMC Lead as root creature and developer/reviewer/tester/security peer creatures inside the terrarium. OMC follows Kohaku's core boundary: creatures are self-contained, terrariums are wiring only, root is outside the terrarium, and prompts do not inline tool lists or tool-call syntax.
- `--runtime fsc` generates FSC mission/task artifacts under `.omc/team/fsc/<run_id>/` and treats FSC as a local swarm execution backend. FSC handles decomposition and scheduling; OMC collects reports and handoff output.

The session layer treats native subagents as ephemeral. OMC persists the durable identity instead: `.omc/team/sessions/*.json` tracks agent state, `.omc/team/invocations/*.json` tracks each model/runtime call, `.omc/team/usage.jsonl` records token/time/cost events with source and confidence, `.omc/team/runs/<run_id>/briefing.md` is the resume packet seed, and `.omc/team/whiteboard/` stores accepted facts, decisions, risks, questions, and handoffs. Context budget rules are explicit: checkpoint at 70%, resume brief at 85%, stop new work at 92%, and force handoff at 95%. A 16-agent run is organized as one lead plus five builder/reviewer/verifier cells rather than one flat chat.

X-CMD and [`abtop`](https://github.com/graykode/abtop) are reference/optional observability inputs, not hard dependencies. X-CMD's Claude usage/session ideas inform accounting and export/import flows; abtop's read-only Claude/Codex monitoring informs future `omc-team top` work for context, rate limits, ports, and child processes.

OMC does not auto-open upstream PRs for FSC or KohakuTerrarium. Any future external contribution flow must first record a public issue/discussion approval trail before PR creation is allowed.

OMC includes native Karpathy-style agent discipline by default, inspired by [`forrestchang/andrej-karpathy-skills`](https://github.com/forrestchang/andrej-karpathy-skills): think before coding, keep implementations simple, make surgical changes, and drive work with explicit verification. `omc-team init` writes this into `CLAUDE.md` and the generated team prompts/subagents carry the same discipline without requiring an external plugin install.

## Inspiration credits

Independent re-implementation, no source code copying. See [CHANGELOG.md § Inspiration](CHANGELOG.md#inspiration--attribution) and [ARCHITECTURE.md § Inspiration](ARCHITECTURE.md#inspiration--attribution) for the full attribution table.

- [codachi](https://github.com/vincent-k2026/codachi) (MIT, vincent-k2026) — Context ETA concept, CJK width awareness, terminal-degrade idea
- [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) (Apache 2.0, Yeachan-Heo) — stdin schema, element catalog, OMC state path conventions
- Codex via `codex-rescue` agent — initial production skeleton (commit [`d813abf`](https://github.com/2233admin/oh-my-claudecode-RS/commit/d813abf))

## License

MIT — see [LICENSE](LICENSE).
