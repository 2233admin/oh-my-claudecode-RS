//! Project Memory MCP tools.
//!
//! Provides tools for reading and writing project memory.
//! Project memory is stored as JSON in `.omc/project-memory.json`.

use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::tools::{McpTool, SchemaProperty, ToolDefinition, ToolResult, ToolSchema};

use chrono::Utc;

const SECTIONS: &[&str] = &[
    "all",
    "techStack",
    "build",
    "conventions",
    "structure",
    "notes",
    "directives",
];

/// Path to the project memory file.
fn memory_path(cwd: &str) -> PathBuf {
    PathBuf::from(cwd).join(".omc").join("project-memory.json")
}

/// Ensure .omc directory exists.
fn ensure_omc_dir(cwd: &str) -> std::io::Result<()> {
    fs::create_dir_all(PathBuf::from(cwd).join(".omc"))
}

/// Atomic write: write to .tmp then rename.
fn atomic_write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(value).unwrap_or_default();
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)
}

/// Load project memory from disk.
fn load_memory(path: &Path) -> Option<Value> {
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn section_enum() -> Vec<String> {
    SECTIONS.iter().map(std::string::ToString::to_string).collect()
}

// ============================================================================
// project_memory_read
// ============================================================================

pub struct ProjectMemoryReadTool;

impl McpTool for ProjectMemoryReadTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "section".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Section to read (default: all)".into()),
                r#enum: Some(section_enum()),
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

        ToolDefinition {
            name: "project_memory_read".into(),
            description: "Read the project memory. Can read the full memory or a specific section."
                .into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec![],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let section = str_arg(&args, "section").unwrap_or("all");
        let cwd = str_arg(&args, "workingDirectory")
            .unwrap_or(".")
            .to_string();
        let path = memory_path(&cwd);

        match load_memory(&path) {
            None => ToolResult::ok(format!(
                "Project memory does not exist.\nExpected path: {}\n\nRun a session to auto-detect project environment, or use project_memory_write to create manually.",
                path.display()
            )),
            Some(memory) => {
                if section == "all" {
                    let pretty = serde_json::to_string_pretty(&memory).unwrap_or_default();
                    return ToolResult::ok(format!(
                        "## Project Memory\n\nPath: {}\n\n```json\n{pretty}\n```",
                        path.display()
                    ));
                }

                let section_key = match section {
                    "techStack" => "techStack",
                    "build" => "build",
                    "conventions" => "conventions",
                    "structure" => "structure",
                    "notes" => "customNotes",
                    "directives" => "userDirectives",
                    _ => return ToolResult::error(format!("Unknown section: {section}")),
                };

                let data = memory.get(section_key).unwrap_or(&Value::Null);
                let pretty = serde_json::to_string_pretty(data).unwrap_or_default();
                ToolResult::ok(format!(
                    "## Project Memory: {section}\n\n```json\n{pretty}\n```"
                ))
            }
        }
    }
}

// ============================================================================
// project_memory_write
// ============================================================================

pub struct ProjectMemoryWriteTool;

