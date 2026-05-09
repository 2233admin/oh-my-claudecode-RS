//! State management MCP tools.
//!
//! Provides tools for reading, writing, and managing mode state files.
//! All paths are resolved relative to the OMC state directory.

use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::tools::{McpTool, SchemaProperty, ToolDefinition, ToolResult, ToolSchema};

const STATE_TOOL_MODES: &[&str] = &[
    "autopilot",
    "autoresearch",
    "team",
    "ralph",
    "ultrawork",
    "ultraqa",
    "deep-interview",
    "self-improve",
    "ralplan",
    "omc-teams",
    "skill-active",
];

/// Resolve the state path for a given mode and optional session.
fn state_path(mode: &str, session_id: Option<&str>, cwd: &str) -> PathBuf {
    let omc = omc_root(cwd);
    match session_id {
        Some(sid) => omc
            .join("state/sessions")
            .join(sid)
            .join(format!("{mode}.json")),
        None => omc.join("state").join(format!("{mode}-state.json")),
    }
}

/// Resolve the OMC root directory from a working directory.
fn omc_root(cwd: &str) -> PathBuf {
    PathBuf::from(cwd).join(".omc")
}

/// List all session IDs from the sessions directory.
fn list_session_ids(cwd: &str) -> Vec<String> {
    let sessions_dir = omc_root(cwd).join("state/sessions");
    if !sessions_dir.exists() {
        return Vec::new();
    }
    fs::read_dir(&sessions_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect()
}

/// Ensure all parent directories exist for a state path.
fn ensure_parent_dirs(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Atomic write: write to .tmp then rename.
fn atomic_write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    ensure_parent_dirs(path)?;
    let tmp = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(value).unwrap_or_default();
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)
}

/// Extract string arg from JSON value.
fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

/// Build the mode enum list for the schema.
fn mode_enum() -> Vec<String> {
    STATE_TOOL_MODES.iter().map(|s| s.to_string()).collect()
}

// ============================================================================
// state_read
// ============================================================================

pub struct StateReadTool;

impl McpTool for StateReadTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "mode".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("The mode to read state for".into()),
                r#enum: Some(mode_enum()),
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "workingDirectory".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Working directory (defaults to cwd)".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "session_id".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Session ID for session-scoped state".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );

        ToolDefinition {
            name: "state_read".into(),
            description: "Read the current state for a specific mode (ralph, ultrawork, autopilot, etc.). Returns the JSON state data or indicates if no state exists.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["mode".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let mode = match str_arg(&args, "mode") {
            Some(m) => m,
            None => return ToolResult::error("Missing required parameter: mode"),
        };
        let cwd = str_arg(&args, "workingDirectory")
            .unwrap_or(".")
            .to_string();
        let session_id = str_arg(&args, "session_id").map(|s| s.to_string());

        let path = state_path(mode, session_id.as_deref(), &cwd);

        if !path.exists() {
            // Check for session-scoped alternatives if no session_id was given
            if session_id.is_none() {
                let session_ids = list_session_ids(&cwd);
                let mut active_sessions = Vec::new();
                for sid in &session_ids {
                    let sp = state_path(mode, Some(sid), &cwd);
                    if sp.exists() {
                        active_sessions.push((sid.clone(), sp));
                    }
                }
                if active_sessions.is_empty() {
                    return ToolResult::ok(format!(
                        "No state found for mode: {mode}\nExpected path: {}",
                        path.display()
                    ));
                }
                let mut output = format!("## State for {mode}\n\n");
                for (sid, sp) in &active_sessions {
                    match fs::read_to_string(sp) {
                        Ok(content) => {
                            output.push_str(&format!(
                                "### Session: {sid}\nPath: {}\n\n```json\n{content}\n```\n\n",
                                sp.display()
                            ));
                        }
                        Err(e) => {
                            output.push_str(&format!(
                                "**Session: {sid}**\nPath: {}\n*Error: {e}*\n\n",
                                sp.display()
                            ));
                        }
                    }
                }
                return ToolResult::ok(output);
            }
            return ToolResult::ok(format!(
                "No state found for mode: {mode}\nExpected path: {}",
                path.display()
            ));
        }

        match fs::read_to_string(&path) {
            Ok(content) => {
                let sid_label = session_id
                    .as_deref()
                    .map(|s| format!(" (session: {s})"))
                    .unwrap_or_default();
                ToolResult::ok(format!(
                    "## State for {mode}{sid_label}\n\nPath: {}\n\n```json\n{content}\n```",
                    path.display()
                ))
            }
            Err(e) => ToolResult::error(format!("Error reading state for {mode}: {e}")),
        }
    }
}

