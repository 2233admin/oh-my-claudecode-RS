# OMC-RS — oh-my-claudecode in Rust

Rust rewrite of oh-my-claudecode: a toolkit for Claude Code that adds agent orchestration, hooks, skills, MCP routing, statusline, context injection, and multi-provider git integration. 17 crates, 42K+ lines, 816 tests.

## Build and Test

```bash
cargo build                          # debug build
cargo test --workspace               # run all 816 tests
cargo test -p omc-team               # single crate
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

All tests must pass before committing. Clippy warnings are treated as errors.

## Code Conventions

- **Edition**: Rust 2024, rust-version 1.85+
- **Naming**: standard Rust — snake_case functions/vars, PascalCase types/traits, SCREAMING_SNAKE constants, kebab-case crate names in Cargo.toml
- **Error handling**: `thiserror` for library error enums (one per crate), `anyhow` for application-level propagation in omc-cli. Re-export error types from crate root. Use `thiserror::Error` derive on every error enum.
- **Async**: tokio runtime. Use `async-trait` for trait objects. All I/O is async.
- **Serialization**: serde + serde_json throughout. JSON config files.
- **Logging**: tracing + tracing-subscriber with env-filter.
- **File length**: keep modules under 400 lines. Extract sub-modules when larger.
- **Prelude**: omc-shared/src/prelude.rs re-exports common types. Prefer `use omc_shared::prelude::*` in downstream crates.
- **No panics in library code**: return Result. Panics are acceptable only in CLI main/parse paths.

## Architecture

### Crate Dependency Direction

```
omc-shared          (foundation — no internal deps)
  |
  +-- omc-macros    (proc macro, depends on omc-shared for types)
  |
  +-- omc-hooks, omc-skills, omc-context, omc-hud, omc-mcp,
      omc-git-provider, omc-notifications, omc-wiki, omc-interop,
      omc-xcmd       (mid-layer crates, depend on omc-shared)
  |
  +-- omc-team       (orchestration, depends on omc-shared + mid-layer)
  |
  +-- omc-cli        (top-level binary, depends on everything)
  +-- omc-installer  (standalone install/update)
```

- Dependencies flow downward only. Never create circular deps.
- omc-shared is the only crate allowed to be depended on by all others.
- Mid-layer crates should not depend on each other — communicate through omc-shared types or omc-team.
- omc-autoresearch, omc-python, omc-installer are partially implemented (stubs/traits).

### Key Crate Responsibilities

| Crate | Role |
|-------|------|
| omc-shared | Types, config, routing, tools, state, memory, resilience |
| omc-team | Agent orchestration — DAG, lifecycle, comms, governance, fault tolerance |
| omc-hooks | Hook system (15 events, 58 tests) |
| omc-skills | Skill templates (38 templates) |
| omc-hud | Statusline elements (13 elements, 205 tests) |
| omc-mcp | MCP tool registry + protocol routing |
| omc-interop | MCP bridge (43 tests) |
| omc-cli | CLI dispatch (26+ commands) |

## Testing

- Every new module gets a `#[cfg(test)] mod tests` block at the bottom of the file.
- Integration tests go in `tests/` directory per crate.
- Use `tempfile` for filesystem tests, `tokio::test` for async tests.
- Test behavior, not implementation. One logical assertion per test case.
- Current: 816 tests, 0 failures. Do not merge code that breaks this.

## Commits and PRs

Follow conventional commits:
- `feat: <description>` for new features
- `fix: <description>` for bug fixes
- `docs:`, `refactor:`, `test:`, `chore:` for other types
- Branches: `dev` for active work, `master` for stable
- Keep PRs focused. One concern per PR.

## Release Profile

Release builds use size optimization: opt-level "z", LTO, single codegen unit, stripped, panic=abort.

## gstack

All web browsing MUST use gstack's `/browse` skill. **Never** use `mcp__claude-in-chrome__*` tools.

### Available Skills

| Skill | Purpose |
|-------|---------|
| `/browse` | Headless browser browsing |
| `/qa` | QA testing |
| `/qa-only` | QA without follow-up |
| `/review` | Code review |
| `/ship` | Ship preparation |
| `/land-and-deploy` | Land and deploy |
| `/canary` | Canary deployment |
| `/benchmark` | Performance benchmarking |
| `/connect-chrome` | Connect to Chrome |
| `/design-consultation` | Design consultation |
| `/design-shotgun` | Rapid design iterations |
| `/design-html` | HTML design |
| `/design-review` | Design review |
| `/plan-ceo-review` | CEO-level plan review |
| `/plan-eng-review` | Engineering plan review |
| `/plan-design-review` | Design plan review |
| `/plan-devex-review` | Developer experience review |
| `/devex-review` | Developer experience review |
| `/office-hours` | Office hours |
| `/retro` | Retrospective |
| `/investigate` | Investigation |
| `/document-release` | Document release |
| `/codex` | Codex integration |
| `/cso` | CSO integration |
| `/autoplan` | Auto planning |
| `/careful` | Careful mode |
| `/freeze` | Freeze state |
| `/guard` | Guard mode |
| `/unfreeze` | Unfreeze state |
| `/learn` | Learning mode |
| `/gstack-upgrade` | Upgrade gstack |
| `/setup-browser-cookies` | Setup browser cookies |
| `/setup-deploy` | Setup deployment |
| `/setup-gbrain` | Setup gbrain |
