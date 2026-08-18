pub mod mcp_bridge;
pub mod omx_team_state;
pub mod shared_state;

use serde::{Deserialize, Serialize};

use crate::mcp_bridge::{InteropMode, can_use_omx_direct_write_bridge, get_interop_mode};
use crate::omx_team_state::{OmxTaskStatus, OmxTeamTask, OmxWorkerInfo};
use crate::shared_state::{
    InteropConfig, InteropSide, SharedMessage, SharedTask, TaskStatus, TaskType,
};

pub const INTEROP_SNAPSHOT_SCHEMA_VERSION: &str = "omc.interop.snapshot.v1";
const DEFAULT_SNAPSHOT_LIMIT: usize = 20;
const MAX_SNAPSHOT_LIMIT: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteropSnapshot {
    pub schema_version: String,
    pub read_only: bool,
    pub working_directory: String,
    pub interop_mode: InteropMode,
    pub direct_write_enabled: bool,
    pub config: Option<InteropConfig>,
    pub shared_task_count: usize,
    pub shared_tasks: Vec<SharedTask>,
    pub shared_message_count: usize,
    pub shared_messages: Vec<SharedMessage>,
    pub normalized_task_count: usize,
    pub normalized_tasks: Vec<InteropTaskView>,
    pub omx_teams: Vec<OmxTeamSnapshot>,
}

