# Developer Onboarding Guide

This guide is for a developer or agent opening `oh-my-claudecode-RS` for the first time, especially from another workstation such as the Australia 5090 machine.

## Prerequisites

- [ ] Windows 11, Git Bash, PowerShell, Linux, or macOS shell
- [ ] Git
- [ ] Rust toolchain with rustc 1.85+
- [ ] Cargo from the same Rust toolchain
- [ ] Claude Code, if you want to use `omc-hud` as a statusline

No Node.js or npm runtime is required for the Rust implementation.

## Setup

### First-time checkout

```bash
mkdir -p D:/projects
git clone -b dev https://github.com/2233admin/oh-my-claudecode-RS.git D:/projects/oh-my-claudecode-RS
cd D:/projects/oh-my-claudecode-RS
cargo build --release
cargo test --workspace
```

### Update an existing checkout

```bash
git -C D:/projects/oh-my-claudecode-RS fetch origin
git -C D:/projects/oh-my-claudecode-RS checkout dev
git -C D:/projects/oh-my-claudecode-RS pull --ff-only origin dev
cargo build --release
cargo test --workspace
```

If `pull --ff-only` fails, inspect local work before doing anything destructive:

```bash
git -C D:/projects/oh-my-claudecode-RS status --short
git -C D:/projects/oh-my-claudecode-RS log --oneline --decorate --graph -20
```

## First Reading Order

Read these files in order before editing code:

1. `README.md` — product summary, quick start, crate overview.
2. `ARCHITECTURE.md` — design goals, dependency direction, data flow, design decisions.
3. `CLAUDE.md` — repository-specific agent instructions, build/test commands, routing rules.
4. `AGENTS.md` — OMC-native control-plane expectations.
5. `docs/DEVELOPMENT_SYNC.md` — workstation sync and 5090 handoff flow.
6. `TEST_COVERAGE_ANALYSIS.md` — current known test coverage gaps, if present in the checkout.

## Project Structure

- `Cargo.toml` — Rust workspace root, release profile, shared dependency versions.
- `Cargo.lock` — committed application lockfile.
- `crates/omc-shared/` — shared types, config, routing, tools, state, memory, resilience.
- `crates/omc-hud/` — Claude Code statusline binary; hot path must stay fast and non-crashing.
- `crates/omc-team/` — multi-agent orchestration, task DAGs, lifecycle, communication, governance.
- `crates/omc-hooks/` — Claude Code hook event model and execution.
- `crates/omc-skills/` — skill templates, loading, registration, host filtering.
- `crates/omc-mcp/` — MCP tool registry and JSON-RPC stdio server.
- `crates/omc-cli/` — top-level command dispatcher.
- `crates/omc-host/` — host adapters such as Claude and Codex.
- `crates/omc-git-provider/` — GitHub, GitLab, Gitea, Bitbucket, Azure DevOps abstraction.
- `crates/omc-wiki/` — wiki knowledge layer.
- `crates/omc-notifications/` — Slack/tmux notification dispatch.
- `crates/omc-context/` — context and rules injection.
- `crates/omc-interop/` — OMC/OMX and MCP bridge interop.
- `crates/omc-autoresearch/`, `crates/omc-python/`, `crates/omc-installer/` — partially implemented subsystems.
- `tests/macro-tests/` — integration tests for proc macros.
- `templates/` — high-level templates used by OMC flows.
- `docs/` — human and agent onboarding / handoff docs.

## Development Workflow

### Before changing code

```bash
git -C D:/projects/oh-my-claudecode-RS status --short
```

If the tree is dirty, identify which files are yours before editing. Do not delete or reset unknown work without explicit confirmation.

### Make a focused change

Use the smallest crate that owns the behavior. Dependency direction is intentional: most crates depend on `omc-shared`; mid-layer crates should not casually depend on each other.

### Validate

Run the narrowest relevant checks first, then the workspace checks before handoff:

```bash
cargo fmt --check
cargo test -p omc-hud
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Use the crate-specific test command that matches your change. For example:

```bash
cargo test -p omc-team
cargo test -p omc-skills
cargo test -p omc-mcp
```

### Commit and publish

```bash
git -C D:/projects/oh-my-claudecode-RS diff
git -C D:/projects/oh-my-claudecode-RS status --short
git -C D:/projects/oh-my-claudecode-RS add <intended-files>
git -C D:/projects/oh-my-claudecode-RS commit -m "type: concise imperative summary"
git -C D:/projects/oh-my-claudecode-RS push origin dev
```

Use conventional commits: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`.

Because `push` changes shared state, an agent should get explicit confirmation before running it.

## Key Concepts

### Clean Rust rewrite

`oh-my-claudecode-RS` is a from-scratch Rust implementation of useful OMC concepts. It consumes external contracts such as Claude Code statusline input, settings paths, and OMC state conventions; it does not copy upstream TypeScript source.

### Fast HUD hot path

`omc-hud` is called by Claude Code as a statusline command. It must parse stdin, render, write stdout, and exit quickly. It should never panic visibly, hang, or block the host process.

### Workspace dependency direction

`omc-shared` is the foundation. Downstream crates should use shared types rather than inventing parallel representations. Avoid circular dependencies and avoid making mid-layer crates depend on each other unless the architecture document is updated.

### File-based agent IPC

Agent orchestration uses file-based JSONL inbox/outbox patterns under `.omc/`. This keeps coordination local, inspectable, and independent of databases or always-on services.

### OMC-native first

For this repository, prefer OMC native commands and adapters for orchestration, tracker updates, sessions, usage, memory, and handoff. x-cmd is optional toolboxing, not the project control plane.

## 5090 Agent Bootstrap Prompt

Use this as the first message to an agent on the Australia 5090 workstation after the repo is cloned:

```text
You are working in D:/projects/oh-my-claudecode-RS on branch dev.
First read README.md, ARCHITECTURE.md, CLAUDE.md, AGENTS.md, docs/DEVELOPMENT_SYNC.md, and docs/onboarding.md.
Then run git status --short and report whether the tree is clean.
Do not edit code until you can explain: the crate dependency direction, what omc-hud must guarantee, how omc-team communicates, and how this workstation syncs with origin/dev.
Use cargo fmt --check, cargo test -p <crate>, cargo test --workspace, and cargo clippy --workspace -- -D warnings for verification.
Never reset, clean, delete unknown files, or push without explicit confirmation.
```

## Troubleshooting

### `rustc` is too old

```bash
rustc --version
rustup update stable
```

The workspace requires rustc 1.85+ and Rust edition 2024.

### `cargo test --workspace` fails after pulling

Run a narrower command to identify the failing crate:

```bash
cargo test -p omc-hud
cargo test -p omc-shared
cargo test -p omc-team
```

Then inspect recent commits and local changes:

```bash
git -C D:/projects/oh-my-claudecode-RS status --short
git -C D:/projects/oh-my-claudecode-RS log --oneline -10
```

### Claude Code statusline does not show OMC HUD

Confirm the binary exists:

```bash
ls D:/projects/oh-my-claudecode-RS/target/release/omc-hud.exe
```

Smoke test:

```bash
printf '{}' | D:/projects/oh-my-claudecode-RS/target/release/omc-hud.exe
```

Then check the configured `statusLine.command` path in `~/.claude/settings.json` or `~/.claude/settings.local.json`.

### Repository has strange root files

If files such as `(`, `0)`, `String\``, or `HudCache\`` appear in `git status`, treat them as accidental shell-fragment files until proven otherwise. Do not delete them in an agent session without confirmation; list them in the handoff report.
