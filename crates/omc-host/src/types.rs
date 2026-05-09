//! Shared types for host adapter implementations.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::adapter::HostKind;

// ── Agent Definitions ──────────────────────────────────────────────────────

/// Agent role — the superset of Claude and Codex agent metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRole {
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    /// Codex-specific: reasoning effort level.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Codex-specific: default/conservative/exploratory.
    pub posture: Option<Posture>,
    /// Codex-specific: model class override (e.g., "o3", "gpt-4.1").
    pub model_class: Option<String>,
    /// Routing role hint (planner, executor, reviewer, etc.).
    pub routing_role: Option<String>,
    /// Which hosts this agent is available on. None = all.
    pub hosts: Option<Vec<HostKind>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Posture {
    Default,
    Conservative,
    Exploratory,
    FrontierOrchestrator,
    DeepWorker,
    FastLane,
}

// ── Config Generation ──────────────────────────────────────────────────────

/// Options for config file generation.
#[derive(Debug, Clone, Default)]
pub struct ConfigGenOptions {
    /// Enable agent teams feature flag.
    pub enable_teams: bool,
    /// MCP server definitions to register.
    pub mcp_servers: Vec<McpServerDef>,
    /// Hook entries to register.
    pub hooks: Vec<HookGenEntry>,
    /// Additional environment variables.
    pub env: std::collections::HashMap<String, String>,
    /// Custom instructions / developer message.
    pub custom_instructions: Option<String>,
}

/// A hook entry for config generation.
#[derive(Debug, Clone)]
pub struct HookGenEntry {
    pub event: crate::unified_hooks::UnifiedHookEvent,
    pub command: String,
    pub timeout_secs: u64,
    pub matcher: Option<String>,
}

/// Generated config output — may be one or more files.
#[derive(Debug, Clone)]
pub struct GeneratedConfig {
    pub files: Vec<GeneratedFile>,
}

/// A single generated file.
#[derive(Debug, Clone)]
pub struct GeneratedFile {
    pub relative_path: PathBuf,
    pub content: String,
}

// ── Agent Generation ───────────────────────────────────────────────────────

/// Options for agent definition generation.
#[derive(Debug, Clone, Default)]
pub struct AgentGenOptions {
    /// Where to write agent files (project-level vs user-level).
    pub project_root: Option<PathBuf>,
}

/// A generated agent definition file.
#[derive(Debug, Clone)]
pub struct GeneratedAgentFile {
    pub relative_path: PathBuf,
    pub content: String,
}

// ── MCP ────────────────────────────────────────────────────────────────────

/// MCP server definition for registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerDef {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<std::collections::HashMap<String, String>>,
}

// ── Team Spawn ─────────────────────────────────────────────────────────────

/// Options for team spawn generation.
#[derive(Debug, Clone, Default)]
pub struct TeamSpawnOpts {
    pub num_workers: usize,
    pub role: Option<String>,
    pub use_worktree: bool,
}

/// Directive for how to spawn a subagent.
#[derive(Debug, Clone)]
pub struct SpawnDirective {
    /// The mechanism: "agent_tool", "tmux_pane", "subprocess", etc.
    pub mechanism: String,
    /// The command or prompt to execute.
    pub payload: String,
    /// Working directory for the spawned agent.
    pub work_dir: Option<PathBuf>,
}
