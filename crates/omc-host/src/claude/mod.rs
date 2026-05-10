//! Claude Code host adapter.

pub mod agents;
pub mod config;
pub mod doctor;
pub mod hooks;
pub mod init;

use async_trait::async_trait;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::adapter::{HookEntry, HostAdapter, HostDoctorReport, HostInitReport};
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

    fn register_skills(
        &self,
        root: &Path,
        sources: &[(PathBuf, String)],
    ) -> Result<Vec<PathBuf>, String> {
        let skills_dir = root.join(self.skills_dir());
        fs::create_dir_all(&skills_dir).map_err(|e| format!("failed to create skills dir: {e}"))?;

        let mut registered = Vec::new();
        for (source_dir, link_name) in sources {
            if !source_dir.exists() {
                continue;
            }
            let link_path = skills_dir.join(link_name);
            if link_path.exists() || link_path.symlink_metadata().is_ok() {
                continue;
            }
            match create_link(source_dir, &link_path) {
                Ok(()) => registered.push(link_path),
                Err(_) => {
                    copy_dir_recursive(source_dir, &link_path)
                        .map_err(|e| format!("copy failed: {e}"))?;
                    registered.push(link_path);
                }
            }
        }
        Ok(registered)
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
        let role_hint = opts.role.as_deref().unwrap_or("executor");
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

// ── Helper functions for register_skills ────────────────────────────────────

fn create_link(source: &Path, link_path: &Path) -> Result<(), io::Error> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, link_path)
    }

    #[cfg(windows)]
    {
        if let Some(parent) = link_path.parent() {
            fs::create_dir_all(parent)?;
        }
        std::os::windows::fs::symlink_dir(source, link_path)
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = (source, link_path);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symlink not supported on this platform",
        ))
    }
}

fn copy_dir_recursive(source: &Path, dest: &Path) -> Result<(), io::Error> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &dest_path)?;
        } else {
            fs::copy(&source_path, &dest_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_skill(dir: &Path, name: &str) {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test\n---\n\nContent."),
        )
        .unwrap();
    }

    #[test]
    fn register_skills_creates_links() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("src")).unwrap();
        create_test_skill(&root.join("src"), "my-skill");

        let adapter = ClaudeHostAdapter::new();
        let sources = vec![(root.join("src").join("my-skill"), "my-skill".into())];
        let registered = adapter.register_skills(root, &sources).unwrap();

        assert_eq!(registered.len(), 1);
        assert!(root.join(".claude/skills/my-skill/SKILL.md").exists());
    }

    #[test]
    fn register_skills_skips_nonexistent() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        let adapter = ClaudeHostAdapter::new();
        let sources = vec![(root.join("nope"), "missing".into())];
        let registered = adapter.register_skills(root, &sources).unwrap();

        assert!(registered.is_empty());
    }

    #[test]
    fn register_skills_idempotent() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("src")).unwrap();
        create_test_skill(&root.join("src"), "skill-a");

        let adapter = ClaudeHostAdapter::new();
        let sources = vec![(root.join("src").join("skill-a"), "skill-a".into())];

        let r1 = adapter.register_skills(root, &sources).unwrap();
        assert_eq!(r1.len(), 1);

        let r2 = adapter.register_skills(root, &sources).unwrap();
        assert!(r2.is_empty(), "second call should skip existing link");
    }

    #[test]
    fn register_skills_multiple_sources() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("src")).unwrap();
        create_test_skill(&root.join("src"), "alpha");
        create_test_skill(&root.join("src"), "beta");

        let adapter = ClaudeHostAdapter::new();
        let sources = vec![
            (root.join("src").join("alpha"), "alpha".into()),
            (root.join("src").join("beta"), "beta".into()),
        ];
        let registered = adapter.register_skills(root, &sources).unwrap();

        assert_eq!(registered.len(), 2);
        assert!(root.join(".claude/skills/alpha/SKILL.md").exists());
        assert!(root.join(".claude/skills/beta/SKILL.md").exists());
    }
}
