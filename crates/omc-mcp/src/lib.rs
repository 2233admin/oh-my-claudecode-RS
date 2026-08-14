//! omc-mcp: MCP Tool Server for oh-my-claudecode-RS
//!
//! Provides MCP tools via JSON-RPC over stdio for agent capability discovery,
//! routing, state management, notepad operations, and project memory.

pub mod agent_tools;
pub mod goal_tools;
pub mod memory_tools;
pub mod notepad_tools;
pub mod protocol_registry;
pub mod python_tools;
pub mod server;
pub mod state_tools;
pub mod team_tools;
pub mod tool_registry;
pub mod tools;

pub use server::run_stdio;
pub use tool_registry::McpToolRegistry;
pub use tools::{McpTool, ToolDefinition, ToolResult};

/// Collect all registered MCP tools.
pub fn all_tools() -> Vec<Box<dyn McpTool>> {
    let mut tools: Vec<Box<dyn McpTool>> = Vec::new();
    tools.extend(agent_tools::agent_tools());
    tools.extend(goal_tools::goal_tools());
    tools.extend(state_tools::state_tools());
    tools.extend(notepad_tools::notepad_tools());
    tools.extend(memory_tools::memory_tools());
    tools.extend(python_tools::python_tools());
    tools.extend(team_tools::team_tools());
    tools
}

#[cfg(test)]
mod catalog_tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn registered_tools_exactly_match_the_shared_catalog() {
        let registered = all_tools()
            .into_iter()
            .map(|tool| tool.definition().name)
            .collect::<BTreeSet<_>>();
        let catalog = omc_shared::capability_catalog::mcp_tool_names()
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        assert_eq!(registered, catalog);
    }
}
