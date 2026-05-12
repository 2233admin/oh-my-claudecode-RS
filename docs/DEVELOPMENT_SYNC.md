# Development Sync and 5090 Handoff

This document is the handoff path for making `oh-my-claudecode-RS` visible and usable from the Australia 5090 workstation.

## Source of truth

- Repository: `https://github.com/2233admin/oh-my-claudecode-RS.git`
- Active development branch: `dev`
- Local working copy on this machine: `D:/projects/oh-my-claudecode-RS`

Use Git as the handoff channel. Do not rely on Claude memory or local notes as the source of truth for another workstation.

## Current local state to resolve before publishing

Before pushing for the 5090 machine to pull, check this machine's working tree:

```bash
git -C D:/projects/oh-my-claudecode-RS status --short
```

As of 2026-05-12, the working tree is not clean: `crates/omc-wiki` has local changes, `TEST_COVERAGE_ANALYSIS.md` is untracked, and there are several accidental zero-byte shell-fragment files in the repository root. Review and clean or intentionally commit them before publishing.

## First-time setup on the 5090 workstation

```bash
mkdir -p D:/projects
git clone -b dev https://github.com/2233admin/oh-my-claudecode-RS.git D:/projects/oh-my-claudecode-RS
cd D:/projects/oh-my-claudecode-RS
cargo build --release
cargo test --workspace
```

Expected toolchain:

- Rust 2024-compatible toolchain, rustc 1.85+
- Git Bash or PowerShell on Windows
- No Node/npm required for the Rust implementation

## Updating an existing 5090 checkout

```bash
git -C D:/projects/oh-my-claudecode-RS fetch origin
git -C D:/projects/oh-my-claudecode-RS checkout dev
git -C D:/projects/oh-my-claudecode-RS pull --ff-only origin dev
cargo build --release
cargo test --workspace
```

If `pull --ff-only` fails, the 5090 checkout has local commits or edits. Inspect with:

```bash
git -C D:/projects/oh-my-claudecode-RS status --short
git -C D:/projects/oh-my-claudecode-RS log --oneline --decorate --graph -20
```

Do not reset or discard local work without explicit confirmation.

## Wiring the HUD on the 5090 workstation

After `cargo build --release`, configure Claude Code statusline on the 5090 machine to point at the local binary:

```json
{
  "statusLine": {
    "type": "command",
    "command": "D:/projects/oh-my-claudecode-RS/target/release/omc-hud.exe"
  }
}
```

On Windows, prefer an absolute path in `~/.claude/settings.json` or `~/.claude/settings.local.json`.

Smoke test the binary directly before editing Claude Code settings:

```bash
printf '{}' | D:/projects/oh-my-claudecode-RS/target/release/omc-hud.exe
```

The command should exit quickly and should not crash Claude Code even if the input is incomplete.

## Publishing changes from this machine

Use this sequence after reviewing the dirty working tree:

```bash
git -C D:/projects/oh-my-claudecode-RS diff
git -C D:/projects/oh-my-claudecode-RS status --short
cargo fmt --check
cargo test --workspace
git -C D:/projects/oh-my-claudecode-RS add <intended-files>
git -C D:/projects/oh-my-claudecode-RS commit -m "docs: add 5090 handoff guide"
git -C D:/projects/oh-my-claudecode-RS push origin dev
```

Pushing changes makes them visible to the 5090 workstation through `git pull`. Because push modifies shared state, get explicit confirmation before running it from an agent session.
