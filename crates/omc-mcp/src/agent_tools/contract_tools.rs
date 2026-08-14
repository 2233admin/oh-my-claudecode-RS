use super::*;

pub struct SubagentResultValidateTool;

impl McpTool for SubagentResultValidateTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "resultType".into(),
            string_property("Named result schema ID."),
        );
        properties.insert(
            "payload".into(),
            SchemaProperty {
                prop_type: "object".into(),
                description: Some("Structured result payload.".into()),
                r#enum: None,
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "requiredFields".into(),
            array_property("Required top-level payload fields."),
        );
        properties.insert(
            "requestId".into(),
            string_property("Optional caller correlation ID."),
        );
        ToolDefinition {
            name: "subagent_result_validate".into(),
            description: "Validate a typed subagent result without running an agent or provider."
                .into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["resultType".into(), "payload".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let request_id = normalize_request_id(
            args.get("requestId").and_then(Value::as_str),
            "mcp-result-validate",
        );
        let result_type = match args
            .get("resultType")
            .and_then(Value::as_str)
            .filter(|v| !v.trim().is_empty())
        {
            Some(value) => value,
            None => {
                return encode(
                    &ToolResponse::<Value>::failure(
                        request_id,
                        ToolError::new("invalid_request", "resultType is required"),
                    ),
                    true,
                );
            }
        };
        let payload = match args.get("payload") {
            Some(value) if value.is_object() => value.clone(),
            _ => {
                return encode(
                    &ToolResponse::<Value>::failure(
                        request_id,
                        ToolError::new("invalid_request", "payload must be an object"),
                    ),
                    true,
                );
            }
        };
        let required_fields = match args.get("requiredFields") {
            Some(value) => match serde_json::from_value::<Vec<String>>(value.clone()) {
                Ok(fields) => fields,
                Err(error) => {
                    return encode(
                        &ToolResponse::<Value>::failure(
                            request_id,
                            ToolError::new("invalid_request", error.to_string()),
                        ),
                        true,
                    );
                }
            },
            None => Vec::new(),
        };
        let schema = ResultSchema::new(result_type, required_fields);
        let result = match TypedSubagentResult::new(schema.schema_id.clone(), payload, Vec::new()) {
            Ok(result) => result,
            Err(error) => {
                return encode(
                    &ToolResponse::<Value>::failure(
                        request_id,
                        ToolError::new("invalid_request", error),
                    ),
                    true,
                );
            }
        };
        match result.validate_against(&schema) {
            Ok(()) => encode(&ToolResponse::success(request_id, result), false),
            Err(error) => encode(
                &ToolResponse::<Value>::failure(
                    request_id,
                    ToolError::new("invalid_request", error),
                ),
                true,
            ),
        }
    }
}

pub struct HashEditTool;

impl McpTool for HashEditTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert("workingDirectory".into(), string_property("Project root."));
        properties.insert(
            "path".into(),
            string_property("Portable project-relative file path."),
        );
        properties.insert(
            "startLine".into(),
            number_property("First 1-based line to replace."),
        );
        properties.insert(
            "endLine".into(),
            number_property("Last 1-based line to replace."),
        );
        properties.insert(
            "anchors".into(),
            array_property("One SHA-256 anchor per line in the range."),
        );
        properties.insert("replacement".into(), string_property("Replacement text."));
        properties.insert(
            "expectedFileSha256".into(),
            string_property("Optional whole-file SHA-256 precondition."),
        );
        properties.insert(
            "requestId".into(),
            string_property("Optional caller correlation ID."),
        );
        ToolDefinition {
            name: "hash_edit".into(),
            description:
                "Apply a hash-anchored edit atomically; stale anchors are rejected before writing."
                    .into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec![
                    "path".into(),
                    "startLine".into(),
                    "endLine".into(),
                    "anchors".into(),
                    "replacement".into(),
                ],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let request_id = normalize_request_id(
            args.get("requestId").and_then(Value::as_str),
            "mcp-hash-edit",
        );
        let root = args
            .get("workingDirectory")
            .and_then(Value::as_str)
            .unwrap_or(".");
        let edit = match serde_json::from_value::<HashEdit>(serde_json::json!({
            "schemaVersion": omc_shared::hash_edit::HASH_EDIT_SCHEMA_VERSION,
            "path": args.get("path"),
            "startLine": args.get("startLine"),
            "endLine": args.get("endLine"),
            "anchors": args.get("anchors"),
            "replacement": args.get("replacement"),
            "expectedFileSha256": args.get("expectedFileSha256")
        })) {
            Ok(edit) => edit,
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
        match edit.apply(std::path::Path::new(root)) {
            Ok(result) => encode(&ToolResponse::success(request_id, result), false),
            Err(error) => {
                let code = if matches!(
                    error,
                    HashEditError::StaleAnchor { .. } | HashEditError::StaleFile { .. }
                ) {
                    "stale_edit"
                } else {
                    "edit_failed"
                };
                encode(
                    &ToolResponse::<Value>::failure(
                        request_id,
                        ToolError::new(code, error.to_string()),
                    ),
                    true,
                )
            }
        }
    }
}

pub struct CodeIntelArtifactQueryTool;

impl McpTool for CodeIntelArtifactQueryTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert("repo".into(), string_property("Code Intel repository key."));
        properties.insert(
            "artifactRoot".into(),
            string_property("Published Code Intel artifact root."),
        );
        properties.insert(
            "repoPath".into(),
            string_property("Optional checkout used for freshness evaluation."),
        );
        properties.insert(
            "artifactSchema".into(),
            string_property("Optional artifact schema filter."),
        );
        properties.insert(
            "artifactType".into(),
            string_property("Optional artifact type filter."),
        );
        properties.insert(
            "contains".into(),
            string_property("Optional text filter applied by Code Intel."),
        );
        properties.insert(
            "artifactUri".into(),
            string_property(
                "Optional canonical omc://artifact/sha256/<digest> for bounded exact inspection.",
            ),
        );
        properties.insert(
            "limit".into(),
            SchemaProperty {
                prop_type: "integer".into(),
                description: Some("Maximum number of matches, from 1 to 100.".into()),
                r#enum: None,
                max_length: None,
                minimum: Some(1),
                maximum: Some(100),
            },
        );
        properties.insert(
            "requestId".into(),
            string_property("Optional caller correlation ID."),
        );

        ToolDefinition {
            name: "code_intel_artifact_query".into(),
            description: "Read committed Code Intel artifacts through the released read-only query surface; artifactUri enables bounded exact inspection. OMC does not reimplement the scanner or artifact index.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["repo".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let request_id = normalize_request_id(
            args.get("requestId").and_then(Value::as_str),
            "mcp-code-intel",
        );
        let request: CodeIntelQueryRequest = match serde_json::from_value(args) {
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
        match query_code_intel(&request) {
            Ok(payload) => encode(&ToolResponse::success(request_id, payload), false),
            Err(error) => encode(&ToolResponse::<Value>::failure(request_id, error), true),
        }
    }
}
