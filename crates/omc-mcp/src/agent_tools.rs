//! Host-neutral OMC-RS agent tools exposed over MCP.

use std::collections::HashMap;

use omc_shared::agent_tool::{
    RouteRequest, ToolError, ToolResponse, capabilities_payload, normalize_request_id,
    route_agent_task,
};
use omc_shared::code_intel::{CodeIntelQueryRequest, query_code_intel};
use omc_shared::dap_adapter::{DebugInspectRequest, inspect_debug};
use omc_shared::hash_edit::{HashEdit, HashEditError};
use omc_shared::lsp_adapter::{LspDocumentSymbolsRequest, LspProjectPool};
use omc_shared::operation_contract::{ResultSchema, TypedSubagentResult};
use omc_shared::workflow_contract::{WorkflowAdvanceRequest, advance_workflow};
use serde::Serialize;
use serde_json::Value;

use crate::tools::{McpTool, SchemaProperty, ToolDefinition, ToolResult, ToolSchema};

mod adapter_tools;
pub use adapter_tools::{DebugInspectTool, LspDocumentSymbolsTool, WorkflowAdvanceTool};
mod contract_tools;
pub use contract_tools::{CodeIntelArtifactQueryTool, HashEditTool, SubagentResultValidateTool};

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

fn number_property(description: &str) -> SchemaProperty {
    SchemaProperty {
        prop_type: "number".into(),
        description: Some(description.into()),
        r#enum: None,
        max_length: None,
        minimum: Some(0),
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

fn array_property(description: &str) -> SchemaProperty {
    SchemaProperty {
        prop_type: "array".into(),
        description: Some(description.into()),
        r#enum: None,
        max_length: None,
        minimum: None,
        maximum: None,
    }
}

fn encode<T: Serialize>(response: &ToolResponse<T>, is_error: bool) -> ToolResult {
    match serde_json::to_string(response) {
        Ok(text) if is_error => ToolResult::error(text),
        Ok(text) => ToolResult::ok(text),
        Err(error) => ToolResult::error(format!("failed to encode tool response: {error}")),
    }
}

pub struct AgentCapabilitiesTool;

impl McpTool for AgentCapabilitiesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "agent_capabilities".into(),
            description: "List the stable, host-neutral OMC-RS capability contract. Call this before selecting other OMC tools.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties: HashMap::new(),
                required: vec![],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let request_id = normalize_request_id(
            args.get("requestId").and_then(Value::as_str),
            "mcp-capabilities",
        );
        encode(
            &ToolResponse::success(request_id, capabilities_payload()),
            false,
        )
    }
}

pub struct AgentRouteTool;

impl McpTool for AgentRouteTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert("task".into(), string_property("Task text to route."));
        properties.insert(
            "agentType".into(),
            string_property("Optional role hint such as explore, executor, or architect."),
        );
        properties.insert(
            "previousFailures".into(),
            number_property("Number of previous failures for the same task."),
        );
        properties.insert(
            "requestId".into(),
            string_property("Optional caller correlation ID."),
        );

        ToolDefinition {
            name: "agent_route".into(),
            description: "Route a task progressively. The result exposes a semantic tier and recommended surface, never a provider-specific model ID.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["task".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let request_id =
            normalize_request_id(args.get("requestId").and_then(Value::as_str), "mcp-route");
        let request: RouteRequest = match serde_json::from_value::<RouteRequest>(args) {
            Ok(request) if !request.task.trim().is_empty() => request,
            Ok(_) => {
                return encode(
                    &ToolResponse::<Value>::failure(
                        request_id,
                        ToolError::new("invalid_request", "task must not be empty"),
                    ),
                    true,
                );
            }
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
            &ToolResponse::success(request_id, route_agent_task(&request)),
            false,
        )
    }
}

pub fn agent_tools() -> Vec<Box<dyn McpTool>> {
    vec![
        Box::new(AgentCapabilitiesTool),
        Box::new(AgentRouteTool),
        Box::new(CodeIntelArtifactQueryTool),
        Box::new(LspDocumentSymbolsTool::default()),
        Box::new(DebugInspectTool),
        Box::new(WorkflowAdvanceTool),
        Box::new(SubagentResultValidateTool),
        Box::new(HashEditTool),
    ]
}

#[cfg(test)]
#[path = "agent_tools_tests.rs"]
mod tests;
