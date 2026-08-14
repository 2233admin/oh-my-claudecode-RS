//! Read-only projections of the existing omc-team observability state.

use std::collections::HashMap;
use std::path::PathBuf;

use omc_interop::mcp_bridge::{
    INTEROP_BRIDGE_SCHEMA_VERSION, InteropBridgeRequest, interop_bridge,
};
use omc_interop::read_snapshot;
use omc_shared::agent_tool::{
    INTEROP_SNAPSHOT_SCHEMA_VERSION, ToolError, ToolResponse, normalize_request_id,
};
use omc_team::team_observability;
use serde::Serialize;
use serde_json::Value;

use crate::tools::{McpTool, SchemaProperty, ToolDefinition, ToolResult, ToolSchema};

const VIEWS: &[&str] = &["sessions", "top", "doctor"];

fn string_property(description: &str, values: Option<&[&str]>) -> SchemaProperty {
    SchemaProperty {
        prop_type: "string".into(),
        description: Some(description.into()),
        r#enum: values.map(|items| items.iter().map(|item| (*item).to_string()).collect()),
        max_length: None,
        minimum: None,
        maximum: None,
    }
}

fn encode<T: Serialize>(response: &ToolResponse<T>) -> ToolResult {
    match serde_json::to_string(response) {
        Ok(text) => ToolResult::ok(text),
        Err(error) => ToolResult::error(format!("failed to encode team observability: {error}")),
    }
}

pub struct TeamObservabilityTool;

impl McpTool for TeamObservabilityTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "view".into(),
            string_property("Read sessions, aggregate usage, or health.", Some(VIEWS)),
        );
        properties.insert(
            "workingDirectory".into(),
            string_property("Project root. Defaults to the current directory.", None),
        );
        properties.insert(
            "requestId".into(),
            string_property("Optional caller correlation ID.", None),
        );

        ToolDefinition {
            name: "team_observability".into(),
            description: "Read the existing omc-team sessions, top usage snapshot, or observability doctor report without starting or mutating a team.".into(),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec!["view".into()],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let request_id = normalize_request_id(
            args.get("requestId").and_then(Value::as_str),
            "mcp-team-observability",
        );
        let view = match args.get("view").and_then(Value::as_str) {
            Some(view) if VIEWS.contains(&view) => view,
            _ => {
                return encode(&ToolResponse::<Value>::failure(
                    request_id,
                    ToolError::new("invalid_request", "view must be sessions, top, or doctor"),
                ));
            }
        };
        let root = PathBuf::from(
            args.get("workingDirectory")
                .and_then(Value::as_str)
                .unwrap_or("."),
        );

        match team_observability(&root, view) {
            Ok(payload) => encode(&ToolResponse::success(request_id, payload)),
            Err(error) => encode(&ToolResponse::<Value>::failure(
                request_id,
                ToolError::new("upstream_failed", error),
            )),
        }
    }
}

pub fn team_tools() -> Vec<Box<dyn McpTool>> {
    vec![
        Box::new(TeamObservabilityTool),
        Box::new(InteropSnapshotTool),
        Box::new(InteropBridgeTool),
    ]
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

fn limit_property() -> SchemaProperty {
    SchemaProperty {
        prop_type: "integer".into(),
        description: Some("Maximum records returned per collection (1..100).".into()),
        r#enum: None,
        max_length: None,
        minimum: Some(1),
        maximum: Some(100),
    }
}

pub struct InteropSnapshotTool;

impl McpTool for InteropSnapshotTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "workingDirectory".into(),
            string_property(
                "Project root containing .omc and .omx state. Defaults to the current directory.",
                None,
            ),
        );
        properties.insert("limit".into(), limit_property());
        properties.insert(
            "requestId".into(),
            string_property("Optional caller correlation ID.", None),
        );

        ToolDefinition {
            name: "interop_snapshot".into(),
            description: format!(
                "Read a bounded, read-only OMC/OMX interop state snapshot ({INTEROP_SNAPSHOT_SCHEMA_VERSION}); never starts or mutates a runtime."
            ),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: Vec::new(),
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let request_id = normalize_request_id(
            args.get("requestId").and_then(Value::as_str),
            "mcp-interop-snapshot",
        );
        let limit = match args.get("limit") {
            None => None,
            Some(value) => match value.as_u64().and_then(|value| usize::try_from(value).ok()) {
                Some(limit) => Some(limit),
                None => {
                    return encode(&ToolResponse::<Value>::failure(
                        request_id,
                        ToolError::new("invalid_request", "limit must be an integer"),
                    ));
                }
            },
        };
        let root = args
            .get("workingDirectory")
            .and_then(Value::as_str)
            .unwrap_or(".");

        match read_snapshot(root, limit) {
            Ok(payload) => encode(&ToolResponse::success(request_id, payload)),
            Err(error) => encode(&ToolResponse::<Value>::failure(
                request_id,
                ToolError::new("invalid_request", error.to_string()),
            )),
        }
    }
}

