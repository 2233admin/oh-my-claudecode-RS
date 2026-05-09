use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// HookEvent represents all possible Claude Code hook events.
///
/// These are divided into three categories:
/// - Claude native events: emitted by Claude Code itself
/// - OMC specific events: custom events added by OMC
/// - omc-team events: events from the agent team orchestration system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    // Claude native events
    UserPromptSubmit,
    SessionStart,
    SessionEnd,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Stop,
    PreCompact,
    PermissionRequest,
    Notification,
    // OMC specific events
    SubagentStart,
    SubagentStop,
    // omc-team events
    TaskCreated,
    TaskCompleted,
    TeammateIdle,
}

impl HookEvent {
    /// Parse a hook event from a string (case-insensitive).
    pub fn parse_str(s: &str) -> Result<Self, String> {
        let normalized = s.trim();
        match normalized {
            // Exact matches
            "UserPromptSubmit" | "user_prompt_submit" | "user-prompt-submit" => {
                Ok(Self::UserPromptSubmit)
            }
            "SessionStart" | "session_start" | "session-start" => Ok(Self::SessionStart),
            "SessionEnd" | "session_end" | "session-end" => Ok(Self::SessionEnd),
            "PreToolUse" | "pre_tool_use" | "pre-tool-use" => Ok(Self::PreToolUse),
            "PostToolUse" | "post_tool_use" | "post-tool-use" => Ok(Self::PostToolUse),
            "PostToolUseFailure" | "post_tool_use_failure" | "post-tool-use-failure" => {
                Ok(Self::PostToolUseFailure)
            }
            "Stop" | "stop" => Ok(Self::Stop),
            "PreCompact" | "pre_compact" | "pre-compact" => Ok(Self::PreCompact),
            "PermissionRequest" | "permission_request" | "permission-request" => {
                Ok(Self::PermissionRequest)
            }
            "Notification" | "notification" => Ok(Self::Notification),
            // OMC specific
            "SubagentStart" | "subagent_start" | "subagent-start" => Ok(Self::SubagentStart),
            "SubagentStop" | "subagent_stop" | "subagent-stop" => Ok(Self::SubagentStop),
            // omc-team
            "TaskCreated" | "task_created" | "task-created" => Ok(Self::TaskCreated),
            "TaskCompleted" | "task_completed" | "task-completed" => Ok(Self::TaskCompleted),
            "TeammateIdle" | "teammate_idle" | "teammate-idle" => Ok(Self::TeammateIdle),
            _ => Err(format!("unknown hook event: {s}")),
        }
    }

    /// Convert a hook event to its canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::Stop => "Stop",
            Self::PreCompact => "PreCompact",
            Self::PermissionRequest => "PermissionRequest",
            Self::Notification => "Notification",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
            Self::TaskCreated => "TaskCreated",
            Self::TaskCompleted => "TaskCompleted",
            Self::TeammateIdle => "TeammateIdle",
        }
    }

    /// Check if this is a Claude native event.
    pub fn is_native(&self) -> bool {
        matches!(
            self,
            Self::UserPromptSubmit
                | Self::SessionStart
                | Self::SessionEnd
                | Self::PreToolUse
                | Self::PostToolUse
                | Self::PostToolUseFailure
                | Self::Stop
                | Self::PreCompact
                | Self::PermissionRequest
                | Self::Notification
        )
    }

    /// Check if this is an OMC-specific event.
    pub fn is_omc_specific(&self) -> bool {
        matches!(self, Self::SubagentStart | Self::SubagentStop)
    }

    /// Check if this is an omc-team event.
    pub fn is_omc_team(&self) -> bool {
        matches!(
            self,
            Self::TaskCreated | Self::TaskCompleted | Self::TeammateIdle
        )
    }
}

impl std::fmt::Display for HookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for HookEvent {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_str(s)
    }
}

