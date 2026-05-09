use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::config::{HookCommand, HookEntry, HooksConfig};
use crate::events::{HookEvent, ToolName};
use crate::executor::{HookExecutor, HookResult};

/// Internal hook handler function type.
pub type InternalHookHandler =
    dyn Fn(&HookEvent, Option<&ToolName>, &str) -> HookResult + Send + Sync + 'static;

/// Internal hook registration.
pub struct InternalHook {
    /// The internal hook identifier.
    pub id: String,
    /// Human-readable description.
    pub description: Option<String>,
    /// Handler function.
    pub handler: Arc<InternalHookHandler>,
}

impl std::fmt::Debug for InternalHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InternalHook")
            .field("id", &self.id)
            .field("description", &self.description)
            .finish()
    }
}

impl Clone for InternalHook {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            description: self.description.clone(),
            handler: self.handler.clone(),
        }
    }
}

/// HookRegistry manages all hook configurations and registrations.
pub struct HookRegistry {
    /// Global hooks configuration (applies to all projects).
    global: RwLock<HooksConfig>,
    /// Project-specific hooks configuration.
    project: RwLock<HooksConfig>,
    /// Internal hook handlers.
    internal: RwLock<HashMap<String, InternalHook>>,
    /// Default hooks config path.
    #[allow(dead_code)]
    default_config_path: Option<PathBuf>,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            global: RwLock::new(HooksConfig::default()),
            project: RwLock::new(HooksConfig::default()),
            internal: RwLock::new(HashMap::new()),
            default_config_path: None,
        }
    }

    /// Create a registry with a default config path.
    pub fn with_config_path<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            default_config_path: Some(path.into()),
            ..Self::new()
        }
    }

    /// Load global hooks from a file.
    pub fn load_global<P: AsRef<str>>(&self, path: P) -> Result<(), HookRegistryError> {
        let path = path.as_ref();
        let config = HooksConfig::load_str(path)
            .map_err(|e| HookRegistryError::ConfigError(e.to_string()))?;

        let mut global = self.global.write();
        *global = config;
        Ok(())
    }

    /// Load project hooks from a file.
    pub fn load_project<P: AsRef<str>>(&self, path: P) -> Result<(), HookRegistryError> {
        let path = path.as_ref();
        let config = HooksConfig::load_str(path)
            .map_err(|e| HookRegistryError::ConfigError(e.to_string()))?;

        let mut project = self.project.write();
        *project = config;
        Ok(())
    }

    /// Register global hooks configuration.
    pub fn register_global(&self, config: HooksConfig) {
        let mut global = self.global.write();
        *global = config;
    }

    /// Register project-specific hooks configuration.
    pub fn register_project(&self, config: HooksConfig) {
        let mut project = self.project.write();
        *project = config;
    }

    /// Register an internal hook handler.
    pub fn register_internal<H>(&self, id: &str, handler: H)
    where
        H: Fn(&HookEvent, Option<&ToolName>, &str) -> HookResult + Send + Sync + 'static,
    {
        let mut internal = self.internal.write();
        internal.insert(
            id.to_string(),
            InternalHook {
                id: id.to_string(),
                description: None,
                handler: Arc::new(Box::new(handler)),
            },
        );
    }

    /// Register an internal hook handler with description.
    pub fn register_internal_with_desc<H>(&self, id: &str, description: &str, handler: H)
    where
        H: Fn(&HookEvent, Option<&ToolName>, &str) -> HookResult + Send + Sync + 'static,
    {
        let mut internal = self.internal.write();
        internal.insert(
            id.to_string(),
            InternalHook {
                id: id.to_string(),
                description: Some(description.to_string()),
                handler: Arc::new(Box::new(handler)),
            },
        );
    }

    /// Unregister an internal hook handler.
    pub fn unregister_internal(&self, id: &str) -> bool {
        let mut internal = self.internal.write();
        internal.remove(id).is_some()
    }

    /// Check if an internal hook is registered.
    pub fn has_internal(&self, id: &str) -> bool {
        let internal = self.internal.read();
        internal.contains_key(id)
    }

    /// Get all registered internal hook IDs.
    pub fn list_internal(&self) -> Vec<String> {
        let internal = self.internal.read();
        internal.keys().cloned().collect()
    }

    /// Get hooks matching the given event and optional tool.
    ///
    /// Returns hooks from both global and project configurations,
    /// with project hooks taking precedence for the same event/tool.
    pub fn get_hooks(&self, event: &HookEvent, tool: Option<&ToolName>) -> Vec<HookMatch> {
        let global = self.global.read();
        let project = self.project.read();

        let mut matches = Vec::new();

        // Global hooks first
        for entry in global.get_hooks(event, tool) {
            matches.push(HookMatch {
                source: HookSource::Global,
                entry: entry.clone(),
            });
        }

        // Project hooks override global hooks for same patterns
        for entry in project.get_hooks(event, tool) {
            matches.push(HookMatch {
                source: HookSource::Project,
                entry: entry.clone(),
            });
        }

        matches
    }

    /// Get all hook commands for the given event and optional tool.
    pub fn get_commands(
        &self,
        event: &HookEvent,
        tool: Option<&ToolName>,
    ) -> Vec<(HookSource, HookCommand)> {
        let global = self.global.read();
        let project = self.project.read();

        let mut commands = Vec::new();

        // Global commands first
        for cmd in global.get_commands(event, tool) {
            commands.push((HookSource::Global, cmd.clone()));
        }

        // Project commands
        for cmd in project.get_commands(event, tool) {
            commands.push((HookSource::Project, cmd.clone()));
        }

        commands
    }

    /// Call an internal hook handler.
    pub fn call_internal(
        &self,
        id: &str,
        event: &HookEvent,
        tool: Option<&ToolName>,
        input: &str,
    ) -> Result<HookResult, HookRegistryError> {
        let internal = self.internal.read();
        let hook = internal
            .get(id)
            .ok_or_else(|| HookRegistryError::InternalHookNotFound(id.to_string()))?;

        let handler = hook.handler.clone();
        drop(internal);

        Ok(handler(event, tool, input))
    }

    /// Execute all matching hooks for an event.
    pub fn execute_hooks(
        &self,
        event: &HookEvent,
        tool: Option<&ToolName>,
        input: &str,
    ) -> Vec<HookExecutionResult> {
        let matches = self.get_hooks(event, tool);
        let mut results = Vec::new();

        for hook_match in matches {
            for command in &hook_match.entry.hooks {
                let result = match command {
                    HookCommand::Command {
                        command: cmd,
                        timeout_secs,
                    } => {
                        let executor = HookExecutor::new()
                            .with_timeout(std::time::Duration::from_secs(*timeout_secs));
                        HookExecutionResult {
                            source: hook_match.source.clone(),
                            command: command.clone(),
                            result: executor.execute(cmd, Some(input)),
                        }
                    }
                    HookCommand::Internal => {
                        // Internal hooks are called separately via call_internal
                        HookExecutionResult {
                            source: hook_match.source.clone(),
                            command: command.clone(),
                            result: HookResult::default(),
                        }
                    }
                };
                results.push(result);
            }
        }

        results
    }

    /// Clear all registered hooks.
    pub fn clear(&self) {
        let mut global = self.global.write();
        let mut project = self.project.write();
        let mut internal = self.internal.write();

        global.clear();
        project.clear();
        internal.clear();
    }

    /// Clear only global hooks.
    pub fn clear_global(&self) {
        let mut global = self.global.write();
        global.clear();
    }

    /// Clear only project hooks.
    pub fn clear_project(&self) {
        let mut project = self.project.write();
        project.clear();
    }

    /// Get statistics about the registry.
    pub fn stats(&self) -> HookRegistryStats {
        let global = self.global.read();
        let project = self.project.read();
        let internal = self.internal.read();

        HookRegistryStats {
            global_events: global.event_count(),
            global_hooks: global.total_hook_count(),
            project_events: project.event_count(),
            project_hooks: project.total_hook_count(),
            internal_hooks: internal.len(),
        }
    }
}

