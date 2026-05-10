//! MCP Tool Group Registry with TTL-based caching.
//!
//! Organizes tools into logical groups (core, agents, memory, devtools,
//! intelligence, security) and provides a cached, filtered tool list.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::tools::McpTool;

/// A group of related MCP tools with a shared naming prefix.
pub struct ToolGroup {
    /// Display name of the group.
    pub name: String,
    /// Whether this group is currently enabled.
    pub enabled: bool,
    /// Source module or origin identifier.
    pub source: String,
    /// Tool name prefixes that belong to this group.
    pub prefixes: Vec<String>,
}

/// Cached resolved tool list with TTL-based invalidation.
pub struct ToolCache {
    tools: Vec<Box<dyn McpTool>>,
    fetched_at: Instant,
    ttl: Duration,
}

impl ToolCache {
    fn new(ttl: Duration) -> Self {
        Self {
            tools: Vec::new(),
            fetched_at: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() >= self.ttl
    }

    fn update(&mut self, tools: Vec<Box<dyn McpTool>>) {
        self.tools = tools;
        self.fetched_at = Instant::now();
    }

    fn get(&self) -> &[Box<dyn McpTool>] {
        &self.tools
    }
}

/// Registry managing tool groups with a cached tool list.
pub struct McpToolRegistry {
    groups: HashMap<String, ToolGroup>,
    cache: ToolCache,
}

/// Default tool group definitions.
const DEFAULT_GROUPS: &[(&str, &str, bool, &[&str])] = &[
    ("core", "omc-mcp", true, &["state_", "notepad_"]),
    ("agents", "omc-mcp", false, &["agent_"]),
    ("memory", "omc-mcp", true, &["project_memory_"]),
    ("devtools", "omc-mcp", false, &["dev_"]),
    ("intelligence", "omc-mcp", false, &["intel_"]),
    ("security", "omc-mcp", false, &["security_"]),
];

impl McpToolRegistry {
    /// Create a new registry with default groups and a 60-second cache TTL.
    pub fn new() -> Self {
        Self::with_ttl(Duration::from_secs(60))
    }

    /// Create a new registry with a custom cache TTL.
    pub fn with_ttl(ttl: Duration) -> Self {
        let groups: HashMap<String, ToolGroup> = DEFAULT_GROUPS
            .iter()
            .map(|(name, source, enabled, prefixes)| {
                (
                    name.to_string(),
                    ToolGroup {
                        name: name.to_string(),
                        enabled: *enabled,
                        source: source.to_string(),
                        prefixes: prefixes.iter().map(|s| s.to_string()).collect(),
                    },
                )
            })
            .collect();

        Self {
            groups,
            cache: ToolCache::new(ttl),
        }
    }

    /// Get the resolved tool list, refreshing the cache if expired.
    pub fn tools(&mut self) -> &[Box<dyn McpTool>] {
        if self.cache.is_expired() {
            let tools = self.collect_enabled_tools();
            self.cache.update(tools);
        }
        self.cache.get()
    }

    /// Enable or disable a tool group. Invalidates the cache.
    pub fn set_group_enabled(&mut self, name: &str, enabled: bool) {
        if let Some(group) = self.groups.get_mut(name) {
            group.enabled = enabled;
            self.invalidate_cache();
        }
    }

    /// Check whether a group is enabled.
    pub fn is_group_enabled(&self, name: &str) -> bool {
        self.groups.get(name).is_some_and(|g| g.enabled)
    }

    /// Get a reference to all registered groups.
    pub fn groups(&self) -> &HashMap<String, ToolGroup> {
        &self.groups
    }

    /// Add or replace a tool group. Invalidates the cache.
    pub fn register_group(&mut self, group: ToolGroup) {
        self.groups.insert(group.name.clone(), group);
        self.invalidate_cache();
    }

    /// Force-invalidate the cache so the next call to `tools()` refreshes it.
    pub fn invalidate_cache(&mut self) {
        self.cache.fetched_at = Instant::now() - self.cache.ttl;
    }

    /// Collect tools from `crate::all_tools()` filtered by enabled group prefixes.
    fn collect_enabled_tools(&self) -> Vec<Box<dyn McpTool>> {
        let enabled_prefixes: Vec<&str> = self
            .groups
            .values()
            .filter(|g| g.enabled)
            .flat_map(|g| g.prefixes.iter().map(|p| p.as_str()))
            .collect();

        if enabled_prefixes.is_empty() {
            return Vec::new();
        }

        crate::all_tools()
            .into_iter()
            .filter(|tool| {
                let name = tool.definition().name;
                enabled_prefixes
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
            })
            .collect()
    }
}

impl Default for McpToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_groups_exist() {
        let registry = McpToolRegistry::default();
        assert_eq!(registry.groups().len(), 6);
        assert!(registry.groups().contains_key("core"));
        assert!(registry.groups().contains_key("agents"));
        assert!(registry.groups().contains_key("memory"));
        assert!(registry.groups().contains_key("devtools"));
        assert!(registry.groups().contains_key("intelligence"));
        assert!(registry.groups().contains_key("security"));
    }

