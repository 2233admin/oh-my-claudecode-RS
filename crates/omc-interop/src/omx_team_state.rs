use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum OmxTeamError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Team not found: {0}")]
    TeamNotFound(String),
}

pub type Result<T> = std::result::Result<T, OmxTeamError>;

// ============================================================================
// Types (matching omx team state format)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmxTeamConfig {
    pub name: String,
    pub task: String,
    pub agent_type: String,
    pub worker_count: u32,
    pub max_workers: u32,
    pub workers: Vec<OmxWorkerInfo>,
    pub created_at: DateTime<Utc>,
    pub tmux_session: String,
    pub next_task_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmxWorkerInfo {
    pub name: String,
    pub index: u32,
    pub role: String,
    pub assigned_tasks: Vec<String>,
    pub pid: Option<u32>,
    pub pane_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmxTeamTask {
    pub id: String,
    pub subject: String,
    pub description: String,
    pub status: OmxTaskStatus,
    pub requires_code_change: Option<bool>,
    pub owner: Option<String>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub blocked_by: Option<Vec<String>>,
    pub depends_on: Option<Vec<String>>,
    pub version: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OmxTaskStatus {
    Pending,
    Blocked,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmxTeamMailboxMessage {
    pub message_id: String,
    pub from_worker: String,
    pub to_worker: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub notified_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmxTeamMailbox {
    pub worker: String,
    pub messages: Vec<OmxTeamMailboxMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmxTeamManifestV2 {
    pub schema_version: u32,
    pub name: String,
    pub task: String,
    pub tmux_session: String,
    pub worker_count: u32,
    pub workers: Vec<OmxWorkerInfo>,
    pub next_task_id: u32,
    pub created_at: DateTime<Utc>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OmxTeamEventType {
    TaskCompleted,
    WorkerIdle,
    WorkerStopped,
    MessageReceived,
    ShutdownAck,
    ApprovalDecision,
    TeamLeaderNudge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OmxNextAction {
    Shutdown,
    ReuseCurrentTeam,
    LaunchNewTeam,
    KeepCheckingStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmxTeamEvent {
    pub event_id: String,
    pub team: String,
    pub event_type: OmxTeamEventType,
    pub worker: String,
    pub task_id: Option<String>,
    pub message_id: Option<String>,
    pub reason: Option<String>,
    pub next_action: Option<OmxNextAction>,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// Path helpers
// ============================================================================

fn omx_state_dir(cwd: &str) -> PathBuf {
    PathBuf::from(cwd).join(".omx").join("state")
}

fn team_dir(team_name: &str, cwd: &str) -> PathBuf {
    omx_state_dir(cwd).join("team").join(team_name)
}

fn _mailbox_path(team_name: &str, worker_name: &str, cwd: &str) -> PathBuf {
    team_dir(team_name, cwd)
        .join("mailbox")
        .join(format!("{worker_name}.json"))
}

fn _task_file_path(team_name: &str, task_id: &str, cwd: &str) -> PathBuf {
    team_dir(team_name, cwd)
        .join("tasks")
        .join(format!("task-{task_id}.json"))
}

fn _event_log_path(team_name: &str, cwd: &str) -> PathBuf {
    team_dir(team_name, cwd)
        .join("events")
        .join("events.ndjson")
}

// ============================================================================
// Discovery
// ============================================================================

/// List active omx teams by scanning `.omx/state/team/` subdirectories.
pub fn list_omx_teams(cwd: &str) -> Result<Vec<String>> {
    let teams_root = omx_state_dir(cwd).join("team");

    if !teams_root.exists() {
        return Ok(Vec::new());
    }

    let mut names = Vec::new();
    for entry in std::fs::read_dir(&teams_root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && let Some(name) = entry.file_name().to_str()
        {
            names.push(name.to_string());
        }
    }

    names.sort();
    Ok(names)
}

// ============================================================================
// Config
// ============================================================================

/// Read team config. Tries `manifest.v2.json` first, falls back to `config.json`.
pub fn read_omx_team_config(team_name: &str, cwd: &str) -> Result<Option<OmxTeamConfig>> {
    let root = team_dir(team_name, cwd);
    if !root.exists() {
        return Ok(None);
    }

    // Try manifest.v2.json first
    let manifest_path = root.join("manifest.v2.json");
    if manifest_path.exists()
        && let Ok(content) = std::fs::read_to_string(&manifest_path)
        && let Ok(manifest) = serde_json::from_str::<OmxTeamManifestV2>(&content)
    {
        return Ok(Some(OmxTeamConfig {
            name: manifest.name,
            task: manifest.task,
            agent_type: manifest
                .workers
                .first()
                .map(|w| w.role.clone())
                .unwrap_or_else(|| "executor".to_string()),
            worker_count: manifest.worker_count,
            max_workers: 20,
            workers: manifest.workers,
            created_at: manifest.created_at,
            tmux_session: manifest.tmux_session,
            next_task_id: manifest.next_task_id,
        }));
    }

    // Fall back to config.json
    let config_path = root.join("config.json");
    if !config_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_path)?;
    let config: OmxTeamConfig = serde_json::from_str(&content)?;
    Ok(Some(config))
}

// ============================================================================
// Mailbox
// ============================================================================

/// Read a worker's mailbox.
pub fn read_omx_mailbox(team_name: &str, worker_name: &str, cwd: &str) -> OmxTeamMailbox {
    let p = _mailbox_path(team_name, worker_name, cwd);

    if !p.exists() {
        return OmxTeamMailbox {
            worker: worker_name.to_string(),
            messages: Vec::new(),
        };
    }

    let Ok(content) = std::fs::read_to_string(&p) else {
        return OmxTeamMailbox {
            worker: worker_name.to_string(),
            messages: Vec::new(),
        };
    };

    let Ok(parsed) = serde_json::from_str::<OmxTeamMailbox>(&content) else {
        return OmxTeamMailbox {
            worker: worker_name.to_string(),
            messages: Vec::new(),
        };
    };

    parsed
}

/// List all messages in a worker's mailbox.
pub fn list_omx_mailbox_messages(
    team_name: &str,
    worker_name: &str,
    cwd: &str,
) -> Vec<OmxTeamMailboxMessage> {
    read_omx_mailbox(team_name, worker_name, cwd).messages
}

/// Send a direct message to an omx worker's mailbox.
pub fn send_omx_direct_message(
    team_name: &str,
    from_worker: &str,
    to_worker: &str,
    body: &str,
    cwd: &str,
) -> Result<OmxTeamMailboxMessage> {
    let msg = OmxTeamMailboxMessage {
        message_id: Uuid::new_v4().to_string(),
        from_worker: from_worker.to_string(),
        to_worker: to_worker.to_string(),
        body: body.to_string(),
        created_at: Utc::now(),
        notified_at: None,
        delivered_at: None,
    };

    let mut mailbox = read_omx_mailbox(team_name, to_worker, cwd);
    mailbox.messages.push(msg.clone());
    let p = _mailbox_path(team_name, to_worker, cwd);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(&mailbox)?)?;

    // Append event
    let _ = append_omx_team_event(
        team_name,
        OmxTeamEventType::MessageReceived,
        to_worker,
        cwd,
        None,
        Some(&msg.message_id),
        None,
    );

    Ok(msg)
}

/// Broadcast a message to all workers in an omx team.
pub fn broadcast_omx_message(
    team_name: &str,
    from_worker: &str,
    body: &str,
    cwd: &str,
) -> Result<Vec<OmxTeamMailboxMessage>> {
    let config = read_omx_team_config(team_name, cwd)?
        .ok_or_else(|| OmxTeamError::TeamNotFound(team_name.to_string()))?;

    let mut delivered = Vec::new();
    for w in &config.workers {
        if w.name == from_worker {
            continue;
        }
        delivered.push(send_omx_direct_message(
            team_name,
            from_worker,
            &w.name,
            body,
            cwd,
        )?);
    }

    Ok(delivered)
}

// ============================================================================
// Tasks
// ============================================================================

/// Read a single omx team task.
pub fn read_omx_task(team_name: &str, task_id: &str, cwd: &str) -> Option<OmxTeamTask> {
    let p = _task_file_path(team_name, task_id, cwd);
    if !p.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&p).ok()?;
    serde_json::from_str(&content).ok()
}

/// List all tasks in an omx team.
pub fn list_omx_tasks(team_name: &str, cwd: &str) -> Result<Vec<OmxTeamTask>> {
    let tasks_root = team_dir(team_name, cwd).join("tasks");

    if !tasks_root.exists() {
        return Ok(Vec::new());
    }

    let mut tasks = Vec::new();

    for entry in std::fs::read_dir(&tasks_root)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Match task-{id}.json
        let Some(id) = name
            .strip_prefix("task-")
            .and_then(|s| s.strip_suffix(".json"))
        else {
            continue;
        };

        if let Some(task) = read_omx_task(team_name, id, cwd) {
            tasks.push(task);
        }
    }

    tasks.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(tasks)
}

// ============================================================================
// Events
// ============================================================================

/// Append an event to the omx team event log (append-only ndjson).
pub fn append_omx_team_event(
    team_name: &str,
    event_type: OmxTeamEventType,
    worker: &str,
    cwd: &str,
    task_id: Option<&str>,
    message_id: Option<&str>,
    reason: Option<&str>,
) -> Result<OmxTeamEvent> {
    let full = OmxTeamEvent {
        event_id: Uuid::new_v4().to_string(),
        team: team_name.to_string(),
        event_type,
        worker: worker.to_string(),
        task_id: task_id.map(String::from),
        message_id: message_id.map(String::from),
        reason: reason.map(String::from),
        next_action: None,
        message: None,
        created_at: Utc::now(),
    };

    let p = _event_log_path(team_name, cwd);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let line = format!("{}\n", serde_json::to_string(&full)?);

    // Concurrency invariant: a single `write_all` against a file opened with
    // `O_APPEND` is atomic across concurrent writers when the payload is
    // smaller than the OS append-atomic limit (Linux PIPE_BUF = 4096 bytes,
    // Windows FILE_APPEND_DATA path conservatively 1024 bytes). Reject lines
    // that exceed the conservative minimum so multi-agent writers never
    // interleave -- this matches the contract enforced in
    // `omc-team::communication::append_jsonl`. If a future event genuinely
    // needs to exceed this, switch to an advisory file lock (e.g. `fs2`)
    // rather than relaxing the guard. (gemini-code-assist HIGH.)
    const ATOMIC_APPEND_LIMIT: usize = 1024;
    if line.len() > ATOMIC_APPEND_LIMIT {
        return Err(OmxTeamError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "omx team event line ({} bytes) exceeds atomic-append limit ({} bytes); \
                 concurrent writers could interleave",
                line.len(),
                ATOMIC_APPEND_LIMIT
            ),
        )));
    }

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&p)?;
    file.write_all(line.as_bytes())?;
    file.flush()?;

    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_omx_config_returns_none_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let config = read_omx_team_config("my-team", cwd).unwrap();
        assert!(config.is_none());
    }

    #[test]
    fn read_omx_tasks_returns_empty_when_no_tasks_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let tasks = list_omx_tasks("my-team", cwd).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn read_omx_mailbox_returns_empty_when_no_mailbox() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let mailbox = read_omx_mailbox("my-team", "worker-1", cwd);
        assert_eq!(mailbox.worker, "worker-1");
        assert!(mailbox.messages.is_empty());
    }

    #[test]
    fn list_omx_teams_returns_empty_when_no_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let teams = list_omx_teams(cwd).unwrap();
        assert!(teams.is_empty());
    }

    #[test]
    fn list_omx_teams_discovers_teams() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let teams_root = PathBuf::from(cwd).join(".omx").join("state").join("team");
        std::fs::create_dir_all(teams_root.join("alpha")).unwrap();
        std::fs::create_dir_all(teams_root.join("beta")).unwrap();

        let teams = list_omx_teams(cwd).unwrap();
        assert_eq!(teams, vec!["alpha", "beta"]);
    }

    #[test]
    fn read_config_from_config_json() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let team_root = PathBuf::from(cwd)
            .join(".omx")
            .join("state")
            .join("team")
            .join("test-team");
        std::fs::create_dir_all(&team_root).unwrap();

        let config = OmxTeamConfig {
            name: "test-team".to_string(),
            task: "do something".to_string(),
            agent_type: "executor".to_string(),
            worker_count: 2,
            max_workers: 5,
            workers: vec![
                OmxWorkerInfo {
                    name: "w1".to_string(),
                    index: 0,
                    role: "executor".to_string(),
                    assigned_tasks: vec![],
                    pid: None,
                    pane_id: None,
                },
                OmxWorkerInfo {
                    name: "w2".to_string(),
                    index: 1,
                    role: "executor".to_string(),
                    assigned_tasks: vec![],
                    pid: None,
                    pane_id: None,
                },
            ],
            created_at: Utc::now(),
            tmux_session: "tmux-1".to_string(),
            next_task_id: 1,
        };

        let config_json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(team_root.join("config.json"), config_json).unwrap();

        let result = read_omx_team_config("test-team", cwd).unwrap().unwrap();
        assert_eq!(result.name, "test-team");
        assert_eq!(result.worker_count, 2);
        assert_eq!(result.workers.len(), 2);
    }

    #[test]
    fn read_config_prefers_manifest_v2() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let team_root = PathBuf::from(cwd)
            .join(".omx")
            .join("state")
            .join("team")
            .join("v2-team");
        std::fs::create_dir_all(&team_root).unwrap();

        let manifest = OmxTeamManifestV2 {
            schema_version: 2,
            name: "v2-team".to_string(),
            task: "v2 task".to_string(),
            tmux_session: "tmux-v2".to_string(),
            worker_count: 1,
            workers: vec![OmxWorkerInfo {
                name: "lead".to_string(),
                index: 0,
                role: "planner".to_string(),
                assigned_tasks: vec![],
                pid: None,
                pane_id: None,
            }],
            next_task_id: 1,
            created_at: Utc::now(),
            extra: std::collections::HashMap::new(),
        };

        let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
        std::fs::write(team_root.join("manifest.v2.json"), manifest_json).unwrap();

        let result = read_omx_team_config("v2-team", cwd).unwrap().unwrap();
        assert_eq!(result.name, "v2-team");
        assert_eq!(result.agent_type, "planner");
    }

    #[test]
    fn send_and_read_direct_message() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        // Create minimal team structure
        let team_root = PathBuf::from(cwd)
            .join(".omx")
            .join("state")
            .join("team")
            .join("msg-team");
        std::fs::create_dir_all(&team_root).unwrap();

        let msg = send_omx_direct_message("msg-team", "w1", "w2", "ping", cwd).unwrap();
        assert_eq!(msg.from_worker, "w1");
        assert_eq!(msg.to_worker, "w2");
        assert_eq!(msg.body, "ping");

        let messages = list_omx_mailbox_messages("msg-team", "w2", cwd);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].body, "ping");
    }

    #[test]
    fn list_omx_tasks_returns_empty_when_no_tasks() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let tasks = list_omx_tasks("team-x", cwd).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn append_and_read_event() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let event = append_omx_team_event(
            "evt-team",
            OmxTeamEventType::TaskCompleted,
            "worker-1",
            cwd,
            Some("task-1"),
            None,
            Some("done"),
        )
        .unwrap();

        assert_eq!(event.team, "evt-team");
        assert_eq!(event.event_type, OmxTeamEventType::TaskCompleted);
        assert_eq!(event.task_id, Some("task-1".to_string()));
        assert_eq!(event.reason, Some("done".to_string()));

        // Verify the ndjson file
        let events_path = team_dir("evt-team", cwd)
            .join("events")
            .join("events.ndjson");
        assert!(events_path.exists());
        let content = std::fs::read_to_string(&events_path).unwrap();
        assert!(content.contains("task_completed"));
    }

    #[test]
    fn omx_task_status_serde_roundtrip() {
        let statuses = vec![
            OmxTaskStatus::Pending,
            OmxTaskStatus::Blocked,
            OmxTaskStatus::InProgress,
            OmxTaskStatus::Completed,
            OmxTaskStatus::Failed,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: OmxTaskStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, deserialized);
        }
    }
}
