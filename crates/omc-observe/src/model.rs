//! Normalized output schema shared by Claude and Codex scanners.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Source CLI a session originated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Claude,
    Codex,
}

/// One observed session.
#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub id: String,
    pub source: SourceKind,
    pub project_path: Option<String>,
    pub context_tokens: u64,
    pub context_pct: f64,
    pub context_max: u64,
    pub token_rate_per_min: f64,
    pub last_activity: DateTime<Utc>,
    pub message_count: u64,
    pub active: bool,
}

/// Aggregate summary across sessions.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub total_sessions: usize,
    pub active_sessions: usize,
    pub total_context_tokens: u64,
}

/// Full snapshot payload.
#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub generated_at: DateTime<Utc>,
    pub sessions: Vec<Session>,
    pub summary: Summary,
}

impl Snapshot {
    pub fn build(generated_at: DateTime<Utc>, mut sessions: Vec<Session>) -> Self {
        sessions.sort_by_key(|s| std::cmp::Reverse(s.last_activity));
        let total_sessions = sessions.len();
        let active_sessions = sessions.iter().filter(|s| s.active).count();
        let total_context_tokens = sessions.iter().map(|s| s.context_tokens).sum();
        Self {
            generated_at,
            sessions,
            summary: Summary {
                total_sessions,
                active_sessions,
                total_context_tokens,
            },
        }
    }
}

/// A single token-accounting datapoint extracted from a jsonl event.
///
/// `delta_tokens` is the number of tokens billed for the event (input + output
/// plus cache_creation; cache_read is excluded from the rate because it is already
/// counted in earlier events and would double-count).
#[derive(Debug, Clone, Copy)]
pub struct UsageEvent {
    pub timestamp: DateTime<Utc>,
    pub delta_tokens: u64,
}

/// Cumulative usage observed in a session, taken from the most recent event
/// that carried a `usage` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct CumulativeUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
}

impl CumulativeUsage {
    /// Effective context size: live input + cache read + cache creation.
    /// Mirrors how Anthropic bills "what is in the model's context right now".
    pub fn context_tokens(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.cache_read_tokens)
            .saturating_add(self.cache_creation_tokens)
    }
}
