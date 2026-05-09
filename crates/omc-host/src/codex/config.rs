//! Codex CLI config.toml and hooks.json generation.

use crate::types::{ConfigGenOptions, GeneratedConfig, GeneratedFile};
use std::path::PathBuf;

/// Generate `.codex/config.toml` + `.codex/hooks.json`.
pub fn generate_codex_config(opts: &ConfigGenOptions) -> Result<GeneratedConfig, String> {
    let mut files = Vec::new();

    // ── config.toml ────────────────────────────────────────────────
    let mut toml_map = toml::map::Map::new();

    // MCP servers
    if !opts.mcp_servers.is_empty() {
        let mut mcp_section = toml::map::Map::new();
        for s in &opts.mcp_servers {
            let mut server_map = toml::map::Map::new();
            server_map.insert(
                "command".into(),
                toml::Value::String(s.command.clone()),
            );
            if !s.args.is_empty() {
                let args: Vec<toml::Value> = s
                    .args
                    .iter()
                    .map(|a| toml::Value::String(a.clone()))
                    .collect();
                server_map.insert("args".into(), toml::Value::Array(args));
            }
            if let Some(ref env) = s.env
                && !env.is_empty() {
                    let env_map: toml::map::Map<String, toml::Value> = env
                        .iter()
                        .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
                        .collect();
                    server_map.insert("env".into(), toml::Value::Table(env_map));
                }
            mcp_section.insert(s.name.clone(), toml::Value::Table(server_map));
        }
        toml_map.insert("mcp_servers".into(), toml::Value::Table(mcp_section));
    }

    // Environment
    if !opts.env.is_empty() {
        let env_map: toml::map::Map<String, toml::Value> = opts
            .env
            .iter()
            .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
            .collect();
        toml_map.insert("env".into(), toml::Value::Table(env_map));
    }

    // Custom instructions
    if let Some(ref instructions) = opts.custom_instructions {
        toml_map.insert(
            "developer_instructions".into(),
            toml::Value::String(instructions.clone()),
        );
    }

    if !toml_map.is_empty() {
        let toml_content =
            toml::to_string_pretty(&toml::Value::Table(toml_map)).map_err(|e| e.to_string())?;
        files.push(GeneratedFile {
            relative_path: PathBuf::from(".codex").join("config.toml"),
            content: toml_content,
        });
    }

    // ── hooks.json ─────────────────────────────────────────────────
    if !opts.hooks.is_empty() {
        let mut hooks_map = serde_json::Map::new();
        for hook in &opts.hooks {
            if let Some(event_str) = hook.event.to_host_event(crate::adapter::HostKind::Codex) {
                let entry = serde_json::json!({
                    "command": hook.command,
                    "timeout": hook.timeout_secs,
                });
                hooks_map
                    .entry(event_str.to_string())
                    .or_insert_with(|| serde_json::json!([]))
                    .as_array_mut()
                    .unwrap()
                    .push(entry);
            }
        }
        if !hooks_map.is_empty() {
            let hooks_json = serde_json::to_string_pretty(&serde_json::Value::Object(hooks_map))
                .map_err(|e| e.to_string())?;
            files.push(GeneratedFile {
                relative_path: PathBuf::from(".codex").join("hooks.json"),
                content: hooks_json,
            });
        }
    }

    Ok(GeneratedConfig { files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{HookGenEntry, McpServerDef};
    use crate::unified_hooks::UnifiedHookEvent;

    #[test]
    fn empty_config_produces_no_files() {
        let opts = ConfigGenOptions::default();
        let cfg = generate_codex_config(&opts).unwrap();
        assert!(cfg.files.is_empty());
    }

    #[test]
    fn mcp_servers_in_config_toml() {
        let opts = ConfigGenOptions {
            mcp_servers: vec![McpServerDef {
                name: "omc-state".into(),
                command: "omc-mcp".into(),
                args: vec!["--server".into()],
                env: None,
            }],
            ..Default::default()
        };
        let cfg = generate_codex_config(&opts).unwrap();
        let toml_file = cfg
            .files
            .iter()
            .find(|f| f.relative_path.to_str().unwrap().contains("config.toml"))
            .unwrap();
        assert!(toml_file.content.contains("[mcp_servers.omc-state]"));
    }

    #[test]
    fn hooks_generate_hooks_json() {
        let opts = ConfigGenOptions {
            hooks: vec![HookGenEntry {
                event: UnifiedHookEvent::PreToolUse,
                command: "omc hook pre-tool-use".into(),
                timeout_secs: 30,
                matcher: None,
            }],
            ..Default::default()
        };
        let cfg = generate_codex_config(&opts).unwrap();
        let hooks_file = cfg
            .files
            .iter()
            .find(|f| f.relative_path.to_str().unwrap().contains("hooks.json"))
            .unwrap();
        assert!(hooks_file.content.contains("pre_tool_use"));
    }

    #[test]
    fn combined_mcp_and_hooks() {
        let opts = ConfigGenOptions {
            mcp_servers: vec![McpServerDef {
                name: "test".into(),
                command: "test-cmd".into(),
                args: vec![],
                env: None,
            }],
            hooks: vec![HookGenEntry {
                event: UnifiedHookEvent::SessionStart,
                command: "init".into(),
                timeout_secs: 10,
                matcher: None,
            }],
            ..Default::default()
        };
        let cfg = generate_codex_config(&opts).unwrap();
        assert_eq!(cfg.files.len(), 2);
    }
}