/// Source of a hook configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookSource {
    Global,
    Project,
}

/// A hook match with its source.
#[derive(Debug, Clone)]
pub struct HookMatch {
    pub source: HookSource,
    pub entry: HookEntry,
}

/// Result of executing a single hook command.
#[derive(Debug, Clone)]
pub struct HookExecutionResult {
    pub source: HookSource,
    pub command: HookCommand,
    pub result: HookResult,
}

/// Statistics about the hook registry.
#[derive(Debug, Clone)]
pub struct HookRegistryStats {
    pub global_events: usize,
    pub global_hooks: usize,
    pub project_events: usize,
    pub project_hooks: usize,
    pub internal_hooks: usize,
}

/// Errors that can occur in the hook registry.
#[derive(Debug, thiserror::Error)]
pub enum HookRegistryError {
    #[error("configuration error: {0}")]
    ConfigError(String),

    #[error("internal hook not found: {0}")]
    InternalHookNotFound(String),

    #[error("hook execution error: {0}")]
    ExecutionError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn create_test_config(json: &str) -> HooksConfig {
        HooksConfig::load_str(json).unwrap()
    }

    #[test]
    fn registry_new() {
        let registry = HookRegistry::new();
        let stats = registry.stats();
        assert_eq!(stats.global_events, 0);
        assert_eq!(stats.global_hooks, 0);
        assert_eq!(stats.project_events, 0);
        assert_eq!(stats.project_hooks, 0);
        assert_eq!(stats.internal_hooks, 0);
    }

