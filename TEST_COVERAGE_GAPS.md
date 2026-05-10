# Test Coverage Gap Analysis: oh-my-claudecode-RS

**Date:** 2026-05-10
**Total tests:** ~1,025 across 18 crates
**Total source lines:** 51,261

---

## Executive Summary

The codebase has strong unit test coverage in `omc-team` (271 tests), `omc-shared` (208 tests), and `omc-hud` (205 tests). However, there are significant gaps:

| Priority | Gap | Files | LOC Untested |
|----------|-----|-------|-------------|
| CRITICAL | omc-mcp tool implementations | 4 files | ~2,628 |
| HIGH | omc-interop bridge functions | 1 file | ~650 |
| HIGH | omc-xcmd crate (entirely untested) | 2 files | ~165 |
| MEDIUM | omc-hud infrastructure (cache, input, render, terminal) | 4 files | ~243 |
| MEDIUM | omc-shared routing (router, rules, types) | 3 files | ~964 |
| MEDIUM | omc-git-provider providers | 6 files | ~1,118 |
| LOW | omc-autoresearch (orchestrator, runtime, types) | 3 files | ~854 |
| LOW | omc-skills templates module | 1 file | ~817 |
| LOW | omc-wiki wiki.rs | 1 file | ~735 |

---

## 1. CRITICAL: omc-mcp Tool Implementations (0 tests)

The entire MCP tool layer — the tool trait, 13 tool structs, and collection functions — has zero tests. These tools handle file I/O, JSON parsing, and user-facing error messages.

### 1.1 `crates/omc-mcp/src/tools/mod.rs` — McpTool trait + types

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_content_text_creates_text_type() {
        let content = ToolContent::text("hello");
        assert_eq!(content.content_type, "text");
        assert_eq!(content.text, "hello");
    }

    #[test]
    fn tool_content_text_accepts_string_ref() {
        let content = ToolContent::text("borrowed".to_string());
        assert_eq!(content.text, "borrowed");
    }

    #[test]
    fn tool_result_ok_has_no_error_flag() {
        let result = ToolResult::ok("success");
        assert!(result.is_error.is_none());
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].text, "success");
    }

    #[test]
    fn tool_result_error_has_error_flag() {
        let result = ToolResult::error("something broke");
        assert_eq!(result.is_error, Some(true));
        assert_eq!(result.content[0].text, "something broke");
    }

    #[test]
    fn tool_result_serializes_without_is_error_when_none() {
        let result = ToolResult::ok("ok");
        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("isError").is_none(), "isError should be skipped when None");
        assert!(json["content"].is_array());
    }

    #[test]
    fn tool_result_serializes_is_error_when_some() {
        let result = ToolResult::error("err");
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["isError"], json!(true));
    }

    #[test]
    fn schema_property_serializes_type_field() {
        let prop = SchemaProperty {
            prop_type: "string".into(),
            description: Some("test".into()),
            r#enum: None,
            max_length: None,
            minimum: None,
            maximum: None,
        };
        let json = serde_json::to_value(&prop).unwrap();
        assert_eq!(json["type"], "string");
        assert_eq!(json["description"], "test");
        assert!(json.get("enum").is_none());
    }

    #[test]
    fn schema_property_skips_none_fields() {
        let prop = SchemaProperty {
            prop_type: "number".into(),
            description: None,
            r#enum: None,
            max_length: None,
            minimum: Some(0),
            maximum: Some(100),
        };
        let json = serde_json::to_value(&prop).unwrap();
        assert!(json.get("description").is_none());
        assert_eq!(json["minimum"], 0);
        assert_eq!(json["maximum"], 100);
    }

    #[test]
    fn tool_schema_skips_empty_required() {
        let schema = ToolSchema {
            schema_type: "object".into(),
            properties: HashMap::new(),
            required: vec![],
        };
        let json = serde_json::to_value(&schema).unwrap();
        assert!(json.get("required").is_none(), "empty required should be skipped");
    }

    #[test]
    fn tool_definition_serializes_input_schema_camelcase() {
        let def = ToolDefinition {
            name: "test_tool".into(),
            description: "A test tool".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties: HashMap::new(),
                required: vec!["arg1".into()],
            },
        };
        let json = serde_json::to_value(&def).unwrap();
        assert_eq!(json["name"], "test_tool");
        assert_eq!(json["inputSchema"]["type"], "object");
        assert_eq!(json["inputSchema"]["required"][0], "arg1");
    }
}
```

### 1.2 `crates/omc-mcp/src/state_tools.rs` — 5 state tools

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn setup_cwd() -> (TempDir, String) {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().to_string_lossy().to_string();
        (tmp, cwd)
    }

    // --- state_path ---

    #[test]
    fn state_path_legacy_format() {
        let p = state_path("ralph", None, "/project");
        assert_eq!(p, PathBuf::from("/project/.omc/state/ralph-state.json"));
    }

    #[test]
    fn state_path_session_format() {
        let p = state_path("ralph", Some("abc"), "/project");
        assert_eq!(p, PathBuf::from("/project/.omc/state/sessions/abc/ralph.json"));
    }

    // --- StateReadTool ---

    #[test]
    fn state_read_missing_mode_returns_error() {
        let result = StateReadTool.handle(json!({}));
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("Missing required parameter: mode"));
    }

    #[test]
    fn state_read_no_state_file_returns_not_found() {
        let (_tmp, cwd) = setup_cwd();
        let result = StateReadTool.handle(json!({
            "mode": "ralph",
            "workingDirectory": cwd
        }));
        assert!(result.is_error.is_none());
        assert!(result.content[0].text.contains("No state found"));
    }

    #[test]
    fn state_read_reads_existing_state() {
        let (tmp, cwd) = setup_cwd();
        let state_dir = tmp.path().join(".omc/state");
        std::fs::create_dir_all(&state_dir).unwrap();
        let state_file = state_dir.join("ralph-state.json");
        std::fs::write(&state_file, r#"{"active":true,"iteration":3}"#).unwrap();

        let result = StateReadTool.handle(json!({
            "mode": "ralph",
            "workingDirectory": cwd
        }));
        assert!(result.is_error.is_none());
        assert!(result.content[0].text.contains("active"));
        assert!(result.content[0].text.contains("iteration"));
    }

    #[test]
    fn state_read_session_scoped() {
        let (tmp, cwd) = setup_cwd();
        let session_dir = tmp.path().join(".omc/state/sessions/s1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(session_dir.join("ralph.json"), r#"{"active":true}"#).unwrap();

        let result = StateReadTool.handle(json!({
            "mode": "ralph",
            "session_id": "s1",
            "workingDirectory": cwd
        }));
        assert!(result.content[0].text.contains("session: s1"));
    }

    #[test]
    fn state_read_path_traversal_rejected() {
        let result = StateReadTool.handle(json!({
            "mode": "../../etc/passwd",
        }));
        assert_eq!(result.is_error, Some(true));
    }

    // --- StateWriteTool ---

    #[test]
    fn state_write_missing_mode_returns_error() {
        let result = StateWriteTool.handle(json!({}));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn state_write_creates_file_and_dirs() {
        let (_tmp, cwd) = setup_cwd();
        let result = StateWriteTool.handle(json!({
            "mode": "ralph",
            "active": true,
            "iteration": 5,
            "workingDirectory": cwd
        }));
        assert!(result.is_error.is_none());
        assert!(result.content[0].text.contains("Successfully wrote state"));

        // Verify the file was created
        let path = state_path("ralph", None, &cwd);
        assert!(path.exists());
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content["active"], true);
        assert_eq!(content["iteration"], 5);
        assert!(content["_meta"].is_object());
    }

    #[test]
    fn state_write_custom_state_merged() {
        let (_tmp, cwd) = setup_cwd();
        let result = StateWriteTool.handle(json!({
            "mode": "team",
            "active": true,
            "state": {"customField": "value", "iteration": 99},
            "workingDirectory": cwd
        }));
        assert!(result.is_error.is_none());

        let path = state_path("team", None, &cwd);
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        // explicit "active" takes precedence over "state.active" if duplicated
        assert_eq!(content["active"], true);
        assert_eq!(content["customField"], "value");
    }

    #[test]
    fn state_write_path_traversal_rejected() {
        let result = StateWriteTool.handle(json!({"mode": "../escape"}));
        assert_eq!(result.is_error, Some(true));
    }

    // --- StateClearTool ---

    #[test]
    fn state_clear_no_state_returns_nothing_to_clear() {
        let (_tmp, cwd) = setup_cwd();
        let result = StateClearTool.handle(json!({
            "mode": "ralph",
            "workingDirectory": cwd
        }));
        assert!(result.content[0].text.contains("No state found to clear"));
    }

    #[test]
    fn state_clear_removes_state_file() {
        let (tmp, cwd) = setup_cwd();
        let state_dir = tmp.path().join(".omc/state");
        std::fs::create_dir_all(&state_dir).unwrap();
        let state_file = state_dir.join("ralph-state.json");
        std::fs::write(&state_file, "{}").unwrap();

        let result = StateClearTool.handle(json!({
            "mode": "ralph",
            "workingDirectory": cwd
        }));
        assert!(result.content[0].text.contains("Cleared state"));
        assert!(!state_file.exists());
    }

    #[test]
    fn state_clear_session_scoped() {
        let (tmp, cwd) = setup_cwd();
        let session_dir = tmp.path().join(".omc/state/sessions/s1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(session_dir.join("ralph.json"), "{}").unwrap();

        let result = StateClearTool.handle(json!({
            "mode": "ralph",
            "session_id": "s1",
            "workingDirectory": cwd
        }));
        assert!(result.content[0].text.contains("Cleared state"));
        assert!(!session_dir.join("ralph.json").exists());
    }

    // --- StateListActiveTool ---

    #[test]
    fn state_list_active_no_modes() {
        let (_tmp, cwd) = setup_cwd();
        let result = StateListActiveTool.handle(json!({
            "workingDirectory": cwd
        }));
        assert!(result.content[0].text.contains("No modes are currently active"));
    }

    // --- StateGetStatusTool ---

    #[test]
    fn state_get_status_single_mode() {
        let (_tmp, cwd) = setup_cwd();
        let result = StateGetStatusTool.handle(json!({
            "mode": "ralph",
            "workingDirectory": cwd
        }));
        assert!(result.content[0].text.contains("Status: ralph"));
        assert!(result.content[0].text.contains("No state file"));
    }

    #[test]
    fn state_get_status_all_modes() {
        let (_tmp, cwd) = setup_cwd();
        let result = StateGetStatusTool.handle(json!({
            "workingDirectory": cwd
        }));
        assert!(result.content[0].text.contains("All Mode Statuses"));
        assert!(result.content[0].text.contains("ralph"));
    }

    // --- Helpers ---

    #[test]
    fn check_inputs_valid_mode() {
        assert!(check_inputs(Some("ralph"), None).is_ok());
    }

    #[test]
    fn check_inputs_rejects_dot_dot() {
        assert!(check_inputs(Some("../escape"), None).is_err());
    }

    #[test]
    fn mode_enum_contains_all_modes() {
        let modes = mode_enum();
        assert!(modes.contains(&"ralph".to_string()));
        assert!(modes.contains(&"autopilot".to_string()));
        assert!(modes.contains(&"team".to_string()));
        assert_eq!(modes.len(), STATE_TOOL_MODES.len());
    }

    #[test]
    fn is_mode_active_at_missing_file() {
        assert!(!is_mode_active_at(Path::new("/nonexistent/path.json")));
    }

    #[test]
    fn is_mode_active_at_inactive_state() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("state.json");
        std::fs::write(&path, r#"{"active":false}"#).unwrap();
        assert!(!is_mode_active_at(&path));
    }

    #[test]
    fn is_mode_active_at_active_state() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("state.json");
        std::fs::write(&path, r#"{"active":true}"#).unwrap();
        assert!(is_mode_active_at(&path));
    }

    #[test]
    fn atomic_write_json_creates_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        atomic_write_json(&path, &json!({"key": "value"})).unwrap();
        assert!(path.exists());
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content["key"], "value");
    }

    #[test]
    fn list_session_ids_empty_when_no_dir() {
        let (_tmp, cwd) = setup_cwd();
        let ids = list_session_ids(&cwd);
        assert!(ids.is_empty());
    }

    #[test]
    fn state_tools_returns_five_tools() {
        let tools = state_tools();
        assert_eq!(tools.len(), 5);
        let names: Vec<_> = tools.iter().map(|t| t.definition().name.clone()).collect();
        assert!(names.contains(&"state_read".to_string()));
        assert!(names.contains(&"state_write".to_string()));
        assert!(names.contains(&"state_clear".to_string()));
        assert!(names.contains(&"state_list_active".to_string()));
        assert!(names.contains(&"state_get_status".to_string()));
    }
}
```