/// Cross-runtime task state. This is a projection only; native state remains
/// owned by the OMC or OMX runtime that wrote it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedTaskStatus {
    Pending,
    Blocked,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteropTaskView {
    pub id: String,
    pub native_id: String,
    pub source: InteropSide,
    pub target: Option<InteropSide>,
    pub team: Option<String>,
    pub task_type: Option<TaskType>,
    pub subject: String,
    pub status: UnifiedTaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmxTeamSnapshot {
    pub name: String,
    pub worker_count: usize,
    pub workers: Vec<OmxWorkerInfo>,
    pub task_count: usize,
    pub tasks: Vec<OmxTeamTask>,
}

fn normalize_shared_status(status: &TaskStatus) -> UnifiedTaskStatus {
    match status {
        TaskStatus::Pending => UnifiedTaskStatus::Pending,
        TaskStatus::InProgress => UnifiedTaskStatus::InProgress,
        TaskStatus::Completed => UnifiedTaskStatus::Completed,
        TaskStatus::Failed => UnifiedTaskStatus::Failed,
    }
}

fn normalize_omx_status(status: &OmxTaskStatus) -> UnifiedTaskStatus {
    match status {
        OmxTaskStatus::Pending => UnifiedTaskStatus::Pending,
        OmxTaskStatus::Blocked => UnifiedTaskStatus::Blocked,
        OmxTaskStatus::InProgress => UnifiedTaskStatus::InProgress,
        OmxTaskStatus::Completed => UnifiedTaskStatus::Completed,
        OmxTaskStatus::Failed => UnifiedTaskStatus::Failed,
    }
}

fn normalize_shared_task(task: &SharedTask) -> InteropTaskView {
    InteropTaskView {
        id: format!("omc/{}", task.id),
        native_id: task.id.clone(),
        source: task.source.clone(),
        target: Some(task.target.clone()),
        team: None,
        task_type: Some(task.task_type.clone()),
        subject: task.description.clone(),
        status: normalize_shared_status(&task.status),
    }
}

fn normalize_omx_task(team: &str, task: &OmxTeamTask) -> InteropTaskView {
    InteropTaskView {
        id: format!("omx/{team}/{}", task.id),
        native_id: task.id.clone(),
        source: InteropSide::Omx,
        target: None,
        team: Some(team.to_string()),
        task_type: None,
        subject: task.subject.clone(),
        status: normalize_omx_status(&task.status),
    }
}

/// Read the bounded OMC/OMX state projection without starting or mutating a runtime.
pub fn read_snapshot(cwd: &str, limit: Option<usize>) -> shared_state::Result<InteropSnapshot> {
    let limit = limit.unwrap_or(DEFAULT_SNAPSHOT_LIMIT);
    if !(1..=MAX_SNAPSHOT_LIMIT).contains(&limit) {
        return Err(shared_state::InteropError::InvalidLimit(limit));
    }

    let shared_tasks = shared_state::read_shared_tasks(cwd, None)?;
    let shared_messages = shared_state::read_shared_messages(cwd, None)?;
    let mut normalized_tasks: Vec<_> = shared_tasks.iter().map(normalize_shared_task).collect();
    let mut omx_teams = Vec::new();

    for name in omx_team_state::list_omx_teams(cwd).map_err(|error| {
        shared_state::InteropError::InvalidName(format!("failed to read OMX teams: {error}"))
    })? {
        let config = omx_team_state::read_omx_team_config(&name, cwd).map_err(|error| {
            shared_state::InteropError::InvalidName(format!(
                "failed to read OMX team {name}: {error}"
            ))
        })?;
        let tasks = omx_team_state::list_omx_tasks(&name, cwd).map_err(|error| {
            shared_state::InteropError::InvalidName(format!(
                "failed to read OMX tasks for {name}: {error}"
            ))
        })?;
        normalized_tasks.extend(tasks.iter().map(|task| normalize_omx_task(&name, task)));
        let workers = config
            .as_ref()
            .map_or_else(Vec::new, |config| config.workers.clone());
        omx_teams.push(OmxTeamSnapshot {
            name,
            worker_count: workers.len(),
            workers,
            task_count: tasks.len(),
            tasks: tasks.into_iter().take(limit).collect(),
        });
    }

    Ok(InteropSnapshot {
        schema_version: INTEROP_SNAPSHOT_SCHEMA_VERSION.into(),
        read_only: true,
        working_directory: cwd.into(),
        interop_mode: get_interop_mode(),
        direct_write_enabled: can_use_omx_direct_write_bridge(),
        config: shared_state::read_interop_config(cwd)?,
        shared_task_count: shared_tasks.len(),
        shared_tasks: shared_tasks.into_iter().take(limit).collect(),
        shared_message_count: shared_messages.len(),
        shared_messages: shared_messages.into_iter().take(limit).collect(),
        normalized_task_count: normalized_tasks.len(),
        normalized_tasks: normalized_tasks.into_iter().take(limit).collect(),
        omx_teams,
    })
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use crate::omx_team_state::{OmxTaskStatus, OmxTeamConfig};
    use crate::shared_state::{add_shared_message, add_shared_task, init_interop_session};
    use chrono::Utc;

    #[test]
    fn snapshot_reads_bounded_omc_and_omx_state() {
        let root = tempfile::tempdir().unwrap();
        let cwd = root.path().to_str().unwrap();
        init_interop_session("snapshot-test", cwd, None).unwrap();
        add_shared_task(
            cwd,
            crate::shared_state::InteropSide::Omc,
            crate::shared_state::InteropSide::Omx,
            crate::shared_state::TaskType::Analyze,
            "inspect",
            None,
            None,
        )
        .unwrap();
        add_shared_message(
            cwd,
            crate::shared_state::InteropSide::Omx,
            crate::shared_state::InteropSide::Omc,
            "done",
            None,
        )
        .unwrap();

        let team_dir = root.path().join(".omx/state/team/demo");
        std::fs::create_dir_all(team_dir.join("tasks")).unwrap();
        let config = OmxTeamConfig {
            name: "demo".into(),
            task: "inspect".into(),
            agent_type: "executor".into(),
            worker_count: 1,
            max_workers: 1,
            workers: Vec::new(),
            created_at: Utc::now(),
            tmux_session: "demo".into(),
            next_task_id: 2,
        };
        std::fs::write(
            team_dir.join("config.json"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        let task = OmxTeamTask {
            id: "1".into(),
            subject: "inspect".into(),
            description: "inspect".into(),
            status: OmxTaskStatus::Completed,
            requires_code_change: Some(false),
            owner: None,
            result: Some("done".into()),
            error: None,
            blocked_by: None,
            depends_on: None,
            version: Some(1),
            created_at: Utc::now(),
            completed_at: Some(Utc::now()),
        };
        std::fs::write(
            team_dir.join("tasks/task-1.json"),
            serde_json::to_vec(&task).unwrap(),
        )
        .unwrap();

        let snapshot = read_snapshot(cwd, Some(1)).unwrap();
        assert_eq!(snapshot.schema_version, INTEROP_SNAPSHOT_SCHEMA_VERSION);
        assert!(snapshot.read_only);
        assert_eq!(snapshot.shared_task_count, 1);
        assert_eq!(snapshot.shared_tasks.len(), 1);
        assert_eq!(snapshot.shared_message_count, 1);
        assert_eq!(snapshot.omx_teams.len(), 1);
        assert_eq!(snapshot.omx_teams[0].task_count, 1);
        assert_eq!(snapshot.omx_teams[0].tasks.len(), 1);
        assert_eq!(snapshot.normalized_task_count, 2);
        assert_eq!(snapshot.normalized_tasks.len(), 1);
        assert_eq!(
            snapshot.normalized_tasks[0].status,
            UnifiedTaskStatus::Pending
        );
    }

    #[test]
    fn normalized_status_preserves_omx_blocked_state() {
        assert_eq!(
            normalize_omx_status(&OmxTaskStatus::Blocked),
            UnifiedTaskStatus::Blocked
        );
        assert_eq!(
            normalize_shared_status(&TaskStatus::InProgress),
            UnifiedTaskStatus::InProgress
        );
    }

    #[test]
    fn snapshot_rejects_invalid_limit() {
        let root = tempfile::tempdir().unwrap();
        let error = read_snapshot(root.path().to_str().unwrap(), Some(0)).unwrap_err();
        assert!(error.to_string().contains("limit"));
    }
}