    #[test]
    fn registry_register_global() {
        let registry = HookRegistry::new();
        let config = create_test_config(
            r#"{
            "hooks": {
                "SessionStart": [{"matcher": "*", "hooks": []}]
            }
        }"#,
        );
        registry.register_global(config);

        let stats = registry.stats();
        assert_eq!(stats.global_events, 1);
    }

    #[test]
    fn registry_register_project() {
        let registry = HookRegistry::new();
        let config = create_test_config(
            r#"{
            "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": []}]
            }
        }"#,
        );
        registry.register_project(config);

        let stats = registry.stats();
        assert_eq!(stats.project_events, 1);
    }

    #[test]
    fn registry_load_config() {
        let registry = HookRegistry::new();
        registry
            .load_global(r#"{"hooks": {"SessionEnd": [{"matcher": "*", "hooks": []}]}}"#)
            .unwrap();
        registry
            .load_project(r#"{"hooks": {"Stop": [{"matcher": "*", "hooks": []}]}}"#)
            .unwrap();

        let stats = registry.stats();
        assert_eq!(stats.global_events, 1);
        assert_eq!(stats.project_events, 1);
    }

    #[test]
    fn registry_register_internal() {
        let registry = HookRegistry::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        registry.register_internal("test_hook", move |_, _, _| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            HookResult::default()
        });

        assert!(registry.has_internal("test_hook"));
        assert_eq!(registry.list_internal(), vec!["test_hook"]);

        registry
            .call_internal("test_hook", &HookEvent::SessionStart, None, "")
            .unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn registry_register_internal_with_description() {
        let registry = HookRegistry::new();
        registry.register_internal_with_desc("logged_hook", "Logs all events", |_, _, _| {
            HookResult::default()
        });

        assert!(registry.has_internal("logged_hook"));
    }

    #[test]
    fn registry_unregister_internal() {
        let registry = HookRegistry::new();
        registry.register_internal("temp_hook", |_, _, _| HookResult::default());

        assert!(registry.has_internal("temp_hook"));
        assert!(registry.unregister_internal("temp_hook"));
        assert!(!registry.has_internal("temp_hook"));
        assert!(!registry.unregister_internal("nonexistent"));
    }

    #[test]
    fn registry_get_hooks() {
        let registry = HookRegistry::new();
        registry.register_global(create_test_config(
            r#"{
            "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": []}, {"matcher": "Read", "hooks": []}]
            }
        }"#,
        ));
        registry.register_project(create_test_config(
            r#"{
            "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": []}]
            }
        }"#,
        ));

        let event = HookEvent::PreToolUse;
        let hooks = registry.get_hooks(&event, Some(&ToolName::Bash));

        assert!(!hooks.is_empty());
        // Should have both global and project matches
        assert!(hooks.iter().any(|m| m.source == HookSource::Global));
        assert!(hooks.iter().any(|m| m.source == HookSource::Project));
    }

    #[test]
    fn registry_get_commands() {
        let registry = HookRegistry::new();
        registry.register_global(create_test_config(
            r#"{
            "hooks": {
                "SessionStart": [{
                    "matcher": "*",
                    "hooks": [{"type": "command", "command": "echo global"}]
                }]
            }
        }"#,
        ));
        registry.register_project(create_test_config(
            r#"{
            "hooks": {
                "SessionStart": [{
                    "matcher": "*",
                    "hooks": [{"type": "command", "command": "echo project"}]
                }]
            }
        }"#,
        ));

        let commands = registry.get_commands(&HookEvent::SessionStart, None);
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn registry_call_internal_not_found() {
        let registry = HookRegistry::new();
        let result = registry.call_internal("nonexistent", &HookEvent::SessionStart, None, "");
        assert!(matches!(
            result,
            Err(HookRegistryError::InternalHookNotFound(_))
        ));
    }

    #[test]
    fn registry_execute_hooks() {
        let registry = HookRegistry::new();
        registry.register_global(create_test_config(
            r#"{
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": "echo test", "timeout_secs": 5}]
                }]
            }
        }"#,
        ));

        let results = registry.execute_hooks(&HookEvent::PreToolUse, Some(&ToolName::Bash), "");
        assert!(!results.is_empty());

        // Check that command was executed (should have no error)
        let result = &results[0];
        assert!(result.result.error.is_none());
    }

    #[test]
    fn registry_clear() {
        let registry = HookRegistry::new();
        registry.register_global(create_test_config(
            r#"{
            "hooks": {"SessionStart": [{"matcher": "*", "hooks": []}]}
        }"#,
        ));
        registry.register_internal("test", |_, _, _| HookResult::default());

        assert_eq!(registry.stats().global_events, 1);
        assert_eq!(registry.stats().internal_hooks, 1);

        registry.clear();

        let stats = registry.stats();
        assert_eq!(stats.global_events, 0);
        assert_eq!(stats.global_hooks, 0);
        assert_eq!(stats.project_events, 0);
        assert_eq!(stats.project_hooks, 0);
        assert_eq!(stats.internal_hooks, 0);
    }

    #[test]
    fn registry_clear_global_only() {
        let registry = HookRegistry::new();
        registry.register_global(create_test_config(
            r#"{
            "hooks": {"SessionStart": [{"matcher": "*", "hooks": []}]}
        }"#,
        ));
        registry.register_project(create_test_config(
            r#"{
            "hooks": {"PreToolUse": [{"matcher": "*", "hooks": []}]}
        }"#,
        ));

        registry.clear_global();

        let stats = registry.stats();
        assert_eq!(stats.global_events, 0);
        assert_eq!(stats.project_events, 1);
    }

    #[test]
    fn registry_clear_project_only() {
        let registry = HookRegistry::new();
        registry.register_global(create_test_config(
            r#"{
            "hooks": {"SessionStart": [{"matcher": "*", "hooks": []}]}
        }"#,
        ));
        registry.register_project(create_test_config(
            r#"{
            "hooks": {"PreToolUse": [{"matcher": "*", "hooks": []}]}
        }"#,
        ));

        registry.clear_project();

        let stats = registry.stats();
        assert_eq!(stats.global_events, 1);
        assert_eq!(stats.project_events, 0);
    }

    #[test]
    fn registry_hook_source_clone() {
        assert_eq!(HookSource::Global, HookSource::Global);
        assert_eq!(HookSource::Project, HookSource::Project);
        assert_ne!(HookSource::Global, HookSource::Project);
    }

    #[test]
    fn registry_default() {
        let registry = HookRegistry::default();
        assert_eq!(registry.stats().global_events, 0);
    }
}
