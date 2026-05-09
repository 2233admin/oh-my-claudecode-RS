//! Project Memory Tools
//!
//! Provides tools for reading and writing project memory.
//! Project memory is a JSON file that persists information about the project
//! environment, tech stack, conventions, and user directives across sessions.
//!
//! Path: `.omc/project-memory.json`

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::ToolResult;
use crate::config::OmcPaths;

/// Project memory data structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMemory {
    pub version: String,
    pub project_root: String,
    pub last_scanned: u64,
    #[serde(default)]
    pub tech_stack: Value,
    #[serde(default)]
    pub build: Value,
    #[serde(default)]
    pub conventions: Value,
    #[serde(default)]
    pub structure: Value,
    #[serde(default)]
    pub custom_notes: Vec<CustomNote>,
    #[serde(default)]
    pub user_directives: Vec<UserDirective>,
}

impl Default for ProjectMemory {
    fn default() -> Self {
        Self {
            version: "1.0.0".into(),
            project_root: String::default(),
            last_scanned: now_ms(),
            tech_stack: Value::Null,
            build: Value::Null,
            conventions: Value::Null,
            structure: Value::Null,
            custom_notes: Vec::default(),
            user_directives: Vec::default(),
        }
    }
}

/// A custom note added to project memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomNote {
    pub timestamp: u64,
    pub category: String,
    pub content: String,
}

/// A user directive that persists across sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDirective {
    pub timestamp: u64,
    pub directive: String,
    #[serde(default)]
    pub context: String,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default = "default_priority")]
    pub priority: String,
}

fn default_source() -> String {
    "explicit".into()
}

