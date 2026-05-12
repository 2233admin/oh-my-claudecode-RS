//! MCP server registration generation for both hosts.

use serde_json::{Value, json};

use crate::types::McpServerDef;

/// Generate Claude Code mcpServers JSON block.
pub fn claude_mcp_json(servers: &[McpServerDef]) -> Value {
    let mut map = serde_json::Map::new();
    for s in servers {
        let mut entry = serde_json::Map::new();
        entry.insert("command".into(), json!(s.command));
        if !s.args.is_empty() {
            entry.insert("args".into(), json!(s.args));
        }
        if let Some(ref env) = s.env
            && !env.is_empty()
        {
            entry.insert("env".into(), json!(env));
        }
        map.insert(s.name.clone(), Value::Object(entry));
    }
    Value::Object(map)
}

/// Generate Codex config.toml mcp_servers section as TOML string.
pub fn codex_mcp_toml(servers: &[McpServerDef]) -> Result<String, String> {
    let mut toml_map = toml::map::Map::new();
    for s in servers {
        let mut server_map = toml::map::Map::new();
        server_map.insert("command".into(), toml::Value::String(s.command.clone()));
        if !s.args.is_empty() {
            let args: Vec<toml::Value> = s
                .args
                .iter()
                .map(|a| toml::Value::String(a.clone()))
                .collect();
            server_map.insert("args".into(), toml::Value::Array(args));
        }
        if let Some(ref env) = s.env
            && !env.is_empty()
        {
            let env_map: toml::map::Map<String, toml::Value> = env
                .iter()
                .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
                .collect();
            server_map.insert("env".into(), toml::Value::Table(env_map));
        }
        toml_map.insert(s.name.clone(), toml::Value::Table(server_map));
    }
    let mut root = toml::map::Map::new();
    root.insert("mcp_servers".into(), toml::Value::Table(toml_map));
    toml::to_string_pretty(&toml::Value::Table(root)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_servers() -> Vec<McpServerDef> {
        vec![
            McpServerDef {
                name: "omc-state".into(),
                command: "omc-mcp".into(),
                args: vec!["--server".into()],
                env: None,
            },
            McpServerDef {
                name: "omc-memory".into(),
                command: "omc-mcp".into(),
                args: vec!["--memory".into()],
                env: Some([("DEBUG".into(), "1".into())].into_iter().collect()),
            },
        ]
    }

    #[test]
    fn claude_mcp_json_format() {
        let json = claude_mcp_json(&sample_servers());
        assert_eq!(json["omc-state"]["command"], "omc-mcp");
        assert_eq!(json["omc-state"]["args"][0], "--server");
        assert_eq!(json["omc-memory"]["env"]["DEBUG"], "1");
    }

    #[test]
    fn codex_mcp_toml_format() {
        let toml_str = codex_mcp_toml(&sample_servers()).unwrap();
        assert!(toml_str.contains("[mcp_servers.omc-state]"));
        assert!(toml_str.contains("command = \"omc-mcp\""));
        assert!(toml_str.contains("[mcp_servers.omc-memory]"));
        assert!(toml_str.contains("DEBUG"));
    }

    #[test]
    fn empty_servers() {
        let json = claude_mcp_json(&[]);
        assert_eq!(json, serde_json::json!({}));
        let toml_str = codex_mcp_toml(&[]).unwrap();
        assert!(toml_str.contains("mcp_servers"));
    }
}