pub struct InteropBridgeTool;

impl McpTool for InteropBridgeTool {
    fn definition(&self) -> ToolDefinition {
        let mut properties = HashMap::new();
        properties.insert(
            "action".into(),
            string_property(
                "Durable bridge operation.",
                Some(&["send_task", "send_message"]),
            ),
        );
        properties.insert(
            "source".into(),
            string_property("Source runtime.", Some(&["omc", "omx"])),
        );
        properties.insert(
            "target".into(),
            string_property("Target runtime.", Some(&["omc", "omx"])),
        );
        properties.insert(
            "type".into(),
            string_property(
                "Task type required for send_task.",
                Some(&["analyze", "implement", "review", "test", "custom"]),
            ),
        );
        properties.insert(
            "description".into(),
            string_property("Task description required for send_task.", None),
        );
        properties.insert(
            "content".into(),
            string_property("Message content required for send_message.", None),
        );
        properties.insert(
            "workingDirectory".into(),
            string_property("Project root containing .omc interop state.", None),
        );
        properties.insert(
            "allowSideEffects".into(),
            boolean_property("Must be true for the durable write to proceed."),
        );
        properties.insert(
            "requestId".into(),
            string_property("Optional caller correlation ID.", None),
        );

        ToolDefinition {
            name: "interop_bridge".into(),
            description: format!(
                "Send one explicit OMC/OMX task or message through {INTEROP_BRIDGE_SCHEMA_VERSION}; requires active interop flags and allowSideEffects=true, never starts workers."
            ),
            input_schema: ToolSchema {
                schema_type: "object".into(),
                properties,
                required: vec![
                    "action".into(),
                    "source".into(),
                    "target".into(),
                    "allowSideEffects".into(),
                ],
            },
        }
    }

    fn handle(&self, args: Value) -> ToolResult {
        let request_id = normalize_request_id(
            args.get("requestId").and_then(Value::as_str),
            "mcp-interop-bridge",
        );
        match serde_json::from_value::<InteropBridgeRequest>(args) {
            Ok(request) => match interop_bridge(&request) {
                Ok(payload) => encode(&ToolResponse::success(request_id, payload)),
                Err(error) => encode(&ToolResponse::<Value>::failure(
                    request_id,
                    ToolError::new(error.code(), error.to_string()),
                )),
            },
            Err(error) => encode(&ToolResponse::<Value>::failure(
                request_id,
                ToolError::new("invalid_request", error.to_string()),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_view_is_read_only_and_versioned() {
        let root = std::env::temp_dir().join(format!(
            "omc-team-observability-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let result = TeamObservabilityTool.handle(serde_json::json!({
            "view": "sessions",
            "workingDirectory": &root,
            "requestId": "team-test"
        }));
        let value: Value = serde_json::from_str(&result.content[0].text).unwrap();
        assert_eq!(value["schema_version"], "omc.tool.v1");
        assert_eq!(value["request_id"], "team-test");
        assert_eq!(value["data"]["schemaVersion"], "omc.team-observability.v1");
        assert_eq!(value["data"]["view"], "sessions");
        assert_eq!(value["data"]["data"], serde_json::json!([]));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unknown_view_is_machine_readable() {
        let result = TeamObservabilityTool.handle(serde_json::json!({"view": "start"}));
        let value: Value = serde_json::from_str(&result.content[0].text).unwrap();
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "invalid_request");
    }

    #[test]
    fn interop_snapshot_is_versioned_and_read_only() {
        let root =
            std::env::temp_dir().join(format!("omc-interop-snapshot-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let result = InteropSnapshotTool.handle(serde_json::json!({
            "workingDirectory": &root,
            "limit": 1,
            "requestId": "interop-test"
        }));
        let value: Value = serde_json::from_str(&result.content[0].text).unwrap();
        assert_eq!(value["schema_version"], "omc.tool.v1");
        assert_eq!(value["request_id"], "interop-test");
        assert_eq!(
            value["data"]["schemaVersion"],
            INTEROP_SNAPSHOT_SCHEMA_VERSION
        );
        assert_eq!(value["data"]["readOnly"], true);
        assert_eq!(value["data"]["sharedTaskCount"], 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interop_snapshot_rejects_invalid_limit() {
        let result = InteropSnapshotTool.handle(serde_json::json!({"limit": 0}));
        let value: Value = serde_json::from_str(&result.content[0].text).unwrap();
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "invalid_request");
    }

    #[test]
    fn interop_bridge_requires_explicit_side_effect_gate() {
        let result = InteropBridgeTool.handle(serde_json::json!({
            "action": "send_message",
            "source": "omc",
            "target": "omx",
            "content": "hello",
            "allowSideEffects": false,
            "requestId": "bridge-test"
        }));
        let value: Value = serde_json::from_str(&result.content[0].text).unwrap();
        assert_eq!(value["schema_version"], "omc.tool.v1");
        assert_eq!(value["request_id"], "bridge-test");
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "side_effects_not_allowed");
    }
}
