//! Claude Code host adapter.

pub mod agents;
pub mod config;
pub mod doctor;
pub mod hooks;
pub mod init;

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::adapter::{HostAdapter, HostDoctorReport, HostInitReport, HookEntry};
use crate::mcp_reg;
use crate::types::{
    AgentGenOptions, AgentRole, ConfigGenOptions, GeneratedAgentFile, GeneratedConfig,
    McpServerDef, SpawnDirective, TeamSpawnOpts,
};
use crate::unified_hooks::UnifiedHookEvent;

/// Host adapter for Claude Code CLI.
pub struct ClaudeHostAdapter;

impl Default for ClaudeHostAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeHostAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl HostAdapter for ClaudeHostAdapter {
    fn kind(&self) -> crate::adapter::HostKind {
        crate::adapter::HostKind::Claude
    }

    fn doctor(&self, root: &Path) -> HostDoctorReport {
        doctor::claude_doctor(root)
    }

    fn check_ready(&self, root: &Path) -> Result<(), String> {
        doctor::claude_check_ready(root)
    }

    fn init_project(&self, root: &Path) -> Result<HostInitReport, String> {
        init::claude_init_project(root)
    }

    fn generate_config(&self, opts: &ConfigGenOptions) -> Result<GeneratedConfig, String> {
        config::generate_claude_config(opts)
    }

    fn config_path(&self) -> PathBuf {
        PathBuf::from(".claude").join("settings.json")
    }

    fn map_hook_event(
        &self,
        event: &UnifiedHookEvent,
        command: &str,
        timeout_secs: u64,
    ) -> Option<HookEntry> {
        hooks::map_claude_hook(event, command, timeout_secs)
    }

    fn generate_agent_definition(
        &self,
        role: &AgentRole,
        opts: &AgentGenOptions,
    ) -> Result<GeneratedAgentFile, String> {
        agents::generate_claude_agent(role, opts)
    }

    fn agents_dir(&self) -> PathBuf {
        PathBuf::from(".claude").join("agents")
    }

    fn workspace_doc_name(&self) -> &str {
        "CLAUDE.md"
    }

    fn inject_discipline_block(&self, existing: &str, block: &str) -> Result<String, String> {
        let marker_start = "<!-- omc-discipline:start -->";
        let marker_end = "<!-- omc-discipline:end -->";

        let new_section = format!("{marker_start}\n{block}\n{marker_end}");

        if existing.contains(marker_start) {
            // Replace existing block
            let start_idx = existing.find(marker_start).unwrap();
            let end_idx = existing
                .find(marker_end)
                .map(|i| i + marker_end.len())
                .unwrap_or(existing.len());
            let mut result = String::with_capacity(existing.len() + block.len());
            result.push_str(&existing[..start_idx]);
            result.push_str(&new_section);
            result.push_str(&existing[end_idx..]);
            Ok(result)
        } else {
            // Append
            let mut result = existing.to_string();
            if !result.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(&new_section);
            result.push('\n');
            Ok(result)
        }
    }

    fn skills_dir(&self) -> PathBuf {
        PathBuf::from(".claude").join("skills")
    }

    fn generate_mcp_registration(
        &self,
        servers: &[McpServerDef],
    ) -> Result<serde_json::Value, String> {
        Ok(mcp_reg::claude_mcp_json(servers))
    }

    fn generate_team_spawn(
        &self,
        mission: &str,
        opts: &TeamSpawnOpts,
    ) -> Result<SpawnDirective, String> {
        // Claude Code uses the native Agent tool
        let role_hint = opts
            .role
            .as_deref()
            .unwrap_or("executor");
        let payload = format!(
            "You are {role_hint}. Mission: {mission}. Work autonomously. Report when done."
        );
        Ok(SpawnDirective {
            mechanism: "agent_tool".into(),
            payload,
            work_dir: None,
        })
    }
}