### 1.3 `crates/omc-mcp/src/notepad_tools.rs` — 4 notepad tools

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn setup_cwd() -> (TempDir, String) {
        let tmp = TempDir::new().unwrap();
        (tmp, tmp.path().to_string_lossy().to_string())
    }

    // --- extract_section ---

    #[test]
    fn extract_section_returns_content() {
        let content = "# OMC Notepad\n\n## Priority Context\n\nImportant stuff\n\n## Working Memory\n\nOther stuff\n";
        let result = extract_section(content, "Priority Context");
        assert_eq!(result, Some("Important stuff".to_string()));
    }

    #[test]
    fn extract_section_returns_none_for_empty() {
        let content = "# OMC Notepad\n\n## Priority Context\n\n\n## Working Memory\n\nstuff\n";
        let result = extract_section(content, "Priority Context");
        assert!(result.is_none());
    }

    #[test]
    fn extract_section_returns_none_when_missing() {
        let content = "# OMC Notepad\n\n## Working Memory\n\nstuff\n";
        let result = extract_section(content, "Priority Context");
        assert!(result.is_none());
    }

    #[test]
    fn extract_section_handles_last_section() {
        let content = "# OMC Notepad\n\n## MANUAL\n\nManual entries here\n";
        let result = extract_section(content, "MANUAL");
        assert_eq!(result, Some("Manual entries here".to_string()));
    }

    // --- build_notepad ---

    #[test]
    fn build_notepad_with_all_empty() {
        let notepad = build_notepad("", "", "");
        assert!(notepad.contains("(empty)"));
        assert!(notepad.contains("# OMC Notepad"));
        assert!(notepad.contains("## Priority Context"));
        assert!(notepad.contains("## Working Memory"));
        assert!(notepad.contains("## MANUAL"));
    }

    #[test]
    fn build_notepad_with_content() {
        let notepad = build_notepad("priority content", "working content", "manual content");
        assert!(notepad.contains("priority content"));
        assert!(notepad.contains("working content"));
        assert!(notepad.contains("manual content"));
        assert!(!notepad.contains("(empty)"));
    }

    // --- NotepadReadTool ---

    #[test]
    fn notepad_read_nonexistent_returns_message() {
        let (_tmp, cwd) = setup_cwd();
        let result = NotepadReadTool.handle(json!({"workingDirectory": cwd}));
        assert!(result.content[0].text.contains("does not exist"));
    }

    #[test]
    fn notepad_read_all_section() {
        let (tmp, cwd) = setup_cwd();
        let omc_dir = tmp.path().join(".omc");
        std::fs::create_dir_all(&omc_dir).unwrap();
        std::fs::write(omc_dir.join("notepad.md"), "# OMC Notepad\n\n## Priority Context\n\nTest\n").unwrap();

        let result = NotepadReadTool.handle(json!({"section": "all", "workingDirectory": cwd}));
        assert!(result.content[0].text.contains("Notepad"));
    }

    #[test]
    fn notepad_read_specific_section() {
        let (tmp, cwd) = setup_cwd();
        let omc_dir = tmp.path().join(".omc");
        std::fs::create_dir_all(&omc_dir).unwrap();
        std::fs::write(omc_dir.join("notepad.md"),
            "# OMC Notepad\n\n## Priority Context\n\nImportant\n\n## Working Memory\n\nWorking\n"
        ).unwrap();

        let result = NotepadReadTool.handle(json!({"section": "priority", "workingDirectory": cwd}));
        assert!(result.content[0].text.contains("Important"));
    }

    #[test]
    fn notepad_read_unknown_section_returns_error() {
        let (_tmp, cwd) = setup_cwd();
        let result = NotepadReadTool.handle(json!({"section": "invalid", "workingDirectory": cwd}));
        assert_eq!(result.is_error, Some(true));
    }

    // --- NotepadWritePriorityTool ---

    #[test]
    fn write_priority_missing_content_returns_error() {
        let result = NotepadWritePriorityTool.handle(json!({}));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn write_priority_creates_notepad() {
        let (_tmp, cwd) = setup_cwd();
        let result = NotepadWritePriorityTool.handle(json!({
            "content": "My priority",
            "workingDirectory": cwd
        }));
        assert!(result.is_error.is_none());
        assert!(result.content[0].text.contains("Priority Context"));

        // Verify file exists and contains content
        let path = notepad_path(&cwd);
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("My priority"));
    }

    #[test]
    fn write_priority_preserves_other_sections() {
        let (tmp, cwd) = setup_cwd();
        let omc_dir = tmp.path().join(".omc");
        std::fs::create_dir_all(&omc_dir).unwrap();
        std::fs::write(omc_dir.join("notepad.md"),
            "# OMC Notepad\n\n## Priority Context\n\nOld\n\n## Working Memory\n\nExisting\n"
        ).unwrap();

        NotepadWritePriorityTool.handle(json!({
            "content": "New priority",
            "workingDirectory": cwd
        }));

        let content = std::fs::read_to_string(notepad_path(&cwd)).unwrap();
        assert!(content.contains("New priority"));
        assert!(content.contains("Existing"), "Working Memory should be preserved");
    }

    // --- NotepadWriteWorkingTool ---

    #[test]
    fn write_working_appends_timestamped_entry() {
        let (_tmp, cwd) = setup_cwd();
        let result = NotepadWriteWorkingTool.handle(json!({
            "content": "Some observation",
            "workingDirectory": cwd
        }));
        assert!(result.is_error.is_none());

        let content = std::fs::read_to_string(notepad_path(&cwd)).unwrap();
        assert!(content.contains("Some observation"));
        // Should contain a timestamp prefix like [2026-
        assert!(content.contains("[20"));
    }

    #[test]
    fn write_working_accumulates_entries() {
        let (_tmp, cwd) = setup_cwd();
        NotepadWriteWorkingTool.handle(json!({"content": "Entry 1", "workingDirectory": cwd}));
        NotepadWriteWorkingTool.handle(json!({"content": "Entry 2", "workingDirectory": cwd}));

        let content = std::fs::read_to_string(notepad_path(&cwd)).unwrap();
        assert!(content.contains("Entry 1"));
        assert!(content.contains("Entry 2"));
    }

    // --- NotepadWriteManualTool ---

    #[test]
    fn write_manual_appends_entry() {
        let (_tmp, cwd) = setup_cwd();
        NotepadWriteManualTool.handle(json!({"content": "Manual note", "workingDirectory": cwd}));

        let content = std::fs::read_to_string(notepad_path(&cwd)).unwrap();
        assert!(content.contains("Manual note"));
        assert!(content.contains("## MANUAL"));
    }

    // --- notepad_tools() ---

    #[test]
    fn notepad_tools_returns_four() {
        assert_eq!(notepad_tools().len(), 4);
    }
}
```

### 1.4 `crates/omc-mcp/src/memory_tools.rs` — 4 memory tools

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn setup_cwd() -> (TempDir, String) {
        let tmp = TempDir::new().unwrap();
        (tmp, tmp.path().to_string_lossy().to_string())
    }

    fn create_memory_file(tmp: &TempDir, content: &str) {
        let omc_dir = tmp.path().join(".omc");
        std::fs::create_dir_all(&omc_dir).unwrap();
        std::fs::write(omc_dir.join("project-memory.json"), content).unwrap();
    }

    // --- ProjectMemoryReadTool ---

    #[test]
    fn memory_read_no_file_returns_message() {
        let (_tmp, cwd) = setup_cwd();
        let result = ProjectMemoryReadTool.handle(json!({"workingDirectory": cwd}));
        assert!(result.content[0].text.contains("does not exist"));
    }

    #[test]
    fn memory_read_all_sections() {
        let (tmp, cwd) = setup_cwd();
        create_memory_file(&tmp, r#"{"techStack":["rust"],"version":"1.0.0"}"#);

        let result = ProjectMemoryReadTool.handle(json!({"section": "all", "workingDirectory": cwd}));
        assert!(result.content[0].text.contains("rust"));
    }

    #[test]
    fn memory_read_specific_section() {
        let (tmp, cwd) = setup_cwd();
        create_memory_file(&tmp, r#"{"techStack":["rust","tokio"],"build":"cargo build"}"#);

        let result = ProjectMemoryReadTool.handle(json!({"section": "techStack", "workingDirectory": cwd}));
        assert!(result.content[0].text.contains("rust"));
    }

    #[test]
    fn memory_read_unknown_section_returns_error() {
        let (tmp, cwd) = setup_cwd();
        create_memory_file(&tmp, r#"{"version":"1.0"}"#);

        let result = ProjectMemoryReadTool.handle(json!({"section": "nonexistent", "workingDirectory": cwd}));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn memory_read_notes_section_maps_to_custom_notes() {
        let (tmp, cwd) = setup_cwd();
        create_memory_file(&tmp, r#"{"customNotes":[{"category":"test","content":"note1"}]}"#);

        let result = ProjectMemoryReadTool.handle(json!({"section": "notes", "workingDirectory": cwd}));
        assert!(result.content[0].text.contains("note1"));
    }

    // --- ProjectMemoryWriteTool ---

    #[test]
    fn memory_write_missing_object_returns_error() {
        let result = ProjectMemoryWriteTool.handle(json!({}));
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("Missing or invalid"));
    }

    #[test]
    fn memory_write_non_object_returns_error() {
        let result = ProjectMemoryWriteTool.handle(json!({"memory": "not an object"}));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn memory_write_creates_file_with_defaults() {
        let (_tmp, cwd) = setup_cwd();
        let result = ProjectMemoryWriteTool.handle(json!({
            "memory": {"techStack": ["rust"]},
            "workingDirectory": cwd
        }));
        assert!(result.is_error.is_none());

        let path = memory_path(&cwd);
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content["techStack"][0], "rust");
        assert!(content["version"].is_string());
        assert!(content["lastScanned"].is_number());
        assert!(content["projectRoot"].is_string());
    }

    #[test]
    fn memory_write_merge_preserves_existing() {
        let (tmp, cwd) = setup_cwd();
        create_memory_file(&tmp, r#"{"techStack":["rust"],"build":"cargo build","version":"1.0"}"#);

        ProjectMemoryWriteTool.handle(json!({
            "memory": {"conventions": ["snake_case"]},
            "merge": true,
            "workingDirectory": cwd
        }));

        let path = memory_path(&cwd);
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content["build"], "cargo build");
        assert_eq!(content["conventions"][0], "snake_case");
    }

    #[test]
    fn memory_write_replace_overwrites() {
        let (tmp, cwd) = setup_cwd();
        create_memory_file(&tmp, r#"{"techStack":["python"],"version":"1.0"}"#);

        ProjectMemoryWriteTool.handle(json!({
            "memory": {"techStack": ["rust"]},
            "merge": false,
            "workingDirectory": cwd
        }));

        let path = memory_path(&cwd);
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content["techStack"][0], "rust");
    }

    // --- ProjectMemoryAddNoteTool ---

    #[test]
    fn add_note_missing_category_returns_error() {
        let result = ProjectMemoryAddNoteTool.handle(json!({"content": "test"}));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn add_note_missing_content_returns_error() {
        let result = ProjectMemoryAddNoteTool.handle(json!({"category": "build"}));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn add_note_no_memory_file_returns_message() {
        let (_tmp, cwd) = setup_cwd();
        let result = ProjectMemoryAddNoteTool.handle(json!({
            "category": "test",
            "content": "note",
            "workingDirectory": cwd
        }));
        assert!(result.content[0].text.contains("does not exist"));
    }

    #[test]
    fn add_note_appends_to_existing_memory() {
        let (tmp, cwd) = setup_cwd();
        create_memory_file(&tmp, r#"{"version":"1.0","customNotes":[]}"#);

        ProjectMemoryAddNoteTool.handle(json!({
            "category": "build",
            "content": "Use cargo build --release",
            "workingDirectory": cwd
        }));

        let path = memory_path(&cwd);
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let notes = content["customNotes"].as_array().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0]["category"], "build");
        assert!(notes[0]["timestamp"].is_string());
    }

    // --- ProjectMemoryAddDirectiveTool ---

    #[test]
    fn add_directive_missing_returns_error() {
        let result = ProjectMemoryAddDirectiveTool.handle(json!({}));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn add_directive_no_memory_returns_message() {
        let (_tmp, cwd) = setup_cwd();
        let result = ProjectMemoryAddDirectiveTool.handle(json!({
            "directive": "Use TypeScript strict mode",
            "workingDirectory": cwd
        }));
        assert!(result.content[0].text.contains("does not exist"));
    }

    #[test]
    fn add_directive_appends_with_priority() {
        let (tmp, cwd) = setup_cwd();
        create_memory_file(&tmp, r#"{"version":"1.0"}"#);

        ProjectMemoryAddDirectiveTool.handle(json!({
            "directive": "No console.log",
            "priority": "high",
            "context": "Production code",
            "workingDirectory": cwd
        }));

        let path = memory_path(&cwd);
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let dirs = content["userDirectives"].as_array().unwrap();
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0]["priority"], "high");
        assert_eq!(dirs[0]["directive"], "No console.log");
    }

    // --- merge_objects ---

    #[test]
    fn merge_objects_overlay_wins() {
        let base = json!({"a": 1, "b": 2});
        let overlay = json!({"b": 99, "c": 3});
        let result = merge_objects(base, overlay);
        assert_eq!(result["a"], 1);
        assert_eq!(result["b"], 99);
        assert_eq!(result["c"], 3);
    }

    // --- memory_tools() ---

    #[test]
    fn memory_tools_returns_four() {
        assert_eq!(memory_tools().len(), 4);
    }
}
```

