//! Claude Code settings.json generation.

use serde_json::json;

use crate::types::{ConfigGenOptions, GeneratedConfig, GeneratedFile};
use std::path::PathBuf;

/// Generate `.claude/settings.json` content.
pub fn generate_claude_config(opts: &ConfigGenOptions) -> Result<GeneratedConfig, String> {
    let mut settings = serde_json::Map::new();

    // Environment
    if opts.enable_teams || !opts.env.is_empty() {
        let mut env = serde_json::Map::new();
        if opts.enable_teams {
            env.insert(
                "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS".into(),
                json!("1"),
            );
        }
        for (k, v) in &opts.env {
            env.insert(k.clone(), json!(v));
        }
        settings.insert("env".into(), json!(env));
    }

    // Hooks
    if !opts.hooks.is_empty() {
        let mut hooks_map = serde_json::Map::new();
        for hook in &opts.hooks {
            if let Some(event_str) = hook.event.to_host_event(crate::adapter::HostKind::Claude) {
                let entry = json!({
                    "matcher": hook.matcher.as_deref().unwrap_or("*"),
                    "hooks": [{
                        "type": "command",
                        "command": hook.command,
                    }]
                });
                hooks_map
                    .entry(event_str.to_string())
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .unwrap()
                    .push(entry);
            }
        }
        if !hooks_map.is_empty() {
            settings.insert("hooks".into(), json!(hooks_map));
        }
    }

    // MCP servers
    if !opts.mcp_servers.is_empty() {
        let mcp = crate::mcp_reg::claude_mcp_json(&opts.mcp_servers);
        settings.insert("mcpServers".into(), mcp);
    }

    // Custom instructions
    if let Some(ref instructions) = opts.custom_instructions {
        settings.insert("customInstructions".into(), json!(instructions));
    }

    let content =
        serde_json::to_string_pretty(&serde_json::Value::Object(settings)).map_err(|e| e.to_string())?;

    Ok(GeneratedConfig {
        files: vec![GeneratedFile {
            relative_path: PathBuf::from(".claude").join("settings.json"),
            content,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HookGenEntry;
    use crate::unified_hooks::UnifiedHookEvent;

    #[test]
    fn empty_config() {
        let opts = ConfigGenOptions::default();
        let cfg = generate_claude_config(&opts).unwrap();
        assert_eq!(cfg.files.len(), 1);
        assert!(cfg.files[0].content.contains("{}"));
    }

    #[test]
    fn teams_enabled() {
        let opts = ConfigGenOptions {
            enable_teams: true,
            ..Default::default()
        };
        let cfg = generate_claude_config(&opts).unwrap();
        let content = &cfg.files[0].content;
        assert!(content.contains("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS"));
    }

    #[test]
    fn hooks_registered() {
        let opts = ConfigGenOptions {
            hooks: vec![HookGenEntry {
                event: UnifiedHookEvent::PreToolUse,
                command: "omc hook pre-tool-use".into(),
                timeout_secs: 30,
                matcher: None,
            }],
            ..Default::default()
        };
        let cfg = generate_claude_config(&opts).unwrap();
        let content = &cfg.files[0].content;
        assert!(content.contains("PreToolUse"));
        assert!(content.contains("omc hook pre-tool-use"));
    }

    #[test]
    fn mcp_servers_registered() {
        let opts = ConfigGenOptions {
            mcp_servers: vec![crate::types::McpServerDef {
                name: "omc-state".into(),
                command: "omc-mcp".into(),
                args: vec!["--server".into()],
                env: None,
            }],
            ..Default::default()
        };
        let cfg = generate_claude_config(&opts).unwrap();
        let content = &cfg.files[0].content;
        assert!(content.contains("mcpServers"));
        assert!(content.contains("omc-state"));
    }

    #[test]
    fn relative_path_is_correct() {
        let opts = ConfigGenOptions::default();
        let cfg = generate_claude_config(&opts).unwrap();
        assert_eq!(
            cfg.files[0].relative_path,
            PathBuf::from(".claude/settings.json")
        );
    }
}
