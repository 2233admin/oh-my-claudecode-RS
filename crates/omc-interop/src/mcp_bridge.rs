use std::env;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::warn;

use crate::omx_team_state;
use crate::shared_state::{self, InteropSide, SharedMessage, SharedTask, TaskType};

// ============================================================================
// Interop Mode
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteropMode {
    Off,
    Observe,
    Active,
}

impl std::fmt::Display for InteropMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => write!(f, "off"),
            Self::Observe => write!(f, "observe"),
            Self::Active => write!(f, "active"),
        }
    }
}

/// Determine the interop mode from environment variables.
static OMX_OMC_INTEROP_MODE: &str = "OMX_OMC_INTEROP_MODE";

pub fn get_interop_mode() -> InteropMode {
    let raw = env::var(OMX_OMC_INTEROP_MODE)
        .unwrap_or_else(|_| "off".to_string())
        .to_lowercase();

    match raw.as_str() {
        "observe" => InteropMode::Observe,
        "active" => InteropMode::Active,
        _ => InteropMode::Off,
    }
}

/// Check whether the OMX direct-write bridge is enabled.
static OMX_OMC_INTEROP_ENABLED: &str = "OMX_OMC_INTEROP_ENABLED";
static OMC_INTEROP_TOOLS_ENABLED: &str = "OMC_INTEROP_TOOLS_ENABLED";

pub fn can_use_omx_direct_write_bridge() -> bool {
    let interop_enabled = env::var(OMX_OMC_INTEROP_ENABLED).as_deref() == Ok("1");
    let tools_enabled = env::var(OMC_INTEROP_TOOLS_ENABLED).as_deref() == Ok("1");
    interop_enabled && tools_enabled && get_interop_mode() == InteropMode::Active
}

pub const INTEROP_BRIDGE_SCHEMA_VERSION: &str = "omc.interop.bridge.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteropBridgeAction {
    SendTask,
    SendMessage,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteropBridgeRequest {
    pub action: InteropBridgeAction,
    pub source: InteropSide,
    pub target: InteropSide,
    #[serde(rename = "type")]
    pub task_type: Option<TaskType>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub working_directory: Option<String>,
    #[serde(default)]
    pub allow_side_effects: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteropBridgeResponse {
    pub schema_version: String,
    pub action: InteropBridgeAction,
    pub read_only: bool,
    pub write_enabled: bool,
    pub source: InteropSide,
    pub target: InteropSide,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<SharedTask>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<SharedMessage>,
}

#[derive(Debug, Error)]
pub enum InteropBridgeError {
    #[error("interop writes require allowSideEffects=true and active interop flags")]
    SideEffectsNotAllowed,

    #[error("invalid interop request: {0}")]
    InvalidRequest(String),

    #[error("interop write failed: {0}")]
    Upstream(#[from] shared_state::InteropError),
}

impl InteropBridgeError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::SideEffectsNotAllowed => "side_effects_not_allowed",
            Self::InvalidRequest(_) => "invalid_request",
            Self::Upstream(_) => "upstream_failed",
        }
    }
}

fn require_text<'a>(value: Option<&'a String>, field: &str) -> Result<&'a str, InteropBridgeError> {
    value
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| InteropBridgeError::InvalidRequest(format!("{field} is required")))
}

fn bridge_response(
    request: &InteropBridgeRequest,
    task: Option<SharedTask>,
    message: Option<SharedMessage>,
) -> InteropBridgeResponse {
    InteropBridgeResponse {
        schema_version: INTEROP_BRIDGE_SCHEMA_VERSION.into(),
        action: request.action.clone(),
        read_only: false,
        write_enabled: true,
        source: request.source.clone(),
        target: request.target.clone(),
        task,
        message,
    }
}

pub struct InteropBridgeCliArgs<'a> {
    pub action: &'a str,
    pub source: &'a str,
    pub target: &'a str,
    pub task_type: Option<&'a str>,
    pub description: Option<&'a str>,
    pub content: Option<&'a str>,
    pub working_directory: &'a str,
    pub allow_side_effects: bool,
}