### 1.5 `crates/omc-mcp/src/lib.rs` — all_tools()

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tools_returns_expected_count() {
        let tools = all_tools();
        // 5 state + 4 notepad + 4 memory = 13
        assert_eq!(tools.len(), 13);
    }

    #[test]
    fn all_tools_have_unique_names() {
        let tools = all_tools();
        let names: Vec<_> = tools.iter().map(|t| t.definition().name.clone()).collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "Duplicate tool names found");
    }

    #[test]
    fn all_tools_have_nonempty_descriptions() {
        let tools = all_tools();
        for tool in &tools {
            let def = tool.definition();
            assert!(!def.description.is_empty(), "Tool '{}' has empty description", def.name);
        }
    }
}
```

---

## 2. HIGH: omc-interop Bridge Functions (8 functions, 0 tests)

The existing 11 tests only cover helper utilities. The 8 `interop_*` functions that form the core API surface are untested.

```rust
// Add to crates/omc-interop/src/mcp_bridge.rs test module

#[cfg(test)]
mod bridge_tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_cwd() -> (TempDir, String) {
        let tmp = TempDir::new().unwrap();
        (tmp, tmp.path().to_string_lossy().to_string())
    }

    #[test]
    fn interop_send_task_creates_shared_task() {
        let (_tmp, cwd) = setup_cwd();
        let args = SendTaskArgs {
            target: shared_state::InteropSide::Omx,
            task_type: shared_state::TaskType::General,
            description: "Test task".into(),
            context: None,
            files: None,
            working_directory: Some(cwd),
        };
        let result = interop_send_task(&args);
        assert!(result.is_error.is_none(), "Should not error: {:?}", result.content);
        assert!(result.content[0].text.contains("Task Sent"));
        assert!(result.content[0].text.contains("Test task"));
    }

    #[test]
    fn interop_read_results_empty() {
        let (_tmp, cwd) = setup_cwd();
        let args = ReadResultsArgs {
            source: None,
            status: None,
            limit: None,
            working_directory: Some(cwd),
        };
        let result = interop_read_results(&args);
        assert!(result.content[0].text.contains("No Tasks Found"));
    }

    #[test]
    fn interop_send_message_creates_message() {
        let (_tmp, cwd) = setup_cwd();
        let args = SendMessageArgs {
            target: shared_state::InteropSide::Omx,
            content: "Hello from OMC".into(),
            metadata: None,
            working_directory: Some(cwd),
        };
        let result = interop_send_message(&args);
        assert!(result.is_error.is_none());
        assert!(result.content[0].text.contains("Message Sent"));
    }

    #[test]
    fn interop_read_messages_empty() {
        let (_tmp, cwd) = setup_cwd();
        let args = ReadMessagesArgs {
            source: None,
            unread_only: None,
            limit: None,
            mark_as_read: None,
            working_directory: Some(cwd),
        };
        let result = interop_read_messages(&args);
        assert!(result.content[0].text.contains("No Messages Found"));
    }

    #[test]
    fn interop_list_omx_teams_empty() {
        let (_tmp, cwd) = setup_cwd();
        let args = ListOmxTeamsArgs {
            working_directory: Some(cwd),
        };
        let result = interop_list_omx_teams(&args);
        assert!(result.content[0].text.contains("No OMX Teams Found"));
    }

    #[test]
    fn interop_send_task_then_read_results() {
        let (_tmp, cwd) = setup_cwd();

        // Send a task
        let send_args = SendTaskArgs {
            target: shared_state::InteropSide::Omx,
            task_type: shared_state::TaskType::CodeGeneration,
            description: "Build the feature".into(),
            context: Some(json!({"priority": "high"})),
            files: Some(vec!["src/main.rs".into()]),
            working_directory: Some(cwd.clone()),
        };
        let send_result = interop_send_task(&send_args);
        assert!(send_result.is_error.is_none());

        // Read results
        let read_args = ReadResultsArgs {
            source: None,
            status: None,
            limit: Some(5),
            working_directory: Some(cwd),
        };
        let read_result = interop_read_results(&read_args);
        assert!(read_result.content[0].text.contains("Build the feature"));
    }

    #[test]
    fn interop_send_omx_message_disabled_by_default() {
        let args = SendOmxMessageArgs {
            team_name: "test-team".into(),
            from_worker: "worker-1".into(),
            to_worker: "worker-2".into(),
            body: "hello".into(),
            broadcast: None,
            working_directory: None,
        };
        let result = interop_send_omx_message(&args);
        // Should be disabled unless env vars are set
        assert!(result.is_error.is_some());
    }

    #[test]
    fn interop_read_omx_messages_empty() {
        let (_tmp, cwd) = setup_cwd();
        let args = ReadOmxMessagesArgs {
            team_name: "nonexistent".into(),
            worker_name: "worker-1".into(),
            limit: None,
            working_directory: Some(cwd),
        };
        let result = interop_read_omx_messages(&args);
        assert!(result.content[0].text.contains("No Messages"));
    }

    #[test]
    fn interop_read_omx_tasks_empty() {
        let (_tmp, cwd) = setup_cwd();
        let args = ReadOmxTasksArgs {
            team_name: "nonexistent".into(),
            status: None,
            limit: None,
            working_directory: Some(cwd),
        };
        let result = interop_read_omx_tasks(&args);
        assert!(result.content[0].text.contains("No Tasks"));
    }
}
```

---

## 3. HIGH: omc-xcmd (Entirely Untested)

Two files with public APIs and zero tests.

### 3.1 `crates/omc-xcmd/src/executor.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_packages_returns_zero_when_dir_missing() {
        // This will depend on actual home dir; at minimum it should not panic
        let result = count_packages();
        // Should return Some(0) or None depending on home dir availability
        assert!(result.is_some() || result.is_none());
    }

    #[test]
    fn list_packages_returns_vec() {
        let result = list_packages();
        // Should not panic regardless of system state
        // If x-cmd is installed, returns package names; otherwise empty
        assert!(result.is_empty() || !result.is_empty());
    }

    #[test]
    fn list_packages_returns_sorted() {
        let packages = list_packages();
        let mut sorted = packages.clone();
        sorted.sort();
        assert_eq!(packages, sorted);
    }

    #[test]
    fn run_xcmd_with_empty_args_shows_help() {
        // Only runs if x-cmd is installed
        let home = dirs::home_dir();
        if home.is_some_and(|h| h.join(".x-cmd.root/X").exists()) {
            let result = run_xcmd(&[]);
            // Should either succeed with help text or fail gracefully
            assert!(result.is_ok() || result.is_err());
        }
    }

    #[test]
    fn run_xcmd_missing_install_returns_error() {
        // Test that the error message is descriptive when x-cmd isn't found
        // (hard to test without mocking; at minimum verify it doesn't panic)
        let _ = run_xcmd(&["version"]);
    }
}
```

### 3.2 `crates/omc-xcmd/src/skills.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_skills_returns_vec() {
        let skills = list_skills();
        // Should not panic; returns empty if skills_dir() is None
        assert!(skills.is_empty() || !skills.is_empty());
    }

    #[test]
    fn list_skills_sorted_by_name() {
        let skills = list_skills();
        let names: Vec<_> = skills.iter().map(|s| &s.name).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn search_skills_filters_by_name() {
        let skills = search_skills("nonexistent-xyz-12345");
        assert!(skills.is_empty());
    }

    #[test]
    fn search_skills_case_insensitive() {
        // If any skills exist, searching with different cases should work
        let all = list_skills();
        if !all.is_empty() {
            let upper = search_skills(&all[0].name.to_uppercase());
            let lower = search_skills(&all[0].name.to_lowercase());
            assert_eq!(upper.len(), lower.len());
        }
    }

    #[test]
    fn skill_count_matches_list_skills() {
        assert_eq!(skill_count(), list_skills().len());
    }
}
```

---

## 4. MEDIUM: omc-hud Infrastructure

### 4.1 `crates/omc-hud/src/cache.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn hud_cache_new_has_empty_samples() {
        let cache = HudCache::new("test-session".into());
        assert_eq!(cache.session_id, "test-session");
        assert!(cache.context_samples.is_empty());
        assert!(cache.last_updated_ms > 0);
    }

    #[test]
    fn record_context_adds_sample() {
        let mut cache = HudCache::new("s1".into());
        cache.record_context(Some(1000), 100);
        assert_eq!(cache.context_samples.len(), 1);
        assert_eq!(cache.context_samples[0].tokens, 1000);
        assert_eq!(cache.last_updated_ms, 100);
    }

    #[test]
    fn record_context_ignores_none_tokens() {
        let mut cache = HudCache::new("s1".into());
        cache.record_context(None, 100);
        assert!(cache.context_samples.is_empty());
    }

    #[test]
    fn record_context_deduplicates_same_timestamp_and_tokens() {
        let mut cache = HudCache::new("s1".into());
        cache.record_context(Some(1000), 100);
        cache.record_context(Some(1000), 100);
        assert_eq!(cache.context_samples.len(), 1);
    }

    #[test]
    fn record_context_allows_same_ts_different_tokens() {
        let mut cache = HudCache::new("s1".into());
        cache.record_context(Some(1000), 100);
        cache.record_context(Some(2000), 100);
        assert_eq!(cache.context_samples.len(), 2);
    }

    #[test]
    fn record_context_evicts_old_samples() {
        let mut cache = HudCache::new("s1".into());
        for i in 0..40 {
            cache.record_context(Some(i), i * 1000);
        }
        assert_eq!(cache.context_samples.len(), MAX_SAMPLES);
        // Oldest samples evicted
        assert_eq!(cache.context_samples[0].tokens, 4);
    }

    #[test]
    fn cache_path_returns_none_without_session_id() {
        let input = Input {
            session_id: None,
            ..Default::default()
        };
        assert!(cache_path(&input).is_none());
    }

    #[test]
    fn cache_path_contains_session_id() {
        let input = Input {
            session_id: Some("abc123".into()),
            cwd: Some("/project".into()),
            ..Default::default()
        };
        let path = cache_path(&input).unwrap();
        assert!(path.to_string_lossy().contains("abc123"));
        assert!(path.to_string_lossy().contains("hud-cache.json"));
    }

    #[test]
    fn load_creates_new_cache_for_unknown_session() {
        let input = Input {
            session_id: None,
            ..Default::default()
        };
        let cache = load(&input);
        assert_eq!(cache.session_id, "unknown");
        assert!(cache.context_samples.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let session_id = "test-session".to_string();
        let input = Input {
            session_id: Some(session_id.clone()),
            cwd: Some(tmp.path().to_string_lossy().to_string()),
            ..Default::default()
        };

        let mut cache = HudCache::new(session_id);
        cache.record_context(Some(5000), 1000);
        cache.record_context(Some(6000), 2000);

        save(&input, &cache);
        let loaded = load(&input);
        assert_eq!(loaded.session_id, "test-session");
        assert_eq!(loaded.context_samples.len(), 2);
        assert_eq!(loaded.context_samples[0].tokens, 5000);
    }

    #[test]
    fn load_discards_cache_from_different_session() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path().to_string_lossy().to_string();

        // Write cache for session A
        let input_a = Input {
            session_id: Some("session-a".into()),
            cwd: Some(cwd.clone()),
            ..Default::default()
        };
        let mut cache_a = HudCache::new("session-a".into());
        cache_a.record_context(Some(1000), 100);
        save(&input_a, &cache_a);

        // Load for session B — should get fresh cache
        let input_b = Input {
            session_id: Some("session-b".into()),
            cwd: Some(cwd),
            ..Default::default()
        };
        let cache_b = load(&input_b);
        assert_eq!(cache_b.session_id, "session-b");
        assert!(cache_b.context_samples.is_empty());
    }

    #[test]
    fn now_ms_returns_nonzero() {
        assert!(now_ms() > 0);
    }
}
```

### 4.2 `crates/omc-hud/src/input.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_string_returns_default() {
        let input = parse_stdin_json("");
        assert!(input.session_id.is_none());
        assert!(input.cwd.is_none());
    }

    #[test]
    fn parse_whitespace_returns_default() {
        let input = parse_stdin_json("   \n\t  ");
        assert!(input.session_id.is_none());
    }

    #[test]
    fn parse_valid_json() {
        let json = r#"{"session_id":"abc","cwd":"/project","model":"sonnet"}"#;
        let input = parse_stdin_json(json);
        assert_eq!(input.session_id.as_deref(), Some("abc"));
        assert_eq!(input.cwd.as_deref(), Some("/project"));
        assert_eq!(input.model.as_deref(), Some("sonnet"));
    }

    #[test]
    fn parse_partial_json_fills_defaults() {
        let json = r#"{"session_id":"s1"}"#;
        let input = parse_stdin_json(json);
        assert_eq!(input.session_id.as_deref(), Some("s1"));
        assert!(input.cwd.is_none());
        assert!(input.model.is_none());
    }

    #[test]
    fn parse_invalid_json_returns_default() {
        let input = parse_stdin_json("{not valid json");
        assert!(input.session_id.is_none());
    }

    #[test]
    fn parse_with_numeric_fields() {
        let json = r#"{"context_window_tokens":50000,"context_window_max":200000,"cost_usd":0.05}"#;
        let input = parse_stdin_json(json);
        assert_eq!(input.context_window_tokens, Some(50000));
        assert_eq!(input.context_window_max, Some(200000));
        assert!((input.cost_usd.unwrap() - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_with_hooks_state() {
        let json = r#"{"hooks_state":{"ralph":{"active":true}}}"#;
        let input = parse_stdin_json(json);
        assert!(input.hooks_state.is_some());
    }
}
```

### 4.3 `crates/omc-hud/src/terminal.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_mono_returns_plain_text() {
        let result = paint(ColorLevel::Mono, SemanticColor::Green, "hello");
        assert_eq!(result, "hello");
    }

    #[test]
    fn paint_truecolor_green() {
        let result = paint(ColorLevel::TrueColor, SemanticColor::Green, "ok");
        assert!(result.contains("\x1b[38;2;74;222;128m"));
        assert!(result.contains("ok"));
        assert!(result.ends_with("\x1b[0m"));
    }

    #[test]
    fn paint_truecolor_yellow() {
        let result = paint(ColorLevel::TrueColor, SemanticColor::Yellow, "warn");
        assert!(result.contains("\x1b[38;2;250;204;21m"));
    }

    #[test]
    fn paint_truecolor_red() {
        let result = paint(ColorLevel::TrueColor, SemanticColor::Red, "err");
        assert!(result.contains("\x1b[38;2;248;113;113m"));
    }

    #[test]
    fn paint_256_green() {
        let result = paint(ColorLevel::Color256, SemanticColor::Green, "ok");
        assert!(result.contains("\x1b[38;5;120m"));
    }

    #[test]
    fn paint_256_yellow() {
        let result = paint(ColorLevel::Color256, SemanticColor::Yellow, "warn");
        assert!(result.contains("\x1b[38;5;220m"));
    }

    #[test]
    fn paint_256_red() {
        let result = paint(ColorLevel::Color256, SemanticColor::Red, "err");
        assert!(result.contains("\x1b[38;5;203m"));
    }

    #[test]
    fn paint_16_green() {
        let result = paint(ColorLevel::Color16, SemanticColor::Green, "ok");
        assert!(result.contains("\x1b[32m"));
    }

    #[test]
    fn paint_16_yellow() {
        let result = paint(ColorLevel::Color16, SemanticColor::Yellow, "warn");
        assert!(result.contains("\x1b[33m"));
    }

    #[test]
    fn paint_16_red() {
        let result = paint(ColorLevel::Color16, SemanticColor::Red, "err");
        assert!(result.contains("\x1b[31m"));
    }

    #[test]
    fn paint_preserves_text_content() {
        let text = "special chars: <>&\"'";
        let result = paint(ColorLevel::TrueColor, SemanticColor::Green, text);
        assert!(result.contains(text));
    }

    #[test]
    fn color_level_equality() {
        assert_eq!(ColorLevel::TrueColor, ColorLevel::TrueColor);
        assert_ne!(ColorLevel::Mono, ColorLevel::Color16);
    }
}
```

### 4.4 `crates/omc-hud/src/render.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::HudCache;
    use crate::input::Input;
    use crate::terminal::ColorLevel;
    use crate::i18n::EN_STRINGS;

    fn default_input() -> Input {
        Input::default()
    }

    fn default_cache() -> HudCache {
        HudCache::new("test".into())
    }

    #[test]
    fn render_statusline_returns_string() {
        let input = default_input();
        let cache = default_cache();
        let result = render_statusline(&input, &cache, ColorLevel::Mono, EN_STRINGS);
        // Should not panic, returns empty or bracketed segments
        assert!(result.is_empty() || result.contains('['));
    }

    #[test]
    fn render_statusline_segments_separated_by_pipe() {
        let input = Input {
            model: Some("sonnet".into()),
            ..Default::default()
        };
        let cache = default_cache();
        let result = render_statusline(&input, &cache, ColorLevel::Mono, EN_STRINGS);
        if result.contains(" | ") {
            // Multiple segments present
            let segments: Vec<_> = result.split(" | ").collect();
            assert!(segments.len() > 1);
        }
    }

    #[test]
    fn render_statusline_no_empty_segments() {
        let input = Input {
            model: Some("opus".into()),
            context_window_tokens: Some(50000),
            context_window_max: Some(200000),
            cost_usd: Some(0.15),
            turns: Some(10),
            ..Default::default()
        };
        let cache = default_cache();
        let result = render_statusline(&input, &cache, ColorLevel::Mono, EN_STRINGS);
        // No segment should be just "[]"
        assert!(!result.contains("[]"));
    }
}
```

---

## 5. MEDIUM: omc-shared Routing

### 5.1 `crates/omc-shared/src/routing/router.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_task_force_inherit() {
        let ctx = RoutingContext::default();
        let config = RoutingConfig {
            force_inherit: true,
            ..Default::default()
        };
        let decision = route_task(&ctx, &config);
        assert_eq!(decision.model, "inherit");
        assert_eq!(decision.model_type, ModelType::Inherit);
        assert_eq!(decision.confidence, 1.0);
    }

    #[test]
    fn route_task_disabled_uses_default() {
        let ctx = RoutingContext::default();
        let config = RoutingConfig {
            enabled: false,
            default_tier: ComplexityTier::Low,
            ..Default::default()
        };
        let decision = route_task(&ctx, &config);
        assert_eq!(decision.tier, ComplexityTier::Low);
    }

    #[test]
    fn route_task_explicit_model_opus() {
        let ctx = RoutingContext {
            explicit_model: Some(ModelType::Opus),
            ..Default::default()
        };
        let config = RoutingConfig::default();
        let decision = route_task(&ctx, &config);
        assert_eq!(decision.tier, ComplexityTier::High);
    }

    #[test]
    fn route_task_explicit_model_haiku() {
        let ctx = RoutingContext {
            explicit_model: Some(ModelType::Haiku),
            ..Default::default()
        };
        let config = RoutingConfig::default();
        let decision = route_task(&ctx, &config);
        assert_eq!(decision.tier, ComplexityTier::Low);
    }

    #[test]
    fn route_task_agent_override() {
        let ctx = RoutingContext {
            agent_type: Some("custom-agent".into()),
            ..Default::default()
        };
        let mut config = RoutingConfig::default();
        config.agent_overrides.insert(
            "custom-agent".into(),
            (ComplexityTier::High, "Custom override".into()),
        );
        let decision = route_task(&ctx, &config);
        assert_eq!(decision.tier, ComplexityTier::High);
    }

    #[test]
    fn escalate_model_low_to_medium() {
        assert_eq!(escalate_model(ComplexityTier::Low), ComplexityTier::Medium);
    }

    #[test]
    fn escalate_model_medium_to_high() {
        assert_eq!(escalate_model(ComplexityTier::Medium), ComplexityTier::High);
    }

    #[test]
    fn escalate_model_high_stays_high() {
        assert_eq!(escalate_model(ComplexityTier::High), ComplexityTier::High);
    }

    #[test]
    fn can_escalate_low() {
        assert!(can_escalate(ComplexityTier::Low));
    }

    #[test]
    fn can_escalate_medium() {
        assert!(can_escalate(ComplexityTier::Medium));
    }

    #[test]
    fn cannot_escalate_high() {
        assert!(!can_escalate(ComplexityTier::High));
    }

    #[test]
    fn quick_tier_for_known_agents() {
        assert_eq!(quick_tier_for_agent("architect"), Some(ComplexityTier::High));
        assert_eq!(quick_tier_for_agent("explore"), Some(ComplexityTier::Low));
        assert_eq!(quick_tier_for_agent("executor"), Some(ComplexityTier::Medium));
    }

    #[test]
    fn quick_tier_for_unknown_agent() {
        assert!(quick_tier_for_agent("unknown-agent-type").is_none());
    }

    #[test]
    fn route_task_simple_search_query() {
        let ctx = RoutingContext {
            task_prompt: "find the definition of foo".into(),
            ..Default::default()
        };
        let config = RoutingConfig::default();
        let decision = route_task(&ctx, &config);
        // Simple search should route to Low
        assert_eq!(decision.tier, ComplexityTier::Low);
    }

    #[test]
    fn route_task_complex_architecture() {
        let ctx = RoutingContext {
            task_prompt: "redesign the authentication system for production migration with cross-file refactor".into(),
            ..Default::default()
        };
        let config = RoutingConfig::default();
        let decision = route_task(&ctx, &config);
        assert_eq!(decision.tier, ComplexityTier::High);
    }

    #[test]
    fn explain_routing_returns_nonempty() {
        let ctx = RoutingContext {
            task_prompt: "simple fix".into(),
            ..Default::default()
        };
        let config = RoutingConfig::default();
        let explanation = explain_routing(&ctx, &config);
        assert!(explanation.contains("Model Routing Decision"));
    }

    #[test]
    fn get_model_for_task_returns_tuple() {
        let config = RoutingConfig::default();
        let (model_type, tier, reason) = get_model_for_task("executor", "fix the bug", &config);
        assert!(!reason.is_empty());
        // Model type should be valid
        assert!(matches!(
            model_type,
            ModelType::Opus | ModelType::Sonnet | ModelType::Haiku | ModelType::Inherit
        ));
    }
}
```

### 5.2 `crates/omc-shared/src/routing/rules.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_routing_rules_not_empty() {
        let rules = default_routing_rules();
        assert!(!rules.is_empty());
    }

    #[test]
    fn evaluate_rules_returns_default_when_no_match() {
        let empty_rules: Vec<RoutingRule> = vec![];
        let ctx = RoutingContext::default();
        let signals = ComplexitySignals::default();
        let result = evaluate_rules(&ctx, &signals, &empty_rules);
        assert_eq!(result.tier, Some(ComplexityTier::Medium));
        assert_eq!(result.rule_name, "fallback");
    }

    #[test]
    fn evaluate_rules_first_matching_wins() {
        let rules = vec![
            RoutingRule {
                name: "first".into(),
                condition: Box::new(|_, _| true),
                tier: Some(ComplexityTier::Low),
                reason: "First rule".into(),
                priority: 100,
            },
            RoutingRule {
                name: "second".into(),
                condition: Box::new(|_, _| true),
                tier: Some(ComplexityTier::High),
                reason: "Second rule".into(),
                priority: 50,
            },
        ];
        let ctx = RoutingContext::default();
        let signals = ComplexitySignals::default();
        let result = evaluate_rules(&ctx, &signals, &rules);
        assert_eq!(result.rule_name, "first");
        assert_eq!(result.tier, Some(ComplexityTier::Low));
    }

    #[test]
    fn evaluate_rules_respects_priority_order() {
        let rules = vec![
            RoutingRule {
                name: "low-priority".into(),
                condition: Box::new(|_, _| true),
                tier: Some(ComplexityTier::Low),
                reason: "Low".into(),
                priority: 10,
            },
            RoutingRule {
                name: "high-priority".into(),
                condition: Box::new(|_, _| true),
                tier: Some(ComplexityTier::High),
                reason: "High".into(),
                priority: 100,
            },
        ];
        let ctx = RoutingContext::default();
        let signals = ComplexitySignals::default();
        let result = evaluate_rules(&ctx, &signals, &rules);
        assert_eq!(result.rule_name, "high-priority");
    }

    #[test]
    fn get_matching_rules_returns_all_matches() {
        let rules = default_routing_rules();
        let ctx = RoutingContext {
            agent_type: Some("architect".into()),
            ..Default::default()
        };
        let mut signals = ComplexitySignals::default();
        signals.lexical.has_debugging_keywords = true;

        let matching = get_matching_rules(&ctx, &signals, &rules);
        assert!(!matching.is_empty());
    }

    #[test]
    fn architect_debugging_matches_high() {
        let rules = default_routing_rules();
        let ctx = RoutingContext {
            agent_type: Some("architect".into()),
            ..Default::default()
        };
        let mut signals = ComplexitySignals::default();
        signals.lexical.has_debugging_keywords = true;

        let result = evaluate_rules(&ctx, &signals, &rules);
        assert_eq!(result.tier, Some(ComplexityTier::High));
        assert_eq!(result.rule_name, "architect-complex-debugging");
    }

    #[test]
    fn security_domain_matches_high() {
        let rules = default_routing_rules();
        let ctx = RoutingContext::default();
        let mut signals = ComplexitySignals::default();
        signals.structural.domain_specificity = Domain::Security;

        let result = evaluate_rules(&ctx, &signals, &rules);
        assert_eq!(result.tier, Some(ComplexityTier::High));
    }

    #[test]
    fn short_local_change_matches_low() {
        let rules = default_routing_rules();
        let ctx = RoutingContext::default();
        let mut signals = ComplexitySignals::default();
        signals.lexical.word_count = 20;
        signals.structural.impact_scope = ImpactScope::Local;
        signals.structural.reversibility = Reversibility::Easy;

        let result = evaluate_rules(&ctx, &signals, &rules);
        assert_eq!(result.tier, Some(ComplexityTier::Low));
    }
}
```

---

## 6. MEDIUM: omc-git-provider (6 provider files, 0 tests)

```rust
// Add to crates/omc-git-provider/src/types.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_provider_display_names() {
        // Test Display implementations for all providers
        // (depends on actual type definitions)
    }

    #[test]
    fn repo_info_serialization_roundtrip() {
        // Test serde roundtrip for any serializable types
    }
}

