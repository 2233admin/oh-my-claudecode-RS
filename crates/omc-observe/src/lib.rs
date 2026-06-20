//! Read-only session and token observability for Claude Code / Codex CLI.
//!
//! Phase 1: scan `~/.claude/projects/**/*.jsonl` and `~/.codex/sessions/**/*.jsonl`,
//! extract per-session token usage, compute a sliding-window token rate, and emit
//! a normalized JSON snapshot. No quota enforcement or claw-router wiring yet.

pub mod claude;
pub mod codex;
pub mod model;
pub mod scan;

pub use model::{Session, Snapshot, SourceKind, Summary};
