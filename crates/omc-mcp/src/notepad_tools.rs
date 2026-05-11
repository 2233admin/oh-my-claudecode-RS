//! Notepad MCP tools.
//!
//! Provides tools for reading and writing notepad sections
//! (Priority Context, Working Memory, MANUAL).

use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::tools::{McpTool, SchemaProperty, ToolDefinition, ToolResult, ToolSchema};

use chrono::Utc;

const SECTION_NAMES: &[&str] = &["all", "priority", "working", "manual"];

/// Path to the notepad file.
fn notepad_path(cwd: &str) -> PathBuf {
    PathBuf::from(cwd).join(".omc").join("notepad.md")
}

/// Ensure the .omc directory exists.
fn ensure_omc_dir(cwd: &str) -> std::io::Result<()> {
    let omc_dir = PathBuf::from(cwd).join(".omc");
    fs::create_dir_all(omc_dir)
}

/// Read notepad content from disk.
fn read_notepad(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    fs::read_to_string(path).ok()
}

/// Write full notepad content to disk.
fn write_notepad(path: &Path, content: &str) -> std::io::Result<()> {
    ensure_omc_dir(
        path.parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
            .as_str(),
    )?;
    fs::write(path, content)
}

/// Extract a section from the notepad markdown.
fn extract_section(content: &str, section_marker: &str) -> Option<String> {
    let marker = format!("## {section_marker}");
    let start = content.find(&marker)?;
    let after_marker = &content[start + marker.len()..];
    // Find the start of content (skip blank lines)
    let content_start = after_marker
        .char_indices()
        .find(|(_, c)| *c != '\n' && *c != '\r')
        .map_or_else(|| after_marker.len(), |(i, _)| i);
    let after_marker = &after_marker[content_start..];
    // Find the next ## section or end of string
    let end = after_marker.find("\n## ").unwrap_or(after_marker.len());
    let section = &after_marker[..end];
    let trimmed = section.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Build full notepad content from sections.
fn build_notepad(priority: &str, working: &str, manual: &str) -> String {
    let mut parts = vec!["# OMC Notepad\n".to_string()];

    parts.push("## Priority Context\n".to_string());
    if priority.is_empty() {
        parts.push("(empty)\n".to_string());
    } else {
        parts.push(format!("{priority}\n"));
    }

    parts.push("## Working Memory\n".to_string());
    if working.is_empty() {
        parts.push("(empty)\n".to_string());
    } else {
        parts.push(format!("{working}\n"));
    }

    parts.push("## MANUAL\n".to_string());
    if manual.is_empty() {
        parts.push("(empty)\n".to_string());
    } else {
        parts.push(format!("{manual}\n"));
    }

    parts.join("\n")
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn section_enum() -> Vec<String> {
    SECTION_NAMES.iter().map(std::string::ToString::to_string).collect()
}

// ============================================================================
// notepad_read
// ============================================================================

pub struct NotepadReadTool;

impl McpTool for NotepadReadTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "section".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some(
                    "Section to read: \"all\" (default), \"priority\", \"working\", or \"manual\""
                        .into(),
                ),
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
            name: "notepad_read".into(),
            description: "Read the notepad content. Can read the full notepad or a specific section (priority, working, manual).".into(),
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
        let path = notepad_path(&cwd);

        match section {
            "all" => match read_notepad(&path) {
                Some(content) => ToolResult::ok(format!(
                    "## Notepad\n\nPath: {}\n\n{content}",
                    path.display()
                )),
                None => ToolResult::ok(
                    "Notepad does not exist. Use notepad_write_* tools to create it.",
                ),
            },
            "priority" | "working" | "manual" => {
                let section_title = match section {
                    "priority" => "Priority Context",
                    "working" => "Working Memory",
                    "manual" => "MANUAL",
                    _ => unreachable!(),
                };
                match read_notepad(&path) {
                    Some(content) => match extract_section(&content, section_title) {
                        Some(text) => ToolResult::ok(format!("## {section_title}\n\n{text}")),
                        None => ToolResult::ok(format!(
                            "## {section_title}\n\n(Empty or notepad does not exist)"
                        )),
                    },
                    None => ToolResult::ok(format!(
                        "## {section_title}\n\n(Empty or notepad does not exist)"
                    )),
                }
            }
            _ => ToolResult::error(format!("Unknown section: {section}")),
        }
    }
}

// ============================================================================
// notepad_write_priority
// ============================================================================

pub struct NotepadWritePriorityTool;