pub fn interop_bridge_request_from_cli(
    args: InteropBridgeCliArgs<'_>,
) -> Result<InteropBridgeRequest, InteropBridgeError> {
    let action = match args.action {
        "send_task" => InteropBridgeAction::SendTask,
        "send_message" => InteropBridgeAction::SendMessage,
        other => {
            return Err(InteropBridgeError::InvalidRequest(format!(
                "invalid action: {other}"
            )));
        }
    };
    let parse_side = |value: &str| match value {
        "omc" => Ok(InteropSide::Omc),
        "omx" => Ok(InteropSide::Omx),
        other => Err(InteropBridgeError::InvalidRequest(format!(
            "invalid runtime: {other}"
        ))),
    };
    Ok(InteropBridgeRequest {
        action,
        source: parse_side(args.source)?,
        target: parse_side(args.target)?,
        task_type: args
            .task_type
            .map(|value| match value {
                "analyze" => Ok(TaskType::Analyze),
                "implement" => Ok(TaskType::Implement),
                "review" => Ok(TaskType::Review),
                "test" => Ok(TaskType::Test),
                "custom" => Ok(TaskType::Custom),
                other => Err(InteropBridgeError::InvalidRequest(format!(
                    "invalid task type: {other}"
                ))),
            })
            .transpose()?,
        description: args.description.map(str::to_owned),
        content: args.content.map(str::to_owned),
        working_directory: Some(args.working_directory.to_owned()),
        allow_side_effects: args.allow_side_effects,
    })
}

/// Execute the small, explicitly gated shared-state bridge. It only writes
/// durable task/message records; it never starts workers or changes task state.
pub fn interop_bridge(
    request: &InteropBridgeRequest,
) -> Result<InteropBridgeResponse, InteropBridgeError> {
    if request.source == request.target {
        return Err(InteropBridgeError::InvalidRequest(
            "source and target must differ".into(),
        ));
    }
    if !request.allow_side_effects || !can_use_omx_direct_write_bridge() {
        return Err(InteropBridgeError::SideEffectsNotAllowed);
    }

    let cwd = request.working_directory.as_deref().unwrap_or(".");
    let response = match &request.action {
        InteropBridgeAction::SendTask => {
            let task_type = request.task_type.clone().ok_or_else(|| {
                InteropBridgeError::InvalidRequest("type is required for send_task".into())
            })?;
            let description = require_text(request.description.as_ref(), "description")?;
            let task = shared_state::add_shared_task(
                cwd,
                request.source.clone(),
                request.target.clone(),
                task_type,
                description,
                None,
                None,
            )?;
            bridge_response(request, Some(task), None)
        }
        InteropBridgeAction::SendMessage => {
            let content = require_text(request.content.as_ref(), "content")?;
            let message = shared_state::add_shared_message(
                cwd,
                request.source.clone(),
                request.target.clone(),
                content,
                None,
            )?;
            bridge_response(request, None, Some(message))
        }
    };

    Ok(response)
}
// MCP tool result envelope
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

fn tool_text(text: impl Into<String>) -> ToolResult {
    ToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: text.into(),
        }],
        is_error: None,
    }
}

fn tool_error(action: &str, err: impl std::fmt::Display) -> ToolResult {
    ToolResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: format!("Error {action}: {err}"),
        }],
        is_error: Some(true),
    }
}

fn truncate_preview(text: &str, max_chars: usize) -> &str {
    if text.len() > max_chars {
        &text[..max_chars]
    } else {
        text
    }
}

fn status_icon(status: &shared_state::TaskStatus) -> &'static str {
    match status {
        shared_state::TaskStatus::Completed => "[done]",
        shared_state::TaskStatus::Failed => "[fail]",
        shared_state::TaskStatus::InProgress => "[...] ",
        shared_state::TaskStatus::Pending => "[ ] ",
    }
}

fn omx_status_icon(status: &omx_team_state::OmxTaskStatus) -> &'static str {
    match status {
        omx_team_state::OmxTaskStatus::Completed => "[done]",
        omx_team_state::OmxTaskStatus::Failed => "[fail]",
        omx_team_state::OmxTaskStatus::InProgress => "[...] ",
        omx_team_state::OmxTaskStatus::Blocked => "[blk] ",
        omx_team_state::OmxTaskStatus::Pending => "[ ] ",
    }
}

// ============================================================================
// interop_send_task
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct SendTaskArgs {
    pub target: InteropSide,
    #[serde(rename = "type")]
    pub task_type: TaskType,
    pub description: String,
    pub context: Option<serde_json::Value>,
    pub files: Option<Vec<String>>,
    pub working_directory: Option<String>,
}

