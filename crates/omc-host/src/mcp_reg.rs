//! MCP server registration generation and persistence for both hosts.

use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::adapter::HostKind;
use crate::types::McpServerDef;

/// The single OMC MCP server consumed by Claude Code, Codex, and other hosts.
pub fn omc_server_definition() -> McpServerDef {
    McpServerDef {
        name: "omc-rs".into(),
        command: "omc".into(),
        args: vec!["mcp".into()],
        env: None,
    }
}

/// Resolve Hermes' global configuration directory without creating it.
pub fn resolve_hermes_home(override_home: Option<&Path>) -> std::path::PathBuf {
    if let Some(home) = override_home {
        return home.to_path_buf();
    }
    if let Ok(home) = std::env::var("HERMES_HOME")
        && !home.trim().is_empty()
    {
        return std::path::PathBuf::from(home);
    }
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join("AppData").join("Local")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("hermes")
    }

    #[cfg(not(windows))]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".hermes")
    }
}

/// Persist the unified OMC MCP server in Hermes' `config.yaml`.
///
/// Hermes is an MCP consumer, not another OMC host engine, so this path only
/// edits its `mcp_servers` map. Matching entries are left untouched and
/// conflicting entries fail closed.
pub fn ensure_hermes_mcp_server(hermes_home: &Path, server: &McpServerDef) -> Result<bool, String> {
    ensure_hermes_mcp_server_with_force(hermes_home, server, false)
}

pub fn ensure_hermes_mcp_server_with_force(
    hermes_home: &Path,
    server: &McpServerDef,
    force: bool,
) -> Result<bool, String> {
    let path = hermes_home.join("config.yaml");
    let mut config = if path.exists() {
        let content =
            std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        serde_yaml::from_str::<serde_yaml::Value>(&content)
            .map_err(|e| format!("parse {}: {e}", path.display()))?
    } else {
        serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
    };

    let root = config
        .as_mapping_mut()
        .ok_or_else(|| format!("{} must contain a YAML mapping", path.display()))?;
    let servers_key = serde_yaml::Value::String("mcp_servers".into());
    let servers = root
        .entry(servers_key)
        .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()))
        .as_mapping_mut()
        .ok_or_else(|| format!("{} mcp_servers must be a YAML mapping", path.display()))?;
    let name_key = serde_yaml::Value::String(server.name.clone());
    let desired = hermes_server_value(server)?;

    if let Some(existing) = servers.get(&name_key) {
        if hermes_server_matches(existing, &desired) {
            return Ok(false);
        }
        if !force {
            return Err(format!(
                "{} already contains a conflicting MCP server named {}",
                path.display(),
                server.name
            ));
        }
    }

    servers.insert(name_key, desired);
    let content =
        serde_yaml::to_string(&config).map_err(|e| format!("serialize {}: {e}", path.display()))?;
    write_config_atomically(&path, content.as_bytes())?;
    Ok(true)
}

fn hermes_server_value(server: &McpServerDef) -> Result<serde_yaml::Value, String> {
    let mut entry = serde_yaml::Mapping::new();
    entry.insert(
        serde_yaml::Value::String("command".into()),
        serde_yaml::Value::String(server.command.clone()),
    );
    entry.insert(
        serde_yaml::Value::String("args".into()),
        serde_yaml::to_value(&server.args).map_err(|e| e.to_string())?,
    );
    if let Some(env) = &server.env
        && !env.is_empty()
    {
        entry.insert(
            serde_yaml::Value::String("env".into()),
            serde_yaml::to_value(env).map_err(|e| e.to_string())?,
        );
    }
    Ok(serde_yaml::Value::Mapping(entry))
}

fn hermes_server_matches(existing: &serde_yaml::Value, desired: &serde_yaml::Value) -> bool {
    let Some(existing) = existing.as_mapping() else {
        return false;
    };
    let Some(desired) = desired.as_mapping() else {
        return false;
    };
    desired
        .iter()
        .all(|(key, expected)| existing.get(key) == Some(expected))
}

/// Persist the unified OMC MCP server in a host project configuration.
///
/// Existing entries under the same name are left untouched only when they
/// already match; a conflicting entry is rejected instead of overwritten.
pub fn ensure_mcp_server(
    root: &Path,
    host: HostKind,
    server: &McpServerDef,
) -> Result<bool, String> {
    ensure_mcp_server_with_force(root, host, server, false)
}

pub fn ensure_mcp_server_with_force(
    root: &Path,
    host: HostKind,
    server: &McpServerDef,
    force: bool,
) -> Result<bool, String> {
    match host {
        HostKind::Claude => ensure_claude_mcp_server(root, server, force),
        HostKind::Codex => ensure_codex_mcp_server(root, server, force),
    }
}