/// ToolName represents the available Claude Code tools.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolName {
    Bash,
    Read,
    Write,
    Edit,
    Grep,
    Glob,
    Lsp,
    WebSearch,
    WebFetch,
    Task,
    NotepadRead,
    NotepadWrite,
    Agent,
    Other(String),
}

impl ToolName {
    /// Parse a tool name from a string (case-insensitive).
    pub fn parse_str(s: &str) -> Result<Self, String> {
        let normalized = s.trim();
        match normalized {
            "Bash" | "bash" => Ok(Self::Bash),
            "Read" | "read" => Ok(Self::Read),
            "Write" | "write" => Ok(Self::Write),
            "Edit" | "edit" => Ok(Self::Edit),
            "Grep" | "grep" => Ok(Self::Grep),
            "Glob" | "glob" => Ok(Self::Glob),
            "Lsp" | "lsp" => Ok(Self::Lsp),
            "WebSearch" | "web_search" | "web-search" | "websearch" => Ok(Self::WebSearch),
            "WebFetch" | "web_fetch" | "web-fetch" | "webfetch" => Ok(Self::WebFetch),
            "Task" | "task" => Ok(Self::Task),
            "NotepadRead" | "notepad_read" | "notepad-read" => Ok(Self::NotepadRead),
            "NotepadWrite" | "notepad_write" | "notepad-write" => Ok(Self::NotepadWrite),
            "Agent" | "agent" => Ok(Self::Agent),
            other => Ok(Self::Other(other.to_string())),
        }
    }

    /// Convert a tool name to its canonical string representation.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Bash => "Bash",
            Self::Read => "Read",
            Self::Write => "Write",
            Self::Edit => "Edit",
            Self::Grep => "Grep",
            Self::Glob => "Glob",
            Self::Lsp => "Lsp",
            Self::WebSearch => "WebSearch",
            Self::WebFetch => "WebFetch",
            Self::Task => "Task",
            Self::NotepadRead => "NotepadRead",
            Self::NotepadWrite => "NotepadWrite",
            Self::Agent => "Agent",
            Self::Other(s) => s,
        }
    }
}

impl std::fmt::Display for ToolName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ToolName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_str(s)
    }
}

