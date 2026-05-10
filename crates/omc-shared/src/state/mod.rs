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

#[cfg(test)]
mod tests {
    use super::*;

    // === AppState ===

    #[test]
    fn app_state_new_is_default() {
        let state = app_state_new();
        let inner = state.blocking_read();
        assert!(!inner.initialized);
    }

    #[test]
    fn app_state_default() {
        let inner = AppStateInner::default();
        assert!(!inner.initialized);
    }

    // === HudState ===

    #[test]
    fn hud_state_new() {
        let state = HudState::new("sess-1".into());
        assert_eq!(state.session_id, "sess-1");
        assert!(state.context_samples.is_empty());
        assert_eq!(state.last_updated_ms, 0);
    }

    #[test]
    fn hud_state_record_context_adds_sample() {
        let mut state = HudState::new("sess-1".into());
        state.record_context(Some(1000), 100);
        assert_eq!(state.context_samples.len(), 1);
        assert_eq!(state.context_samples[0].tokens, 1000);
        assert_eq!(state.last_updated_ms, 100);
    }

    #[test]
    fn hud_state_record_context_none_is_noop() {
        let mut state = HudState::new("sess-1".into());
        state.record_context(None, 100);
        assert!(state.context_samples.is_empty());
        assert_eq!(state.last_updated_ms, 0);
    }

    #[test]
    fn hud_state_dedup_same_ts_and_tokens() {
        let mut state = HudState::new("sess-1".into());
        state.record_context(Some(1000), 100);
        state.record_context(Some(1000), 100); // duplicate
        assert_eq!(state.context_samples.len(), 1);
    }

    #[test]
    fn hud_state_allows_same_ts_different_tokens() {
        let mut state = HudState::new("sess-1".into());
        state.record_context(Some(1000), 100);
        state.record_context(Some(2000), 100); // same ts, different tokens
        assert_eq!(state.context_samples.len(), 2);
    }

    #[test]
    fn hud_state_overflow_evicts_oldest() {
        let mut state = HudState::new("sess-1".into());
        for i in 0..40 {
            state.record_context(Some(i * 100), i * 1000);
        }
        assert_eq!(state.context_samples.len(), 36);
        // Oldest entries should have been evicted
        assert_eq!(state.context_samples[0].tokens, 4 * 100);
    }

    // === SessionState ===

    #[test]
    fn session_state_serialization_roundtrip() {
        let variants = vec![
            SessionState::Planned,
            SessionState::Spawned,
            SessionState::Active,
            SessionState::Checkpointing,
            SessionState::Saturated,
            SessionState::HandoffReady,
            SessionState::Completed,
            SessionState::Abandoned,
            SessionState::Resumable,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: SessionState = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn session_state_uses_snake_case() {
        assert_eq!(
            serde_json::to_string(&SessionState::HandoffReady).unwrap(),
            "\"handoff_ready\""
        );
        assert_eq!(
            serde_json::to_string(&SessionState::Active).unwrap(),
            "\"active\""
        );
    }

    // === SessionInfo ===

    #[test]
    fn session_info_serialization_roundtrip() {
        let info = SessionInfo {
            agent_id: "agent-1".into(),
            run_id: "run-1".into(),
            role: "executor".into(),
            cell_id: Some("cell-a".into()),
            current_task: "build feature".into(),
            state: SessionState::Active,
            epoch: 3,
            last_handoff: None,
            created_at: 1000,
            last_updated: 2000,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.agent_id, "agent-1");
        assert_eq!(deserialized.state, SessionState::Active);
        assert!(deserialized.last_handoff.is_none());
    }

    // === TeamRunRecord ===

    #[test]
    fn team_run_record_active() {
        let record = TeamRunRecord {
            run_id: "run-1".into(),
            team_name: "alpha".into(),
            mission_path: "/tmp/mission.md".into(),
            tracker: None,
            issue_ref: None,
            started_at: 1000,
            ended_at: None,
            status: "active".into(),
            agent_count: 3,
        };
        assert!(record.is_active());
    }

    #[test]
    fn team_run_record_planned_is_active() {
        let record = TeamRunRecord {
            run_id: "run-1".into(),
            team_name: "alpha".into(),
            mission_path: "/tmp/mission.md".into(),
            tracker: None,
            issue_ref: None,
            started_at: 1000,
            ended_at: None,
            status: "planned".into(),
            agent_count: 3,
        };
        assert!(record.is_active());
    }

    #[test]
    fn team_run_record_completed_is_not_active() {
        let record = TeamRunRecord {
            run_id: "run-1".into(),
            team_name: "alpha".into(),
            mission_path: "/tmp/mission.md".into(),
            tracker: None,
            issue_ref: None,
            started_at: 1000,
            ended_at: Some(2000),
            status: "completed".into(),
            agent_count: 3,
        };
        assert!(!record.is_active());
    }

    #[test]
    fn team_run_record_serialization_roundtrip() {
        let record = TeamRunRecord {
            run_id: "run-42".into(),
            team_name: "beta".into(),
            mission_path: "/home/user/mission.md".into(),
            tracker: Some("github".into()),
            issue_ref: Some("#123".into()),
            started_at: 1000,
            ended_at: Some(5000),
            status: "completed".into(),
            agent_count: 5,
        };
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: TeamRunRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.run_id, "run-42");
        assert_eq!(deserialized.tracker, Some("github".into()));
        assert!(!deserialized.is_active());
    }

    // === ContextSample ===

    #[test]
    fn context_sample_default() {
        let sample = ContextSample::default();
        assert_eq!(sample.ts_ms, 0);
        assert_eq!(sample.tokens, 0);
    }
}
