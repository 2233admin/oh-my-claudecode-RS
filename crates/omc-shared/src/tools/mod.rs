//! OMC MCP Tool implementations.
//!
//! Provides tool functions for LSP, AST, state management, notepad, and project memory.

pub mod ast_tools;
pub mod lsp_tools;
pub mod mcp_adapter;
pub mod memory_tools;
pub mod notepad_tools;
pub mod state_tools;
pub mod tool_trait;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during tool execution.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("State error: {0}")]
    State(String),
}

/// Content block returned by a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ToolResult {
    /// Create a successful text result.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent {
                content_type: "text".into(),
                text: text.into(),
            }],
            is_error: None,
        }
    }

    /// Create an error result.
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent {
                content_type: "text".into(),
                text: text.into(),
            }],
            is_error: Some(true),
        }
    }
}

/// Tool category for filtering and organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolCategory {
    Lsp,
    Ast,
    State,
    Memory,
    Notepad,
    Session,
    Skills,
    Trace,
}

/// MCP tool annotations per the MCP specification.
///
/// Used by clients (e.g. Claude Code) to prioritize tool loading
/// and avoid deferring critical tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolAnnotations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,
}

/// Metadata for a tool definition — everything except the handler.
///
/// Used for tool listing/registration. The handler is stored separately
/// in the concrete tool module.
pub struct ToolInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub category: ToolCategory,
    pub annotations: ToolAnnotations,
    pub input_schema: serde_json::Value,
}
