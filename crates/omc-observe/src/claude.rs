//! Claude Code session log parser.
//!
//! Each session is one `*.jsonl` file under `~/.claude/projects/<encoded-cwd>/`.
//! Lines are JSON events; we care about `type=assistant` events whose
//! `message.usage` block carries the Anthropic billing counters.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::model::{CumulativeUsage, Session, SourceKind, UsageEvent};
use crate::scan::{is_active, tokens_per_min};

/// Default context window assumed for Claude sessions when the log does not
/// announce one. 200k matches every shipping Claude model as of 2026-05.
pub const DEFAULT_CONTEXT_MAX: u64 = 200_000;

const RATE_WINDOW_SECONDS: i64 = 60;

#[derive(Debug, Deserialize)]
struct RawEvent {
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default, rename = "sessionId")]
    session_id: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    message: Option<RawMessage>,
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    usage: Option<RawUsage>,
}

#[derive(Debug, Deserialize, Default)]
struct RawUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
}

/// Returns the default Claude projects directory: `~/.claude/projects/`.
pub fn default_projects_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("projects"))
}

/// Enumerate every `*.jsonl` file under `projects/<dir>/`.
pub fn list_session_files(projects_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(top) = std::fs::read_dir(projects_dir) else {
        return out;
    };
    for entry in top.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(inner) = std::fs::read_dir(&path) else {
            continue;
        };
        for f in inner.flatten() {
            let p = f.path();
            if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                out.push(p);
            }
        }
    }
    out
}

/// Parse a single Claude session file. Lines that fail to parse are skipped
/// silently (count returned in `skipped`). Missing or empty files yield `None`.
pub fn parse_session(path: &Path, now: DateTime<Utc>) -> Result<Option<Session>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let mut project_path: Option<String> = None;
    let mut last_activity: Option<DateTime<Utc>> = None;
    let mut message_count: u64 = 0;
    let mut events: Vec<UsageEvent> = Vec::new();
    let mut latest_usage: Option<(DateTime<Utc>, CumulativeUsage)> = None;

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(ev) = serde_json::from_str::<RawEvent>(trimmed) else {
            continue;
        };

        if let Some(sid) = &ev.session_id {
            id = sid.clone();
        }
        if project_path.is_none()
            && let Some(cwd) = ev.cwd.as_ref()
        {
            project_path = Some(cwd.clone());
        }

        let ts = ev
            .timestamp
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc));
        if let Some(t) = ts {
            last_activity = Some(last_activity.map_or(t, |prev| prev.max(t)));
        }

        if ev.r#type.as_deref() == Some("assistant") {
            let synthetic = ev
                .message
                .as_ref()
                .and_then(|m| m.model.as_deref())
                .is_some_and(|m| m.starts_with('<'));
            if !synthetic {
                message_count += 1;
            }
            if let (Some(msg), Some(t)) = (ev.message.as_ref(), ts)
                && let Some(usage) = msg.usage.as_ref()
            {
                let cum = CumulativeUsage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cache_read_tokens: usage.cache_read_input_tokens,
                    cache_creation_tokens: usage.cache_creation_input_tokens,
                };
                let delta = usage
                    .input_tokens
                    .saturating_add(usage.output_tokens)
                    .saturating_add(usage.cache_creation_input_tokens);
                if delta > 0 {
                    events.push(UsageEvent {
                        timestamp: t,
                        delta_tokens: delta,
                    });
                }
                // Synthetic events report zero usage; skip them for context
                // tracking but still let them update last_activity above.
                if !synthetic && cum.context_tokens() > 0 {
                    let take = latest_usage.is_none_or(|(prev_t, _)| t >= prev_t);
                    if take {
                        latest_usage = Some((t, cum));
                    }
                }
            }
        }
    }

    let Some(last_activity) = last_activity else {
        return Ok(None);
    };
    let cum = latest_usage.map(|(_, c)| c).unwrap_or_default();
    let context_tokens = cum.context_tokens();
    let context_max = DEFAULT_CONTEXT_MAX;
    let context_pct = if context_max == 0 {
        0.0
    } else {
        context_tokens as f64 / context_max as f64
    };
    let token_rate = tokens_per_min(&events, now, RATE_WINDOW_SECONDS);
    let active = is_active(last_activity, now, 5);

    Ok(Some(Session {
        id,
        source: SourceKind::Claude,
        project_path,
        context_tokens,
        context_pct,
        context_max,
        token_rate_per_min: token_rate,
        last_activity,
        message_count,
        active,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn parses_minimal_session() {
        let now = DateTime::parse_from_rfc3339("2026-05-25T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let session = parse_session(&fixture("claude_minimal.jsonl"), now)
            .expect("parse ok")
            .expect("session present");
        assert_eq!(session.source, SourceKind::Claude);
        assert_eq!(session.id, "test-session-1");
        assert_eq!(session.message_count, 2);
        // Latest non-synthetic usage: input=10 + cache_read=5000 + cache_creation=2000
        assert_eq!(session.context_tokens, 7010);
        assert_eq!(session.context_max, DEFAULT_CONTEXT_MAX);
    }

    #[test]
    fn skips_garbage_lines() {
        let now = DateTime::parse_from_rfc3339("2026-05-25T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let session = parse_session(&fixture("claude_garbage.jsonl"), now)
            .expect("parse ok")
            .expect("session present");
        // Only the one valid assistant event should count.
        assert_eq!(session.message_count, 1);
    }

    #[test]
    fn empty_file_returns_none() {
        let now = DateTime::parse_from_rfc3339("2026-05-25T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let session = parse_session(&fixture("claude_empty.jsonl"), now).expect("parse ok");
        assert!(session.is_none());
    }
}
