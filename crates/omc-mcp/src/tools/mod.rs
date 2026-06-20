//! Tool trait and common types for MCP tool definitions.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

/// MCP content block returned by tool handlers.
#[derive(Debug, Clone, Serialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

impl ToolContent {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content_type: "text".into(),
            text: text.into(),
        }
    }
}

/// Result returned by a tool handler.
#[derive(Debug, Clone, Serialize)]
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ToolResult {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            is_error: None,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            is_error: Some(true),
        }
    }
}

/// Schema property definition for tool input.
#[derive(Debug, Clone, Serialize)]
pub struct SchemaProperty {
    #[serde(rename = "type")]
    pub prop_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#enum: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<i64>,
}

/// Tool input schema.
#[derive(Debug, Clone, Serialize)]
pub struct ToolSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: HashMap<String, SchemaProperty>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub required: Vec<String>,
}

/// Definition of an MCP tool.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: ToolSchema,
}

/// Trait for MCP tool implementations.
pub trait McpTool: Send + Sync {
    /// Return the tool definition (name, description, schema).
    fn definition(&self) -> ToolDefinition;

    /// Handle a tool call with the given arguments.
    fn handle(&self, args: Value) -> ToolResult;
}