impl McpTool for ProjectMemoryWriteTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "memory".into(),
            SchemaProperty {
                prop_type: "object".into(),
                description: Some("The memory object to write".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "merge".into(),
            SchemaProperty {
                prop_type: "boolean".into(),
                description: Some(
                    "If true, merge with existing memory (default: false = replace)".into(),
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

        ToolDefinition {
            name: "project_memory_write".into(),
            description:
                "Write/update project memory. Can replace entirely or merge with existing memory."
                    .into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["memory".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let memory_obj = match args.get("memory") {
            Some(m) if m.is_object() => m.clone(),
            _ => {
                return ToolResult::error(
                    "Missing or invalid required parameter: memory (must be an object)",
                );
            }
        };
        let merge = args.get("merge").and_then(serde_json::Value::as_bool).unwrap_or(false);
        let cwd = str_arg(&args, "workingDirectory")
            .unwrap_or(".")
            .to_string();
        let path = memory_path(&cwd);

        if let Err(e) = ensure_omc_dir(&cwd) {
            return ToolResult::error(format!("Error creating .omc directory: {e}"));
        }

        let mut final_memory = if merge {
            match load_memory(&path) {
                Some(existing) => merge_objects(existing, memory_obj),
                None => memory_obj,
            }
        } else {
            memory_obj
        };

        // Ensure required fields
        if let Some(obj) = final_memory.as_object_mut() {
            if !obj.contains_key("version") {
                obj.insert("version".into(), Value::String("1.0.0".into()));
            }
            if !obj.contains_key("lastScanned") {
                obj.insert(
                    "lastScanned".into(),
                    Value::Number(serde_json::Number::from(Utc::now().timestamp_millis())),
                );
            }
            if !obj.contains_key("projectRoot") {
                obj.insert("projectRoot".into(), Value::String(cwd.clone()));
            }
        }

        match atomic_write_json(&path, &final_memory) {
            Ok(()) => {
                let action = if merge { "merged" } else { "wrote" };
                ToolResult::ok(format!(
                    "Successfully {action} project memory.\nPath: {}",
                    path.display()
                ))
            }
            Err(e) => ToolResult::error(format!("Error writing project memory: {e}")),
        }
    }
}

/// Merge two JSON objects, with `overlay` taking precedence.
fn merge_objects(base: Value, overlay: Value) -> Value {
    let mut base = base;
    if let (Some(base_obj), Some(overlay_obj)) = (base.as_object_mut(), overlay.as_object()) {
        for (key, value) in overlay_obj {
            base_obj.insert(key.clone(), value.clone());
        }
    }
    base
}

// ============================================================================
// project_memory_add_note
// ============================================================================

pub struct ProjectMemoryAddNoteTool;

impl McpTool for ProjectMemoryAddNoteTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "category".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some(
                    "Note category (e.g., \"build\", \"test\", \"deploy\", \"env\", \"architecture\")".into(),
                ),
                r#enum: None,
                max_length: Some(50),
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "content".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Note content".into()),
                r#enum: None,
                max_length: Some(1000),
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

        ToolDefinition {
            name: "project_memory_add_note".into(),
            description: "Add a custom note to project memory. Notes are categorized and persisted across sessions.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["category".into(), "content".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let category = match str_arg(&args, "category") {
            Some(c) => c.to_string(),
            None => return ToolResult::error("Missing required parameter: category"),
        };
        let content = match str_arg(&args, "content") {
            Some(c) => c.to_string(),
            None => return ToolResult::error("Missing required parameter: content"),
        };
        let cwd = str_arg(&args, "workingDirectory")
            .unwrap_or(".")
            .to_string();
        let path = memory_path(&cwd);

        let mut memory = match load_memory(&path) {
            Some(m) => m,
            None => {
                return ToolResult::ok(
                    "Project memory does not exist. Run a session first to auto-detect project environment.",
                );
            }
        };

        // Add note to customNotes array
        let notes = memory.as_object_mut().and_then(|m| {
            m.entry("customNotes")
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
        });

        if let Some(notes_arr) = notes {
            let now = Utc::now().to_rfc3339();
            notes_arr.push(serde_json::json!({
                "timestamp": now,
                "category": category,
                "content": content,
            }));
        }

        match atomic_write_json(&path, &memory) {
            Ok(()) => ToolResult::ok(format!(
                "Successfully added note to project memory.\n\n- **Category:** {category}\n- **Content:** {content}"
            )),
            Err(e) => ToolResult::error(format!("Error adding note: {e}")),
        }
    }
}

// ============================================================================
// project_memory_add_directive
// ============================================================================

pub struct ProjectMemoryAddDirectiveTool;

impl McpTool for ProjectMemoryAddDirectiveTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "directive".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some(
                    "The directive (e.g., \"Always use TypeScript strict mode\")".into(),
                ),
                r#enum: None,
                max_length: Some(500),
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "context".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Additional context for the directive".into()),
                r#enum: None,
                max_length: Some(500),
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "priority".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Priority level (default: normal)".into()),
                r#enum: Some(vec!["high".into(), "normal".into()]),
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

        ToolDefinition {
            name: "project_memory_add_directive".into(),
            description: "Add a user directive to project memory. Directives are instructions that persist across sessions and survive compaction.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["directive".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let directive = match str_arg(&args, "directive") {
            Some(d) => d.to_string(),
            None => return ToolResult::error("Missing required parameter: directive"),
        };
        let context = str_arg(&args, "context").unwrap_or("").to_string();
        let priority = str_arg(&args, "priority").unwrap_or("normal").to_string();
        let cwd = str_arg(&args, "workingDirectory")
            .unwrap_or(".")
            .to_string();
        let path = memory_path(&cwd);

        let mut memory = match load_memory(&path) {
            Some(m) => m,
            None => {
                return ToolResult::ok(
                    "Project memory does not exist. Run a session first to auto-detect project environment.",
                );
            }
        };

        // Add directive to userDirectives array
        let directives = memory.as_object_mut().and_then(|m| {
            m.entry("userDirectives")
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
        });

        if let Some(directives_arr) = directives {
            let now = Utc::now().to_rfc3339();
            directives_arr.push(serde_json::json!({
                "timestamp": now,
                "directive": directive,
                "context": context,
                "source": "explicit",
                "priority": priority,
            }));
        }

        match atomic_write_json(&path, &memory) {
            Ok(()) => ToolResult::ok(format!(
                "Successfully added directive to project memory.\n\n- **Directive:** {directive}\n- **Priority:** {priority}\n- **Context:** {}",
                if context.is_empty() {
                    "(none)"
                } else {
                    &context
                }
            )),
            Err(e) => ToolResult::error(format!("Error adding directive: {e}")),
        }
    }
}

/// Collect all memory tools.
pub fn memory_tools() -> Vec<Box<dyn McpTool>> {
    vec![
        Box::new(ProjectMemoryReadTool),
        Box::new(ProjectMemoryWriteTool),
        Box::new(ProjectMemoryAddNoteTool),
        Box::new(ProjectMemoryAddDirectiveTool),
    ]
}
