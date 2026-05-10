//! Integration tests for omc-macros derive(Tool).

#[cfg(test)]
mod tests {
    use omc_macros::Tool;
    use omc_shared::tools::tool_trait::{ExecResult, Tool, ToolParameters, ToolRiskLevel};

    // -----------------------------------------------------------------------
    // Example tool: read_file
    // -----------------------------------------------------------------------

    #[derive(Tool, Default, serde::Deserialize)]
    #[tool(
        name = "read_file",
        description = "Read file contents",
        risk = "ReadOnly"
    )]
    struct ReadFileTool {
        /// Path to the file
        path: String,
        /// Optional line range to read
        line_range: Option<(usize, usize)>,
    }

    impl ReadFileTool {
        async fn run(&self) -> anyhow::Result<ExecResult> {
            Ok(ExecResult::ok(format!(
                "Read {} (range: {:?})",
                self.path, self.line_range
            )))
        }
    }

    // -----------------------------------------------------------------------
    // Example tool: delete_file (Dangerous)
    // -----------------------------------------------------------------------

    #[derive(Tool, Default, serde::Deserialize)]
    #[tool(
        name = "delete_file",
        description = "Delete a file permanently",
        risk = "Dangerous"
    )]
    struct DeleteFileTool {
        /// File path to delete
        path: String,
        /// Skip confirmation
        force: bool,
    }

    impl DeleteFileTool {
        async fn run(&self) -> anyhow::Result<ExecResult> {
            if self.force {
                Ok(ExecResult::ok(format!("Deleted {}", self.path)))
            } else {
                Ok(ExecResult::err("Confirmation required"))
            }
        }
    }

    // -----------------------------------------------------------------------
    // Example tool: search (Standard, default risk)
    // -----------------------------------------------------------------------

    #[derive(Tool, Default, serde::Deserialize)]
    #[tool(name = "search", description = "Search codebase")]
    struct SearchTool {
        /// Search query
        query: String,
        /// File glob pattern
        glob: Option<String>,
        /// Max results
        limit: Option<u32>,
    }

    impl SearchTool {
        async fn run(&self) -> anyhow::Result<ExecResult> {
            Ok(ExecResult::ok(format!(
                "Searched: {} (glob: {:?}, limit: {:?})",
                self.query, self.glob, self.limit
            )))
        }
    }

    // -----------------------------------------------------------------------
    // Example tool with Vec field
    // -----------------------------------------------------------------------

    #[derive(Tool, Default, serde::Deserialize)]
    #[tool(name = "batch", description = "Run batch operations")]
    struct BatchTool {
        /// List of commands
        commands: Vec<String>,
    }

    impl BatchTool {
        async fn run(&self) -> anyhow::Result<ExecResult> {
            Ok(ExecResult::ok(format!(
                "Ran {} commands",
                self.commands.len()
            )))
        }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[test]
    fn name_and_description() {
        let tool = ReadFileTool::default();
        assert_eq!(tool.name(), "read_file");
        assert_eq!(tool.description(), "Read file contents");
    }

    #[test]
    fn risk_level_read_only() {
        let tool = ReadFileTool::default();
        assert_eq!(tool.risk_level(), ToolRiskLevel::ReadOnly);
    }

    #[test]
    fn risk_level_dangerous() {
        let tool = DeleteFileTool::default();
        assert_eq!(tool.risk_level(), ToolRiskLevel::Dangerous);
    }

    #[test]
    fn risk_level_default_standard() {
        let tool = SearchTool::default();
        assert_eq!(tool.risk_level(), ToolRiskLevel::Standard);
    }

    #[test]
    fn parameters_schema_has_required_fields() {
        let tool = ReadFileTool::default();
        let schema = tool.parameters();

        assert_eq!(schema["type"], "object");
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(required.contains(&"path"));
        assert!(!required.contains(&"line_range")); // Option is optional
    }

    #[test]
    fn parameters_schema_field_types() {
        let tool = ReadFileTool::default();
        let schema = tool.parameters();
        let props = &schema["properties"];

        assert_eq!(props["path"]["type"], "string");
        // Option<(usize, usize)> → array type (tuple = array)
        assert_eq!(props["line_range"]["type"], "array");
    }

    #[test]
    fn parameters_schema_vec_field() {
        let tool = BatchTool::default();
        let schema = tool.parameters();
        let props = &schema["properties"];

        assert_eq!(props["commands"]["type"], "array");
        assert_eq!(props["commands"]["items"]["type"], "string");
    }

    #[test]
    fn parameters_schema_descriptions_from_doc_comments() {
        let tool = ReadFileTool::default();
        let schema = tool.parameters();
        let props = &schema["properties"];

        assert_eq!(props["path"]["description"], "Path to the file");
        assert_eq!(
            props["line_range"]["description"],
            "Optional line range to read"
        );
    }

    #[tokio::test]
    async fn execute_with_valid_params() {
        let tool = ReadFileTool::default();
        let mut params = ToolParameters::new();
        params.insert("path".to_string(), serde_json::json!("src/main.rs"));
        let result = tool.execute(params).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("src/main.rs"));
    }

    #[tokio::test]
    async fn execute_with_optional_params() {
        let tool = ReadFileTool::default();
        let mut params = ToolParameters::new();
        params.insert("path".to_string(), serde_json::json!("lib.rs"));
        params.insert("line_range".to_string(), serde_json::json!([10, 20]));
        let result = tool.execute(params).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("10"));
    }

    #[tokio::test]
    async fn execute_missing_required_param_fails() {
        let tool = ReadFileTool::default();
        let params = ToolParameters::new(); // missing "path"
        let result = tool.execute(params).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("parameter deserialization failed")
        );
    }

    #[tokio::test]
    async fn execute_bool_field() {
        let tool = DeleteFileTool::default();
        let mut params = ToolParameters::new();
        params.insert("path".to_string(), serde_json::json!("tmp.txt"));
        params.insert("force".to_string(), serde_json::json!(true));
        let result = tool.execute(params).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("Deleted"));
    }

    #[tokio::test]
    async fn execute_vec_field() {
        let tool = BatchTool::default();
        let mut params = ToolParameters::new();
        params.insert(
            "commands".to_string(),
            serde_json::json!(["echo hello", "echo world"]),
        );
        let result = tool.execute(params).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("2 commands"));
    }

    #[tokio::test]
    async fn execute_optional_none_works() {
        let tool = SearchTool::default();
        let mut params = ToolParameters::new();
        params.insert("query".to_string(), serde_json::json!("TODO"));
        // glob and limit are omitted (None)
        let result = tool.execute(params).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("glob: None"));
    }
}