impl Serialize for ToolName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ToolName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_event_from_str_exact() {
        assert_eq!(
            HookEvent::parse_str("UserPromptSubmit").unwrap(),
            HookEvent::UserPromptSubmit
        );
        assert_eq!(
            HookEvent::parse_str("SessionStart").unwrap(),
            HookEvent::SessionStart
        );
        assert_eq!(
            HookEvent::parse_str("SessionEnd").unwrap(),
            HookEvent::SessionEnd
        );
        assert_eq!(
            HookEvent::parse_str("PreToolUse").unwrap(),
            HookEvent::PreToolUse
        );
        assert_eq!(
            HookEvent::parse_str("PostToolUse").unwrap(),
            HookEvent::PostToolUse
        );
        assert_eq!(
            HookEvent::parse_str("PostToolUseFailure").unwrap(),
            HookEvent::PostToolUseFailure
        );
        assert_eq!(HookEvent::parse_str("Stop").unwrap(), HookEvent::Stop);
        assert_eq!(
            HookEvent::parse_str("PreCompact").unwrap(),
            HookEvent::PreCompact
        );
        assert_eq!(
            HookEvent::parse_str("PermissionRequest").unwrap(),
            HookEvent::PermissionRequest
        );
        assert_eq!(
            HookEvent::parse_str("Notification").unwrap(),
            HookEvent::Notification
        );
        assert_eq!(
            HookEvent::parse_str("SubagentStart").unwrap(),
            HookEvent::SubagentStart
        );
        assert_eq!(
            HookEvent::parse_str("SubagentStop").unwrap(),
            HookEvent::SubagentStop
        );
        assert_eq!(
            HookEvent::parse_str("TaskCreated").unwrap(),
            HookEvent::TaskCreated
        );
        assert_eq!(
            HookEvent::parse_str("TaskCompleted").unwrap(),
            HookEvent::TaskCompleted
        );
        assert_eq!(
            HookEvent::parse_str("TeammateIdle").unwrap(),
            HookEvent::TeammateIdle
        );
    }

    #[test]
    fn hook_event_from_str_snake_case() {
        assert_eq!(
            HookEvent::parse_str("user_prompt_submit").unwrap(),
            HookEvent::UserPromptSubmit
        );
        assert_eq!(
            HookEvent::parse_str("post_tool_use_failure").unwrap(),
            HookEvent::PostToolUseFailure
        );
        assert_eq!(
            HookEvent::parse_str("subagent_start").unwrap(),
            HookEvent::SubagentStart
        );
        assert_eq!(
            HookEvent::parse_str("task_completed").unwrap(),
            HookEvent::TaskCompleted
        );
    }

    #[test]
    fn hook_event_from_str_kebab_case() {
        assert_eq!(
            HookEvent::parse_str("user-prompt-submit").unwrap(),
            HookEvent::UserPromptSubmit
        );
        assert_eq!(
            HookEvent::parse_str("post-tool-use-failure").unwrap(),
            HookEvent::PostToolUseFailure
        );
        assert_eq!(
            HookEvent::parse_str("subagent-stop").unwrap(),
            HookEvent::SubagentStop
        );
    }

    #[test]
    fn hook_event_from_str_invalid() {
        assert!(HookEvent::parse_str("InvalidEvent").is_err());
        assert!(HookEvent::parse_str("").is_err());
    }

    #[test]
    fn hook_event_as_str() {
        assert_eq!(HookEvent::UserPromptSubmit.as_str(), "UserPromptSubmit");
        assert_eq!(HookEvent::SubagentStart.as_str(), "SubagentStart");
        assert_eq!(HookEvent::TaskCreated.as_str(), "TaskCreated");
    }

    #[test]
    fn hook_event_roundtrip() {
        for event in [
            HookEvent::UserPromptSubmit,
            HookEvent::SessionStart,
            HookEvent::SessionEnd,
            HookEvent::PreToolUse,
            HookEvent::PostToolUse,
            HookEvent::PostToolUseFailure,
            HookEvent::Stop,
            HookEvent::PreCompact,
            HookEvent::PermissionRequest,
            HookEvent::Notification,
            HookEvent::SubagentStart,
            HookEvent::SubagentStop,
            HookEvent::TaskCreated,
            HookEvent::TaskCompleted,
            HookEvent::TeammateIdle,
        ] {
            assert_eq!(HookEvent::parse_str(event.as_str()).unwrap(), event);
        }
    }

    #[test]
    fn hook_event_category_checks() {
        assert!(HookEvent::UserPromptSubmit.is_native());
        assert!(!HookEvent::UserPromptSubmit.is_omc_specific());
        assert!(!HookEvent::UserPromptSubmit.is_omc_team());

        assert!(HookEvent::SubagentStart.is_omc_specific());
        assert!(!HookEvent::SubagentStart.is_native());
        assert!(!HookEvent::SubagentStart.is_omc_team());

        assert!(HookEvent::TaskCreated.is_omc_team());
        assert!(!HookEvent::TaskCreated.is_native());
        assert!(!HookEvent::TaskCreated.is_omc_specific());
    }

    #[test]
    fn tool_name_from_str() {
        assert_eq!(ToolName::parse_str("Bash").unwrap(), ToolName::Bash);
        assert_eq!(ToolName::parse_str("Read").unwrap(), ToolName::Read);
        assert_eq!(ToolName::parse_str("Write").unwrap(), ToolName::Write);
        assert_eq!(ToolName::parse_str("Edit").unwrap(), ToolName::Edit);
        assert_eq!(ToolName::parse_str("Grep").unwrap(), ToolName::Grep);
        assert_eq!(ToolName::parse_str("Glob").unwrap(), ToolName::Glob);
        assert_eq!(ToolName::parse_str("Lsp").unwrap(), ToolName::Lsp);
        assert_eq!(
            ToolName::parse_str("WebSearch").unwrap(),
            ToolName::WebSearch
        );
        assert_eq!(ToolName::parse_str("WebFetch").unwrap(), ToolName::WebFetch);
        assert_eq!(ToolName::parse_str("Task").unwrap(), ToolName::Task);
        assert_eq!(
            ToolName::parse_str("NotepadRead").unwrap(),
            ToolName::NotepadRead
        );
        assert_eq!(
            ToolName::parse_str("NotepadWrite").unwrap(),
            ToolName::NotepadWrite
        );
        assert_eq!(ToolName::parse_str("Agent").unwrap(), ToolName::Agent);
    }

    #[test]
    fn tool_name_from_str_lowercase() {
        assert_eq!(ToolName::parse_str("bash").unwrap(), ToolName::Bash);
        assert_eq!(ToolName::parse_str("read").unwrap(), ToolName::Read);
        assert_eq!(
            ToolName::parse_str("web_search").unwrap(),
            ToolName::WebSearch
        );
        assert_eq!(
            ToolName::parse_str("web-search").unwrap(),
            ToolName::WebSearch
        );
        assert_eq!(
            ToolName::parse_str("websearch").unwrap(),
            ToolName::WebSearch
        );
    }

    #[test]
    fn tool_name_from_str_other() {
        let tool = ToolName::parse_str("CustomTool").unwrap();
        assert!(matches!(tool, ToolName::Other(s) if s == "CustomTool"));
    }

    #[test]
    fn tool_name_as_str() {
        assert_eq!(ToolName::Bash.as_str(), "Bash");
        assert_eq!(ToolName::Read.as_str(), "Read");
        assert_eq!(ToolName::WebSearch.as_str(), "WebSearch");
        assert_eq!(ToolName::Other("Custom".to_string()).as_str(), "Custom");
    }

    #[test]
    fn tool_name_roundtrip() {
        for tool in [
            ToolName::Bash,
            ToolName::Read,
            ToolName::Write,
            ToolName::Edit,
            ToolName::Grep,
            ToolName::Glob,
            ToolName::Lsp,
            ToolName::WebSearch,
            ToolName::WebFetch,
            ToolName::Task,
            ToolName::NotepadRead,
            ToolName::NotepadWrite,
            ToolName::Agent,
        ] {
            assert_eq!(ToolName::parse_str(tool.as_str()).unwrap(), tool);
        }
    }

    #[test]
    fn hook_event_display() {
        assert_eq!(
            format!("{}", HookEvent::UserPromptSubmit),
            "UserPromptSubmit"
        );
        assert_eq!(format!("{}", HookEvent::TaskCreated), "TaskCreated");
    }

    #[test]
    fn tool_name_display() {
        assert_eq!(format!("{}", ToolName::Bash), "Bash");
        assert_eq!(
            format!("{}", ToolName::Other("Custom".to_string())),
            "Custom"
        );
    }

    #[test]
    fn hook_event_serde() {
        let event = HookEvent::PreToolUse;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, "\"PreToolUse\"");
        let parsed: HookEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);
    }

    #[test]
    fn tool_name_serde() {
        let tool = ToolName::Bash;
        let json = serde_json::to_string(&tool).unwrap();
        assert_eq!(json, "\"Bash\"");
        let parsed: ToolName = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, tool);

        let custom = ToolName::Other("MyTool".to_string());
        let custom_json = serde_json::to_string(&custom).unwrap();
        assert_eq!(custom_json, "\"MyTool\"");
    }

    #[test]
    fn hook_event_parse_trait() {
        let event: HookEvent = "PostToolUse".parse().unwrap();
        assert_eq!(event, HookEvent::PostToolUse);

        let result: Result<HookEvent, _> = "Invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn tool_name_parse_trait() {
        let tool: ToolName = "Bash".parse().unwrap();
        assert_eq!(tool, ToolName::Bash);

        let tool: ToolName = "CustomTool".parse().unwrap();
        assert!(matches!(tool, ToolName::Other(s) if s == "CustomTool"));
    }
}