/// Send a task to the other tool (OMC <-> OMX).
pub fn interop_send_task(args: &SendTaskArgs) -> ToolResult {
    if !can_use_omx_direct_write_bridge() {
        return tool_error(
            "sending task",
            "interop writes are disabled; enable active interop flags explicitly",
        );
    }
    let cwd = args.working_directory.as_deref().unwrap_or(".");
    let source = args.target.other();

    match shared_state::add_shared_task(
        cwd,
        source,
        args.target.clone(),
        args.task_type.clone(),
        &args.description,
        args.context.clone(),
        args.files.clone(),
    ) {
        Ok(task) => {
            let mut text = format!(
                "## Task Sent to {}\n\n\
                 **Task ID:** {}\n\
                 **Type:** {:?}\n\
                 **Description:** {}\n\
                 **Status:** {:?}\n\
                 **Created:** {}\n",
                args.target,
                task.id,
                task.task_type,
                task.description,
                task.status,
                task.created_at,
            );
            if let Some(ref files) = task.files {
                let _ = write!(text, "**Files:** {}\n\n", files.join(", "));
            }
            let _ = write!(
                text,
                "The task has been queued for {} to pick up.",
                args.target
            );
            tool_text(text)
        }
        Err(e) => tool_error("sending task", e),
    }
}

// ============================================================================
// interop_read_results
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ReadResultsArgs {
    pub source: Option<InteropSide>,
    pub status: Option<shared_state::TaskStatus>,
    pub limit: Option<usize>,
    pub working_directory: Option<String>,
}

/// Read task results from the shared interop state.
pub fn interop_read_results(args: &ReadResultsArgs) -> ToolResult {
    let cwd = args.working_directory.as_deref().unwrap_or(".");
    let limit = args.limit.unwrap_or(10);

    let filter = shared_state::SharedTaskFilter {
        source: args.source.clone(),
        target: None,
        status: args.status.clone(),
    };

    match shared_state::read_shared_tasks(cwd, Some(&filter)) {
        Ok(tasks) => {
            if tasks.is_empty() {
                return tool_text("## No Tasks Found\n\nNo tasks match the specified filters.");
            }

            let limited: Vec<_> = tasks.iter().take(limit).collect();
            let mut lines = vec![format!(
                "## Tasks ({}{})\n",
                limited.len(),
                if tasks.len() > limit {
                    format!(" of {}", tasks.len())
                } else {
                    String::new()
                }
            )];

            for task in &limited {
                lines.push(format!("### {} {}", status_icon(&task.status), task.id));
                lines.push(format!("- **Type:** {:?}", task.task_type));
                lines.push(format!(
                    "- **Source:** {} -> **Target:** {}",
                    task.source, task.target
                ));
                lines.push(format!("- **Status:** {:?}", task.status));
                lines.push(format!("- **Description:** {}", task.description));
                lines.push(format!("- **Created:** {}", task.created_at));

                if let Some(ref files) = task.files
                    && !files.is_empty()
                {
                    lines.push(format!("- **Files:** {}", files.join(", ")));
                }

                if let Some(ref result) = task.result {
                    lines.push(format!("- **Result:** {}", truncate_preview(result, 200)));
                }

                if let Some(ref error) = task.error {
                    lines.push(format!("- **Error:** {error}"));
                }

                if let Some(completed) = task.completed_at {
                    lines.push(format!("- **Completed:** {completed}"));
                }

                lines.push(String::default());
            }

            tool_text(lines.join("\n"))
        }
        Err(e) => tool_error("reading tasks", e),
    }
}

// ============================================================================
// interop_send_message
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct SendMessageArgs {
    pub target: InteropSide,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub working_directory: Option<String>,
}

/// Send a message to the other tool.
pub fn interop_send_message(args: &SendMessageArgs) -> ToolResult {
    if !can_use_omx_direct_write_bridge() {
        return tool_error(
            "sending message",
            "interop writes are disabled; enable active interop flags explicitly",
        );
    }
    let cwd = args.working_directory.as_deref().unwrap_or(".");
    let source = args.target.other();

    match shared_state::add_shared_message(
        cwd,
        source,
        args.target.clone(),
        &args.content,
        args.metadata.clone(),
    ) {
        Ok(message) => tool_text(format!(
            "## Message Sent to {}\n\n\
             **Message ID:** {}\n\
             **Content:** {}\n\
             **Timestamp:** {}\n\n\
             The message has been queued for {} to pick up.",
            args.target, message.id, message.content, message.timestamp, args.target,
        )),
        Err(e) => tool_error("sending message", e),
    }
}