impl McpTool for NotepadWritePriorityTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "content".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Content to write (recommend under 500 chars)".into()),
                r#enum: None,
                max_length: Some(2000),
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
            name: "notepad_write_priority".into(),
            description: "Write to the Priority Context section. This REPLACES the existing content. Keep under 500 chars - this is always loaded at session start.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["content".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let content = match str_arg(&args, "content") {
            Some(c) => c.to_string(),
            None => return ToolResult::error("Missing required parameter: content"),
        };
        let cwd = str_arg(&args, "workingDirectory")
            .unwrap_or(".")
            .to_string();
        let path = notepad_path(&cwd);

        // Read existing notepad to preserve other sections
        let existing = read_notepad(&path).unwrap_or_default();
        let working = extract_section(&existing, "Working Memory").unwrap_or_default();
        let manual = extract_section(&existing, "MANUAL").unwrap_or_default();

        let notepad = build_notepad(&content, &working, &manual);

        match write_notepad(&path, &notepad) {
            Ok(()) => ToolResult::ok(format!(
                "Successfully wrote to Priority Context ({} chars)",
                content.len()
            )),
            Err(e) => ToolResult::error(format!("Error writing to Priority Context: {e}")),
        }
    }
}

// ============================================================================
// notepad_write_working
// ============================================================================

pub struct NotepadWriteWorkingTool;

impl McpTool for NotepadWriteWorkingTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "content".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Content to add as a new entry".into()),
                r#enum: None,
                max_length: Some(4000),
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
            name: "notepad_write_working".into(),
            description: "Add an entry to Working Memory section. Entries are timestamped and auto-pruned after 7 days.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["content".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let content = match str_arg(&args, "content") {
            Some(c) => c.to_string(),
            None => return ToolResult::error("Missing required parameter: content"),
        };
        let cwd = str_arg(&args, "workingDirectory")
            .unwrap_or(".")
            .to_string();
        let path = notepad_path(&cwd);

        let existing = read_notepad(&path).unwrap_or_default();
        let priority = extract_section(&existing, "Priority Context").unwrap_or_default();
        let manual = extract_section(&existing, "MANUAL").unwrap_or_default();

        // Build working memory: existing entries + new timestamped entry
        let mut working_entries = extract_section(&existing, "Working Memory").unwrap_or_default();
        if !working_entries.is_empty() {
            working_entries.push('\n');
        }
        let ts = Utc::now().format("%Y-%m-%d %H:%M UTC");
        working_entries.push_str(&format!("[{ts}] {content}"));

        let notepad = build_notepad(&priority, &working_entries, &manual);

        match write_notepad(&path, &notepad) {
            Ok(()) => ToolResult::ok(format!(
                "Successfully added entry to Working Memory ({} chars)",
                content.len()
            )),
            Err(e) => ToolResult::error(format!("Error writing to Working Memory: {e}")),
        }
    }
}

// ============================================================================
// notepad_write_manual
// ============================================================================

pub struct NotepadWriteManualTool;

impl McpTool for NotepadWriteManualTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "content".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Content to add as a new entry".into()),
                r#enum: None,
                max_length: Some(4000),
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
            name: "notepad_write_manual".into(),
            description:
                "Add an entry to the MANUAL section. Content in this section is never auto-pruned."
                    .into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["content".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let content = match str_arg(&args, "content") {
            Some(c) => c.to_string(),
            None => return ToolResult::error("Missing required parameter: content"),
        };
        let cwd = str_arg(&args, "workingDirectory")
            .unwrap_or(".")
            .to_string();
        let path = notepad_path(&cwd);

        let existing = read_notepad(&path).unwrap_or_default();
        let priority = extract_section(&existing, "Priority Context").unwrap_or_default();
        let working = extract_section(&existing, "Working Memory").unwrap_or_default();

        let mut manual_entries = extract_section(&existing, "MANUAL").unwrap_or_default();
        if !manual_entries.is_empty() {
            manual_entries.push('\n');
        }
        let ts = Utc::now().format("%Y-%m-%d %H:%M UTC");
        manual_entries.push_str(&format!("[{ts}] {content}"));

        let notepad = build_notepad(&priority, &working, &manual_entries);

        match write_notepad(&path, &notepad) {
            Ok(()) => ToolResult::ok(format!(
                "Successfully added entry to MANUAL section ({} chars)",
                content.len()
            )),
            Err(e) => ToolResult::error(format!("Error writing to MANUAL: {e}")),
        }
    }
}

/// Collect all notepad tools.
pub fn notepad_tools() -> Vec<Box<dyn McpTool>> {
    vec![
        Box::new(NotepadReadTool),
        Box::new(NotepadWritePriorityTool),
        Box::new(NotepadWriteWorkingTool),
        Box::new(NotepadWriteManualTool),
    ]
}