fn ensure_claude_mcp_server(
    root: &Path,
    server: &McpServerDef,
    force: bool,
) -> Result<bool, String> {
    let path = root.join(".mcp.json");
    let mut config = if path.exists() {
        let content =
            std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        serde_json::from_str::<Value>(&content)
            .map_err(|e| format!("parse {}: {e}", path.display()))?
    } else {
        Value::Object(serde_json::Map::new())
    };
    let object = config
        .as_object_mut()
        .ok_or_else(|| format!("{} must contain a JSON object", path.display()))?;
    let servers = object
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let servers = servers
        .as_object_mut()
        .ok_or_else(|| format!("{} mcpServers must be a JSON object", path.display()))?;
    let desired = claude_server_value(server);

    if let Some(existing) = servers.get(&server.name) {
        if existing == &desired {
            return Ok(false);
        }
        if !force {
            return Err(format!(
                "{} already contains a conflicting MCP server named {}",
                path.display(),
                server.name
            ));
        }
    }

    servers.insert(server.name.clone(), desired);
    let content = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    write_config_atomically(&path, format!("{content}\n").as_bytes())?;
    Ok(true)
}

fn ensure_codex_mcp_server(
    root: &Path,
    server: &McpServerDef,
    force: bool,
) -> Result<bool, String> {
    let path = root.join(".codex/config.toml");
    let mut config = if path.exists() {
        let content =
            std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        content
            .parse::<toml::Value>()
            .map_err(|e| format!("parse {}: {e}", path.display()))?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };
    let root_table = config
        .as_table_mut()
        .ok_or_else(|| format!("{} must contain a TOML table", path.display()))?;
    let servers = root_table
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let servers = servers
        .as_table_mut()
        .ok_or_else(|| format!("{} mcp_servers must be a TOML table", path.display()))?;
    let desired = codex_server_value(server);

    if let Some(existing) = servers.get(&server.name) {
        if existing == &desired {
            return Ok(false);
        }
        if !force {
            return Err(format!(
                "{} already contains a conflicting MCP server named {}",
                path.display(),
                server.name
            ));
        }
    }

    servers.insert(server.name.clone(), desired);
    let content = toml::to_string_pretty(&config).map_err(|e| e.to_string())?;
    write_config_atomically(&path, content.as_bytes())?;
    Ok(true)
}

fn write_config_atomically(path: &Path, content: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;

    if path.exists() {
        let backup = path.with_extension(format!(
            "{}.bak",
            path.extension()
                .and_then(|value| value.to_str())
                .unwrap_or("config")
        ));
        std::fs::copy(path, &backup)
            .map_err(|e| format!("backup {} to {}: {e}", path.display(), backup.display()))?;
    }

    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| format!("create temporary config in {}: {e}", parent.display()))?;
    temporary
        .write_all(content)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|e| format!("write temporary config for {}: {e}", path.display()))?;
    temporary
        .persist(path)
        .map_err(|e| format!("replace {}: {}", path.display(), e.error))?;
    Ok(())
}

fn claude_server_value(server: &McpServerDef) -> Value {
    let mut entry = serde_json::Map::new();
    entry.insert("command".into(), json!(server.command));
    if !server.args.is_empty() {
        entry.insert("args".into(), json!(server.args));
    }
    if let Some(env) = &server.env
        && !env.is_empty()
    {
        entry.insert("env".into(), json!(env));
    }
    Value::Object(entry)
}

fn codex_server_value(server: &McpServerDef) -> toml::Value {
    let mut entry = toml::map::Map::new();
    entry.insert(
        "command".into(),
        toml::Value::String(server.command.clone()),
    );
    if !server.args.is_empty() {
        entry.insert(
            "args".into(),
            toml::Value::Array(
                server
                    .args
                    .iter()
                    .map(|arg| toml::Value::String(arg.clone()))
                    .collect(),
            ),
        );
    }
    if let Some(env) = &server.env
        && !env.is_empty()
    {
        entry.insert(
            "env".into(),
            toml::Value::Table(
                env.iter()
                    .map(|(key, value)| (key.clone(), toml::Value::String(value.clone())))
                    .collect(),
            ),
        );
    }
    toml::Value::Table(entry)
}

/// Generate Claude Code mcpServers JSON block.
pub fn claude_mcp_json(servers: &[McpServerDef]) -> Value {
    let mut map = serde_json::Map::new();
    for s in servers {
        map.insert(s.name.clone(), claude_server_value(s));
    }
    Value::Object(map)
}

/// Generate Codex config.toml mcp_servers section as TOML string.
pub fn codex_mcp_toml(servers: &[McpServerDef]) -> Result<String, String> {
    let mut toml_map = toml::map::Map::new();
    for s in servers {
        toml_map.insert(s.name.clone(), codex_server_value(s));
    }
    let mut root = toml::map::Map::new();
    root.insert("mcp_servers".into(), toml::Value::Table(toml_map));
    toml::to_string_pretty(&toml::Value::Table(root)).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "mcp_reg_tests.rs"]
mod tests;
