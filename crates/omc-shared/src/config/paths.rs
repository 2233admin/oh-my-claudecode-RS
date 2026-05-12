//! Path management for OMC-RS
//!
//! Provides centralized path resolution following OMC conventions:
//! - `~/.omc/` - base directory (configurable via `OMC_HOME`)
//! - `~/.omc/state/` - runtime state
//! - `~/.omc/state/sessions/` - session data
//! - `~/.omc/logs/` - log files
//! - `~/.omc/prompts/` - prompt templates
//! - `~/.omc/team/` - team configurations
//! - `~/.omc/hooks/hooks.json` - hook registry
//! - `~/.omc/skills/` - skills directory

use std::path::{Path, PathBuf};

/// Resolves OMC directory paths with support for `OMC_HOME` environment variable.
#[derive(Debug, Clone)]
pub struct OmcPaths {
    /// Base OMC directory (~/.omc/)
    pub home: PathBuf,
    /// State directory (~/.omc/state/)
    pub state: PathBuf,
    /// Sessions directory (~/.omc/state/sessions/)
    pub sessions: PathBuf,
    /// Logs directory (~/.omc/logs/)
    pub logs: PathBuf,
    /// Prompts directory (~/.omc/prompts/)
    pub prompts: PathBuf,
    /// Team directory (~/.omc/team/)
    pub team: PathBuf,
    /// Hooks file (~/.omc/hooks/hooks.json)
    pub hooks: PathBuf,
    /// Skills directory (~/.omc/skills/)
    pub skills: PathBuf,
}

impl Default for OmcPaths {
    fn default() -> Self {
        Self::new()
    }
}

impl OmcPaths {
    /// Creates a new `OmcPaths` with paths resolved from environment or defaults.
    ///
    /// Environment variables:
    /// - `OMC_HOME` - override the base directory (takes precedence over all)
    ///
    /// Falls back to `~/.omc/` when no environment variable is set.
    pub fn new() -> Self {
        let home = Self::resolve_home();

        Self {
            home: home.clone(),
            state: home.join("state"),
            sessions: home.join("state/sessions"),
            logs: home.join("logs"),
            prompts: home.join("prompts"),
            team: home.join("team"),
            hooks: home.join("hooks/hooks.json"),
            skills: home.join("skills"),
        }
    }

    /// Resolves the home directory from environment or default.
    fn resolve_home() -> PathBuf {
        // OMC_HOME takes precedence
        if let Some(home) = std::env::var_os("OMC_HOME") {
            let path = PathBuf::from(home);
            if path.is_absolute() {
                return path;
            }
            // Resolve relative paths against current dir
            if let Ok(cwd) = std::env::current_dir() {
                return cwd.join(path);
            }
            return path;
        }

        // Fallback to ~/.omc
        dirs::home_dir().map_or_else(|| PathBuf::from(".omc"), |h| h.join(".omc"))
    }

    /// Returns the user-level configuration path.
    ///
    /// Format: `<home>/config.json`
    ///
    /// For Windows: `%USERPROFILE%\.omc\config.json`
    /// For Unix: `~/.omc/config.json`
    pub fn user_config(&self) -> PathBuf {
        self.home.join("config.json")
    }

