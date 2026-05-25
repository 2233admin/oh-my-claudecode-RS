//! Codex CLI session log parser.
//!
//! Sessions live at `~/.codex/sessions/<yyyy>/<mm>/<dd>/rollout-*.jsonl`.
//! Token accounting comes from `event_msg` payloads of type `token_count` whose
//! `info.total_token_usage` is a cumulative running total. `model_context_window`
//! is broadcast on the same events.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::model::{CumulativeUsage, Session, SourceKind, UsageEvent};
use crate::scan::{is_active, tokens_per_min};

const RATE_WINDOW_SECONDS: i64 = 60;
const DEFAULT_CONTEXT_MAX: u64 = 200_000;

#[derive(Debug, Deserialize)]
struct RawLine {
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    payload: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
struct TokenInfo {
    #[serde(default)]
    total_token_usage: Option<TokenUsage>,
    #[serde(default)]
    model_context_window: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct TokenUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cached_input_tokens: u64,
    #[serde(default)]
    reasoning_output_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

#[derive(Debug, Deserialize, Default)]
struct SessionMeta {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
}

/// Default Codex sessions root.
pub fn default_sessions_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex").join("sessions"))
}

/// Recursively enumerate rollout `*.jsonl` files (year/month/day layout).
pub fn list_session_files(sessions_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(sessions_dir, &mut out);
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(p);
        }
    }
}

/// Parse a single Codex rollout file.
pub fn parse_session(path: &Path, now: DateTime<Utc>) -> Result<Option<Session>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut id = id_from_filename(path);
    let mut project_path: Option<String> = None;
    let mut last_activity: Option<DateTime<Utc>> = None;
    let mut message_count: u64 = 0;
    let mut events: Vec<UsageEvent> = Vec::new();
    let mut latest_total: u64 = 0;
    let mut context_max: u64 = DEFAULT_CONTEXT_MAX;
    let mut prev_total: u64 = 0;

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(raw) = serde_json::from_str::<RawLine>(trimmed) else {
            continue;
        };

        let ts = raw
            .timestamp
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc));
        if let Some(t) = ts {
            last_activity = Some(last_activity.map_or(t, |prev| prev.max(t)));
        }

        match raw.r#type.as_deref() {
            Some("session_meta") => {
                if let Some(payload) = raw.payload.as_ref()
                    && let Ok(meta) = serde_json::from_value::<SessionMeta>(payload.clone())
                {
                    if let Some(sid) = meta.id {
                        id = sid;
                    }
                    if let Some(cwd) = meta.cwd {
                        project_path = Some(cwd);
                    }
                }
            }
            Some("event_msg") => {
                let Some(payload) = raw.payload.as_ref() else {
                    continue;
                };
                let Some(kind) = payload.get("type").and_then(|v| v.as_str()) else {
                    continue;
                };
                if kind == "task_started" || kind == "agent_message" {
                    message_count += 1;
                }
                if kind != "token_count" {
                    continue;
                }
                let info_val = payload
                    .get("info")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let info: TokenInfo = serde_json::from_value(info_val).unwrap_or_default();
                if let Some(cw) = info.model_context_window
                    && cw > 0
                {
                    context_max = cw;
                }
                let Some(total) = info.total_token_usage else {
                    continue;
                };
                let cum = CumulativeUsage {
                    input_tokens: total.input_tokens,
                    output_tokens: total
                        .output_tokens
                        .saturating_add(total.reasoning_output_tokens),
                    cache_read_tokens: total.cached_input_tokens,
                    cache_creation_tokens: 0,
                };
                let new_total = if total.total_tokens > 0 {
                    total.total_tokens
                } else {
                    cum.context_tokens()
                };
                if let Some(t) = ts {
                    let delta = new_total.saturating_sub(prev_total);
                    if delta > 0 {
                        events.push(UsageEvent {
                            timestamp: t,
                            delta_tokens: delta,
                        });
                    }
                }
                prev_total = new_total;
                latest_total = cum.context_tokens();
            }
            _ => {}
        }
    }

    let Some(last_activity) = last_activity else {
        return Ok(None);
    };

    let context_pct = if context_max == 0 {
        0.0
    } else {
        latest_total as f64 / context_max as f64
    };
    let token_rate = tokens_per_min(&events, now, RATE_WINDOW_SECONDS);
    let active = is_active(last_activity, now, 5);

    Ok(Some(Session {
        id,
        source: SourceKind::Codex,
        project_path,
        context_tokens: latest_total,
        context_pct,
        context_max,
        token_rate_per_min: token_rate,
        last_activity,
        message_count,
        active,
    }))
}

fn id_from_filename(path: &Path) -> String {
    // Codex rollout filenames look like:
    // rollout-2026-03-31T13-17-00-019d4252-dd42-76a2-9d5c-b0e9a1e1a4d8.jsonl
    // The UUID is the trailing 5 hyphen-separated chunks.
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let parts: Vec<&str> = stem.split('-').collect();
    if parts.len() >= 5 {
        let tail = &parts[parts.len() - 5..];
        tail.join("-")
    } else {
        stem.to_string()
    }
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
    fn parses_codex_session() {
        let now = DateTime::parse_from_rfc3339("2026-03-31T05:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let session = parse_session(&fixture("codex_minimal.jsonl"), now)
            .expect("parse ok")
            .expect("session present");
        assert_eq!(session.source, SourceKind::Codex);
        assert_eq!(session.id, "019d4252-dd42-76a2-9d5c-b0e9a1e1a4d8");
        assert_eq!(session.context_max, 258400);
        // total_tokens from the second token_count event = 55582
        assert_eq!(session.context_tokens, 55111 + 5504);
    }

    #[test]
    fn id_extraction_from_filename() {
        let p =
            PathBuf::from("rollout-2026-03-31T13-17-00-019d4252-dd42-76a2-9d5c-b0e9a1e1a4d8.jsonl");
        assert_eq!(id_from_filename(&p), "019d4252-dd42-76a2-9d5c-b0e9a1e1a4d8");
    }
}
