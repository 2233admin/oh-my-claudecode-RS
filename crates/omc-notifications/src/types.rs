//! Core types for the notification system.

use serde::{Deserialize, Serialize};

/// Events that trigger notifications.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationEvent {
    #[default]
    SessionStart,
    SessionStop,
    SessionEnd,
    SessionIdle,
    AskUserQuestion,
    AgentCall,
}

impl std::fmt::Display for NotificationEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::SessionStart => "session-start",
            Self::SessionStop => "session-stop",
            Self::SessionEnd => "session-end",
            Self::SessionIdle => "session-idle",
            Self::AskUserQuestion => "ask-user-question",
            Self::AgentCall => "agent-call",
        };
        write!(f, "{s}")
    }
}

/// Supported notification platforms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationPlatform {
    Slack,
    Tmux,
    Webhook,
}

impl std::fmt::Display for NotificationPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Slack => "slack",
            Self::Tmux => "tmux",
            Self::Webhook => "webhook",
        };
        write!(f, "{s}")
    }
}

/// Payload sent with each notification.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPayload {
    /// The event that triggered this notification.
    pub event: NotificationEvent,
    /// Session identifier.
    pub session_id: String,
    /// Pre-formatted message text.
    #[serde(default)]
    pub message: String,
    /// ISO timestamp.
    pub timestamp: String,
    /// Current tmux session name (if in tmux).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmux_session: Option<String>,
    /// Project directory path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    /// Basename of the project directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    /// Active OMC modes during this session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modes_used: Option<Vec<String>>,
    /// Context summary of what was done.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_summary: Option<String>,
    /// Session duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Number of agents spawned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents_spawned: Option<u32>,
    /// Number of agents completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents_completed: Option<u32>,
    /// Stop/end reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Active mode name (for stop events).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_mode: Option<String>,
    /// Current iteration (for stop events).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration: Option<u32>,
    /// Max iterations (for stop events).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u32>,
    /// Question text (for ask-user-question events).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    /// Incomplete task count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incomplete_tasks: Option<u32>,
    /// tmux pane ID for reply injection target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmux_pane_id: Option<String>,
    /// Agent name for agent-call events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    /// Agent type for agent-call events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    /// Captured tmux pane content (last N lines).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tmux_tail: Option<String>,
}

/// Result of a single notification send attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationResult {
    pub platform: NotificationPlatform,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
}

/// Result of dispatching notifications for an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResult {
    pub event: NotificationEvent,
    pub results: Vec<NotificationResult>,
    pub any_success: bool,
}

/// Slack webhook configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub enabled: bool,
    pub webhook_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mention: Option<String>,
}

/// Generic webhook configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub enabled: bool,
    pub url: String,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
    #[serde(default = "default_method")]
    pub method: String,
}

fn default_method() -> String {
    "POST".to_string()
}

/// Top-level notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slack: Option<SlackConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook: Option<WebhookConfig>,
}