// ============================================================================
// state_write
// ============================================================================

pub struct StateWriteTool;

impl McpTool for StateWriteTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "mode".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("The mode to write state for".into()),
                r#enum: Some(mode_enum()),
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "active".into(),
            SchemaProperty {
                prop_type: "boolean".into(),
                description: Some("Whether the mode is currently active".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "iteration".into(),
            SchemaProperty {
                prop_type: "number".into(),
                description: Some("Current iteration number".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "max_iterations".into(),
            SchemaProperty {
                prop_type: "number".into(),
                description: Some("Maximum iterations allowed".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "current_phase".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Current execution phase".into()),
                r#enum: None,
                max_length: Some(200),
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "task_description".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Description of the task being executed".into()),
                r#enum: None,
                max_length: Some(2000),
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "plan_path".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Path to the plan file".into()),
                r#enum: None,
                max_length: Some(500),
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "started_at".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("ISO timestamp when the mode started".into()),
                r#enum: None,
                max_length: Some(100),
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "completed_at".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("ISO timestamp when the mode completed".into()),
                r#enum: None,
                max_length: Some(100),
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "error".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Error message if the mode failed".into()),
                r#enum: None,
                max_length: Some(2000),
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "state".into(),
            SchemaProperty {
                prop_type: "object".into(),
                description: Some(
                    "Additional custom state fields (merged with explicit parameters)".into(),
                ),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "workingDirectory".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Working directory (defaults to cwd)".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "session_id".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Session ID for session-scoped state".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );

        ToolDefinition {
            name: "state_write".into(),
            description: "Write/update state for a specific mode. Creates the state file and directories if they do not exist. Common fields (active, iteration, phase, etc.) can be set directly. Additional custom fields can be passed via the optional `state` parameter.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["mode".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let mode = match str_arg(&args, "mode") {
            Some(m) => m,
            None => return ToolResult::error("Missing required parameter: mode"),
        };
        let cwd = str_arg(&args, "workingDirectory")
            .unwrap_or(".")
            .to_string();
        let session_id = str_arg(&args, "session_id").map(|s| s.to_string());

        let path = state_path(mode, session_id.as_deref(), &cwd);

        // Build state from explicit params + custom state
        let mut built = serde_json::Map::new();

        for key in &[
            "active",
            "iteration",
            "max_iterations",
            "current_phase",
            "task_description",
            "plan_path",
            "started_at",
            "completed_at",
            "error",
        ] {
            if let Some(v) = args.get(*key)
                && !v.is_null()
            {
                built.insert((*key).to_string(), v.clone());
            }
        }

        // Merge custom state fields (explicit params take precedence)
        if let Some(custom) = args.get("state").and_then(|v| v.as_object()) {
            for (k, v) in custom {
                if !built.contains_key(k) {
                    built.insert(k.clone(), v.clone());
                }
            }
        }

        // Add metadata
        let now = chrono::Utc::now().to_rfc3339();
        built.insert(
            "_meta".into(),
            serde_json::json!({
                "mode": mode,
                "sessionId": session_id,
                "updatedAt": now,
                "updatedBy": "state_write_tool"
            }),
        );

        let value = Value::Object(built);

        match atomic_write_json(&path, &value) {
            Ok(()) => {
                let sid_info = session_id
                    .as_deref()
                    .map(|s| format!(" (session: {s})"))
                    .unwrap_or_else(|| " (legacy path)".into());
                let pretty = serde_json::to_string_pretty(&value).unwrap_or_default();
                ToolResult::ok(format!(
                    "Successfully wrote state for {mode}{sid_info}\nPath: {}\n\n```json\n{pretty}\n```",
                    path.display()
                ))
            }
            Err(e) => ToolResult::error(format!("Error writing state for {mode}: {e}")),
        }
    }
}

// ============================================================================
// state_clear
// ============================================================================

pub struct StateClearTool;

impl McpTool for StateClearTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "mode".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("The mode to clear state for".into()),
                r#enum: Some(mode_enum()),
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "workingDirectory".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Working directory (defaults to cwd)".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "session_id".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Session ID for session-scoped state".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );

        ToolDefinition {
            name: "state_clear".into(),
            description: "Clear/delete state for a specific mode. Removes the state file and any associated marker files.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["mode".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let mode = match str_arg(&args, "mode") {
            Some(m) => m,
            None => return ToolResult::error("Missing required parameter: mode"),
        };
        let cwd = str_arg(&args, "workingDirectory")
            .unwrap_or(".")
            .to_string();
        let session_id = str_arg(&args, "session_id").map(|s| s.to_string());

        let mut cleared_count = 0;
        let mut errors = Vec::new();

        if let Some(sid) = &session_id {
            // Clear session-specific state
            let path = state_path(mode, Some(sid), &cwd);
            if path.exists() {
                match fs::remove_file(&path) {
                    Ok(()) => cleared_count += 1,
                    Err(_) => errors.push(format!("session: {sid}")),
                }
            }
            // Also clear legacy path for this session
            let legacy = state_path(mode, None, &cwd);
            if legacy.exists() {
                match fs::remove_file(&legacy) {
                    Ok(()) => cleared_count += 1,
                    Err(_) => errors.push("legacy path".into()),
                }
            }
        } else {
            // Clear legacy + all sessions
            let legacy = state_path(mode, None, &cwd);
            if legacy.exists() {
                match fs::remove_file(&legacy) {
                    Ok(()) => cleared_count += 1,
                    Err(_) => errors.push("legacy path".into()),
                }
            }
            for sid in list_session_ids(&cwd) {
                let path = state_path(mode, Some(&sid), &cwd);
                if path.exists() {
                    match fs::remove_file(&path) {
                        Ok(()) => cleared_count += 1,
                        Err(_) => errors.push(format!("session: {sid}")),
                    }
                }
            }
        }

        if cleared_count == 0 && errors.is_empty() {
            return ToolResult::ok(format!("No state found to clear for mode: {mode}"));
        }

        let mut msg =
            format!("Cleared state for mode: {mode}\n- Locations cleared: {cleared_count}");
        if !errors.is_empty() {
            msg.push_str(&format!("\n- Errors: {}", errors.join(", ")));
        }
        if session_id.is_none() {
            msg.push_str(
                "\nWARNING: No session_id provided. Cleared legacy plus all session-scoped state.",
            );
        }
        ToolResult::ok(msg)
    }
}

// ============================================================================
// state_list_active
// ============================================================================

pub struct StateListActiveTool;

impl McpTool for StateListActiveTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "workingDirectory".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Working directory (defaults to cwd)".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "session_id".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Session ID for session-scoped state".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );

        ToolDefinition {
            name: "state_list_active".into(),
            description:
                "List all currently active modes. Returns which modes have active state files."
                    .into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec![],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let cwd = str_arg(&args, "workingDirectory")
            .unwrap_or(".")
            .to_string();
        let session_id = str_arg(&args, "session_id").map(|s| s.to_string());

        // Check each mode for active state
        let mut active_modes: Vec<String> = Vec::new();

        if let Some(sid) = &session_id {
            for mode in STATE_TOOL_MODES {
                let path = state_path(mode, Some(sid), &cwd);
                if is_mode_active_at(&path) {
                    active_modes.push(mode.to_string());
                }
            }
            if active_modes.is_empty() {
                return ToolResult::ok(format!(
                    "## Active Modes (session: {sid})\n\nNo modes are currently active in this session."
                ));
            }
            let list = active_modes
                .iter()
                .map(|m| format!("- **{m}**"))
                .collect::<Vec<_>>()
                .join("\n");
            return ToolResult::ok(format!(
                "## Active Modes (session: {sid}, {})\n\n{list}",
                active_modes.len()
            ));
        }

        // No session_id: check legacy + all sessions
        let mut mode_session_map: HashMap<String, Vec<String>> = HashMap::new();

        // Check legacy paths
        for mode in STATE_TOOL_MODES {
            let path = state_path(mode, None, &cwd);
            if is_mode_active_at(&path) {
                mode_session_map
                    .entry(mode.to_string())
                    .or_default()
                    .push("legacy".into());
            }
        }

        // Check all sessions
        for sid in list_session_ids(&cwd) {
            for mode in STATE_TOOL_MODES {
                let path = state_path(mode, Some(&sid), &cwd);
                if is_mode_active_at(&path) {
                    mode_session_map
                        .entry(mode.to_string())
                        .or_default()
                        .push(sid.clone());
                }
            }
        }

        if mode_session_map.is_empty() {
            return ToolResult::ok("## Active Modes\n\nNo modes are currently active.");
        }

        let mut lines = vec![format!("## Active Modes ({})\n", mode_session_map.len())];
        let mut sorted: Vec<_> = mode_session_map.into_iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        for (mode, sessions) in sorted {
            lines.push(format!("- **{mode}** ({})", sessions.join(", ")));
        }
        ToolResult::ok(lines.join("\n"))
    }
}