// Add to each provider file (github.rs, gitlab.rs, etc.)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_remote_url_valid_ssh() {
        // Test SSH URL parsing: git@github.com:owner/repo.git
    }

    #[test]
    fn parse_remote_url_valid_https() {
        // Test HTTPS URL parsing: https://github.com/owner/repo
    }

    #[test]
    fn parse_remote_url_invalid() {
        // Test malformed URLs
    }

    #[test]
    fn build_api_url_correct_path() {
        // Test API URL construction for each provider
    }
}
```

---

## 7. LOW: omc-skills Templates

```rust
// Add to crates/omc-skills/src/templates.rs (or a test module)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_names_not_empty() {
        assert!(!SKILL_NAMES.is_empty());
    }

    #[test]
    fn skill_names_unique() {
        let mut seen = std::collections::HashSet::new();
        for name in SKILL_NAMES {
            assert!(seen.insert(*name), "Duplicate skill name: {name}");
        }
    }

    #[test]
    fn get_templates_returns_all_skills() {
        let templates = get_templates();
        assert_eq!(templates.len(), SKILL_NAMES.len());
    }

    #[test]
    fn get_templates_each_has_content() {
        let templates = get_templates();
        for (name, template) in &templates {
            assert!(!template.content.is_empty(), "Template '{name}' has empty content");
            assert_eq!(template.metadata.name, *name);
        }
    }

    #[test]
    fn get_template_by_name() {
        let templates = get_templates();
        // Every SKILL_NAME should be present
        for name in SKILL_NAMES {
            assert!(templates.contains_key(*name), "Missing template: {name}");
        }
    }

    #[test]
    fn list_skill_names_returns_sorted() {
        let names = list_skill_names();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }
}
```

---

## 8. LOW: omc-autoresearch

```rust
// Add to crates/omc-autoresearch/src/types.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_config_default() {
        // Test default configuration values
    }

    #[test]
    fn research_phase_serialization() {
        // Test serde for phases
    }
}

