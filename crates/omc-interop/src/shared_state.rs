use std::path::PathBuf;

use chrono::{DateTime, Utc};
use omc_shared::paths::validate_path_segment;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum InteropError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("invalid name: {0}")]
    InvalidName(String),
}

fn check_segment(name: &str, kind: &str) -> Result<()> {
    validate_path_segment(name, kind).map_err(|e| InteropError::InvalidName(e.to_string()))
}

pub type Result<T> = std::result::Result<T, InteropError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteropSide {
    Omc,
    Omx,
}

impl std::fmt::Display for InteropSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Omc => write!(f, "OMC"),
            Self::Omx => write!(f, "OMX"),
        }
    }
}

impl InteropSide {
    pub fn other(&self) -> Self {
        match self {
            Self::Omc => Self::Omx,
            Self::Omx => Self::Omc,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    Analyze,
    Implement,
    Review,
    Test,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteropConfig {
    pub session_id: String,
    pub created_at: DateTime<Utc>,
    pub omc_cwd: String,
    pub omx_cwd: Option<String>,
    pub status: InteropStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteropStatus {
    Active,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedTask {
    pub id: String,
    pub source: InteropSide,
    pub target: InteropSide,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub description: String,
    pub context: Option<serde_json::Value>,
    pub files: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub status: TaskStatus,
    pub result: Option<String>,
    pub error: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedMessage {
    pub id: String,
    pub source: InteropSide,
    pub target: InteropSide,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub read: bool,
}

pub struct SharedTaskFilter {
    pub source: Option<InteropSide>,
    pub target: Option<InteropSide>,
    pub status: Option<TaskStatus>,
}

pub struct SharedMessageFilter {
    pub source: Option<InteropSide>,
    pub target: Option<InteropSide>,
    pub unread_only: bool,
}

/// Get the interop directory path for a worktree: `{cwd}/.omc/state/interop/`
pub fn interop_dir(cwd: &str) -> PathBuf {
    PathBuf::from(cwd)
        .join(".omc")
        .join("state")
        .join("interop")
}

/// Initialize an interop session.
/// Creates the interop directory and writes session config.
pub fn init_interop_session(
    session_id: &str,
    omc_cwd: &str,
    omx_cwd: Option<&str>,
) -> Result<InteropConfig> {
    let dir = interop_dir(omc_cwd);
    std::fs::create_dir_all(&dir)?;

    let config = InteropConfig {
        session_id: session_id.to_string(),
        created_at: Utc::now(),
        omc_cwd: omc_cwd.to_string(),
        omx_cwd: omx_cwd.map(String::from),
        status: InteropStatus::Active,
    };

    let config_path = dir.join("config.json");
    std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

    Ok(config)
}

/// Read interop configuration if it exists.
pub fn read_interop_config(cwd: &str) -> Result<Option<InteropConfig>> {
    let config_path = interop_dir(cwd).join("config.json");

    if !config_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&config_path)?;
    let config: InteropConfig = serde_json::from_str(&content)?;
    Ok(Some(config))
}

fn generate_id(prefix: &str) -> String {
    let ts = Utc::now().timestamp_millis();
    let short = Uuid::new_v4().to_string().replace('-', "")[..9].to_string();
    format!("{prefix}-{ts}-{short}")
}

/// Add a shared task for cross-tool communication.
pub fn add_shared_task(
    cwd: &str,
    source: InteropSide,
    target: InteropSide,
    task_type: TaskType,
    description: &str,
    context: Option<serde_json::Value>,
    files: Option<Vec<String>>,
) -> Result<SharedTask> {
    let dir = interop_dir(cwd);
    let tasks_dir = dir.join("tasks");
    std::fs::create_dir_all(&tasks_dir)?;

    let task = SharedTask {
        id: generate_id("task"),
        source,
        target,
        task_type,
        description: description.to_string(),
        context,
        files,
        created_at: Utc::now(),
        status: TaskStatus::Pending,
        result: None,
        error: None,
        completed_at: None,
    };

    let task_path = tasks_dir.join(format!("{}.json", task.id));
    std::fs::write(&task_path, serde_json::to_string_pretty(&task)?)?;

    Ok(task)
}

/// Read shared tasks, optionally filtered.
pub fn read_shared_tasks(cwd: &str, filter: Option<&SharedTaskFilter>) -> Result<Vec<SharedTask>> {
    let tasks_dir = interop_dir(cwd).join("tasks");

    if !tasks_dir.exists() {
        return Ok(Vec::new());
    }

    let mut tasks = Vec::new();

    for entry in std::fs::read_dir(&tasks_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        let Ok(task) = serde_json::from_str::<SharedTask>(&content) else {
            continue;
        };

        if let Some(f) = filter {
            if f.source.as_ref().is_some_and(|s| *s != task.source) {
                continue;
            }
            if f.target.as_ref().is_some_and(|t| *t != task.target) {
                continue;
            }
            if f.status.as_ref().is_some_and(|st| *st != task.status) {
                continue;
            }
        }

        tasks.push(task);
    }

    tasks.sort_by_key(|t| std::cmp::Reverse(t.created_at));
    Ok(tasks)
}

/// Update a shared task.
pub fn update_shared_task(
    cwd: &str,
    task_id: &str,
    status: Option<TaskStatus>,
    result: Option<&str>,
    error: Option<&str>,
) -> Result<Option<SharedTask>> {
    check_segment(task_id, "task_id")?;
    let task_path = interop_dir(cwd)
        .join("tasks")
        .join(format!("{task_id}.json"));

    if !task_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&task_path)?;
    let mut task: SharedTask = serde_json::from_str(&content)?;

    if let Some(s) = status {
        task.status = s;
    }
    if let Some(r) = result {
        task.result = Some(r.to_string());
    }
    if let Some(e) = error {
        task.error = Some(e.to_string());
    }
    if matches!(task.status, TaskStatus::Completed | TaskStatus::Failed)
        && task.completed_at.is_none()
    {
        task.completed_at = Some(Utc::now());
    }

    std::fs::write(&task_path, serde_json::to_string_pretty(&task)?)?;
    Ok(Some(task))
}

/// Add a shared message for cross-tool communication.
pub fn add_shared_message(
    cwd: &str,
    source: InteropSide,
    target: InteropSide,
    content: &str,
    metadata: Option<serde_json::Value>,
) -> Result<SharedMessage> {
    let dir = interop_dir(cwd);
    let messages_dir = dir.join("messages");
    std::fs::create_dir_all(&messages_dir)?;

    let message = SharedMessage {
        id: generate_id("msg"),
        source,
        target,
        content: content.to_string(),
        metadata,
        timestamp: Utc::now(),
        read: false,
    };

    let message_path = messages_dir.join(format!("{}.json", message.id));
    std::fs::write(&message_path, serde_json::to_string_pretty(&message)?)?;

    Ok(message)
}

/// Read shared messages, optionally filtered.
pub fn read_shared_messages(
    cwd: &str,
    filter: Option<&SharedMessageFilter>,
) -> Result<Vec<SharedMessage>> {
    let messages_dir = interop_dir(cwd).join("messages");

    if !messages_dir.exists() {
        return Ok(Vec::new());
    }

    let mut messages = Vec::new();

    for entry in std::fs::read_dir(&messages_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        let Ok(message) = serde_json::from_str::<SharedMessage>(&content) else {
            continue;
        };

        if let Some(f) = filter {
            if f.source.as_ref().is_some_and(|s| *s != message.source) {
                continue;
            }
            if f.target.as_ref().is_some_and(|t| *t != message.target) {
                continue;
            }
            if f.unread_only && message.read {
                continue;
            }
        }

        messages.push(message);
    }

    messages.sort_by_key(|m| std::cmp::Reverse(m.timestamp));
    Ok(messages)
}

/// Mark a message as read.
pub fn mark_message_as_read(cwd: &str, message_id: &str) -> Result<bool> {
    check_segment(message_id, "message_id")?;
    let message_path = interop_dir(cwd)
        .join("messages")
        .join(format!("{message_id}.json"));

    if !message_path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&message_path)?;
    let mut message: SharedMessage = serde_json::from_str(&content)?;
    message.read = true;
    std::fs::write(&message_path, serde_json::to_string_pretty(&message)?)?;

    Ok(true)
}

/// Cleanup options for interop session.
pub struct CleanupOptions {
    pub keep_tasks: bool,
    pub keep_messages: bool,
    pub older_than_ms: Option<u64>,
}

pub struct CleanupResult {
    pub tasks_deleted: usize,
    pub messages_deleted: usize,
}

/// Clean up interop session data.
pub fn cleanup_interop(cwd: &str, options: &CleanupOptions) -> Result<CleanupResult> {
    let dir = interop_dir(cwd);
    let now = Utc::now();
    let mut tasks_deleted = 0usize;
    let mut messages_deleted = 0usize;

    if !options.keep_tasks {
        let tasks_dir = dir.join("tasks");
        if tasks_dir.exists() {
            for entry in std::fs::read_dir(&tasks_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(task) = serde_json::from_str::<SharedTask>(&content) else {
                    continue;
                };
                let should_delete = match options.older_than_ms {
                    Some(ms) => {
                        let cutoff = now - chrono::Duration::milliseconds(ms as i64);
                        task.created_at < cutoff
                    }
                    None => true,
                };
                if should_delete {
                    std::fs::remove_file(&path)?;
                    tasks_deleted += 1;
                }
            }
        }
    }

    if !options.keep_messages {
        let messages_dir = dir.join("messages");
        if messages_dir.exists() {
            for entry in std::fs::read_dir(&messages_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(message) = serde_json::from_str::<SharedMessage>(&content) else {
                    continue;
                };
                let should_delete = match options.older_than_ms {
                    Some(ms) => {
                        let cutoff = now - chrono::Duration::milliseconds(ms as i64);
                        message.timestamp < cutoff
                    }
                    None => true,
                };
                if should_delete {
                    std::fs::remove_file(&path)?;
                    messages_deleted += 1;
                }
            }
        }
    }

    Ok(CleanupResult {
        tasks_deleted,
        messages_deleted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_directory_and_config() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let config = init_interop_session("test-session", cwd, Some("/omx")).unwrap();
        assert_eq!(config.session_id, "test-session");
        assert_eq!(config.omc_cwd, cwd);
        assert_eq!(config.omx_cwd, Some("/omx".to_string()));
        assert_eq!(config.status, InteropStatus::Active);

        // Config file should exist
        let config_path = interop_dir(cwd).join("config.json");
        assert!(config_path.exists());
    }

    #[test]
    fn read_config_returns_none_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let result = read_interop_config(cwd).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn read_config_returns_config_after_init() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        init_interop_session("s1", cwd, None).unwrap();

        let config = read_interop_config(cwd).unwrap().unwrap();
        assert_eq!(config.session_id, "s1");
        assert!(config.omx_cwd.is_none());
    }

    #[test]
    fn add_shared_task_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let task = add_shared_task(
            cwd,
            InteropSide::Omc,
            InteropSide::Omx,
            TaskType::Analyze,
            "analyze the codebase",
            None,
            None,
        )
        .unwrap();

        assert_eq!(task.source, InteropSide::Omc);
        assert_eq!(task.target, InteropSide::Omx);
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.id.starts_with("task-"));

        let task_path = interop_dir(cwd)
            .join("tasks")
            .join(format!("{}.json", task.id));
        assert!(task_path.exists());
    }

    #[test]
    fn read_shared_tasks_returns_all() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        add_shared_task(
            cwd,
            InteropSide::Omc,
            InteropSide::Omx,
            TaskType::Analyze,
            "task 1",
            None,
            None,
        )
        .unwrap();
        add_shared_task(
            cwd,
            InteropSide::Omx,
            InteropSide::Omc,
            TaskType::Implement,
            "task 2",
            None,
            None,
        )
        .unwrap();

        let tasks = read_shared_tasks(cwd, None).unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn read_shared_tasks_filter_by_source() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        add_shared_task(
            cwd,
            InteropSide::Omc,
            InteropSide::Omx,
            TaskType::Analyze,
            "from omc",
            None,
            None,
        )
        .unwrap();
        add_shared_task(
            cwd,
            InteropSide::Omx,
            InteropSide::Omc,
            TaskType::Implement,
            "from omx",
            None,
            None,
        )
        .unwrap();

        let filter = SharedTaskFilter {
            source: Some(InteropSide::Omc),
            target: None,
            status: None,
        };
        let tasks = read_shared_tasks(cwd, Some(&filter)).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].source, InteropSide::Omc);
    }

    #[test]
    fn read_shared_tasks_filter_by_status() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let task = add_shared_task(
            cwd,
            InteropSide::Omc,
            InteropSide::Omx,
            TaskType::Analyze,
            "will complete",
            None,
            None,
        )
        .unwrap();

        update_shared_task(
            cwd,
            &task.id,
            Some(TaskStatus::Completed),
            Some("done"),
            None,
        )
        .unwrap();

        add_shared_task(
            cwd,
            InteropSide::Omc,
            InteropSide::Omx,
            TaskType::Test,
            "still pending",
            None,
            None,
        )
        .unwrap();

        let filter = SharedTaskFilter {
            source: None,
            target: None,
            status: Some(TaskStatus::Completed),
        };
        let tasks = read_shared_tasks(cwd, Some(&filter)).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Completed);
    }

    #[test]
    fn read_shared_tasks_empty_when_no_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let tasks = read_shared_tasks(cwd, None).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn update_shared_task_changes_status_and_result() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let task = add_shared_task(
            cwd,
            InteropSide::Omc,
            InteropSide::Omx,
            TaskType::Analyze,
            "do this",
            None,
            None,
        )
        .unwrap();

        let updated = update_shared_task(
            cwd,
            &task.id,
            Some(TaskStatus::Completed),
            Some("all done"),
            None,
        )
        .unwrap()
        .unwrap();

        assert_eq!(updated.status, TaskStatus::Completed);
        assert_eq!(updated.result.as_deref(), Some("all done"));
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn update_shared_task_returns_none_for_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let result =
            update_shared_task(cwd, "nonexistent", Some(TaskStatus::Failed), None, None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn update_shared_task_sets_error_and_failed_completed_at() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let task = add_shared_task(
            cwd,
            InteropSide::Omc,
            InteropSide::Omx,
            TaskType::Test,
            "will fail",
            None,
            None,
        )
        .unwrap();

        let updated =
            update_shared_task(cwd, &task.id, Some(TaskStatus::Failed), None, Some("broke"))
                .unwrap()
                .unwrap();

        assert_eq!(updated.status, TaskStatus::Failed);
        assert_eq!(updated.error.as_deref(), Some("broke"));
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn message_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let msg = add_shared_message(cwd, InteropSide::Omc, InteropSide::Omx, "hello there", None)
            .unwrap();

        assert_eq!(msg.source, InteropSide::Omc);
        assert_eq!(msg.target, InteropSide::Omx);
        assert_eq!(msg.content, "hello there");
        assert!(!msg.read);

        let messages = read_shared_messages(cwd, None).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, msg.id);
    }

    #[test]
    fn read_shared_messages_empty_when_no_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let messages = read_shared_messages(cwd, None).unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn mark_message_as_read_updates_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let msg =
            add_shared_message(cwd, InteropSide::Omc, InteropSide::Omx, "unread", None).unwrap();

        let changed = mark_message_as_read(cwd, &msg.id).unwrap();
        assert!(changed);

        let messages = read_shared_messages(cwd, None).unwrap();
        assert!(messages[0].read);
    }

    #[test]
    fn mark_message_as_read_returns_false_for_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let result = mark_message_as_read(cwd, "no-such-msg").unwrap();
        assert!(!result);
    }

    #[test]
    fn read_messages_filter_unread_only() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let msg1 =
            add_shared_message(cwd, InteropSide::Omc, InteropSide::Omx, "first", None).unwrap();
        add_shared_message(cwd, InteropSide::Omc, InteropSide::Omx, "second", None).unwrap();

        mark_message_as_read(cwd, &msg1.id).unwrap();

        let filter = SharedMessageFilter {
            source: None,
            target: None,
            unread_only: true,
        };
        let unread = read_shared_messages(cwd, Some(&filter)).unwrap();
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].content, "second");
    }

    #[test]
    fn cleanup_removes_tasks_and_messages() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        add_shared_task(
            cwd,
            InteropSide::Omc,
            InteropSide::Omx,
            TaskType::Analyze,
            "cleanup me",
            None,
            None,
        )
        .unwrap();
        add_shared_message(
            cwd,
            InteropSide::Omc,
            InteropSide::Omx,
            "cleanup me too",
            None,
        )
        .unwrap();

        let result = cleanup_interop(
            cwd,
            &CleanupOptions {
                keep_tasks: false,
                keep_messages: false,
                older_than_ms: None,
            },
        )
        .unwrap();

        assert_eq!(result.tasks_deleted, 1);
        assert_eq!(result.messages_deleted, 1);

        assert!(read_shared_tasks(cwd, None).unwrap().is_empty());
        assert!(read_shared_messages(cwd, None).unwrap().is_empty());
    }

    #[test]
    fn cleanup_keep_tasks_preserves_tasks() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        add_shared_task(
            cwd,
            InteropSide::Omc,
            InteropSide::Omx,
            TaskType::Analyze,
            "keep me",
            None,
            None,
        )
        .unwrap();

        let result = cleanup_interop(
            cwd,
            &CleanupOptions {
                keep_tasks: true,
                keep_messages: true,
                older_than_ms: None,
            },
        )
        .unwrap();

        assert_eq!(result.tasks_deleted, 0);
        assert_eq!(result.messages_deleted, 0);
    }

    #[test]
    fn add_task_with_context_and_files() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().to_str().unwrap();

        let ctx = serde_json::json!({"key": "value"});
        let files = vec!["src/main.rs".to_string(), "Cargo.toml".to_string()];

        let task = add_shared_task(
            cwd,
            InteropSide::Omc,
            InteropSide::Omx,
            TaskType::Review,
            "review these",
            Some(ctx.clone()),
            Some(files.clone()),
        )
        .unwrap();

        assert_eq!(task.context, Some(ctx));
        assert_eq!(task.files, Some(files));
    }

    #[test]
    fn interop_side_other_and_display() {
        assert_eq!(InteropSide::Omc.other(), InteropSide::Omx);
        assert_eq!(InteropSide::Omx.other(), InteropSide::Omc);
        assert_eq!(InteropSide::Omc.to_string(), "OMC");
        assert_eq!(InteropSide::Omx.to_string(), "OMX");
    }

    #[test]
    fn interop_dir_path() {
        let dir = tempdir().unwrap();
        let p = interop_dir(dir.path());
        assert_eq!(p, dir.path().join(".omc/state/interop"));
    }
}
