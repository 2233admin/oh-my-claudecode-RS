//! Host adapter trait — central abstraction over Claude Code / Codex CLI specifics.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::types::{
    AgentGenOptions, AgentRole, ConfigGenOptions, GeneratedAgentFile, GeneratedConfig,
    McpServerDef, SpawnDirective, TeamSpawnOpts,
};
use crate::unified_hooks::UnifiedHookEvent;

// ── HostKind ───────────────────────────────────────────────────────────────

/// Identifies which host CLI engine is being used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HostKind {
    Claude,
    Codex,
}

impl HostKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    /// Directory name used for host-specific config (`.claude/`, `.codex/`).
    pub fn config_dir_name(&self) -> &'static str {
        match self {
            Self::Claude => ".claude",
            Self::Codex => ".codex",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "claude" | "claude-code" => Ok(Self::Claude),
            "codex" | "codex-cli" | "openai-codex" => Ok(Self::Codex),
            other => Err(format!("unknown host: {other}")),
        }
    }
}

impl std::fmt::Display for HostKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Reports ────────────────────────────────────────────────────────────────

/// Report from `doctor` / readiness check.
#[derive(Debug, Clone, Serialize)]
pub struct HostDoctorReport {
    pub host: HostKind,
    pub ready: bool,
    pub messages: Vec<String>,
}

/// Report from `init` / project setup.
#[derive(Debug, Clone, Serialize)]
pub struct HostInitReport {
    pub host: HostKind,
    pub created: Vec<PathBuf>,
    pub updated: Vec<PathBuf>,
    pub unchanged: Vec<PathBuf>,
}

// ── HostAdapter ────────────────────────────────────────────────────────────

/// The central abstraction over host CLI specifics.
///
/// Each host (Claude Code, Codex CLI) implements this trait to provide
/// host-specific configuration generation, hook registration, agent
/// definitions, and project initialization.
#[async_trait]
pub trait HostAdapter: Send + Sync {
    /// Which host this adapter is for.
    fn kind(&self) -> HostKind;

    // ── Readiness ──────────────────────────────────────────────────

    /// Full readiness check (version, config, deps).
    fn doctor(&self, root: &Path) -> HostDoctorReport;

    /// Quick readiness check (subset of doctor).
    fn check_ready(&self, root: &Path) -> Result<(), String>;

    // ── Project Initialization ─────────────────────────────────────

    /// Generate all host-specific config files, agent definitions,
    /// skill symlinks, and hook registrations.
    fn init_project(&self, root: &Path) -> Result<HostInitReport, String>;

    // ── Config Generation ──────────────────────────────────────────

    /// Generate the host-specific config file content.
    /// Claude: settings.json. Codex: config.toml + hooks.json.
    fn generate_config(&self, opts: &ConfigGenOptions) -> Result<GeneratedConfig, String>;

    /// Return the relative path to the host config file.
    fn config_path(&self) -> PathBuf;

    // ── Hook Registration ──────────────────────────────────────────

    /// Map a unified hook event + command to host-specific format.
    fn map_hook_event(
        &self,
        event: &UnifiedHookEvent,
        command: &str,
        timeout_secs: u64,
    ) -> Option<HookEntry>;

    // ── Agent Definitions ──────────────────────────────────────────

    /// Generate an agent definition file for the given role.
    fn generate_agent_definition(
        &self,
        role: &AgentRole,
        opts: &AgentGenOptions,
    ) -> Result<GeneratedAgentFile, String>;

    /// Return the directory where agent definitions live.
    fn agents_dir(&self) -> PathBuf;

    // ── Context Injection ──────────────────────────────────────────

    /// Return the name of the workspace instruction file.
    fn workspace_doc_name(&self) -> &str;

    /// Generate the discipline block to inject into workspace docs.
    fn inject_discipline_block(&self, existing: &str, block: &str) -> Result<String, String>;

    // ── Skill Registration ─────────────────────────────────────────

    /// Return the path where skills are discovered.
    fn skills_dir(&self) -> PathBuf;

    // ── MCP Registration ───────────────────────────────────────────

    /// Generate MCP server registration entries for the host config.
    fn generate_mcp_registration(
        &self,
        servers: &[McpServerDef],
    ) -> Result<serde_json::Value, String>;

    // ── Team Spawn ─────────────────────────────────────────────────

    /// Generate the command or mechanism to spawn a subagent.
    fn generate_team_spawn(
        &self,
        mission: &str,
        opts: &TeamSpawnOpts,
    ) -> Result<SpawnDirective, String>;
}

/// A single hook registration entry, host-agnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEntry {
    pub event_name: String,
    pub command: String,
    pub timeout_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_kind_parse_claude() {
        assert_eq!(HostKind::parse("claude").unwrap(), HostKind::Claude);
        assert_eq!(HostKind::parse("Claude").unwrap(), HostKind::Claude);
        assert_eq!(HostKind::parse("claude-code").unwrap(), HostKind::Claude);
    }

    #[test]
    fn host_kind_parse_codex() {
        assert_eq!(HostKind::parse("codex").unwrap(), HostKind::Codex);
        assert_eq!(HostKind::parse("Codex").unwrap(), HostKind::Codex);
        assert_eq!(HostKind::parse("codex-cli").unwrap(), HostKind::Codex);
        assert_eq!(HostKind::parse("openai-codex").unwrap(), HostKind::Codex);
    }

    #[test]
    fn host_kind_parse_unknown() {
        assert!(HostKind::parse("gemini").is_err());
        assert!(HostKind::parse("").is_err());
    }

    #[test]
    fn host_kind_as_str_roundtrip() {
        for kind in [HostKind::Claude, HostKind::Codex] {
            let s = kind.as_str();
            assert_eq!(HostKind::parse(s).unwrap(), kind);
        }
    }

    #[test]
    fn host_kind_config_dir_name() {
        assert_eq!(HostKind::Claude.config_dir_name(), ".claude");
        assert_eq!(HostKind::Codex.config_dir_name(), ".codex");
    }

    #[test]
    fn host_kind_display() {
        assert_eq!(format!("{}", HostKind::Claude), "claude");
        assert_eq!(format!("{}", HostKind::Codex), "codex");
    }

    #[test]
    fn host_kind_serde_roundtrip() {
        for kind in [HostKind::Claude, HostKind::Codex] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: HostKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, kind);
        }
    }
}
