//! Application state management and persistence.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod reader;
pub mod writer;

pub use reader::StateReader;
pub use writer::StateWriter;

/// Inner application state
#[derive(Debug, Default)]
pub struct AppStateInner {
    pub initialized: bool,
}

/// Thread-safe application state
pub type AppState = Arc<RwLock<AppStateInner>>;

/// Create a new application state
pub fn app_state_new() -> AppState {
    Arc::new(RwLock::new(AppStateInner::default()))
}

/// Errors that can occur during state operations.
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("State not found: {0}")]
    NotFound(String),

    #[error("Invalid state: {0}")]
    Invalid(String),
}

/// Context sample for tracking token usage over time.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextSample {
    pub ts_ms: u64,
    pub tokens: u64,
}

/// HUD state containing session context history.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HudState {
    pub session_id: String,
    pub context_samples: Vec<ContextSample>,
    pub last_updated_ms: u64,
}

impl HudState {
    /// Create a new HUD state for a session.
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            context_samples: Vec::new(),
            last_updated_ms: 0,
        }
    }

    /// Record a context sample.
    pub fn record_context(&mut self, tokens: Option<u64>, ts_ms: u64) {
        let Some(tokens) = tokens else {
            return;
        };

        if self
            .context_samples
            .last()
            .is_some_and(|sample| sample.ts_ms == ts_ms && sample.tokens == tokens)
        {
            return;
        }

        self.context_samples.push(ContextSample { ts_ms, tokens });
        const MAX_SAMPLES: usize = 36;
        if self.context_samples.len() > MAX_SAMPLES {
            let overflow = self.context_samples.len() - MAX_SAMPLES;
            self.context_samples.drain(0..overflow);
        }
        self.last_updated_ms = ts_ms;
    }
}

/// Session information for agent tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub agent_id: String,
    pub run_id: String,
    pub role: String,
    pub cell_id: Option<String>,
    pub current_task: String,
    pub state: SessionState,
    pub epoch: u32,
    pub last_handoff: Option<String>,
    pub created_at: u64,
    pub last_updated: u64,
}

/// Session state enum matching AgentSessionState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Planned,
    Spawned,
    Active,
    Checkpointing,
    Saturated,
    HandoffReady,
    Completed,
    Abandoned,
    Resumable,
}

/// Team run record for tracking team operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRunRecord {
    pub run_id: String,
    pub team_name: String,
    pub mission_path: String,
    pub tracker: Option<String>,
    pub issue_ref: Option<String>,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub status: String,
    pub agent_count: u8,
}

impl TeamRunRecord {
    /// Check if the run is still active.
    pub fn is_active(&self) -> bool {
        self.status == "active" || self.status == "planned"
    }
}