    /// Returns the project-level configuration path.
    ///
    /// Format: `<cwd>/.claude/omc.json`
    ///
    /// This path is resolved relative to the current working directory.
    pub fn project_config(&self) -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".claude/omc.json")
    }

    /// Ensures all required directories exist, creating them if necessary.
    ///
    /// Creates the following directories:
    /// - `home` (~/.omc/)
    /// - `state` (~/.omc/state/)
    /// - `sessions` (~/.omc/state/sessions/)
    /// - `logs` (~/.omc/logs/)
    /// - `prompts` (~/.omc/prompts/)
    /// - `team` (~/.omc/team/)
    /// - `skills` (~/.omc/skills/)
    ///
    /// Does NOT create the hooks directory - the hooks file is created by the
    /// hooks module when initializing.
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        let dirs = [
            &self.home,
            &self.state,
            &self.sessions,
            &self.logs,
            &self.prompts,
            &self.team,
            &self.skills,
        ];

        for dir in dirs {
            std::fs::create_dir_all(dir)?;
        }

        Ok(())
    }

    /// Returns the path for a specific session.
    ///
    /// Format: `<sessions>/<session_id>/`
    pub fn session_path(&self, session_id: &str) -> PathBuf {
        self.sessions.join(session_id)
    }

    /// Returns the log file path for a specific session.
    ///
    /// Format: `<logs>/<session_id>.log`
    pub fn session_log(&self, session_id: &str) -> PathBuf {
        self.logs.join(format!("{session_id}.log"))
    }

    /// Creates a new `OmcPaths` with an explicit root directory.
    pub fn new_with_root(root: PathBuf) -> Self {
        Self {
            home: root.clone(),
            state: root.join("state"),
            sessions: root.join("state/sessions"),
            logs: root.join("logs"),
            prompts: root.join("prompts"),
            team: root.join("team"),
            hooks: root.join("hooks/hooks.json"),
            skills: root.join("skills"),
        }
    }

    /// Returns the HUD state file path for a session.
    pub fn hud_state_path(&self, session_id: &str) -> PathBuf {
        self.state.join("hud").join(format!("{session_id}.json"))
    }

    /// Returns the sessions directory.
    pub fn sessions_dir(&self) -> PathBuf {
        self.sessions.clone()
    }

    /// Returns the team runs directory.
    pub fn team_runs_dir(&self) -> PathBuf {
        self.team.join("runs")
    }

    /// Returns the host-specific skills directory.
    ///
    /// Format: `~/.omc/skills/<host>/`
    ///
    /// e.g., `~/.omc/skills/claude/` or `~/.omc/skills/codex/`
    pub fn host_skills_dir(&self, host: &str) -> PathBuf {
        self.skills.join(host)
    }

    /// Returns the project-local skills directory.
    ///
    /// Format: `<root>/.omc/skills/`
    pub fn project_skills_dir(root: &Path) -> PathBuf {
        root.join(".omc").join("skills")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_paths() {
        let paths = OmcPaths::default();

        assert!(paths.home.ends_with(".omc"));
        assert!(paths.home.components().any(|c| c.as_os_str() == ".omc"));

        // Check subdirectory structure
        assert!(paths.state.ends_with("state"));
        assert!(paths.sessions.ends_with("state/sessions"));
        assert!(paths.logs.ends_with("logs"));
        assert!(paths.prompts.ends_with("prompts"));
        assert!(paths.team.ends_with("team"));
        assert!(paths.hooks.file_name().unwrap() == "hooks.json");
        assert!(paths.skills.ends_with("skills"));
    }

    #[test]
    fn test_user_config_path() {
        let paths = OmcPaths::default();
        let config = paths.user_config();

        assert!(config.file_name().unwrap() == "config.json");
        assert!(config.parent().unwrap() == paths.home);
    }

    #[test]
    fn test_project_config_path() {
        let paths = OmcPaths::default();
        let config = paths.project_config();

        assert!(config.file_name().unwrap() == "omc.json");
        assert!(config.components().any(|c| c.as_os_str() == ".claude"));
    }

    #[test]
    fn test_session_paths() {
        let paths = OmcPaths::default();
        let session_id = "test-session-123";

        let session_path = paths.session_path(session_id);
        assert!(session_path.ends_with(session_id));
        assert!(session_path.parent().unwrap() == paths.sessions);

        let log_path = paths.session_log(session_id);
        assert!(log_path.file_name().unwrap() == "test-session-123.log");
        assert!(log_path.parent().unwrap() == paths.logs);
    }

    #[test]
    fn test_omc_home_override() {
        // This test verifies the path resolution logic
        // We can't easily test env var override in unit tests without mocking
        let paths = OmcPaths::default();

        // Home should always end with .omc when using default
        assert!(
            paths.home.to_string_lossy().ends_with(".omc"),
            "home should end with .omc, got: {}",
            paths.home.display()
        );
    }

    #[test]
    fn test_host_skills_dir() {
        let tmp = tempdir().unwrap();
        let paths = OmcPaths::new_with_root(tmp.path().to_path_buf());

        let claude_skills = paths.host_skills_dir("claude");
        assert!(claude_skills.ends_with("skills/claude"));
        assert_eq!(claude_skills.parent().unwrap(), paths.skills);

        let codex_skills = paths.host_skills_dir("codex");
        assert!(codex_skills.ends_with("skills/codex"));
    }

    #[test]
    fn test_project_skills_dir() {
        let root = PathBuf::from("/home/user/my-project");
        let skills_dir = OmcPaths::project_skills_dir(&root);

        assert!(skills_dir.ends_with(".omc/skills"));
        assert!(skills_dir.components().any(|c| c.as_os_str() == ".omc"));
    }

    #[test]
    fn test_skills_field_in_new_with_root() {
        let tmp = tempdir().unwrap();
        let paths = OmcPaths::new_with_root(tmp.path().to_path_buf());
        assert_eq!(paths.skills, tmp.path().join("skills"));
    }
}