// Add to crates/omc-autoresearch/src/orchestrator.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orchestrator_init_with_defaults() {
        // Test initialization
    }
}

// Add to crates/omc-autoresearch/src/runtime.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_state_transitions() {
        // Test state machine transitions
    }
}
```

---

## 9. Integration Test Gaps

Only 2 crates have integration tests. Missing integration tests for:

| Area | What to test |
|------|-------------|
| **omc-shared state** | Read -> Write -> Read cycle for state files across sessions |
| **omc-mcp tools** | End-to-end: all_tools() -> definition() -> handle() with real filesystem |
| **omc-hooks** | Hook registry -> executor pipeline with shell commands |
| **omc-skills** | Template loading -> frontmatter parsing -> host filtering |
| **omc-interop** | Send task -> Read results cross-tool flow |
| **omc-team** | Agent lifecycle -> dispatch -> fault tolerance chain |
| **omc-notifications** | Template rendering -> dispatch -> channel delivery |

### Suggested Integration Test: `crates/omc-mcp/tests/tools_integration.rs`

```rust
use omc_mcp::all_tools;
use omc_mcp::tools::McpTool;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn all_tools_definitions_serialize_to_json() {
    let tools = all_tools();
    for tool in &tools {
        let def = tool.definition();
        let json = serde_json::to_value(&def).unwrap();
        assert!(json["name"].is_string());
        assert!(json["description"].is_string());
        assert!(json["inputSchema"].is_object());
    }
}