// ============================================================================
// interop_read_messages
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ReadMessagesArgs {
    pub source: Option<InteropSide>,
    pub unread_only: Option<bool>,
    pub limit: Option<usize>,
    pub mark_as_read: Option<bool>,
    pub working_directory: Option<String>,
}

/// Read messages from the shared interop state.
pub fn interop_read_messages(args: &ReadMessagesArgs) -> ToolResult {
    let cwd = args.working_directory.as_deref().unwrap_or(".");
    let limit = args.limit.unwrap_or(10);
    let mark_as_read = args.mark_as_read.unwrap_or(false);

    let filter = shared_state::SharedMessageFilter {
        source: args.source.clone(),
        target: None,
        unread_only: args.unread_only.unwrap_or(false),
    };

    match shared_state::read_shared_messages(cwd, Some(&filter)) {
        Ok(messages) => {
            if messages.is_empty() {
                return tool_text(
                    "## No Messages Found\n\nNo messages match the specified filters.",
                );
            }

            let limited: Vec<_> = messages.iter().take(limit).collect();

            if mark_as_read {
                for msg in &limited {
                    let _ = shared_state::mark_message_as_read(cwd, &msg.id);
                }
            }

            let mut lines = vec![format!(
                "## Messages ({}{})\n",
                limited.len(),
                if messages.len() > limit {
                    format!(" of {}", messages.len())
                } else {
                    String::default()
                }
            )];

            for message in &limited {
                let read_icon = if message.read { "[r]" } else { "[ ]" };
                lines.push(format!("### {} {}", read_icon, message.id));
                lines.push(format!(
                    "- **From:** {} -> **To:** {}",
                    message.source, message.target
                ));
                lines.push(format!("- **Content:** {}", message.content));
                lines.push(format!("- **Timestamp:** {}", message.timestamp));
                lines.push(format!(
                    "- **Read:** {}",
                    if message.read { "Yes" } else { "No" }
                ));

                if let Some(ref meta) = message.metadata {
                    lines.push(format!("- **Metadata:** {meta}"));
                }

                lines.push(String::default());
            }

            if mark_as_read {
                lines.push(format!("\n*{} message(s) marked as read*", limited.len()));
            }

            tool_text(lines.join("\n"))
        }
        Err(e) => tool_error("reading messages", e),
    }
}

// ============================================================================
// interop_list_omx_teams
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ListOmxTeamsArgs {
    pub working_directory: Option<String>,
}

/// List active OMX teams.
pub fn interop_list_omx_teams(args: &ListOmxTeamsArgs) -> ToolResult {
    let cwd = args.working_directory.as_deref().unwrap_or(".");

    match omx_team_state::list_omx_teams(cwd) {
        Ok(teams) => {
            if teams.is_empty() {
                return tool_text(
                    "## No OMX Teams Found\n\nNo active OMX teams detected in .omx/state/team/.",
                );
            }

            let mut lines = vec![format!("## OMX Teams ({})\n", teams.len())];

            for name in &teams {
                match omx_team_state::read_omx_team_config(name, cwd) {
                    Ok(Some(config)) => {
                        lines.push(format!("### {name}"));
                        lines.push(format!("- **Task:** {}", config.task));
                        lines.push(format!(
                            "- **Workers:** {} ({})",
                            config.worker_count, config.agent_type
                        ));
                        lines.push(format!("- **Created:** {}", config.created_at));
                        let worker_names: Vec<_> =
                            config.workers.iter().map(|w| w.name.as_str()).collect();
                        lines.push(format!("- **Workers:** {}", worker_names.join(", ")));
                        lines.push(String::default());
                    }
                    Ok(None) => {
                        lines.push(format!("### {name} (config not readable)\n"));
                    }
                    Err(e) => {
                        warn!("Failed to read config for team {name}: {e}");
                        lines.push(format!("### {name} (error reading config)\n"));
                    }
                }
            }

            tool_text(lines.join("\n"))
        }
        Err(e) => tool_error("listing OMX teams", e),
    }
}

// ============================================================================
// interop_send_omx_message
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct SendOmxMessageArgs {
    pub team_name: String,
    pub from_worker: String,
    pub to_worker: String,
    pub body: String,
    pub broadcast: Option<bool>,
    pub working_directory: Option<String>,
}

