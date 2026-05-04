# oh-my-claudecode-RS

Rust rewrite of [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode). Performance and maintainability over the TS upstream.

## Why

- **Performance**: TS HUD spawn-per-render hits 390-502ms cold-start (upstream issue #2843). Rust binary target: <5ms.
- **Memory**: OMC's bun + haiku MCP stack is 663MB. The sibling `omc-hub-rs` already proved a 7.4MB Rust replacement — same playbook applied to the rest of OMC.
- **Maintenance**: Single-author Rust fork = full control, no upstream churn dependency, no npm dep hell.
- **Decoupling**: TS upstream's design and maintainer style are immaterial to this project.

## Status

**Pre-alpha.** Just started. First MVP: `omc-hud` (Rust statusline).

## Architecture

Cargo workspace, multi-crate. Each subsystem ships as an independent binary or library.

```
oh-my-claudecode-RS/
├── crates/
│   ├── omc-hud/        statusline binary (MVP)
│   ├── omc-hooks/      pre/post-tool-use, session-start/end (planned)
│   ├── omc-cli/        autopilot/ralph/ultrawork (planned)
│   ├── omc-team/       orchestration (planned)
│   └── omc-shared/     config / state-path / protocol (extracted as needed)
└── docs/
```

## Sibling project

- **[omc-hub-rs](https://github.com/2233admin/omc-hub-rs)** — independent MCP server replacement (already shipped, v0.1.0+). Lives separately. This monorepo references it as a runtime dependency where needed. **Will not be absorbed.**

## Roadmap (rough)

| Phase | Crate | Description | Status |
|-------|-------|-------------|--------|
| 0 | omc-hud | Statusline replacement (Context ETA, color degrade, CJK width, i18n, productivity stats) | 🚧 starting |
| 1 | omc-shared | Common config/state/path utilities | ⚪ planned |
| 2 | omc-hooks | Hook engine (PostToolUse, SessionStart, etc.) | ⚪ planned |
| 3 | omc-cli | Top-level commands (autopilot/ralph/ultrawork/team) | ⚪ planned |
| 4 | omc-team | Multi-agent orchestration | ⚪ planned |

## Inspiration credits

- Algorithm/UX inspiration from [codachi](https://github.com/vincent-k2026/codachi) (MIT, vincent-k2026) — independently re-implemented in Rust. Not a code port.
- Reference TS implementation: [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) (Apache 2.0, Yeachan-Heo). This project does not copy upstream code; it consumes the same external interfaces (Claude Code stdin schema, settings.json contracts).

## License

MIT.
