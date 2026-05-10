//! # omc-host — Host Abstraction Layer
//!
//! Provides a unified interface over Claude Code and Codex CLI,
//! allowing OMC-RS to generate host-specific configs, hooks, agents,
//! and skill registrations for either platform.

pub mod adapter;
pub mod claude;
pub mod codex;
pub mod config_state;
pub mod mcp_reg;
pub mod types;
pub mod unified_hooks;

// Re-exports for convenience
pub use adapter::{HookEntry, HostAdapter, HostDoctorReport, HostInitReport, HostKind};
pub use claude::ClaudeHostAdapter;
pub use codex::CodexHostAdapter;
pub use types::{
    AgentGenOptions, AgentRole, ConfigGenOptions, GeneratedAgentFile, GeneratedConfig,
    GeneratedFile, HookGenEntry, McpServerDef, Posture, ReasoningEffort, SpawnDirective,
    TeamSpawnOpts,
};
pub use unified_hooks::UnifiedHookEvent;

/// Create the appropriate host adapter for the given host kind.
pub fn create_adapter(host: HostKind) -> Box<dyn HostAdapter> {
    match host {
        HostKind::Claude => Box::new(ClaudeHostAdapter::new()),
        HostKind::Codex => Box::new(CodexHostAdapter::new()),
    }
}
