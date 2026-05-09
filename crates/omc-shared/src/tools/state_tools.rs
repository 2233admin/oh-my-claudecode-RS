//! State Management Tools
//!
//! Provides tools for reading, writing, and managing mode state files.
//! All paths are validated to stay within the `.omc` boundary.
//!
//! State path: `.omc/state/sessions/<session_id>/state.json`
//! Modes: autopilot, autoresearch, team, ralph, ultrawork, ultraqa,
//!        deep-interview, self-improve, ralplan, omc-teams, skill-active

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::ToolResult;
use crate::config::OmcPaths;

/// All supported execution modes for state tools.
pub const EXECUTION_MODES: &[&str] = &[
    "autopilot",
    "autoresearch",
    "team",
    "ralph",
    "ultrawork",
    "ultraqa",
    "deep-interview",
    "self-improve",
];

/// Extended modes including state-only modes.
pub const ALL_STATE_MODES: &[&str] = &[
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

/// Metadata attached to every written state file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMeta {
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub updated_at: String,
    pub updated_by: String,
}

/// State payload that wraps user state with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatePayload {
    #[serde(flatten)]
    pub fields: HashMap<String, Value>,
    #[serde(rename = "_meta")]
    pub meta: StateMeta,
}

/// Result of a state_list_active operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveMode {
    pub mode: String,
    pub sessions: Vec<String>,
}

/// Result of a state_get_status operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeStatus {
    pub mode: String,
    pub active: bool,
    pub state_path: String,
    pub exists: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_preview: Option<Value>,
}

fn resolve_session_state_dir(paths: &OmcPaths, session_id: &str) -> PathBuf {
    paths.sessions.join(session_id)
}

fn resolve_mode_state_path(paths: &OmcPaths, mode: &str, session_id: Option<&str>) -> PathBuf {
    if let Some(sid) = session_id {
        resolve_session_state_dir(paths, sid).join(format!("{mode}-state.json"))
    } else {
        paths.state.join(format!("{mode}-state.json"))
    }
}

fn validate_mode(mode: &str) -> Result<(), String> {
    if !ALL_STATE_MODES.contains(&mode) {
        return Err(format!(
            "Invalid mode: '{mode}'. Valid modes: {}",
            ALL_STATE_MODES.join(", ")
        ));
    }
    Ok(())
}

fn ensure_dir(path: &Path) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Read the current state for a specific mode.
///
/// If `session_id` is provided, reads session-scoped state.
/// Otherwise reads from the legacy shared path.
pub fn state_read(
    mode: &str,
    working_directory: Option<&str>,
    session_id: Option<&str>,
) -> ToolResult {
    let mode = mode.trim();

    if let Err(e) = validate_mode(mode) {
        return ToolResult::error(e);
    }

    let paths = resolve_paths(working_directory);

    if let Some(sid) = session_id {
        let state_path = resolve_mode_state_path(&paths, mode, Some(sid));

        if !state_path.exists() {
            return ToolResult::text(format!(
                "No state found for mode: {mode} in session: {sid}\nExpected path: {}",
                state_path.display()
            ));
        }

        match fs::read_to_string(&state_path) {
            Ok(content) => match serde_json::from_str::<Value>(&content) {
                Ok(state) => ToolResult::text(format!(
                    "## State for {mode} (session: {sid})\n\nPath: {}\n\n```json\n{}\n```",
                    state_path.display(),
                    serde_json::to_string_pretty(&state).unwrap_or_default()
                )),
                Err(e) => ToolResult::error(format!("Error parsing state: {e}")),
            },
            Err(e) => ToolResult::error(format!("Error reading state: {e}")),
        }
    } else {
        let state_path = resolve_mode_state_path(&paths, mode, None);

        if !state_path.exists() {
            // Scan sessions
            let session_states = scan_session_states(&paths, mode);
            if session_states.is_empty() {
                return ToolResult::text(format!(
                    "No state found for mode: {mode}\nExpected legacy path: {}\nNo active sessions found.",
                    state_path.display()
                ));
            }

            let mut output = format!("## State for {mode}\n\n");
            for (sid, state_path, state) in &session_states {
                output.push_str(&format!(
                    "### Session: {sid}\nPath: {}\n\n```json\n{}\n```\n\n",
                    state_path.display(),
                    serde_json::to_string_pretty(state).unwrap_or_default()
                ));
            }
            return ToolResult::text(output);
        }

        match fs::read_to_string(&state_path) {
            Ok(content) => match serde_json::from_str::<Value>(&content) {
                Ok(state) => {
                    let mut output = format!(
                        "## State for {mode}\n\n### Legacy Path (shared)\nPath: {}\n\n```json\n{}\n```\n\n",
                        state_path.display(),
                        serde_json::to_string_pretty(&state).unwrap_or_default()
                    );

                    let session_states = scan_session_states(&paths, mode);
                    if !session_states.is_empty() {
                        output.push_str(&format!(
                            "### Active Sessions ({})\n\n",
                            session_states.len()
                        ));
                        for (sid, sp, st) in &session_states {
                            output.push_str(&format!(
                                "**Session: {sid}**\nPath: {}\n\n```json\n{}\n```\n\n",
                                sp.display(),
                                serde_json::to_string_pretty(st).unwrap_or_default()
                            ));
                        }
                    }

                    ToolResult::text(output)
                }
                Err(e) => ToolResult::error(format!("Error parsing state: {e}")),
            },
            Err(e) => ToolResult::error(format!("Error reading state: {e}")),
        }
    }
}

