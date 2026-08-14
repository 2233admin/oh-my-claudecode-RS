//! Explicit-side-effect Python eval tool.

use std::collections::HashMap;
use std::path::Path;

use omc_python::{PythonReplService, PythonSessionError, PythonToolPayload, PythonToolRequest};
use omc_shared::PYTHON_SCHEMA_VERSION;
use omc_shared::agent_tool::{ToolError, ToolResponse, error_codes, normalize_request_id};
use serde_json::Value;

use crate::tools::{McpTool, SchemaProperty, ToolDefinition, ToolResult, ToolSchema};

pub struct PythonReplTool {
    service: PythonReplService,
}

impl Default for PythonReplTool {
    fn default() -> Self {
        Self {
            service: PythonReplService::new(),
        }
    }
}

pub fn python_tools() -> Vec<Box<dyn McpTool>> {
    vec![Box::new(PythonReplTool::default())]
}

impl McpTool for PythonReplTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "action".into(),
            SchemaProperty {
                prop_type: "string".into(),
                description: Some("Explicit operation for the Python session.".into()),
                r#enum: Some(vec![
                    "execute".into(),
                    "get_state".into(),
                    "reset".into(),
                    "interrupt".into(),
                ]),
                max_length: None,
                minimum: None,
                maximum: None,
            },
        );
        properties.insert(
            "sessionId".into(),
            string_property("Stable session key. State persists within this MCP process."),
        );
        properties.insert(
            "code".into(),
            string_property("Python cell source; required for execute."),
        );
        properties.insert(
            "projectDir".into(),
            string_property("Existing working directory for the Python subprocess."),
        );
        properties.insert(
            "executionTimeout".into(),
            SchemaProperty {
                prop_type: "integer".into(),
                description: Some("Execution timeout in milliseconds (1..300000).".into()),
                r#enum: None,
                max_length: None,
                minimum: Some(1),
                maximum: Some(300_000),
            },
        );
        properties.insert(
            "allowSideEffects".into(),
            boolean_property("Required for execute, reset, or interrupt."),
        );
        properties.insert(
            "requestId".into(),
            string_property("Optional caller correlation ID."),
        );
        ToolDefinition {
            name: "python_repl".into(),
            description: format!(
                "{PYTHON_SCHEMA_VERSION}: execute persistent local Python cells. This is not a sandbox; mutating actions require allowSideEffects=true."
            ),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["action".into(), "sessionId".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let request_id = normalize_request_id(
            args.get("requestId").and_then(Value::as_str),
            "mcp-python-repl",
        );
        let request: PythonToolRequest = match serde_json::from_value(args) {
            Ok(request) => request,
            Err(error) => {
                return encode_error(
                    request_id,
                    ToolError::new(error_codes::INVALID_REQUEST, error.to_string()),
                );
            }
        };
        let action = request.action.clone();
        let session_id = request.session_id.clone();
        let project_dir = request.project_dir.clone();
        let (input, allow_side_effects) = match request.into_input() {
            Ok(input) => input,
            Err(error) => return encode_error(request_id, map_error(error)),
        };
        if !matches!(action, omc_python::ReplAction::GetState) && !allow_side_effects {
            return encode_error(
                request_id,
                ToolError::new(
                    error_codes::SIDE_EFFECTS_NOT_ALLOWED,
                    "allowSideEffects=true is required for Python execution or session mutation",
                ),
            );
        }

        let result = match action {
            omc_python::ReplAction::Execute => self.service.execute(&input).map(|value| {
                serde_json::to_value(value)
                    .map_err(|error| PythonSessionError::Failed(error.to_string()))
            }),
            omc_python::ReplAction::GetState => self
                .service
                .state(&session_id, project_dir.as_deref())
                .map(|value| {
                    serde_json::to_value(value)
                        .map_err(|error| PythonSessionError::Failed(error.to_string()))
                }),
            omc_python::ReplAction::Reset => self
                .service
                .reset(&session_id, project_dir.as_deref())
                .map(|value| {
                    serde_json::to_value(value)
                        .map_err(|error| PythonSessionError::Failed(error.to_string()))
                }),
            omc_python::ReplAction::Interrupt => self
                .service
                .interrupt(&session_id, project_dir.as_deref())
                .map(|value| {
                    serde_json::to_value(value)
                        .map_err(|error| PythonSessionError::Failed(error.to_string()))
                }),
        };
        match result.and_then(|value| value) {
            Ok(result) => {
                let project_dir = project_dir
                    .as_deref()
                    .map(Path::new)
                    .and_then(|path| path.canonicalize().ok())
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
                    .to_string_lossy()
                    .into_owned();
                encode_success(
                    request_id,
                    PythonToolPayload {
                        operation: "python.eval",
                        action,
                        session_id,
                        project_dir,
                        session_scope: "mcp-process",
                        result,
                        side_effects: if allow_side_effects {
                            vec!["local Python code may read/write files, spawn processes, or use network".into()]
                        } else {
                            Vec::new()
                        },
                    },
                )
            }
            Err(error) => encode_error(request_id, map_error(error)),
        }
    }
}

fn string_property(description: &str) -> SchemaProperty {
    SchemaProperty {
        prop_type: "string".into(),
        description: Some(description.into()),
        r#enum: None,
        max_length: None,
        minimum: None,
        maximum: None,
    }
}

fn boolean_property(description: &str) -> SchemaProperty {
    SchemaProperty {
        prop_type: "boolean".into(),
        description: Some(description.into()),
        r#enum: None,
        max_length: None,
        minimum: None,
        maximum: None,
    }
}

fn map_error(error: PythonSessionError) -> ToolError {
    let code = match &error {
        PythonSessionError::InvalidRequest(_) => error_codes::INVALID_REQUEST,
        PythonSessionError::Unavailable(_) => error_codes::ADAPTER_UNAVAILABLE,
        PythonSessionError::Timeout(_) => "execution_timeout",
        PythonSessionError::Failed(_) => error_codes::UPSTREAM_FAILED,
    };
    ToolError::new(code, error.to_string())
}

fn encode_success<T: serde::Serialize>(request_id: String, value: T) -> ToolResult {
    match serde_json::to_string(&ToolResponse::success(request_id, value)) {
        Ok(text) => ToolResult::ok(text),
        Err(error) => ToolResult::error(error.to_string()),
    }
}

fn encode_error(request_id: String, error: ToolError) -> ToolResult {
    match serde_json::to_string(&ToolResponse::<Value>::failure(request_id, error)) {
        Ok(text) => ToolResult::error(text),
        Err(error) => ToolResult::error(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn python_tool_requires_side_effect_opt_in() {
        let result = PythonReplTool::default().handle(json!({
            "action": "execute",
            "sessionId": "test",
            "code": "print(1)"
        }));
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("side_effects_not_allowed"));
    }
}