/// Send a message to an OMX team worker mailbox.
pub fn interop_send_omx_message(args: &SendOmxMessageArgs) -> ToolResult {
    if !can_use_omx_direct_write_bridge() {
        return ToolResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: "Direct OMX mailbox writes are disabled. Use broker-mediated team_* MCP path or enable active interop flags explicitly.".to_string(),
            }],
            is_error: Some(true),
        };
    }

    let cwd = args.working_directory.as_deref().unwrap_or(".");

    if args.broadcast.unwrap_or(false) {
        match omx_team_state::broadcast_omx_message(
            &args.team_name,
            &args.from_worker,
            &args.body,
            cwd,
        ) {
            Ok(messages) => tool_text(format!(
                "## Broadcast Sent to OMX Team: {}\n\n\
                 **From:** {}\n\
                 **Recipients:** {}\n\
                 **Message IDs:** {}\n\n\
                 Message delivered to {} worker mailbox(es).",
                args.team_name,
                args.from_worker,
                messages.len(),
                messages
                    .iter()
                    .map(|m| m.message_id.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                messages.len(),
            )),
            Err(e) => tool_error("broadcasting OMX message", e),
        }
    } else {
        match omx_team_state::send_omx_direct_message(
            &args.team_name,
            &args.from_worker,
            &args.to_worker,
            &args.body,
            cwd,
        ) {
            Ok(msg) => tool_text(format!(
                "## Message Sent to OMX Worker\n\n\
                 **Team:** {}\n\
                 **From:** {}\n\
                 **To:** {}\n\
                 **Message ID:** {}\n\
                 **Created:** {}\n\n\
                 Message delivered to {}'s mailbox.",
                args.team_name,
                msg.from_worker,
                msg.to_worker,
                msg.message_id,
                msg.created_at,
                msg.to_worker,
            )),
            Err(e) => tool_error("sending OMX message", e),
        }
    }
}

// ============================================================================
// interop_read_omx_messages
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ReadOmxMessagesArgs {
    pub team_name: String,
    pub worker_name: String,
    pub limit: Option<usize>,
    pub working_directory: Option<String>,
}

/// Read messages from an OMX team worker mailbox.
pub fn interop_read_omx_messages(args: &ReadOmxMessagesArgs) -> ToolResult {
    let cwd = args.working_directory.as_deref().unwrap_or(".");
    let limit = args.limit.unwrap_or(20);

    let messages =
        omx_team_state::list_omx_mailbox_messages(&args.team_name, &args.worker_name, cwd);

    if messages.is_empty() {
        return tool_text(format!(
            "## No Messages\n\nNo messages in {}'s mailbox for team {}.",
            args.worker_name, args.team_name,
        ));
    }

    // Take most recent N
    let skip = messages.len().saturating_sub(limit);
    let limited: Vec<_> = messages.iter().skip(skip).collect();

    let mut lines = vec![format!(
        "## OMX Mailbox: {} @ {} ({}{})\n",
        args.worker_name,
        args.team_name,
        limited.len(),
        if messages.len() > limit {
            format!(" of {}", messages.len())
        } else {
            String::default()
        }
    )];

    for msg in &limited {
        let icon = if msg.delivered_at.is_some() {
            "[d]"
        } else {
            "[ ]"
        };
        lines.push(format!("### {} {}", icon, msg.message_id));
        lines.push(format!("- **From:** {}", msg.from_worker));
        lines.push(format!("- **To:** {}", msg.to_worker));
        lines.push(format!("- **Body:** {}", truncate_preview(&msg.body, 300)));
        lines.push(format!("- **Created:** {}", msg.created_at));
        if let Some(delivered) = msg.delivered_at {
            lines.push(format!("- **Delivered:** {delivered}"));
        }
        lines.push(String::default());
    }

    tool_text(lines.join("\n"))
}

// ============================================================================
// interop_read_omx_tasks
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ReadOmxTasksArgs {
    pub team_name: String,
    pub status: Option<omx_team_state::OmxTaskStatus>,
    pub limit: Option<usize>,
    pub working_directory: Option<String>,
}