fn default_priority() -> String {
    "normal".into()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn memory_path(paths: &OmcPaths) -> PathBuf {
    paths.home.join("project-memory.json")
}

fn ensure_omc_dir(paths: &OmcPaths) -> Result<(), std::io::Error> {
    fs::create_dir_all(&paths.home)
}

/// Load project memory from disk.
fn load_memory(path: &Path) -> Option<ProjectMemory> {
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save project memory to disk atomically.
fn save_memory(path: &Path, memory: &ProjectMemory) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let content = serde_json::to_vec_pretty(memory)
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

/// Merge two ProjectMemory objects, preferring `update` fields when present.
fn merge_memory(base: ProjectMemory, update: &HashMap<String, Value>) -> ProjectMemory {
    let mut result = base;

    for (key, value) in update {
        match key.as_str() {
            "techStack" | "tech_stack" => {
                if !value.is_null() {
                    result.tech_stack = merge_json(&result.tech_stack, value);
                }
            }
            "build" => {
                if !value.is_null() {
                    result.build = merge_json(&result.build, value);
                }
            }
            "conventions" => {
                if !value.is_null() {
                    result.conventions = merge_json(&result.conventions, value);
                }
            }
            "structure" if !value.is_null() => {
                result.structure = merge_json(&result.structure, value);
            }
            _ => {}
        }
    }

    result.last_scanned = now_ms();
    result
}

/// Merge two JSON values. Objects are recursively merged; arrays and scalars are replaced.
fn merge_json(base: &Value, update: &Value) -> Value {
    match (base, update) {
        (Value::Object(base_map), Value::Object(update_map)) => {
            let mut merged = base_map.clone();
            for (k, v) in update_map {
                let existing = merged.get(k).cloned().unwrap_or(Value::Null);
                merged.insert(k.clone(), merge_json(&existing, v));
            }
            Value::Object(merged)
        }
        _ => update.clone(),
    }
}

/// Read the project memory.
///
/// Can read the full memory or a specific section.
pub fn project_memory_read(section: Option<&str>, working_directory: Option<&str>) -> ToolResult {
    let paths = resolve_paths(working_directory);
    let path = memory_path(&paths);

    let memory = match load_memory(&path) {
        Some(m) => m,
        None => {
            return ToolResult::text(format!(
                "Project memory does not exist.\nExpected path: {}\n\nRun a session to auto-detect project environment, or use project_memory_write to create manually.",
                path.display()
            ));
        }
    };

    match section {
        None | Some("all") => ToolResult::text(format!(
            "## Project Memory\n\nPath: {}\n\n```json\n{}\n```",
            path.display(),
            serde_json::to_string_pretty(&memory).unwrap_or_default()
        )),
        Some(sec) => {
            let data = match sec {
                "techStack" | "tech_stack" => &memory.tech_stack,
                "build" => &memory.build,
                "conventions" => &memory.conventions,
                "structure" => &memory.structure,
                "notes" | "customNotes" | "custom_notes" => {
                    return ToolResult::text(format!(
                        "## Project Memory: notes\n\n```json\n{}\n```",
                        serde_json::to_string_pretty(&memory.custom_notes).unwrap_or_default()
                    ));
                }
                "directives" | "userDirectives" | "user_directives" => {
                    return ToolResult::text(format!(
                        "## Project Memory: directives\n\n```json\n{}\n```",
                        serde_json::to_string_pretty(&memory.user_directives).unwrap_or_default()
                    ));
                }
                _ => {
                    return ToolResult::error(format!(
                        "Unknown section: '{sec}'. Valid sections: all, techStack, build, conventions, structure, notes, directives"
                    ));
                }
            };
            ToolResult::text(format!(
                "## Project Memory: {sec}\n\n```json\n{}\n```",
                serde_json::to_string_pretty(data).unwrap_or_default()
            ))
        }
    }
}

/// Write/update project memory.
///
/// Can replace entirely or merge with existing memory.
pub fn project_memory_write(
    memory: &HashMap<String, Value>,
    merge: bool,
    working_directory: Option<&str>,
) -> ToolResult {
    let paths = resolve_paths(working_directory);
    if let Err(e) = ensure_omc_dir(&paths) {
        return ToolResult::error(format!("Error creating directory: {e}"));
    }

    let path = memory_path(&paths);
    let final_memory = if merge {
        let existing = load_memory(&path).unwrap_or_default();
        merge_memory(existing, memory)
    } else {
        let mut m: ProjectMemory = serde_json::from_value(Value::Object(
            memory.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        ))
        .unwrap_or_default();
        if m.version.is_empty() {
            m.version = "1.0.0".into();
        }
        if m.last_scanned == 0 {
            m.last_scanned = now_ms();
        }
        if m.project_root.is_empty() {
            m.project_root = working_directory.unwrap_or(".").to_string();
        }
        m
    };

    match save_memory(&path, &final_memory) {
        Ok(()) => ToolResult::text(format!(
            "Successfully {} project memory.\nPath: {}",
            if merge { "merged" } else { "wrote" },
            path.display()
        )),
        Err(e) => ToolResult::error(format!("Error writing project memory: {e}")),
    }
}

/// Add a custom note to project memory.
///
/// Notes are categorized and persisted across sessions.
pub fn project_memory_add_note(
    category: &str,
    content: &str,
    working_directory: Option<&str>,
) -> ToolResult {
    let paths = resolve_paths(working_directory);
    let path = memory_path(&paths);

    let mut memory = match load_memory(&path) {
        Some(m) => m,
        None => {
            return ToolResult::text(
                "Project memory does not exist. Run a session first to auto-detect project environment.",
            );
        }
    };

    memory.custom_notes.push(CustomNote {
        timestamp: now_ms(),
        category: category.to_string(),
        content: content.to_string(),
    });
    memory.last_scanned = now_ms();

    match save_memory(&path, &memory) {
        Ok(()) => ToolResult::text(format!(
            "Successfully added note to project memory.\n\n- **Category:** {category}\n- **Content:** {content}"
        )),
        Err(e) => ToolResult::error(format!("Error adding note: {e}")),
    }
}

/// Add a user directive to project memory.
///
/// Directives are instructions that persist across sessions and survive compaction.
pub fn project_memory_add_directive(
    directive: &str,
    context: Option<&str>,
    priority: Option<&str>,
    working_directory: Option<&str>,
) -> ToolResult {
    let paths = resolve_paths(working_directory);
    let path = memory_path(&paths);

    let mut memory = match load_memory(&path) {
        Some(m) => m,
        None => {
            return ToolResult::text(
                "Project memory does not exist. Run a session first to auto-detect project environment.",
            );
        }
    };

    let prio = priority.unwrap_or("normal");

    memory.user_directives.push(UserDirective {
        timestamp: now_ms(),
        directive: directive.to_string(),
        context: context.unwrap_or("").to_string(),
        source: "explicit".into(),
        priority: prio.to_string(),
    });
    memory.last_scanned = now_ms();

    match save_memory(&path, &memory) {
        Ok(()) => ToolResult::text(format!(
            "Successfully added directive to project memory.\n\n- **Directive:** {directive}\n- **Priority:** {prio}\n- **Context:** {}",
            context.unwrap_or("(none)")
        )),
        Err(e) => ToolResult::error(format!("Error adding directive: {e}")),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, String) {
        let tmp = TempDir::new().unwrap();
        let wd = tmp.path().to_string_lossy().to_string();
        (tmp, wd)
    }

    #[test]
    fn test_memory_read_empty() {
        let (_tmp, wd) = setup();
        let result = project_memory_read(None, Some(&wd));
        assert!(result.content[0].text.contains("does not exist"));
    }

    #[test]
    fn test_memory_write_and_read() {
        let (_tmp, wd) = setup();

        let mut memory = HashMap::new();
        memory.insert("version".to_string(), Value::String("1.0.0".into()));
        memory.insert("project_root".to_string(), Value::String("/test".into()));

        project_memory_write(&memory, false, Some(&wd));
        let result = project_memory_read(None, Some(&wd));
        assert!(result.content[0].text.contains("Project Memory"));
        assert!(result.content[0].text.contains("1.0.0"));
    }

    #[test]
    fn test_memory_merge() {
        let (_tmp, wd) = setup();

        // Write initial
        let mut memory = HashMap::new();
        memory.insert("version".to_string(), Value::String("1.0.0".into()));
        project_memory_write(&memory, false, Some(&wd));

        // Merge update
        let mut update = HashMap::new();
        update.insert(
            "build".to_string(),
            serde_json::json!({"command": "cargo build"}),
        );
        project_memory_write(&update, true, Some(&wd));

        // Verify merge preserved version
        let result = project_memory_read(Some("build"), Some(&wd));
        assert!(result.content[0].text.contains("cargo build"));
    }

    #[test]
    fn test_add_note() {
        let (_tmp, wd) = setup();

        // Create memory first
        let mut memory = HashMap::new();
        memory.insert("version".to_string(), Value::String("1.0.0".into()));
        project_memory_write(&memory, false, Some(&wd));

        // Add note
        let result = project_memory_add_note("build", "Use cargo build", Some(&wd));
        assert!(result.content[0].text.contains("Successfully added note"));

        // Read and verify
        let result = project_memory_read(Some("notes"), Some(&wd));
        assert!(result.content[0].text.contains("Use cargo build"));
    }

    #[test]
    fn test_add_directive() {
        let (_tmp, wd) = setup();

        // Create memory first
        let mut memory = HashMap::new();
        memory.insert("version".to_string(), Value::String("1.0.0".into()));
        project_memory_write(&memory, false, Some(&wd));

        // Add directive
        let result = project_memory_add_directive(
            "Always use TypeScript strict mode",
            Some("Enforced by tsconfig.json"),
            Some("high"),
            Some(&wd),
        );
        assert!(
            result.content[0]
                .text
                .contains("Successfully added directive")
        );

        // Read and verify
        let result = project_memory_read(Some("directives"), Some(&wd));
        assert!(result.content[0].text.contains("strict mode"));
    }

    #[test]
    fn test_memory_section_read() {
        let (_tmp, wd) = setup();
        let paths = resolve_paths(Some(&wd));
        ensure_omc_dir(&paths).unwrap();
        let path = memory_path(&paths);
        let json = serde_json::json!({
            "version": "1.0.0",
            "project_root": ".",
            "last_scanned": 1000,
            "tech_stack": {"languages": ["rust", "typescript"]},
            "build": null,
            "conventions": null,
            "structure": null,
            "custom_notes": [],
            "user_directives": []
        });
        std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

        let result = project_memory_read(Some("tech_stack"), Some(&wd));
        assert!(
            !result.is_error.unwrap_or(false),
            "Read failed: {:?}",
            result.content
        );
        assert!(
            result.content[0].text.contains("rust"),
            "Expected 'rust' in: {}",
            result.content[0].text
        );
    }

    #[test]
    fn test_unknown_section() {
        let (_tmp, wd) = setup();

        let mut memory = HashMap::new();
        memory.insert("version".to_string(), Value::String("1.0.0".into()));
        project_memory_write(&memory, false, Some(&wd));

        let result = project_memory_read(Some("nonexistent"), Some(&wd));
        assert!(result.is_error == Some(true));
    }
}
