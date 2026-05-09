use std::collections::HashMap;

use thiserror::Error;

use crate::config::{InstallerConfig, InstallerPaths, VersionMetadata};

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Config directory not found")]
    ConfigDirNotFound,
    #[error("Installation failed: {0}")]
    Other(String),
}

/// Options controlling installation behavior.
#[derive(Debug, Clone, Default)]
pub struct InstallOptions {
    /// Overwrite existing files.
    pub force: bool,
    /// Specific version to install (defaults to current).
    pub version: Option<String>,
    /// Print verbose output.
    pub verbose: bool,
    /// Skip the Claude Code installation check.
    pub skip_claude_check: bool,
    /// Force overwrite of non-OMC hooks.
    pub force_hooks: bool,
    /// Skip HUD statusline installation.
    pub skip_hud: bool,
}

/// Result of an installation operation.
#[derive(Debug, Clone, Default)]
pub struct InstallResult {
    pub success: bool,
    pub message: String,
    pub installed_agents: Vec<String>,
    pub installed_commands: Vec<String>,
    pub installed_skills: Vec<String>,
    pub hooks_configured: bool,
    pub hook_conflicts: Vec<HookConflict>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HookConflict {
    pub event_type: String,
    pub existing_command: String,
}

/// Core installer that manages OMC agent, skill, and hook installation
/// into the Claude Code config directory.
pub struct Installer {
    paths: InstallerPaths,
    options: InstallOptions,
}

impl Installer {
    pub fn new(options: InstallOptions) -> Option<Self> {
        let paths = InstallerPaths::default_config()?;
        Some(Self { paths, options })
    }

    pub fn with_paths(paths: InstallerPaths, options: InstallOptions) -> Self {
        Self { paths, options }
    }

    /// Run the full installation flow.
    pub fn install(&self) -> InstallResult {
        let mut result = InstallResult::default();

        if !self.paths.config_dir.exists()
            && let Err(e) = std::fs::create_dir_all(&self.paths.config_dir)
        {
            result
                .errors
                .push(format!("Failed to create config dir: {e}"));
            result.message = format!("Installation failed: {e}");
            return result;
        }

        let install_result = self.install_agents(&mut result);
        if let Err(e) = install_result {
            result.errors.push(e.to_string());
        }

        let skills_result = self.install_skills(&mut result);
        if let Err(e) = skills_result {
            result.errors.push(e.to_string());
        }

        self.install_hooks(&mut result);
        self.install_hud(&mut result);
        self.save_version_metadata(&mut result);

        result.success = result.errors.is_empty();
        if result.success {
            result.message = format!(
                "Installed {} agents, {} commands, {} skills",
                result.installed_agents.len(),
                result.installed_commands.len(),
                result.installed_skills.len(),
            );
        }

        result
    }

    /// Copy agent definition files to the agents directory.
    fn install_agents(&self, result: &mut InstallResult) -> Result<(), InstallError> {
        if !self.paths.agents_dir.exists() {
            std::fs::create_dir_all(&self.paths.agents_dir)?;
        }

        let agents = self.load_agent_definitions()?;
        for (filename, content) in agents {
            let filepath = self.paths.agents_dir.join(&filename);
            if filepath.exists() && !self.options.force {
                continue;
            }
            std::fs::write(&filepath, content)?;
            result.installed_agents.push(filename);
        }

        Ok(())
    }

    /// Copy skill definitions to the skills directory.
    fn install_skills(&self, _result: &mut InstallResult) -> Result<(), InstallError> {
        if !self.paths.skills_dir.exists() {
            std::fs::create_dir_all(&self.paths.skills_dir)?;
        }

        // Skill installation is scaffolded here; concrete implementation
        // depends on the skill directory layout in the package.
        Ok(())
    }

    /// Configure hooks in settings.json (scaffold).
    fn install_hooks(&self, result: &mut InstallResult) {
        // Hook configuration is scaffolded here; will be ported from
        // the TypeScript hooks.ts module.
        result.hooks_configured = true;
    }

    /// Install the HUD statusline script (scaffold).
    fn install_hud(&self, result: &mut InstallResult) {
        if self.options.skip_hud {
            return;
        }

        let config = InstallerConfig::load(&self.paths.config_dir);
        if !config.is_hud_enabled() {
            return;
        }

        if !self.paths.hud_dir.exists()
            && let Err(e) = std::fs::create_dir_all(&self.paths.hud_dir)
        {
            result.errors.push(format!("Failed to create HUD dir: {e}"));
        }
    }

    /// Save version metadata to disk.
    fn save_version_metadata(&self, result: &mut InstallResult) {
        let version = self
            .options
            .version
            .clone()
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
        let now = chrono::Utc::now().to_rfc3339();

        let metadata = VersionMetadata {
            version,
            installed_at: now.clone(),
            install_method: "cargo".to_string(),
            last_check_at: now,
        };

        if let Err(e) = std::fs::write(
            &self.paths.version_file,
            serde_json::to_string_pretty(&metadata).unwrap_or_default(),
        ) {
            result
                .errors
                .push(format!("Failed to save version metadata: {e}"));
        }
    }

    /// Load agent definitions from the package agents/ directory.
    fn load_agent_definitions(&self) -> Result<HashMap<String, String>, InstallError> {
        // Returns an empty map in skeleton; populated once the agents
        // directory is wired up during packaging.
        Ok(HashMap::new())
    }

    /// Check if OMC is already installed by looking for the version file.
    pub fn is_installed(&self) -> bool {
        self.paths.version_file.exists()
    }

    /// Read persisted version metadata from disk.
    pub fn get_install_info(&self) -> Option<VersionMetadata> {
        let content = std::fs::read_to_string(&self.paths.version_file).ok()?;
        serde_json::from_str(&content).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_result_defaults() {
        let result = InstallResult::default();
        assert!(!result.success);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn hook_conflict_fields() {
        let conflict = HookConflict {
            event_type: "Stop".to_string(),
            existing_command: "my-hook.sh".to_string(),
        };
        assert_eq!(conflict.event_type, "Stop");
    }
}
