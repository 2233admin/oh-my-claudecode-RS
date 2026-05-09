use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Events that can trigger hooks
#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    UserPromptSubmit,
    SessionStart,
    SessionEnd,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Stop,
    PreCompact,
    PermissionRequest,
    SubagentStart,
    SubagentStop,
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