/// Read tasks from an OMX team.
pub fn interop_read_omx_tasks(args: &ReadOmxTasksArgs) -> ToolResult {
    let cwd = args.working_directory.as_deref().unwrap_or(".");
    let limit = args.limit.unwrap_or(20);

    match omx_team_state::list_omx_tasks(&args.team_name, cwd) {
        Ok(mut tasks) => {
            if let Some(ref status) = args.status {
                tasks.retain(|t| &t.status == status);
            }

            if tasks.is_empty() {
                let status_msg = args
                    .status
                    .as_ref()
                    .map(|s| format!(" with status \"{s:?}\""))
                    .unwrap_or_default();
                return tool_text(format!(
                    "## No Tasks\n\nNo tasks found for OMX team {}{status_msg}.",
                    args.team_name,
                ));
            }

            let limited: Vec<_> = tasks.iter().take(limit).collect();

            let mut lines = vec![format!(
                "## OMX Tasks: {} ({}{})\n",
                args.team_name,
                limited.len(),
                if tasks.len() > limit {
                    format!(" of {}", tasks.len())
                } else {
                    String::new()
                }
            )];

            for task in &limited {
                lines.push(format!(
                    "### {} Task {}: {}",
                    omx_status_icon(&task.status),
                    task.id,
                    task.subject
                ));
                lines.push(format!("- **Status:** {:?}", task.status));
                if let Some(ref owner) = task.owner {
                    lines.push(format!("- **Owner:** {owner}"));
                }
                lines.push(format!(
                    "- **Description:** {}",
                    truncate_preview(&task.description, 200)
                ));
                lines.push(format!("- **Created:** {}", task.created_at));
                if let Some(ref result) = task.result {
                    lines.push(format!("- **Result:** {}", truncate_preview(result, 200)));
                }
                if let Some(ref error) = task.error {
                    lines.push(format!("- **Error:** {error}"));
                }
                if let Some(completed) = task.completed_at {
                    lines.push(format!("- **Completed:** {completed}"));
                }
                lines.push(String::default());
            }

            tool_text(lines.join("\n"))
        }
        Err(e) => tool_error("reading OMX tasks", e),
    }
}

// ============================================================================
// Tool registry
// ============================================================================

/// Interop tool name constants for registration.
pub const TOOL_SEND_TASK: &str = "interop_send_task";
pub const TOOL_READ_RESULTS: &str = "interop_read_results";
pub const TOOL_SEND_MESSAGE: &str = "interop_send_message";
pub const TOOL_READ_MESSAGES: &str = "interop_read_messages";
pub const TOOL_LIST_OMX_TEAMS: &str = "interop_list_omx_teams";
pub const TOOL_SEND_OMX_MESSAGE: &str = "interop_send_omx_message";
pub const TOOL_READ_OMX_MESSAGES: &str = "interop_read_omx_messages";
pub const TOOL_READ_OMX_TASKS: &str = "interop_read_omx_tasks";
pub const TOOL_INTEROP_BRIDGE: &str = "interop_bridge";

/// All interop tool names.
pub const ALL_TOOLS: &[&str] = &[
    TOOL_SEND_TASK,
    TOOL_READ_RESULTS,
    TOOL_SEND_MESSAGE,
    TOOL_READ_MESSAGES,
    TOOL_LIST_OMX_TEAMS,
    TOOL_SEND_OMX_MESSAGE,
    TOOL_READ_OMX_MESSAGES,
    TOOL_READ_OMX_TASKS,
    TOOL_INTEROP_BRIDGE,
];

#[cfg(test)]
mod tests {
    use super::*;

    static OMX_OMC_INTEROP_MODE: &str = "OMX_OMC_INTEROP_MODE";
    static OMX_OMC_INTEROP_ENABLED: &str = "OMX_OMC_INTEROP_ENABLED";
    static OMC_INTEROP_TOOLS_ENABLED: &str = "OMC_INTEROP_TOOLS_ENABLED";

