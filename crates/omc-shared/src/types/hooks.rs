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
            HookEvent::SubagentStart,
            HookEvent::SubagentStop,
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
        assert_eq!(
            std::mem::variant_count::<HookEvent>(),
            11,
            "HookEvent should have exactly 11 variants"
        );
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
