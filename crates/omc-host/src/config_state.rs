//! Abstract config state for host-specific serialization.

use serde_json::Value;

/// Abstract config state that adapters mutate during setup.
///
/// Implementations handle the serialization format (JSON for Claude,
/// TOML for Codex).
pub trait HostConfigState: Send + Sync {
    /// Set a top-level key to a JSON value.
    fn set_value(&mut self, key: &str, value: Value);

    /// Get a top-level value.
    fn get_value(&self, key: &str) -> Option<&Value>;

    /// Serialize to the host's native format (JSON for Claude, TOML for Codex).
    fn serialize(&self) -> Result<String, String>;

    /// All keys currently set.
    fn keys(&self) -> Vec<&str>;
}

/// JSON-backed config state (for Claude Code settings.json).
#[derive(Debug, Clone, Default)]
pub struct JsonConfigState {
    inner: serde_json::Map<String, Value>,
}

impl JsonConfigState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_map(map: serde_json::Map<String, Value>) -> Self {
        Self { inner: map }
    }
}

impl HostConfigState for JsonConfigState {
    fn set_value(&mut self, key: &str, value: Value) {
        self.inner.insert(key.to_string(), value);
    }

    fn get_value(&self, key: &str) -> Option<&Value> {
        self.inner.get(key)
    }

    fn serialize(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&self.inner).map_err(|e| e.to_string())
    }

    fn keys(&self) -> Vec<&str> {
        self.inner.keys().map(|s| s.as_str()).collect()
    }
}

/// TOML-backed config state (for Codex config.toml).
#[derive(Debug, Clone, Default)]
pub struct TomlConfigState {
    inner: serde_json::Map<String, Value>,
}

impl TomlConfigState {
    pub fn new() -> Self {
        Self::default()
    }
}

impl HostConfigState for TomlConfigState {
    fn set_value(&mut self, key: &str, value: Value) {
        self.inner.insert(key.to_string(), value);
    }

    fn get_value(&self, key: &str) -> Option<&Value> {
        self.inner.get(key)
    }

    fn serialize(&self) -> Result<String, String> {
        // Convert JSON map to toml::Value via serde
        let json_val = Value::Object(self.inner.clone());
        let toml_val: toml::Value =
            serde_json::from_value(json_val).map_err(|e| format!("json->toml: {e}"))?;
        toml::to_string_pretty(&toml_val).map_err(|e| e.to_string())
    }

    fn keys(&self) -> Vec<&str> {
        self.inner.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_config_state_basic_ops() {
        let mut state = JsonConfigState::new();
        state.set_value("foo", serde_json::json!("bar"));
        assert_eq!(state.get_value("foo"), Some(&serde_json::json!("bar")));
        assert_eq!(state.get_value("missing"), None);
        assert!(state.keys().contains(&"foo"));
    }

    #[test]
    fn json_config_state_serialize() {
        let mut state = JsonConfigState::new();
        state.set_value("key", serde_json::json!(42));
        let output = state.serialize().unwrap();
        assert!(output.contains("\"key\""));
        assert!(output.contains("42"));
    }

    #[test]
    fn toml_config_state_serialize() {
        let mut state = TomlConfigState::new();
        state.set_value("server", serde_json::json!("localhost"));
        let output = state.serialize().unwrap();
        assert!(output.contains("server"));
        assert!(output.contains("localhost"));
    }

    #[test]
    fn toml_config_state_nested() {
        let mut state = TomlConfigState::new();
        state.set_value(
            "mcp_servers",
            serde_json::json!({ "omc-state": { "command": "omc-mcp", "args": ["--server"] } }),
        );
        let output = state.serialize().unwrap();
        assert!(output.contains("mcp_servers"));
        assert!(output.contains("omc-state"));
    }
}
