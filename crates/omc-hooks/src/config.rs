use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::events::{HookEvent, ToolName};

/// HookCommand represents the command to execute for a hook.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HookCommand {
    /// External command execution with optional timeout.
    Command {
        #[serde(default = "default_command")]
        command: String,
        #[serde(default = "default_timeout")]
        timeout_secs: u64,
    },
    /// Internal hook handler reference.
    Internal,
}

fn default_command() -> String {
    String::default()
}

fn default_timeout() -> u64 {
    30
}

/// HookEntry represents a single hook configuration with a matcher and commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEntry {
    /// Matcher pattern for the hook (glob pattern or "*" for all).
    pub matcher: String,
    /// List of hook commands to execute.
    #[serde(default)]
    pub hooks: Vec<HookCommand>,
}

impl HookEntry {
    /// Check if this hook entry matches the given tool name.
    ///
    /// When tool is `None`, only wildcard matchers ("*") match.
    /// When tool is `Some`, the matcher pattern is checked against the tool name.
    pub fn matches(&self, tool: Option<&ToolName>) -> bool {
        match tool {
            Some(t) => glob_match_str(&self.matcher, t.as_str()),
            None => self.matcher == "*",
        }
    }
}

/// EventHooks represents all hooks registered for a specific event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventHooks {
    /// Matcher pattern for the event (glob pattern or "*" for all).
    pub matcher: String,
    /// List of hook entries for this event.
    #[serde(default)]
    pub hooks: Vec<HookEntry>,
}

impl EventHooks {
    /// Check if this event hooks match the given event and optional tool.
    pub fn matches(&self, event: &HookEvent, tool: Option<&ToolName>) -> bool {
        // First check if the event matcher matches (always true for "*" or exact match)
        if self.matcher != "*"
            && !self.matcher.is_empty()
            && !glob_match_str(&self.matcher, event.as_str())
        {
            return false;
        }
        // Then check if any hook entry matches the tool
        self.hooks.iter().any(|entry| entry.matches(tool))
    }
}

/// HooksConfig represents the complete hooks configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HooksConfig {
    /// Human-readable description of this hooks configuration.
    #[serde(default)]
    pub description: Option<String>,
    /// Map of event names to their hook configurations.
    #[serde(default)]
    pub hooks: HashMap<String, Vec<HookEntry>>,
}

impl HooksConfig {
    /// Load hooks configuration from a JSON file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, HooksConfigError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .map_err(|e| HooksConfigError::IoError(path.display().to_string(), e.to_string()))?;

        serde_json::from_str(&content)
            .map_err(|e| HooksConfigError::ParseError(path.display().to_string(), e.to_string()))
    }

    /// Load hooks configuration from a JSON string.
    pub fn load_str(content: &str) -> Result<Self, HooksConfigError> {
        serde_json::from_str(content)
            .map_err(|e| HooksConfigError::ParseError("string".to_string(), e.to_string()))
    }

    /// Get all hooks that match the given event and optional tool.
    pub fn get_hooks(&self, event: &HookEvent, tool: Option<&ToolName>) -> Vec<&HookEntry> {
        let event_name = event.as_str();
        let event_hooks = match self.hooks.get(event_name) {
            Some(hooks) => hooks,
            None => return Vec::new(),
        };

        event_hooks
            .iter()
            .filter(|entry| entry.matches(tool))
            .collect()
    }

    /// Get all hook commands for the given event and optional tool.
    pub fn get_commands(&self, event: &HookEvent, tool: Option<&ToolName>) -> Vec<&HookCommand> {
        self.get_hooks(event, tool)
            .iter()
            .flat_map(|entry| entry.hooks.iter())
            .collect()
    }

    /// Add a hook entry for a specific event.
    pub fn add_hook(&mut self, event: &str, entry: HookEntry) {
        self.hooks.entry(event.to_string()).or_default().push(entry);
    }

    /// Remove all hooks for a specific event.
    pub fn remove_event_hooks(&mut self, event: &str) {
        self.hooks.remove(event);
    }

    /// Clear all hooks.
    pub fn clear(&mut self) {
        self.hooks.clear();
    }

    /// Check if this configuration has any hooks.
    pub fn is_empty(&self) -> bool {
        self.hooks.values().all(|v| v.is_empty())
    }

    /// Get the number of events with hooks.
    pub fn event_count(&self) -> usize {
        self.hooks.len()
    }

    /// Get the total number of hook entries across all events.
    pub fn total_hook_count(&self) -> usize {
        self.hooks.values().map(|v| v.len()).sum()
    }
}

/// Errors that can occur when loading or parsing hooks configuration.
#[derive(Debug, thiserror::Error)]
pub enum HooksConfigError {
    #[error("failed to read hooks file '{0}': {1}")]
    IoError(String, String),

    #[error("failed to parse hooks file '{0}': {1}")]
    ParseError(String, String),
}

