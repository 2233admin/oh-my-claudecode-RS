//! `omc-observe` -- print a JSON snapshot of active Claude / Codex sessions.

use anyhow::Result;
use chrono::Utc;
use clap::{Parser, ValueEnum};

use omc_observe::{Session, Snapshot, SourceKind, claude, codex, scan};

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Format {
    /// Pretty-printed JSON (default).
    Json,
    /// Single-line JSON, suitable for piping.
    CompactJson,
}

#[derive(Parser, Debug)]
#[command(
    name = "omc-observe",
    version,
    about = "Read-only Claude / Codex session observability"
)]
struct Cli {
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Json)]
    format: Format,
    /// Only emit the session matching this id (Claude session UUID or Codex rollout id).
    #[arg(long)]
    session: Option<String>,
    /// Only scan Codex sessions.
    #[arg(long, conflicts_with = "claude_only")]
    codex_only: bool,
    /// Only scan Claude sessions.
    #[arg(long, conflicts_with = "codex_only")]
    claude_only: bool,
    /// Drop sessions whose last activity is older than this many minutes.
    #[arg(long, default_value_t = 60)]
    max_age_min: i64,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("omc-observe: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let now = Utc::now();

    let want_claude = !cli.codex_only;
    let want_codex = !cli.claude_only;

    let mut sessions: Vec<Session> = Vec::new();

    if want_claude && let Some(dir) = claude::default_projects_dir() {
        for path in claude::list_session_files(&dir) {
            if let Ok(Some(s)) = claude::parse_session(&path, now) {
                sessions.push(s);
            }
        }
    }

    if want_codex
        && let Some(dir) = codex::default_sessions_dir()
        && dir.exists()
    {
        for path in codex::list_session_files(&dir) {
            if let Ok(Some(s)) = codex::parse_session(&path, now) {
                sessions.push(s);
            }
        }
    }

    if cli.max_age_min > 0 {
        sessions.retain(|s| scan::is_active(s.last_activity, now, cli.max_age_min));
    }

    if let Some(target) = cli.session.as_deref() {
        sessions.retain(|s| s.id == target);
    }

    // Stable ordering: Claude first, then Codex; within each by recency.
    sessions.sort_by(|a, b| match (a.source, b.source) {
        (SourceKind::Claude, SourceKind::Codex) => std::cmp::Ordering::Less,
        (SourceKind::Codex, SourceKind::Claude) => std::cmp::Ordering::Greater,
        _ => b.last_activity.cmp(&a.last_activity),
    });

    let snapshot = Snapshot::build(now, sessions);
    let rendered = match cli.format {
        Format::Json => serde_json::to_string_pretty(&snapshot)?,
        Format::CompactJson => serde_json::to_string(&snapshot)?,
    };
    println!("{rendered}");
    Ok(())
}
