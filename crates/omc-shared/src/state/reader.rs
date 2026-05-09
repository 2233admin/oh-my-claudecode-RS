//! State reader for reading application state from disk.

use std::fs;

use super::{HudState, SessionInfo, StateError, TeamRunRecord};
use crate::config::OmcPaths;

/// Reader for accessing persisted state files.
#[derive(Debug)]
pub struct StateReader {
    paths: OmcPaths,
}

impl StateReader {
    /// Create a new state reader.
    pub fn new() -> Self {
        Self {
            paths: OmcPaths::new(),
        }
    }

    /// Read the HUD state for the current session.
    pub fn read_hud_state(&self, session_id: &str) -> Result<HudState, StateError> {
        let path = self.paths.session_path(session_id).join("hud-state.json");
        if !path.exists() {
            return Ok(HudState::default());
        }
        let content = fs::read_to_string(&path)?;
        let state: HudState = serde_json::from_str(&content)?;
        Ok(state)
    }

    /// Read all active sessions.
    pub fn read_sessions(&self) -> Result<Vec<SessionInfo>, StateError> {
        let dir = &self.paths.sessions;
        let mut sessions = Vec::new();

        if !dir.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(dir).map_err(StateError::Io)? {
            let entry = entry.map_err(StateError::Io)?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let content = fs::read_to_string(&path).map_err(StateError::Io)?;
            match serde_json::from_str::<SessionInfo>(&content) {
                Ok(session) => sessions.push(session),
                Err(e) => {
                    tracing::warn!("failed to parse session {:?}: {}", path, e);
                }
            }
        }

        sessions.sort_by_key(|b| std::cmp::Reverse(b.last_updated));
        Ok(sessions)
    }

    /// Read all team run records.
    pub fn read_team_runs(&self) -> Result<Vec<TeamRunRecord>, StateError> {
        let dir = self.paths.team.join("runs");
        let mut records = Vec::new();

        if !dir.exists() {
            return Ok(records);
        }

        for entry in fs::read_dir(&dir).map_err(StateError::Io)? {
            let entry = entry.map_err(StateError::Io)?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let content = fs::read_to_string(&path).map_err(StateError::Io)?;
            match serde_json::from_str::<TeamRunRecord>(&content) {
                Ok(record) => records.push(record),
                Err(e) => {
                    tracing::warn!("failed to parse team run {:?}: {}", path, e);
                }
            }
        }

        records.sort_by_key(|b| std::cmp::Reverse(b.started_at));
        Ok(records)
    }

    /// Get the paths helper.
    pub fn paths(&self) -> &OmcPaths {
        &self.paths
    }
}

impl Default for StateReader {
    fn default() -> Self {
        Self {}
    }
}