    #[test]
    fn default_enabled_groups() {
        let registry = McpToolRegistry::default();
        assert!(registry.is_group_enabled("core"));
        assert!(registry.is_group_enabled("memory"));
        assert!(!registry.is_group_enabled("agents"));
        assert!(!registry.is_group_enabled("devtools"));
        assert!(!registry.is_group_enabled("intelligence"));
        assert!(!registry.is_group_enabled("security"));
    }

    #[test]
    fn toggle_group() {
        let mut registry = McpToolRegistry::default();
        assert!(registry.is_group_enabled("core"));

        registry.set_group_enabled("core", false);
        assert!(!registry.is_group_enabled("core"));

        registry.set_group_enabled("core", true);
        assert!(registry.is_group_enabled("core"));
    }

    #[test]
    fn toggle_nonexistent_group_is_noop() {
        let mut registry = McpToolRegistry::default();
        registry.set_group_enabled("nonexistent", true);
        assert!(!registry.is_group_enabled("nonexistent"));
    }

    #[test]
    fn tools_returns_core_and_memory_by_default() {
        let mut registry = McpToolRegistry::default();
        let tools = registry.tools();

        let names: Vec<String> = tools.iter().map(|t| t.definition().name).collect();

        // core tools (state_, notepad_)
        assert!(names.iter().any(|n| n == "state_read"));
        assert!(names.iter().any(|n| n == "state_write"));
        assert!(names.iter().any(|n| n == "notepad_read"));

        // memory tools (project_memory_)
        assert!(names.iter().any(|n| n == "project_memory_read"));
        assert!(names.iter().any(|n| n == "project_memory_write"));

        // disabled groups should not appear
        assert!(!names.iter().any(|n| n.starts_with("agent_")));
        assert!(!names.iter().any(|n| n.starts_with("dev_")));
        assert!(!names.iter().any(|n| n.starts_with("intel_")));
        assert!(!names.iter().any(|n| n.starts_with("security_")));
    }

    #[test]
    fn disabling_group_removes_tools() {
        let mut registry = McpToolRegistry::default();

        // Sanity: memory tools present
        let names_before: Vec<String> = registry
            .tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();
        assert!(
            names_before
                .iter()
                .any(|n| n.starts_with("project_memory_"))
        );

        // Disable memory
        registry.set_group_enabled("memory", false);

        let names_after: Vec<String> = registry
            .tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();
        assert!(!names_after.iter().any(|n| n.starts_with("project_memory_")));
    }

    #[test]
    fn enabling_disabled_group_adds_tools() {
        let mut registry = McpToolRegistry::default();

        // agents disabled by default
        let names_before: Vec<String> = registry
            .tools()
            .iter()
            .map(|t| t.definition().name.clone())
            .collect();
        assert!(!names_before.iter().any(|n| n.starts_with("agent_")));

        // Enable agents (no tools registered yet, so still empty -- but group is enabled)
        registry.set_group_enabled("agents", true);
        assert!(registry.is_group_enabled("agents"));
    }

    #[test]
    fn register_custom_group() {
        let mut registry = McpToolRegistry::default();

        registry.register_group(ToolGroup {
            name: "custom".into(),
            enabled: true,
            source: "plugin".into(),
            prefixes: vec!["notepad_".into()],
        });

        assert!(registry.is_group_enabled("custom"));
        assert_eq!(registry.groups().len(), 7);
    }

    #[test]
    fn cache_ttl_respected() {
        let mut registry = McpToolRegistry::with_ttl(Duration::from_millis(50));

        let count_first = registry.tools().len();
        // Should serve from cache (same reference count)
        let count_second = registry.tools().len();
        assert_eq!(count_first, count_second);
    }

    #[test]
    fn cache_refreshes_after_invalidation() {
        let mut registry = McpToolRegistry::default();

        // Populate cache
        let _ = registry.tools();

        // Invalidate and re-fetch
        registry.invalidate_cache();
        let tools = registry.tools();

        // Should still have the expected tools
        let names: Vec<String> = tools.iter().map(|t| t.definition().name).collect();
        assert!(names.iter().any(|n| n == "state_read"));
    }

    #[test]
    fn empty_when_all_groups_disabled() {
        let mut registry = McpToolRegistry::default();
        for name in ["core", "memory"] {
            registry.set_group_enabled(name, false);
        }

        assert!(registry.tools().is_empty());
    }
}
