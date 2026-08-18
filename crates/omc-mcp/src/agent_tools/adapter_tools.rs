use super::*;

pub struct WorkflowAdvanceTool;

impl McpTool for WorkflowAdvanceTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "currentStage".into(),
            string_property("Current workflow stage."),
        );
        for (name, description) in [
            (
                "requirementsClarified",
                "Requirements interview is complete.",
            ),
            (
                "allTasksAssigned",
                "The host/team has assigned the plan tasks.",
            ),
            ("planApproved", "The plan has passed its approval gate."),
            (
                "allTasksCompleted",
                "The host/team reports all execution tasks complete.",
            ),
            ("verificationPassed", "The verification evidence passed."),
            (
                "hasFailures",
                "A current execution or verification failure exists.",
            ),
            ("hasBlockers", "A blocker requires host or human input."),
        ] {
            properties.insert(name.into(), boolean_property(description));
        }
        properties.insert(
            "fixAttempts".into(),
            number_property("Number of fix attempts already consumed."),
        );
        properties.insert(
            "maxFixAttempts".into(),
            number_property("Maximum fix attempts before the workflow fails."),
        );
        properties.insert(
            "requestId".into(),
            string_property("Optional caller correlation ID."),
        );
        ToolDefinition {
            name: "workflow_advance".into(),
            description: "Advance a clarify-plan-execute-verify workflow only from supplied host evidence; it never starts an agent or task executor.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["currentStage".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let request_id = normalize_request_id(
            args.get("requestId").and_then(Value::as_str),
            "mcp-workflow-advance",
        );
        let request: WorkflowAdvanceRequest = match serde_json::from_value(args) {
            Ok(request) => request,
            Err(error) => {
                return encode(
                    &ToolResponse::<Value>::failure(
                        request_id,
                        ToolError::new("invalid_request", error.to_string()),
                    ),
                    true,
                );
            }
        };
        encode(
            &ToolResponse::success(request_id, advance_workflow(&request)),
            false,
        )
    }
}

#[derive(Default)]
pub struct LspDocumentSymbolsTool {
    pool: LspProjectPool,
}

impl McpTool for LspDocumentSymbolsTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "workingDirectory".into(),
            string_property("Project root used to bound the file path."),
        );
        properties.insert(
            "file".into(),
            string_property("Project-relative Rust source file."),
        );
        properties.insert(
            "timeoutMs".into(),
            SchemaProperty {
                prop_type: "integer".into(),
                description: Some("Request timeout in milliseconds (5000..60000).".into()),
                r#enum: None,
                max_length: None,
                minimum: Some(5000),
                maximum: Some(60000),
            },
        );
        properties.insert(
            "requestId".into(),
            string_property("Optional caller correlation ID."),
        );
        ToolDefinition {
            name: "lsp_document_symbols".into(),
            description: "Read Rust document symbols through a bounded project-scoped rust-analyzer pool owned by this MCP process.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["file".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let request_id = normalize_request_id(
            args.get("requestId").and_then(Value::as_str),
            "mcp-lsp-document-symbols",
        );
        let request: LspDocumentSymbolsRequest = match serde_json::from_value(args) {
            Ok(request) => request,
            Err(error) => {
                return encode(
                    &ToolResponse::<Value>::failure(
                        request_id,
                        ToolError::new("invalid_request", error.to_string()),
                    ),
                    true,
                );
            }
        };
        match self.pool.query_document_symbols(&request) {
            Ok(payload) => encode(&ToolResponse::success(request_id, payload), false),
            Err(error) => encode(&ToolResponse::<Value>::failure(request_id, error), true),
        }
    }
}

pub struct DebugInspectTool;

impl McpTool for DebugInspectTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "adapterCommand".into(),
            string_property("External DAP adapter executable or command name."),
        );
        properties.insert(
            "adapterArgs".into(),
            array_property("Arguments passed directly to the external adapter, without a shell."),
        );
        properties.insert(
            "workingDirectory".into(),
            string_property("Existing adapter working directory."),
        );
        properties.insert(
            "mode".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Explicit DAP session mode.".into()),
                r#enum: Some(vec!["launch".into(), "attach".into()]),
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "action".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Read-only DAP inspection action.".into()),
                r#enum: Some(vec![
                    "threads".into(),
                    "stackTrace".into(),
                    "scopes".into(),
                    "variables".into(),
                    "modules".into(),
                    "loadedSources".into(),
                    "output".into(),
                ]),
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        for name in ["launchArguments", "attachArguments"] {
            properties.insert(
                name.into(),
                SchemaProperty {
                    prop_type: "object".into(),
                    description: Some(
                        "Adapter-specific JSON object; the selected mode must provide one.".into(),
                    ),
                    r#enum: None,
                    max_length: None,
                    minimum: None,
                    maximum: None,
                },
            );
        }
        for (name, description) in [
            ("threadId", "Thread ID required by stackTrace."),
            ("frameId", "Frame ID required by scopes."),
            (
                "variablesReference",
                "Variables reference required by variables.",
            ),
        ] {
            properties.insert(
                name.into(),
                SchemaProperty {
                    prop_type: "integer".into(),
                    description: Some(description.into()),
                    r#enum: None,
                    max_length: None,
                    minimum: Some(1),
                    maximum: None,
                },
            );
        }
        properties.insert(
            "timeoutMs".into(),
            SchemaProperty {
                prop_type: "integer".into(),
                description: Some("DAP request timeout in milliseconds (5000..300000).".into()),
                r#enum: None,
                max_length: None,
                minimum: Some(5000),
                maximum: Some(300000),
            },
        );
        properties.insert(
            "allowSideEffects".into(),
            boolean_property("Required explicit opt-in for launch or attach."),
        );
        properties.insert(
            "requestId".into(),
            string_property("Optional caller correlation ID."),
        );

        ToolDefinition {
            name: "debug_inspect".into(),
            description: "Launch or attach one external stdio DAP session and return only bounded read-only inspection results; OMC-RS does not install or implement the debugger adapter.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec![
                    "adapterCommand".into(),
                    "mode".into(),
                    "action".into(),
                    "allowSideEffects".into(),
                ],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let request_id = normalize_request_id(
            args.get("requestId").and_then(Value::as_str),
            "mcp-debug-inspect",
        );
        let request: DebugInspectRequest = match serde_json::from_value(args) {
            Ok(request) => request,
            Err(error) => {
                return encode(
                    &ToolResponse::<Value>::failure(
                        request_id,
                        ToolError::new("invalid_request", error.to_string()),
                    ),
                    true,
                );
            }
        };
        match inspect_debug(&request) {
            Ok(payload) => encode(&ToolResponse::success(request_id, payload), false),
            Err(error) => encode(&ToolResponse::<Value>::failure(request_id, error), true),
        }
    }
}
