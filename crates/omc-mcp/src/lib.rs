//! omc-mcp: MCP Tool Server for oh-my-claudecode-RS
//!
//! Provides MCP tools via JSON-RPC over stdio for state management,
//! notepad operations, and project memory.

pub mod memory_tools;
pub mod notepad_tools;
pub mod protocol_registry;
pub mod state_tools;
pub mod tool_registry;
pub mod tools;

pub use tool_registry::McpToolRegistry;
pub use tools::{McpTool, ToolDefinition, ToolResult};

/// Collect all registered MCP tools.
pub fn all_tools() -> Vec<Box<dyn McpTool>> {
    let mut tools: Vec<Box<dyn McpTool>> = Vec::new();
    tools.extend(state_tools::state_tools());
    tools.extend(notepad_tools::notepad_tools());
    tools.extend(memory_tools::memory_tools());
    tools
}