fn is_mode_active_at(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<Value>(&content)
            .ok()
            .and_then(|v| v.get("active").and_then(|a| a.as_bool()))
            .unwrap_or(false),
        Err(_) => false,
    }
}

// ============================================================================
// state_get_status
// ============================================================================

pub struct StateGetStatusTool;

impl McpTool for StateGetStatusTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "mode".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Specific mode to check (omit for all modes)".into()),
                r#enum: Some(mode_enum()),
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "workingDirectory".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Working directory (defaults to cwd)".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "session_id".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Session ID for session-scoped state".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );

        ToolDefinition {
            name: "state_get_status".into(),
            description:
                "Get detailed status for a specific mode or all modes. Shows active status, file paths, and state contents."
                    .into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec![],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let mode = str_arg(&args, "mode").map(|s| s.to_string());
        let cwd = str_arg(&args, "workingDirectory")
            .unwrap_or(".")
            .to_string();
        let session_id = str_arg(&args, "session_id").map(|s| s.to_string());

        if let Some(mode) = &mode {
            // Single mode status
            let path = state_path(mode, session_id.as_deref(), &cwd);
            let active = is_mode_active_at(&path);
            let exists = path.exists();

            let state_preview = if exists {
                match fs::read_to_string(&path) {
                    Ok(content) => {
                        if content.len() > 500 {
                            format!("{}...(truncated)", &content[..500])
                        } else {
                            content
                        }
                    }
                    Err(_) => "Error reading state file".into(),
                }
            } else {
                "No state file".into()
            };

            let sid_label = session_id
                .as_deref()
                .map(|s| format!(" (session: {s})"))
                .unwrap_or_default();

            return ToolResult::ok(format!(
                "## Status: {mode}{sid_label}\n\n- **Active:** {}\n- **State Path:** {}\n- **Exists:** {exists}\n\n### State Preview\n```json\n{state_preview}\n```",
                if active { "Yes" } else { "No" },
                path.display(),
            ));
        }

        // All modes status
        let mut lines = vec!["## All Mode Statuses\n".to_string()];

        for mode in STATE_TOOL_MODES {
            let path = state_path(mode, session_id.as_deref(), &cwd);
            let active = is_mode_active_at(&path);
            let icon = if active { "[ACTIVE]" } else { "[INACTIVE]" };
            lines.push(format!(
                "{icon} **{mode}**: {}\n   Path: `{}`",
                if active { "Active" } else { "Inactive" },
                path.display()
            ));
        }

        ToolResult::ok(lines.join("\n"))
    }
}

/// Collect all state tools.
pub fn state_tools() -> Vec<Box<dyn McpTool>> {
    vec![
        Box::new(StateReadTool),
        Box::new(StateWriteTool),
        Box::new(StateClearTool),
        Box::new(StateListActiveTool),
        Box::new(StateGetStatusTool),
    ]
}
