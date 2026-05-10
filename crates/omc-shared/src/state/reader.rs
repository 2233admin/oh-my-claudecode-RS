//! State reader for reading application state from disk.

use std::fs;

use super::{HudState, SessionInfo, StateError, TeamRunRecord};
use crate::config::OmcPaths;

/// Reader for accessing persisted state files.
#[derive(Debug, Default)]
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


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn reader_with_root(root: &std::path::Path) -> StateReader {
        StateReader {
            paths: crate::config::OmcPaths::new_with_root(root.to_path_buf()),
        }
    }

    #[test]
    fn read_hud_state_missing_file_returns_default() {
        let tmp = TempDir::new().unwrap();
        let reader = reader_with_root(tmp.path());
        let state = reader.read_hud_state("nonexistent").unwrap();
        assert!(state.context_samples.is_empty());
        assert_eq!(state.session_id, "");
    }

    #[test]
    fn read_hud_state_parses_valid_json() {
        let tmp = TempDir::new().unwrap();
        let session_dir = tmp.path().join("state/sessions/test-sess");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("hud-state.json"),
            r#"{"session_id":"test-sess","context_samples":[{"ts_ms":100,"tokens":500}],"last_updated_ms":100}"#,
        ).unwrap();

        let reader = reader_with_root(tmp.path());
        let state = reader.read_hud_state("test-sess").unwrap();
        assert_eq!(state.session_id, "test-sess");
        assert_eq!(state.context_samples.len(), 1);
        assert_eq!(state.context_samples[0].tokens, 500);
    }

    #[test]
    fn read_hud_state_invalid_json_returns_error() {
        let tmp = TempDir::new().unwrap();
        let session_dir = tmp.path().join("state/sessions/bad-sess");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(session_dir.join("hud-state.json"), "not json").unwrap();

        let reader = reader_with_root(tmp.path());
        let result = reader.read_hud_state("bad-sess");
        assert!(result.is_err());
    }

    #[test]
    fn read_sessions_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let reader = reader_with_root(tmp.path());
        let sessions = reader.read_sessions().unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn read_sessions_parses_json_files() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("state/sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        let session = SessionInfo {
            agent_id: "agent-1".into(),
            run_id: "run-1".into(),
            role: "executor".into(),
            cell_id: None,
            current_task: "task".into(),
            state: SessionState::Active,
            epoch: 1,
            last_handoff: None,
            created_at: 100,
            last_updated: 200,
        };
        let json = serde_json::to_string(&session).unwrap();
        std::fs::write(sessions_dir.join("agent-1.json"), &json).unwrap();

        let reader = reader_with_root(tmp.path());
        let sessions = reader.read_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].agent_id, "agent-1");
    }

    #[test]
    fn read_sessions_skips_malformed_files() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("state/sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        // Valid file
        let session = SessionInfo {
            agent_id: "agent-1".into(),
            run_id: "run-1".into(),
            role: "executor".into(),
            cell_id: None,
            current_task: "task".into(),
            state: SessionState::Active,
            epoch: 1,
            last_handoff: None,
            created_at: 100,
            last_updated: 200,
        };
        std::fs::write(
            sessions_dir.join("good.json"),
            serde_json::to_string(&session).unwrap(),
        )
        .unwrap();
        // Invalid file
        std::fs::write(sessions_dir.join("bad.json"), "not json").unwrap();

        let reader = reader_with_root(tmp.path());
        let sessions = reader.read_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].agent_id, "agent-1");
    }

    #[test]
    fn read_sessions_sorted_by_last_updated_desc() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("state/sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        for (id, ts) in [("a", 100), ("b", 300), ("c", 200)] {
            let session = SessionInfo {
                agent_id: id.into(),
                run_id: format!("run-{}", id),
                role: "executor".into(),
                cell_id: None,
                current_task: "task".into(),
                state: SessionState::Active,
                epoch: 1,
                last_handoff: None,
                created_at: ts,
                last_updated: ts,
            };
            std::fs::write(
                sessions_dir.join(format!("{}.json", id)),
                serde_json::to_string(&session).unwrap(),
            )
            .unwrap();
        }

        let reader = reader_with_root(tmp.path());
        let sessions = reader.read_sessions().unwrap();
        assert_eq!(sessions.len(), 3);
        assert_eq!(sessions[0].agent_id, "b");
        assert_eq!(sessions[1].agent_id, "c");
        assert_eq!(sessions[2].agent_id, "a");
    }

    #[test]
    fn read_team_runs_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let reader = reader_with_root(tmp.path());
        let runs = reader.read_team_runs().unwrap();
        assert!(runs.is_empty());
    }

    #[test]
    fn read_team_runs_parses_records() {
        let tmp = TempDir::new().unwrap();
        let runs_dir = tmp.path().join("team/runs");
        std::fs::create_dir_all(&runs_dir).unwrap();

        let record = TeamRunRecord {
            run_id: "run-42".into(),
            team_name: "alpha".into(),
            mission_path: "/tmp/mission.md".into(),
            tracker: None,
            issue_ref: None,
            started_at: 1000,
            ended_at: None,
            status: "active".into(),
            agent_count: 3,
        };
        std::fs::write(
            runs_dir.join("run-42.json"),
            serde_json::to_string(&record).unwrap(),
        )
        .unwrap();

        let reader = reader_with_root(tmp.path());
        let runs = reader.read_team_runs().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, "run-42");
        assert!(runs[0].is_active());
    }

    #[test]
    fn read_team_runs_sorted_by_started_at_desc() {
        let tmp = TempDir::new().unwrap();
        let runs_dir = tmp.path().join("team/runs");
        std::fs::create_dir_all(&runs_dir).unwrap();

        for (id, ts) in [("r1", 1000), ("r2", 3000), ("r3", 2000)] {
            let record = TeamRunRecord {
                run_id: id.into(),
                team_name: "alpha".into(),
                mission_path: "/tmp/mission.md".into(),
                tracker: None,
                issue_ref: None,
                started_at: ts,
                ended_at: None,
                status: "active".into(),
                agent_count: 1,
            };
            std::fs::write(
                runs_dir.join(format!("{}.json", id)),
                serde_json::to_string(&record).unwrap(),
            )
            .unwrap();
        }

        let reader = reader_with_root(tmp.path());
        let runs = reader.read_team_runs().unwrap();
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].run_id, "r2");
        assert_eq!(runs[1].run_id, "r3");
        assert_eq!(runs[2].run_id, "r1");
    }
}