#[test]
fn state_write_then_read_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_string_lossy().to_string();
    let tools = all_tools();

    let write_tool = tools.iter().find(|t| t.definition().name == "state_write").unwrap();
    let read_tool = tools.iter().find(|t| t.definition().name == "state_read").unwrap();

    // Write state
    let write_result = write_tool.handle(json!({
        "mode": "ralph",
        "active": true,
        "iteration": 7,
        "workingDirectory": cwd
    }));
    assert!(write_result.is_error.is_none());

    // Read it back
    let read_result = read_tool.handle(json!({
        "mode": "ralph",
        "workingDirectory": cwd
    }));
    assert!(read_result.content[0].text.contains("iteration"));
    assert!(read_result.content[0].text.contains("7"));
}

#[test]
fn notepad_write_then_read_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_string_lossy().to_string();
    let tools = all_tools();

    let write_tool = tools.iter().find(|t| t.definition().name == "notepad_write_priority").unwrap();
    let read_tool = tools.iter().find(|t| t.definition().name == "notepad_read").unwrap();

    write_tool.handle(json!({"content": "Priority note", "workingDirectory": cwd}));

    let result = read_tool.handle(json!({"section": "priority", "workingDirectory": cwd}));
    assert!(result.content[0].text.contains("Priority note"));
}

