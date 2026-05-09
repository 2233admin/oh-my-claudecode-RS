//! State writer for persisting state to disk with atomic operations.

use std::fs;

use super::{HudState, SessionInfo, StateError, TeamRunRecord};
use crate::config::OmcPaths;

/// Writer for persisting state to disk with atomic operations.
#[derive(Debug)]
pub struct StateWriter {
    paths: OmcPaths,
}

impl StateWriter {
    /// Create a new state writer.
    pub fn new() -> Self {
        Self {
            paths: OmcPaths::new(),
        }
    }

    /// Write HUD state atomically using .tmp + rename pattern.
    pub fn write_hud_state(&self, session_id: &str, state: &HudState) -> Result<(), StateError> {
        let path = self.paths.session_path(session_id).join("hud-state.json");

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(StateError::Io)?;
        }

        let tmp = path.with_extension("json.tmp");
        let content = serde_json::to_vec_pretty(state).map_err(StateError::Serialize)?;

        fs::write(&tmp, content).map_err(StateError::Io)?;

        match fs::rename(&tmp, &path) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                Err(StateError::Io(e))
            }
        }
    }

    /// Update a session record.
    pub fn update_session(&self, session: &SessionInfo) -> Result<(), StateError> {
        let dir = &self.paths.sessions;
        fs::create_dir_all(dir).map_err(StateError::Io)?;

        let path = dir.join(format!("{}.json", session.agent_id));
        let tmp = path.with_extension("json.tmp");
        let content = serde_json::to_vec_pretty(session).map_err(StateError::Serialize)?;

        fs::write(&tmp, content).map_err(StateError::Io)?;

        match fs::rename(&tmp, &path) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                Err(StateError::Io(e))
            }
        }
    }

    /// Write a team run record.
    pub fn write_team_run(&self, record: &TeamRunRecord) -> Result<(), StateError> {
        let dir = self.paths.team.join("runs");
        fs::create_dir_all(&dir).map_err(StateError::Io)?;

        let path = dir.join(format!("{}.json", record.run_id));
        let tmp = path.with_extension("json.tmp");
        let content = serde_json::to_vec_pretty(record).map_err(StateError::Serialize)?;

        fs::write(&tmp, content).map_err(StateError::Io)?;

        match fs::rename(&tmp, &path) {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                Err(StateError::Io(e))
            }
        }
    }

    /// Get the paths helper.
    pub fn paths(&self) -> &OmcPaths {
        &self.paths
    }
}

impl Default for StateWriter {
    fn default() -> Self {
        Self::new()
    }
}
