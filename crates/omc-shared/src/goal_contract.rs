//! Stable goal and checkpoint contract for cross-host OMC workflows.
//!
//! A goal is control-plane state. It does not own an Agent loop, provider
//! call, or task executor. Task IDs may be attached for correlation, while
//! task lifecycle remains governed by omc.task.v1.

use serde::{Deserialize, Serialize};

use crate::operation_contract::ArtifactRef;

pub const GOAL_SCHEMA_VERSION: &str = "omc.goal.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Planned,
    Active,
    Blocked,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoalCheckpoint {
    pub checkpoint_id: String,
    pub summary: String,
    pub recorded_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<ArtifactRef>,
}

impl GoalCheckpoint {
    pub fn validate(&self) -> Result<(), String> {
        validate_identifier(&self.checkpoint_id, "checkpoint_id")?;
        validate_non_empty(&self.summary, "checkpoint summary")?;
        validate_non_empty(&self.recorded_at, "checkpoint recorded_at")?;
        for reference in &self.artifact_refs {
            reference.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoalRecord {
    pub schema_version: String,
    pub goal_id: String,
    pub objective: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub status: GoalStatus,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch_task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoints: Vec<GoalCheckpoint>,
}

impl GoalRecord {
    pub fn new(
        goal_id: impl Into<String>,
        objective: impl Into<String>,
        created_at: impl Into<String>,
    ) -> Self {
        let created_at = created_at.into();
        Self {
            schema_version: GOAL_SCHEMA_VERSION.into(),
            goal_id: goal_id.into(),
            objective: objective.into(),
            owner: None,
            status: GoalStatus::Planned,
            created_at: created_at.clone(),
            updated_at: created_at,
            blocker: None,
            dispatch_task_id: None,
            checkpoints: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != GOAL_SCHEMA_VERSION {
            return Err("goal schema version is not supported".into());
        }
        validate_identifier(&self.goal_id, "goal_id")?;
        validate_non_empty(&self.objective, "goal objective")?;
        validate_non_empty(&self.created_at, "goal created_at")?;
        validate_non_empty(&self.updated_at, "goal updated_at")?;
        if let Some(owner) = &self.owner {
            validate_non_empty(owner, "goal owner")?;
        }
        if let Some(blocker) = &self.blocker {
            validate_non_empty(blocker, "goal blocker")?;
        }
        if self.status == GoalStatus::Blocked && self.blocker.is_none() {
            return Err("blocked goals must include a blocker".into());
        }
        if self.status != GoalStatus::Blocked && self.blocker.is_some() {
            return Err("only blocked goals may include a blocker".into());
        }
        if let Some(task_id) = &self.dispatch_task_id {
            validate_identifier(task_id, "dispatch_task_id")?;
        }
        for checkpoint in &self.checkpoints {
            checkpoint.validate()?;
        }
        Ok(())
    }

    pub fn start(&mut self, updated_at: impl Into<String>) -> Result<(), String> {
        if !matches!(self.status, GoalStatus::Planned | GoalStatus::Blocked) {
            return Err(format!("cannot start a {} goal", status_name(self.status)));
        }
        self.status = GoalStatus::Active;
        self.blocker = None;
        self.updated_at = updated_at.into();
        self.validate()
    }

    pub fn block(
        &mut self,
        blocker: impl Into<String>,
        updated_at: impl Into<String>,
    ) -> Result<(), String> {
        if self.status == GoalStatus::Completed {
            return Err("cannot block a completed goal".into());
        }
        self.status = GoalStatus::Blocked;
        self.blocker = Some(blocker.into());
        self.updated_at = updated_at.into();
        self.validate()
    }

    pub fn complete(&mut self, updated_at: impl Into<String>) -> Result<(), String> {
        if self.status != GoalStatus::Active {
            return Err(format!(
                "only active goals can be completed; current status is {}",
                status_name(self.status)
            ));
        }
        self.status = GoalStatus::Completed;
        self.blocker = None;
        self.updated_at = updated_at.into();
        self.validate()
    }

    pub fn attach_task(&mut self, task_id: impl Into<String>) -> Result<(), String> {
        let task_id = task_id.into();
        validate_identifier(&task_id, "dispatch_task_id")?;
        self.dispatch_task_id = Some(task_id);
        self.validate()
    }

    pub fn add_checkpoint(&mut self, checkpoint: GoalCheckpoint) -> Result<(), String> {
        if self.status == GoalStatus::Completed {
            return Err("cannot add a checkpoint to a completed goal".into());
        }
        checkpoint.validate()?;
        if self
            .checkpoints
            .iter()
            .any(|existing| existing.checkpoint_id == checkpoint.checkpoint_id)
        {
            return Err(format!(
                "checkpoint already exists: {}",
                checkpoint.checkpoint_id
            ));
        }
        self.updated_at = checkpoint.recorded_at.clone();
        self.checkpoints.push(checkpoint);
        Ok(())
    }
}

fn status_name(status: GoalStatus) -> &'static str {
    match status {
        GoalStatus::Planned => "planned",
        GoalStatus::Active => "active",
        GoalStatus::Blocked => "blocked",
        GoalStatus::Completed => "completed",
    }
}

fn validate_non_empty(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_identifier(value: &str, field: &str) -> Result<(), String> {
    validate_non_empty(value, field)?;
    if value.len() > 128 || value == "." || value == ".." {
        return Err(format!("{field} is not a valid identifier"));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(format!(
            "{field} may contain only ASCII letters, digits, '.', '_' or '-'"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn goal() -> GoalRecord {
        GoalRecord::new("goal-1", "internalize upstream capabilities", "t0")
    }

    #[test]
    fn lifecycle_requires_active_before_completion() {
        let mut goal = goal();
        assert!(goal.complete("t1").is_err());
        goal.start("t2").unwrap();
        goal.complete("t3").unwrap();
        assert_eq!(goal.status, GoalStatus::Completed);
        assert!(goal.start("t4").is_err());
    }

    #[test]
    fn blocked_goal_can_resume_and_carries_reason() {
        let mut goal = goal();
        goal.block("waiting for source confirmation", "t1").unwrap();
        assert_eq!(goal.status, GoalStatus::Blocked);
        assert_eq!(
            goal.blocker.as_deref(),
            Some("waiting for source confirmation")
        );
        goal.start("t2").unwrap();
        assert_eq!(goal.status, GoalStatus::Active);
        assert!(goal.blocker.is_none());
    }

    #[test]
    fn checkpoints_reject_duplicates_and_roundtrip() {
        let mut goal = goal();
        goal.add_checkpoint(GoalCheckpoint {
            checkpoint_id: "cp-1".into(),
            summary: "S0 verified".into(),
            recorded_at: "t1".into(),
            artifact_refs: Vec::new(),
        })
        .unwrap();
        assert!(
            goal.add_checkpoint(GoalCheckpoint {
                checkpoint_id: "cp-1".into(),
                summary: "duplicate".into(),
                recorded_at: "t2".into(),
                artifact_refs: Vec::new(),
            })
            .is_err()
        );
        let restored: GoalRecord =
            serde_json::from_value(serde_json::to_value(&goal).unwrap()).unwrap();
        assert_eq!(restored.checkpoints.len(), 1);
        assert_eq!(restored.schema_version, GOAL_SCHEMA_VERSION);
    }

    #[test]
    fn validation_rejects_path_like_identifiers() {
        let mut goal = goal();
        goal.goal_id = "../escape".into();
        assert!(goal.validate().is_err());
    }
}