/// Check if a glob pattern matches a target string.
///
/// Supports:
/// - `*` matches any sequence of characters
/// - `?` matches any single character
fn glob_match_str(pattern: &str, target: &str) -> bool {
    // "*" matches everything
    if pattern == "*" {
        return true;
    }

    // Simple glob matching
    let mut pattern_chars = pattern.chars().peekable();
    let mut target_chars = target.chars().peekable();

    while pattern_chars.peek().is_some() || target_chars.peek().is_some() {
        match pattern_chars.next() {
            None => {
                // Pattern exhausted, target must also be exhausted
                return target_chars.peek().is_none();
            }
            Some('*') => {
                // Star matches zero or more characters
                // Try matching at current position and advancing pattern
                while target_chars.peek().is_some() {
                    let remaining_pattern: String = pattern_chars.clone().collect();
                    let remaining_target: String = target_chars.clone().collect();
                    if glob_match_str(&remaining_pattern, &remaining_target) {
                        return true;
                    }
                    target_chars.next();
                }
                // Continue with pattern
            }
            Some(c) => match target_chars.next() {
                Some(tc) if tc == c => {}
                _ => return false,
            },
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_command_serde_command() {
        let cmd = HookCommand::Command {
            command: "echo hello".to_string(),
            timeout_secs: 30,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"type\":\"command\""));
        assert!(json.contains("\"command\":\"echo hello\""));
        assert!(json.contains("\"timeout_secs\":30"));
    }

    #[test]
    fn hook_command_serde_internal() {
        let cmd = HookCommand::Internal;
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"type\":\"internal\""));
    }

    #[test]
    fn hook_command_deserialize_command() {
        let json = r#"{"type":"command","command":"echo test","timeout_secs":60}"#;
        let cmd: HookCommand = serde_json::from_str(json).unwrap();
        match cmd {
            HookCommand::Command {
                command,
                timeout_secs,
            } => {
                assert_eq!(command, "echo test");
                assert_eq!(timeout_secs, 60);
            }
            HookCommand::Internal => panic!("expected Command variant"),
        }
    }

    #[test]
    fn hook_command_deserialize_internal() {
        let json = r#"{"type":"internal"}"#;
        let cmd: HookCommand = serde_json::from_str(json).unwrap();
        match cmd {
            HookCommand::Internal => {}
            HookCommand::Command { .. } => panic!("expected Internal variant"),
        }
    }

    #[test]
    fn hook_entry_matches() {
        let entry = HookEntry {
            matcher: "Bash".to_string(),
            hooks: vec![],
        };
        assert!(entry.matches(Some(&ToolName::Bash)));
        assert!(!entry.matches(Some(&ToolName::Read)));
        assert!(!entry.matches(None));
    }

    #[test]
    fn hook_entry_matches_wildcard() {
        let entry = HookEntry {
            matcher: "*".to_string(),
            hooks: vec![],
        };
        assert!(entry.matches(Some(&ToolName::Bash)));
        assert!(entry.matches(Some(&ToolName::Read)));
        assert!(entry.matches(None));
    }

    #[test]
    fn hooks_config_load() {
        let json = r#"{
            "description": "Test hooks",
            "hooks": {
                "PreToolUse": [
                    {"matcher": "Bash", "hooks": [{"type": "command", "command": "echo pre"}]}
                ]
            }
        }"#;
        let config = HooksConfig::load_str(json).unwrap();
        assert_eq!(config.description.as_deref(), Some("Test hooks"));
        assert_eq!(config.event_count(), 1);
        assert_eq!(config.total_hook_count(), 1);
    }

    #[test]
    fn hooks_config_get_hooks() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    {"matcher": "Bash", "hooks": [{"type": "command", "command": "echo bash"}]},
                    {"matcher": "Read", "hooks": [{"type": "command", "command": "echo read"}]}
                ]
            }
        }"#;
        let config = HooksConfig::load_str(json).unwrap();

        let event = HookEvent::PreToolUse;
        let bash_hooks = config.get_hooks(&event, Some(&ToolName::Bash));
        assert_eq!(bash_hooks.len(), 1);

        let read_hooks = config.get_hooks(&event, Some(&ToolName::Read));
        assert_eq!(read_hooks.len(), 1);

        let all_hooks = config.get_hooks(&event, None);
        assert_eq!(all_hooks.len(), 0); // No wildcard matchers
    }

    #[test]
    fn hooks_config_add_remove() {
        let mut config = HooksConfig::default();
        config.add_hook(
            "SessionStart",
            HookEntry {
                matcher: "*".to_string(),
                hooks: vec![],
            },
        );
        assert_eq!(config.event_count(), 1);

        config.remove_event_hooks("SessionStart");
        assert_eq!(config.event_count(), 0);
    }

    #[test]
    fn hooks_config_empty() {
        let config = HooksConfig::default();
        assert!(config.is_empty());
        assert_eq!(config.event_count(), 0);
        assert_eq!(config.total_hook_count(), 0);
    }

    #[test]
    fn glob_match_tests() {
        assert!(glob_match_str("*", "anything"));
        assert!(glob_match_str("Bash", "Bash"));
        assert!(!glob_match_str("Read", "Write"));
        assert!(glob_match_str("", ""));
    }

    #[test]
    fn hooks_config_get_commands() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    {"matcher": "Bash", "hooks": [
                        {"type": "command", "command": "echo 1"},
                        {"type": "command", "command": "echo 2"}
                    ]}
                ]
            }
        }"#;
        let config = HooksConfig::load_str(json).unwrap();
        let event = HookEvent::PreToolUse;
        let commands = config.get_commands(&event, Some(&ToolName::Bash));
        assert_eq!(commands.len(), 2);
    }
}