/// Write/update state for a specific mode.
///
/// Creates the state file and directories if they do not exist.
/// Merges explicit fields with any additional custom fields from `extra`.
#[allow(clippy::too_many_arguments)]
pub fn state_write(
    mode: &str,
    active: Option<bool>,
    iteration: Option<u64>,
    max_iterations: Option<u64>,
    current_phase: Option<&str>,
    task_description: Option<&str>,
    plan_path: Option<&str>,
    started_at: Option<&str>,
    completed_at: Option<&str>,
    error: Option<&str>,
    extra: Option<&HashMap<String, Value>>,
    working_directory: Option<&str>,
    session_id: Option<&str>,
) -> ToolResult {
    let mode = mode.trim();

    if let Err(e) = validate_mode(mode) {
        return ToolResult::error(e);
    }

    let paths = resolve_paths(working_directory);

    // Build state fields
    let mut fields = HashMap::new();

    if let Some(v) = active {
        fields.insert("active".to_string(), Value::Bool(v));
    }
    if let Some(v) = iteration {
        fields.insert("iteration".to_string(), Value::Number(v.into()));
    }
    if let Some(v) = max_iterations {
        fields.insert("max_iterations".to_string(), Value::Number(v.into()));
    }
    if let Some(v) = current_phase {
        fields.insert("current_phase".to_string(), Value::String(v.to_string()));
    }
    if let Some(v) = task_description {
        fields.insert("task_description".to_string(), Value::String(v.to_string()));
    }
    if let Some(v) = plan_path {
        fields.insert("plan_path".to_string(), Value::String(v.to_string()));
    }
    if let Some(v) = started_at {
        fields.insert("started_at".to_string(), Value::String(v.to_string()));
    }
    if let Some(v) = completed_at {
        fields.insert("completed_at".to_string(), Value::String(v.to_string()));
    }
    if let Some(v) = error {
        fields.insert("error".to_string(), Value::String(v.to_string()));
    }

    // Merge extra fields (explicit params take precedence)
    if let Some(extra_fields) = extra {
        for (key, value) in extra_fields {
            fields.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }

    let payload = StatePayload {
        fields,
        meta: StateMeta {
            mode: mode.to_string(),
            session_id: session_id.map(|s| s.to_string()),
            updated_at: chrono::Utc::now().to_rfc3339(),
            updated_by: "state_write_tool".into(),
        },
    };

    let state_path = if let Some(sid) = session_id {
        let dir = resolve_session_state_dir(&paths, sid);
        if let Err(e) = fs::create_dir_all(&dir) {
            return ToolResult::error(format!("Error creating session dir: {e}"));
        }
        dir.join(format!("{mode}-state.json"))
    } else {
        if let Err(e) = ensure_dir(&paths.state.join("x")) {
            return ToolResult::error(format!("Error creating state dir: {e}"));
        }
        resolve_mode_state_path(&paths, mode, None)
    };

    match atomic_write_json(&state_path, &payload) {
        Ok(()) => {
            let session_info = session_id
                .map(|s| format!(" (session: {s})"))
                .unwrap_or_else(|| " (legacy path)".to_string());
            let warning = if session_id.is_none() {
                "\n\nWARNING: No session_id provided. State written to legacy shared path. Pass session_id for session-scoped isolation."
            } else {
                ""
            };
            let pretty = serde_json::to_string_pretty(&payload).unwrap_or_default();
            ToolResult::text(format!(
                "Successfully wrote state for {mode}{session_info}\nPath: {}\n\n```json\n{pretty}\n```{warning}",
                state_path.display()
            ))
        }
        Err(e) => ToolResult::error(format!("Error writing state: {e}")),
    }
}

/// Clear/delete state for a specific mode.
///
/// Removes the state file and any associated marker files.
pub fn state_clear(
    mode: &str,
    working_directory: Option<&str>,
    session_id: Option<&str>,
) -> ToolResult {
    let mode = mode.trim();

    if let Err(e) = validate_mode(mode) {
        return ToolResult::error(e);
    }

    let paths = resolve_paths(working_directory);
    let mut cleared = 0usize;
    let mut errors = Vec::new();

    if let Some(sid) = session_id {
        // Clear session-scoped state
        let state_path = resolve_mode_state_path(&paths, mode, Some(sid));
        if state_path.exists() {
            match fs::remove_file(&state_path) {
                Ok(()) => cleared += 1,
                Err(_) => errors.push(format!("session: {sid}")),
            }
        }
    } else {
        // Clear legacy path
        let legacy_path = resolve_mode_state_path(&paths, mode, None);
        if legacy_path.exists() {
            match fs::remove_file(&legacy_path) {
                Ok(()) => cleared += 1,
                Err(_) => errors.push("legacy path".into()),
            }
        }

        // Clear all session-scoped state
        if paths.sessions.exists()
            && let Ok(entries) = fs::read_dir(&paths.sessions)
        {
            for entry in entries.flatten() {
                let sid = entry.file_name().to_string_lossy().to_string();
                let state_path = resolve_mode_state_path(&paths, mode, Some(&sid));
                if state_path.exists() {
                    match fs::remove_file(&state_path) {
                        Ok(()) => cleared += 1,
                        Err(_) => errors.push(format!("session: {sid}")),
                    }
                }
            }
        }
    }

    if cleared == 0 && errors.is_empty() {
        return ToolResult::text(format!("No state found to clear for mode: {mode}"));
    }

    let mut message = format!("Cleared state for mode: {mode}\n- Locations cleared: {cleared}");
    if !errors.is_empty() {
        message.push_str(&format!("\n- Errors: {}", errors.join(", ")));
    }
    if session_id.is_none() {
        message.push_str(
            "\nWARNING: No session_id provided. Cleared legacy plus all session-scoped state.",
        );
    }

    ToolResult::text(message)
}

/// List all currently active modes.
///
/// Returns which modes have active state files.
pub fn state_list_active(working_directory: Option<&str>, session_id: Option<&str>) -> ToolResult {
    let paths = resolve_paths(working_directory);
    let mut mode_sessions: HashMap<String, Vec<String>> = HashMap::new();

    if let Some(sid) = session_id {
        // Check only the specified session
        for &mode in ALL_STATE_MODES {
            let state_path = resolve_mode_state_path(&paths, mode, Some(sid));
            if is_mode_active_file(&state_path) {
                mode_sessions
                    .entry(mode.to_string())
                    .or_default()
                    .push(sid.to_string());
            }
        }
    } else {
        // Check legacy path
        for &mode in ALL_STATE_MODES {
            let legacy_path = resolve_mode_state_path(&paths, mode, None);
            if is_mode_active_file(&legacy_path) {
                mode_sessions
                    .entry(mode.to_string())
                    .or_default()
                    .push("legacy".into());
            }
        }

        // Check all sessions
        if paths.sessions.exists()
            && let Ok(entries) = fs::read_dir(&paths.sessions)
        {
            for entry in entries.flatten() {
                let sid = entry.file_name().to_string_lossy().to_string();
                for &mode in ALL_STATE_MODES {
                    let state_path = resolve_mode_state_path(&paths, mode, Some(&sid));
                    if is_mode_active_file(&state_path) {
                        mode_sessions
                            .entry(mode.to_string())
                            .or_default()
                            .push(sid.clone());
                    }
                }
            }
        }
    }

    if mode_sessions.is_empty() {
        let scope = session_id
            .map(|s| format!(" (session: {s})"))
            .unwrap_or_default();
        return ToolResult::text(format!(
            "## Active Modes{scope}\n\nNo modes are currently active."
        ));
    }

    let count = mode_sessions.len();
    let scope = session_id
        .map(|s| format!(" (session: {s}, {count})"))
        .unwrap_or_else(|| format!(" ({count})"));
    let mut output = format!("## Active Modes{scope}\n\n");

    for (mode, sessions) in &mode_sessions {
        output.push_str(&format!("- **{mode}** ({})\n", sessions.join(", ")));
    }

    ToolResult::text(output)
}

/// Get detailed status for a specific mode or all modes.
pub fn state_get_status(
    mode: Option<&str>,
    working_directory: Option<&str>,
    session_id: Option<&str>,
) -> ToolResult {
    let paths = resolve_paths(working_directory);

    if let Some(mode) = mode {
        let mode = mode.trim();

        if let Err(e) = validate_mode(mode) {
            return ToolResult::error(e);
        }

        let mut lines = vec![format!("## Status: {mode}\n")];

        if let Some(sid) = session_id {
            let state_path = resolve_mode_state_path(&paths, mode, Some(sid));
            let active = is_mode_active_file(&state_path);

            lines.push(format!("### Session: {sid}"));
            lines.push(format!(
                "- **Active:** {}",
                if active { "Yes" } else { "No" }
            ));
            lines.push(format!("- **State Path:** {}", state_path.display()));
            lines.push(format!(
                "- **Exists:** {}",
                if state_path.exists() { "Yes" } else { "No" }
            ));

            if state_path.exists() {
                let preview = read_state_preview(&state_path, 500);
                lines.push(format!("\n### State Preview\n```json\n{preview}\n```"));
            }
        } else {
            let legacy_path = resolve_mode_state_path(&paths, mode, None);
            let legacy_active = is_mode_active_file(&legacy_path);

            lines.push("### Legacy Path".into());
            lines.push(format!(
                "- **Active:** {}",
                if legacy_active { "Yes" } else { "No" }
            ));
            lines.push(format!("- **State Path:** {}", legacy_path.display()));
            lines.push(format!(
                "- **Exists:** {}",
                if legacy_path.exists() { "Yes" } else { "No" }
            ));

            let active_sessions = find_active_sessions(&paths, mode);
            if active_sessions.is_empty() {
                lines.push("\n### Active Sessions\nNo active sessions for this mode.".into());
            } else {
                lines.push(format!("\n### Active Sessions ({})", active_sessions.len()));
                for sid in &active_sessions {
                    lines.push(format!("- {sid}"));
                }
            }
        }

        return ToolResult::text(lines.join("\n"));
    }

    // All modes status
    let scope = session_id
        .map(|s| format!(" (session: {s})"))
        .unwrap_or_default();
    let mut lines = vec![format!("## All Mode Statuses{scope}\n")];

    for &mode in ALL_STATE_MODES {
        let state_path = if let Some(sid) = session_id {
            resolve_mode_state_path(&paths, mode, Some(sid))
        } else {
            resolve_mode_state_path(&paths, mode, None)
        };
        let active = is_mode_active_file(&state_path);
        let icon = if active { "[ACTIVE]" } else { "[INACTIVE]" };

        lines.push(format!(
            "{icon} **{mode}**: {}",
            if active { "Active" } else { "Inactive" }
        ));
        lines.push(format!("   Path: `{}`", state_path.display()));

        if session_id.is_none() {
            let active_sessions = find_active_sessions(&paths, mode);
            if !active_sessions.is_empty() {
                lines.push(format!(
                    "   Active sessions: {}",
                    active_sessions.join(", ")
                ));
            }
        }
    }

    ToolResult::text(lines.join("\n"))
}

/// Update a single key in an existing state file without replacing the entire state.
///
/// This is a convenience tool that reads the current state, sets the specified key,
/// and writes it back atomically.
pub fn state_update_key(
    mode: &str,
    key: &str,
    value: &Value,
    working_directory: Option<&str>,
    session_id: Option<&str>,
) -> ToolResult {
    let mode = mode.trim();

    if let Err(e) = validate_mode(mode) {
        return ToolResult::error(e);
    }

    let paths = resolve_paths(working_directory);
    let state_path = if let Some(sid) = session_id {
        resolve_mode_state_path(&paths, mode, Some(sid))
    } else {
        resolve_mode_state_path(&paths, mode, None)
    };

    // Read existing state or start with empty
    let mut payload = if state_path.exists() {
        match fs::read_to_string(&state_path) {
            Ok(content) => {
                serde_json::from_str::<StatePayload>(&content).unwrap_or_else(|_| StatePayload {
                    fields: HashMap::new(),
                    meta: StateMeta {
                        mode: mode.to_string(),
                        session_id: session_id.map(|s| s.to_string()),
                        updated_at: chrono::Utc::now().to_rfc3339(),
                        updated_by: "state_update_key_tool".into(),
                    },
                })
            }
            Err(_) => StatePayload {
                fields: HashMap::new(),
                meta: StateMeta {
                    mode: mode.to_string(),
                    session_id: session_id.map(|s| s.to_string()),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                    updated_by: "state_update_key_tool".into(),
                },
            },
        }
    } else {
        StatePayload {
            fields: HashMap::new(),
            meta: StateMeta {
                mode: mode.to_string(),
                session_id: session_id.map(|s| s.to_string()),
                updated_at: chrono::Utc::now().to_rfc3339(),
                updated_by: "state_update_key_tool".into(),
            },
        }
    };

    payload.fields.insert(key.to_string(), value.clone());
    payload.meta.updated_at = chrono::Utc::now().to_rfc3339();
    payload.meta.updated_by = "state_update_key_tool".into();

    if let Some(sid) = session_id {
        let dir = resolve_session_state_dir(&paths, sid);
        let _ = fs::create_dir_all(&dir);
    }

    match atomic_write_json(&state_path, &payload) {
        Ok(()) => ToolResult::text(format!(
            "Updated key '{key}' in state for mode: {mode}\nPath: {}",
            state_path.display()
        )),
        Err(e) => ToolResult::error(format!("Error updating state: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_paths(working_directory: Option<&str>) -> OmcPaths {
    match working_directory {
        Some(dir) => OmcPaths::new_with_root(PathBuf::from(dir).join(".omc")),
        None => OmcPaths::new(),
    }
}

fn is_mode_active_file(path: &Path) -> bool {
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

fn scan_session_states(paths: &OmcPaths, mode: &str) -> Vec<(String, PathBuf, Value)> {
    let mut results = Vec::new();

    if !paths.sessions.exists() {
        return results;
    }

    if let Ok(entries) = fs::read_dir(&paths.sessions) {
        for entry in entries.flatten() {
            let sid = entry.file_name().to_string_lossy().to_string();
            let state_path = resolve_mode_state_path(paths, mode, Some(&sid));
            if state_path.exists()
                && let Ok(content) = fs::read_to_string(&state_path)
                && let Ok(state) = serde_json::from_str::<Value>(&content)
            {
                results.push((sid, state_path, state));
            }
        }
    }

    results
}

fn find_active_sessions(paths: &OmcPaths, mode: &str) -> Vec<String> {
    let mut active = Vec::new();

    if !paths.sessions.exists() {
        return active;
    }

    if let Ok(entries) = fs::read_dir(&paths.sessions) {
        for entry in entries.flatten() {
            let sid = entry.file_name().to_string_lossy().to_string();
            let state_path = resolve_mode_state_path(paths, mode, Some(&sid));
            if is_mode_active_file(&state_path) {
                active.push(sid);
            }
        }
    }

    active
}

fn read_state_preview(path: &Path, max_chars: usize) -> String {
    match fs::read_to_string(path) {
        Ok(content) => {
            let preview: String = content.chars().take(max_chars).collect();
            if preview.len() >= max_chars {
                format!("{preview}\n...(truncated)")
            } else {
                preview
            }
        }
        Err(_) => "Error reading state file".into(),
    }
}

fn atomic_write_json<T: Serialize>(path: &Path, data: &T) -> Result<(), std::io::Error> {
    ensure_dir(path)?;
    let tmp = path.with_extension("json.tmp");
    let content = serde_json::to_vec_pretty(data)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    fs::write(&tmp, content)?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, OmcPaths) {
        let tmp = TempDir::new().unwrap();
        let paths = OmcPaths::new_with_root(tmp.path().join(".omc"));
        (tmp, paths)
    }

    #[test]
    fn test_validate_mode() {
        assert!(validate_mode("ralph").is_ok());
        assert!(validate_mode("team").is_ok());
        assert!(validate_mode("ralplan").is_ok());
        assert!(validate_mode("invalid-mode").is_err());
    }

    #[test]
    fn test_state_write_and_read() {
        let (tmp, _) = setup();
        let wd = tmp.path().to_string_lossy();

        let result = state_write(
            "ralph",
            Some(true),
            Some(1),
            None,
            Some("planning"),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(&wd),
            Some("test-session"),
        );
        assert!(result.is_error.is_none());

        let result = state_read("ralph", Some(&wd), Some("test-session"));
        assert!(result.is_error.is_none());
        assert!(result.content[0].text.contains("ralph"));
        assert!(result.content[0].text.contains("test-session"));
    }

    #[test]
    fn test_state_list_active_empty() {
        let (tmp, _) = setup();
        let wd = tmp.path().to_string_lossy();

        let result = state_list_active(Some(&wd), None);
        assert!(result.is_error.is_none());
        assert!(
            result.content[0]
                .text
                .contains("No modes are currently active")
        );
    }

    #[test]
    fn test_state_clear() {
        let (tmp, _) = setup();
        let wd = tmp.path().to_string_lossy();

        // Write then clear
        state_write(
            "ralph",
            Some(true),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(&wd),
            Some("test-session"),
        );

        let result = state_clear("ralph", Some(&wd), Some("test-session"));
        assert!(result.is_error.is_none());
        assert!(result.content[0].text.contains("Cleared state"));
    }

    #[test]
    fn test_state_update_key() {
        let (tmp, _) = setup();
        let wd = tmp.path().to_string_lossy();

        // Write initial state
        state_write(
            "ralph",
            Some(true),
            Some(1),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(&wd),
            Some("test-session"),
        );

        // Update a single key
        let result = state_update_key(
            "ralph",
            "iteration",
            &Value::Number(42.into()),
            Some(&wd),
            Some("test-session"),
        );
        assert!(result.is_error.is_none());

        // Verify
        let read_result = state_read("ralph", Some(&wd), Some("test-session"));
        assert!(read_result.content[0].text.contains("42"));
    }

    #[test]
    fn test_state_get_status() {
        let (tmp, _) = setup();
        let wd = tmp.path().to_string_lossy();

        let result = state_get_status(Some("ralph"), Some(&wd), None);
        assert!(result.is_error.is_none());
        assert!(result.content[0].text.contains("Status: ralph"));
    }
}
