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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{SessionInfo, SessionState};
    use tempfile::TempDir;

    fn writer_with_root(root: &std::path::Path) -> StateWriter {
        StateWriter {
            paths: crate::config::OmcPaths::new_with_root(root.to_path_buf()),
        }
    }

    #[test]
    fn write_and_read_hud_state_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_with_root(tmp.path());

        let mut state = HudState::new("sess-1".into());
        state.record_context(Some(1000), 100);
        state.record_context(Some(2000), 200);

        writer.write_hud_state("sess-1", &state).unwrap();

        // Verify file exists and is valid JSON (not .tmp)
        let path = tmp.path().join("state/sessions/sess-1/hud-state.json");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: HudState = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.session_id, "sess-1");
        assert_eq!(loaded.context_samples.len(), 2);
    }

    #[test]
    fn write_hud_state_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_with_root(tmp.path());
        let state = HudState::new("new-sess".into());

        writer.write_hud_state("new-sess", &state).unwrap();

        let path = tmp.path().join("state/sessions/new-sess/hud-state.json");
        assert!(path.exists());
    }

    #[test]
    fn write_hud_state_no_tmp_file_left_behind() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_with_root(tmp.path());
        let state = HudState::new("sess-1".into());

        writer.write_hud_state("sess-1", &state).unwrap();

        let tmp_path = tmp.path().join("state/sessions/sess-1/hud-state.json.tmp");
        assert!(
            !tmp_path.exists(),
            ".tmp file should be cleaned up after rename"
        );
    }

    #[test]
    fn update_session_creates_file() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_with_root(tmp.path());

        let session = SessionInfo {
            agent_id: "agent-1".into(),
            run_id: "run-1".into(),
            role: "executor".into(),
            cell_id: None,
            current_task: "build".into(),
            state: SessionState::Active,
            epoch: 1,
            last_handoff: None,
            created_at: 100,
            last_updated: 200,
        };

        writer.update_session(&session).unwrap();

        let path = tmp.path().join("state/sessions/agent-1.json");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: SessionInfo = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.agent_id, "agent-1");
    }

    #[test]
    fn update_session_overwrites_existing() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_with_root(tmp.path());

        let mut session = SessionInfo {
            agent_id: "agent-1".into(),
            run_id: "run-1".into(),
            role: "executor".into(),
            cell_id: None,
            current_task: "build".into(),
            state: SessionState::Active,
            epoch: 1,
            last_handoff: None,
            created_at: 100,
            last_updated: 200,
        };

        writer.update_session(&session).unwrap();

        session.epoch = 2;
        session.state = SessionState::Completed;
        writer.update_session(&session).unwrap();

        let path = tmp.path().join("state/sessions/agent-1.json");
        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: SessionInfo = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.epoch, 2);
        assert_eq!(loaded.state, SessionState::Completed);
    }

    #[test]
    fn write_team_run_creates_file() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_with_root(tmp.path());

        let record = TeamRunRecord {
            run_id: "run-1".into(),
            team_name: "alpha".into(),
            mission_path: "/tmp/mission.md".into(),
            tracker: Some("github".into()),
            issue_ref: Some("#42".into()),
            started_at: 1000,
            ended_at: None,
            status: "active".into(),
            agent_count: 3,
        };

        writer.write_team_run(&record).unwrap();

        let path = tmp.path().join("team/runs/run-1.json");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: TeamRunRecord = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.run_id, "run-1");
        assert!(loaded.is_active());
    }

    #[test]
    fn write_team_run_no_tmp_left_behind() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_with_root(tmp.path());

        let record = TeamRunRecord {
            run_id: "run-1".into(),
            team_name: "alpha".into(),
            mission_path: "/tmp/mission.md".into(),
            tracker: None,
            issue_ref: None,
            started_at: 1000,
            ended_at: None,
            status: "active".into(),
            agent_count: 1,
        };

        writer.write_team_run(&record).unwrap();

        let tmp_path = tmp.path().join("team/runs/run-1.json.tmp");
        assert!(!tmp_path.exists());
    }
}
