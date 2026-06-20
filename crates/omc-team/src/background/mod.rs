mod concurrency;
mod manager;

pub use concurrency::ConcurrencyManager;
pub use manager::{BackgroundManager, DEFAULT_TASK_TTL_MS};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundTaskStatus {
    Queued,
    Running,
    Completed,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgress {
    pub tool_calls: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_tool: Option<String>,
    pub last_update: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_at: Option<DateTime<Utc>>,
}

impl Default for TaskProgress {
    fn default() -> Self {
        Self {
            tool_calls: 0,
            last_tool: None,
            last_update: Utc::now(),
            last_message: None,
            last_message_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundTask {
    pub id: String,
    pub session_id: String,
    pub parent_session_id: String,
    pub description: String,
    pub prompt: String,
    pub agent: String,
    pub status: BackgroundTaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queued_at: Option<DateTime<Utc>>,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub progress: TaskProgress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub concurrency_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LaunchInput {
    pub description: String,
    pub prompt: String,
    pub agent: String,
    pub parent_session_id: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResumeInput {
    pub session_id: String,
    pub prompt: String,
    pub parent_session_id: String,
}

#[derive(Debug, Clone)]
pub struct ResumeContext {
    pub session_id: String,
    pub previous_prompt: String,
    pub tool_call_count: u32,
    pub last_tool_used: Option<String>,
    pub last_output_summary: Option<String>,
    pub started_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct BackgroundTaskConfig {
    pub default_concurrency: Option<usize>,
    pub model_concurrency: HashMap<String, usize>,
    pub provider_concurrency: HashMap<String, usize>,
    pub max_total_tasks: Option<usize>,
    pub task_timeout_ms: Option<u64>,
    pub max_queue_size: Option<usize>,
    pub stale_threshold_ms: Option<u64>,
}