    #[test]
    fn interop_mode_serde_roundtrip() {
        let modes = vec![InteropMode::Off, InteropMode::Observe, InteropMode::Active];
        for mode in modes {
            let json = serde_json::to_string(&mode).unwrap();
            let deserialized: InteropMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, deserialized);
        }
    }

    #[test]
    fn interop_mode_display() {
        assert_eq!(InteropMode::Off.to_string(), "off");
        assert_eq!(InteropMode::Observe.to_string(), "observe");
        assert_eq!(InteropMode::Active.to_string(), "active");
    }

    #[test]
    fn interop_mode_and_bridge_env_behavior() {
        // Mutex serializes env-var tests to avoid races with parallel tests.
        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap();

        let saved_mode = std::env::var(OMX_OMC_INTEROP_MODE).ok();
        let saved_enabled = std::env::var(OMX_OMC_INTEROP_ENABLED).ok();
        let saved_tools = std::env::var(OMC_INTEROP_TOOLS_ENABLED).ok();

        unsafe {
            std::env::remove_var(OMX_OMC_INTEROP_MODE);
            std::env::remove_var(OMX_OMC_INTEROP_ENABLED);
            std::env::remove_var(OMC_INTEROP_TOOLS_ENABLED);
        }

        assert_eq!(get_interop_mode(), InteropMode::Off);
        assert!(!can_use_omx_direct_write_bridge());

        unsafe { std::env::set_var(OMX_OMC_INTEROP_MODE, "OBSERVE") };
        assert_eq!(get_interop_mode(), InteropMode::Observe);

        unsafe { std::env::set_var(OMX_OMC_INTEROP_MODE, "active") };
        assert_eq!(get_interop_mode(), InteropMode::Active);

        // Bridge requires all three flags
        unsafe {
            std::env::set_var(OMX_OMC_INTEROP_ENABLED, "1");
            std::env::set_var(OMC_INTEROP_TOOLS_ENABLED, "1");
        }
        assert!(can_use_omx_direct_write_bridge());

        // Missing one flag => false
        unsafe { std::env::remove_var(OMC_INTEROP_TOOLS_ENABLED) };
        assert!(!can_use_omx_direct_write_bridge());

        // Mode not active => false
        unsafe {
            std::env::set_var(OMC_INTEROP_TOOLS_ENABLED, "1");
            std::env::set_var(OMX_OMC_INTEROP_MODE, "observe");
        }
        assert!(!can_use_omx_direct_write_bridge());

        // Restore original values
        macro_rules! restore {
            ($var:expr, $saved:expr) => {
                match $saved {
                    Some(val) => unsafe { std::env::set_var($var, val) },
                    None => unsafe { std::env::remove_var($var) },
                }
            };
        }
        restore!(OMX_OMC_INTEROP_MODE, saved_mode);
        restore!(OMX_OMC_INTEROP_ENABLED, saved_enabled);
        restore!(OMC_INTEROP_TOOLS_ENABLED, saved_tools);
    }

    #[test]
    fn truncate_preview_short_text() {
        assert_eq!(truncate_preview("hello", 100), "hello");
    }

    #[test]
    fn truncate_preview_long_text() {
        let text = "a".repeat(500);
        assert_eq!(truncate_preview(&text, 200).len(), 200);
    }

    #[test]
    fn status_icon_returns_correct_values() {
        assert_eq!(status_icon(&shared_state::TaskStatus::Completed), "[done]");
        assert_eq!(status_icon(&shared_state::TaskStatus::Failed), "[fail]");
        assert_eq!(status_icon(&shared_state::TaskStatus::InProgress), "[...] ");
        assert_eq!(status_icon(&shared_state::TaskStatus::Pending), "[ ] ");
    }

    #[test]
    fn omx_status_icon_returns_correct_values() {
        assert_eq!(
            omx_status_icon(&omx_team_state::OmxTaskStatus::Completed),
            "[done]"
        );
        assert_eq!(
            omx_status_icon(&omx_team_state::OmxTaskStatus::Failed),
            "[fail]"
        );
        assert_eq!(
            omx_status_icon(&omx_team_state::OmxTaskStatus::InProgress),
            "[...] "
        );
        assert_eq!(
            omx_status_icon(&omx_team_state::OmxTaskStatus::Blocked),
            "[blk] "
        );
        assert_eq!(
            omx_status_icon(&omx_team_state::OmxTaskStatus::Pending),
            "[ ] "
        );
    }

    #[test]
    fn tool_text_creates_correct_structure() {
        let result = tool_text("hello");
        assert!(result.is_error.is_none());
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].content_type, "text");
        assert_eq!(result.content[0].text, "hello");
    }

    #[test]
    fn tool_error_creates_correct_structure() {
        let result = tool_error("doing stuff", "boom");
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("doing stuff"));
        assert!(result.content[0].text.contains("boom"));
    }

    #[test]
    fn all_tool_names_are_nonempty() {
        for name in ALL_TOOLS {
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn tool_name_constants_unique() {
        let mut seen = std::collections::HashSet::new();
        for name in ALL_TOOLS {
            assert!(seen.insert(*name), "duplicate tool name: {name}");
        }
    }
}
