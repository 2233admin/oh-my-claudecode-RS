use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Events that can trigger hooks.
///
/// These are divided into three categories:
/// - Claude native events: emitted by Claude Code itself
/// - OMC specific events: custom events added by OMC
/// - omc-team events: events from the agent team orchestration system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
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

/// Type of hook execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookType {
    Command,
    Handler,
}

/// A single hook definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookDefinition {
    #[serde(rename = "type")]
    pub hook_type: HookType,
    pub command: String,
    #[serde(default)]
    pub timeout: Option<u64>,
}

/// Matcher for conditional hook execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher {
    pub matcher: String,
    pub hooks: Vec<HookDefinition>,
}

/// Top-level hooks configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    pub description: String,
    pub hooks: BTreeMap<HookEvent, Vec<HookMatcher>>,
}

/// Result from hook execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResult {
    #[serde(default)]
    pub continue_flag: bool,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub modified_input: Option<String>,
}

impl Default for HookResult {
    fn default() -> Self {
        Self {
            continue_flag: true,
            message: None,
            modified_input: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    // === HookEvent serialization roundtrip ===

    #[test]
    fn hook_event_serialization_roundtrip() {
        let variants = vec![
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
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: HookEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized, "roundtrip failed for {:?}", variant);
        }
    }

    #[test]
    fn hook_event_uses_pascal_case() {
        let json = serde_json::to_string(&HookEvent::UserPromptSubmit).unwrap();
        assert_eq!(json, "\"UserPromptSubmit\"");
        let json = serde_json::to_string(&HookEvent::PostToolUseFailure).unwrap();
        assert_eq!(json, "\"PostToolUseFailure\"");
    }

    #[test]
    fn hook_event_variant_count() {
        // Manual count since std::mem::variant_count is unstable
        let all = [
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
        ];
        assert_eq!(all.len(), 15, "HookEvent should have exactly 15 variants");
    }

    // === HookEvent string parsing ===

    #[test]
    fn hook_event_parse_str_exact() {
        assert_eq!(
            HookEvent::parse_str("UserPromptSubmit").unwrap(),
            HookEvent::UserPromptSubmit
        );
        assert_eq!(
            HookEvent::parse_str("Notification").unwrap(),
            HookEvent::Notification
        );
        assert_eq!(
            HookEvent::parse_str("TaskCreated").unwrap(),
            HookEvent::TaskCreated
        );
        assert_eq!(
            HookEvent::parse_str("TeammateIdle").unwrap(),
            HookEvent::TeammateIdle
        );
    }

    #[test]
    fn hook_event_parse_str_snake_case() {
        assert_eq!(
            HookEvent::parse_str("user_prompt_submit").unwrap(),
            HookEvent::UserPromptSubmit
        );
        assert_eq!(
            HookEvent::parse_str("post_tool_use_failure").unwrap(),
            HookEvent::PostToolUseFailure
        );
        assert_eq!(
            HookEvent::parse_str("task_completed").unwrap(),
            HookEvent::TaskCompleted
        );
    }

    #[test]
    fn hook_event_parse_str_kebab_case() {
        assert_eq!(
            HookEvent::parse_str("user-prompt-submit").unwrap(),
            HookEvent::UserPromptSubmit
        );
        assert_eq!(
            HookEvent::parse_str("subagent-stop").unwrap(),
            HookEvent::SubagentStop
        );
    }

    #[test]
    fn hook_event_parse_str_invalid() {
        assert!(HookEvent::parse_str("InvalidEvent").is_err());
        assert!(HookEvent::parse_str("").is_err());
    }

    #[test]
    fn hook_event_as_str_roundtrip() {
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

    // === HookEvent Display / FromStr ===

    #[test]
    fn hook_event_display() {
        assert_eq!(
            format!("{}", HookEvent::UserPromptSubmit),
            "UserPromptSubmit"
        );
        assert_eq!(format!("{}", HookEvent::TaskCreated), "TaskCreated");
    }

    #[test]
    fn hook_event_from_str_trait() {
        let event: HookEvent = "PostToolUse".parse().unwrap();
        assert_eq!(event, HookEvent::PostToolUse);

        let result: Result<HookEvent, _> = "Invalid".parse();
        assert!(result.is_err());
    }

    // === HookEvent category checks ===

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

    // === HookType serialization ===

    #[test]
    fn hook_type_serialization_roundtrip() {
        let variants = vec![HookType::Command, HookType::Handler];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: HookType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn hook_type_uses_lowercase() {
        assert_eq!(
            serde_json::to_string(&HookType::Command).unwrap(),
            "\"command\""
        );
        assert_eq!(
            serde_json::to_string(&HookType::Handler).unwrap(),
            "\"handler\""
        );
    }

    // === HookDefinition ===

    #[test]
    fn hook_definition_with_timeout() {
        let json = r#"{"type":"command","command":"echo hello","timeout":5000}"#;
        let def: HookDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(def.hook_type, HookType::Command);
        assert_eq!(def.command, "echo hello");
        assert_eq!(def.timeout, Some(5000));
    }

    #[test]
    fn hook_definition_without_timeout() {
        let json = r#"{"type":"handler","command":"process"}"#;
        let def: HookDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(def.hook_type, HookType::Handler);
        assert_eq!(def.command, "process");
        assert_eq!(def.timeout, None);
    }

    // === HookMatcher ===

    #[test]
    fn hook_matcher_roundtrip() {
        let matcher = HookMatcher {
            matcher: "*.py".into(),
            hooks: vec![HookDefinition {
                hook_type: HookType::Command,
                command: "ruff check".into(),
                timeout: Some(10000),
            }],
        };
        let json = serde_json::to_string(&matcher).unwrap();
        let deserialized: HookMatcher = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.matcher, "*.py");
        assert_eq!(deserialized.hooks.len(), 1);
    }

    // === HooksConfig ===

    #[test]
    fn hooks_config_roundtrip() {
        let mut hooks = BTreeMap::new();
        hooks.insert(
            HookEvent::PreToolUse,
            vec![HookMatcher {
                matcher: "*".into(),
                hooks: vec![HookDefinition {
                    hook_type: HookType::Command,
                    command: "validate".into(),
                    timeout: None,
                }],
            }],
        );
        let config = HooksConfig {
            description: "test hooks".into(),
            hooks,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: HooksConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.description, "test hooks");
        assert!(deserialized.hooks.contains_key(&HookEvent::PreToolUse));
    }

    // === HookResult ===

    #[test]
    fn hook_result_default() {
        let result = HookResult::default();
        assert!(result.continue_flag);
        assert!(result.message.is_none());
        assert!(result.modified_input.is_none());
    }

    #[test]
    fn hook_result_with_all_fields() {
        let result = HookResult {
            continue_flag: false,
            message: Some("blocked".into()),
            modified_input: Some("modified".into()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: HookResult = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.continue_flag);
        assert_eq!(deserialized.message.as_deref(), Some("blocked"));
        assert_eq!(deserialized.modified_input.as_deref(), Some("modified"));
    }

    #[test]
    fn hook_result_json_defaults() {
        let json = r#"{}"#;
        let result: HookResult = serde_json::from_str(json).unwrap();
        assert!(!result.continue_flag, "default for bool is false");
        assert!(result.message.is_none());
        assert!(result.modified_input.is_none());
    }
}
