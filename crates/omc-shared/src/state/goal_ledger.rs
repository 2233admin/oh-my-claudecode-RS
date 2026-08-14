//! Project-scoped durable goal ledger.
//!
//! Goal files live under the project OMC state directory. Writes use the same
//! temporary-file plus rename pattern as the existing state writer.

use std::fs;
use std::path::{Path, PathBuf};

use super::StateError;
use crate::config::OmcPaths;
use crate::goal_contract::GoalRecord;

#[derive(Debug, Clone)]
pub struct GoalLedger {
    paths: OmcPaths,
}

impl GoalLedger {
    pub fn new(paths: OmcPaths) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &OmcPaths {
        &self.paths
    }

    pub fn create(&self, goal: &GoalRecord) -> Result<(), StateError> {
        goal.validate().map_err(StateError::Invalid)?;
        let path = self.goal_path(&goal.goal_id)?;
        if path.exists() {
            return Err(StateError::Invalid(format!(
                "goal already exists: {}",
                goal.goal_id
            )));
        }
        self.write(&path, goal)
    }

    pub fn load(&self, goal_id: &str) -> Result<GoalRecord, StateError> {
        let path = self.goal_path(goal_id)?;
        if !path.exists() {
            return Err(StateError::NotFound(goal_id.to_string()));
        }
        let content = fs::read_to_string(&path)?;
        let goal: GoalRecord = serde_json::from_str(&content)?;
        goal.validate().map_err(StateError::Invalid)?;
        Ok(goal)
    }

    pub fn save(&self, goal: &GoalRecord) -> Result<(), StateError> {
        goal.validate().map_err(StateError::Invalid)?;
        let path = self.goal_path(&goal.goal_id)?;
        if !path.exists() {
            return Err(StateError::NotFound(goal.goal_id.clone()));
        }
        self.write(&path, goal)
    }

    pub fn list(&self) -> Result<Vec<GoalRecord>, StateError> {
        if !self.paths.goals.exists() {
            return Ok(Vec::new());
        }

        let mut goals = Vec::new();
        for entry in fs::read_dir(&self.paths.goals)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let content = fs::read_to_string(&path)?;
            let goal: GoalRecord = serde_json::from_str(&content)?;
            goal.validate().map_err(StateError::Invalid)?;
            goals.push(goal);
        }
        goals.sort_by(|left, right| left.goal_id.cmp(&right.goal_id));
        Ok(goals)
    }

    fn goal_path(&self, goal_id: &str) -> Result<PathBuf, StateError> {
        validate_identifier(goal_id)?;
        Ok(self.paths.goals.join(format!("{goal_id}.json")))
    }

    fn write(&self, path: &Path, goal: &GoalRecord) -> Result<(), StateError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let content = serde_json::to_vec_pretty(goal)?;
        fs::write(&tmp, content)?;
        match fs::rename(&tmp, path) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = fs::remove_file(&tmp);
                Err(StateError::Io(error))
            }
        }
    }
}

fn validate_identifier(value: &str) -> Result<(), StateError> {
    if value.is_empty()
        || value.len() > 128
        || value == "."
        || value == ".."
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(StateError::Invalid(format!(
            "invalid goal identifier: {value}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal_contract::{GoalRecord, GoalStatus};
    use tempfile::TempDir;

    fn ledger(tmp: &TempDir) -> GoalLedger {
        GoalLedger::new(OmcPaths::new_with_root(tmp.path().join(".omc")))
    }

    #[test]
    fn create_load_update_and_list_use_project_state() {
        let tmp = TempDir::new().unwrap();
        let ledger = ledger(&tmp);
        let mut goal = GoalRecord::new("goal-1", "ship the adapter", "t0");

        ledger.create(&goal).unwrap();
        goal.start("t1").unwrap();
        ledger.save(&goal).unwrap();

        let loaded = ledger.load("goal-1").unwrap();
        assert_eq!(loaded.status, GoalStatus::Active);
        assert_eq!(ledger.list().unwrap().len(), 1);
        assert!(tmp.path().join(".omc/state/goals/goal-1.json").exists());
        assert!(!tmp.path().join(".omc/state/goals/goal-1.json.tmp").exists());
    }

    #[test]
    fn duplicate_and_traversal_ids_are_rejected() {
        let tmp = TempDir::new().unwrap();
        let ledger = ledger(&tmp);
        let goal = GoalRecord::new("goal-1", "ship the adapter", "t0");
        ledger.create(&goal).unwrap();
        assert!(ledger.create(&goal).is_err());
        assert!(ledger.load("../escape").is_err());
    }

    #[test]
    fn corrupt_goal_is_not_silently_ignored() {
        let tmp = TempDir::new().unwrap();
        let ledger = ledger(&tmp);
        fs::create_dir_all(&ledger.paths.goals).unwrap();
        fs::write(ledger.paths.goals.join("bad.json"), "not-json").unwrap();
        assert!(ledger.list().is_err());
    }
}