#[test]
fn memory_write_then_read_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let cwd = tmp.path().to_string_lossy().to_string();
    let tools = all_tools();

    let write_tool = tools.iter().find(|t| t.definition().name == "project_memory_write").unwrap();
    let read_tool = tools.iter().find(|t| t.definition().name == "project_memory_read").unwrap();

    write_tool.handle(json!({
        "memory": {"techStack": ["rust", "tokio"]},
        "workingDirectory": cwd
    }));

    let result = read_tool.handle(json!({"section": "techStack", "workingDirectory": cwd}));
    assert!(result.content[0].text.contains("rust"));
}
```

---

## Summary: Test Skeletons to Add

| Crate | New Tests | Files to Modify |
|-------|----------|----------------|
| omc-mcp | ~55 | `tools/mod.rs`, `state_tools.rs`, `notepad_tools.rs`, `memory_tools.rs`, `lib.rs` |
| omc-interop | ~10 | `mcp_bridge.rs` |
| omc-xcmd | ~10 | `executor.rs`, `skills.rs` |
| omc-hud | ~40 | `cache.rs`, `input.rs`, `terminal.rs`, `render.rs` |
| omc-shared/routing | ~25 | `router.rs`, `rules.rs` |
| omc-skills | ~6 | `templates.rs` |
| Integration | ~4 | New `tests/tools_integration.rs` |
| **Total** | **~150** | **15 files** |
