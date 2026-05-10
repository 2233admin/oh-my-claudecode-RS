use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

const OMC_HOOK_EVENTS: &[&str] = &["PreToolUse", "PostToolUse", "Stop", "SessionStart"];
const HOOK_TIMEOUT_MS: u64 = 30_000;
const OMC_HOOK_PREFIX: &str = "omc-hook";

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

    /// Copy built-in skill templates to the skills directory.
    fn install_skills(&self, result: &mut InstallResult) -> Result<(), InstallError> {
        if !self.paths.skills_dir.exists() {
            std::fs::create_dir_all(&self.paths.skills_dir)?;
        }

        for (name, content) in Self::builtin_skill_templates() {
            let skill_dir = self.paths.skills_dir.join(&name);
            let readme_path = skill_dir.join("README.md");
            if readme_path.exists() && !self.options.force {
                continue;
            }
            std::fs::create_dir_all(&skill_dir)?;
            std::fs::write(&readme_path, content)?;
            result.installed_skills.push(name);
        }

        Ok(())
    }

    /// Register external skill sources by symlinking (or copying) into the skills directory.
    ///
    /// Each source path can be a directory (used as-is) or a file (stem used as skill name).
    /// Returns the list of registered skill names.
    pub fn register_external_skills(
        &self,
        sources: &[PathBuf],
    ) -> Result<Vec<String>, InstallError> {
        if !self.paths.skills_dir.exists() {
            std::fs::create_dir_all(&self.paths.skills_dir)?;
        }

        let mut registered = Vec::new();

        for source in sources {
            if !source.exists() {
                tracing::warn!(source = %source.display(), "Skipping non-existent external skill source");
                continue;
            }

            let skill_name = if source.is_dir() {
                source
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            } else {
                source
                    .file_stem()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            };

            if skill_name.is_empty() {
                continue;
            }

            let dest = self.paths.skills_dir.join(&skill_name);

            if dest.exists() && !self.options.force {
                continue;
            }

            if dest.exists() {
                if dest.is_dir() {
                    std::fs::remove_dir_all(&dest)?;
                } else {
                    std::fs::remove_file(&dest)?;
                }
            }

            if let Err(e) = Self::try_symlink(source, &dest) {
                tracing::debug!(
                    error = %e,
                    "Symlink failed, falling back to copy"
                );
                Self::copy_dir_recursive(source, &dest)?;
            }

            registered.push(skill_name);
        }

        Ok(registered)
    }

    /// Attempt to create a directory symlink. Platform-specific implementation.
    fn try_symlink(original: &Path, link: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(original, link)
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(original, link)
        }
    }

    /// Recursively copy a directory from `src` to `dst`.
    fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), InstallError> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let dest_path = dst.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                Self::copy_dir_recursive(&entry.path(), &dest_path)?;
            } else {
                std::fs::copy(entry.path(), &dest_path)?;
            }
        }
        Ok(())
    }

    /// Built-in skill templates shipped with the installer.
    ///
    /// Returns `(skill_name, readme_content)` pairs. Each skill is installed
    /// as a subdirectory under the skills directory with a README.md.
    fn builtin_skill_templates() -> Vec<(String, String)> {
        vec![(
            "custom-skills-guide".to_string(),
            include_str!("../templates/custom-skills-guide/README.md").to_string(),
        )]
    }

    /// Configure hooks in settings.json.
    ///
    /// Reads the existing settings, merges OMC hook entries for each event,
    /// detects conflicts with user hooks, and writes back the file.
    fn install_hooks(&self, result: &mut InstallResult) {
        let settings_path = &self.paths.settings_file;

        let mut settings: serde_json::Value = match std::fs::read_to_string(settings_path) {
            Ok(content) => serde_json::from_str(&content)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
            Err(_) => serde_json::Value::Object(serde_json::Map::new()),
        };

        if !settings.is_object() {
            settings = serde_json::Value::Object(serde_json::Map::new());
        }

        let hooks_obj = settings
            .as_object_mut()
            .unwrap()
            .entry("hooks")
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

        if !hooks_obj.is_object() {
            *hooks_obj = serde_json::Value::Object(serde_json::Map::new());
        }

        let hooks_map = hooks_obj.as_object_mut().unwrap();

        for event in OMC_HOOK_EVENTS {
            let omc_command = format!("{OMC_HOOK_PREFIX} {event}");
            let hook_entry = serde_json::json!({
                "matcher": "",
                "command": omc_command,
                "timeout": HOOK_TIMEOUT_MS,
            });

            if let Some(existing) = hooks_map.get(*event) {
                if let Some(entries) = existing.as_array() {
                    let has_omc = entries.iter().any(|entry| {
                        entry
                            .get("command")
                            .and_then(|c| c.as_str())
                            .is_some_and(|cmd| cmd == omc_command)
                    });

                    if has_omc {
                        continue;
                    }

                    let has_other = entries.iter().any(|entry| {
                        entry
                            .get("command")
                            .and_then(|c| c.as_str())
                            .is_some_and(|cmd| !cmd.starts_with(OMC_HOOK_PREFIX))
                    });

                    if has_other && !self.options.force_hooks {
                        for entry in entries {
                            if let Some(cmd) = entry.get("command").and_then(|c| c.as_str())
                                && !cmd.starts_with(OMC_HOOK_PREFIX)
                            {
                                result.hook_conflicts.push(HookConflict {
                                    event_type: event.to_string(),
                                    existing_command: cmd.to_string(),
                                });
                            }
                        }
                        continue;
                    }

                    let mut new_entries = entries.clone();
                    new_entries.push(hook_entry);
                    hooks_map.insert(event.to_string(), serde_json::Value::Array(new_entries));
                }
            } else {
                hooks_map.insert(
                    event.to_string(),
                    serde_json::Value::Array(vec![hook_entry]),
                );
            }
        }

        if let Err(e) = std::fs::write(
            settings_path,
            serde_json::to_string_pretty(&settings).unwrap_or_default(),
        ) {
            result
                .errors
                .push(format!("Failed to write settings.json: {e}"));
            return;
        }

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
    use tempfile::TempDir;

    fn make_installer(tmp: &TempDir) -> Installer {
        let paths = InstallerPaths::from_config_dir(tmp.path().join(".claude"));
        let options = InstallOptions::default();
        Installer::with_paths(paths, options)
    }

    fn make_installer_force(tmp: &TempDir) -> Installer {
        let paths = InstallerPaths::from_config_dir(tmp.path().join(".claude"));
        let options = InstallOptions {
            force: true,
            ..Default::default()
        };
        Installer::with_paths(paths, options)
    }

    fn read_settings(installer: &Installer) -> serde_json::Value {
        let content = std::fs::read_to_string(&installer.paths.settings_file).unwrap();
        serde_json::from_str(&content).unwrap()
    }

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

    #[test]
    fn install_skills_creates_directory_and_populates_readme() {
        let tmp = TempDir::new().unwrap();
        let installer = make_installer(&tmp);
        let mut result = InstallResult::default();

        installer.install_skills(&mut result).unwrap();

        assert!(installer.paths.skills_dir.exists());
        let skill_dir = installer.paths.skills_dir.join("custom-skills-guide");
        assert!(skill_dir.exists());
        assert!(skill_dir.join("README.md").exists());
        assert_eq!(result.installed_skills, vec!["custom-skills-guide"]);
    }

    #[test]
    fn install_skills_skips_existing_without_force() {
        let tmp = TempDir::new().unwrap();
        let installer = make_installer(&tmp);
        let mut result = InstallResult::default();

        installer.install_skills(&mut result).unwrap();
        assert_eq!(result.installed_skills.len(), 1);

        // Second install without force should skip.
        let mut result2 = InstallResult::default();
        installer.install_skills(&mut result2).unwrap();
        assert!(result2.installed_skills.is_empty());
    }

    #[test]
    fn install_skills_force_overwrites_existing() {
        let tmp = TempDir::new().unwrap();
        let installer = make_installer_force(&tmp);
        let mut result = InstallResult::default();

        installer.install_skills(&mut result).unwrap();
        assert_eq!(result.installed_skills.len(), 1);

        let mut result2 = InstallResult::default();
        installer.install_skills(&mut result2).unwrap();
        assert_eq!(result2.installed_skills.len(), 1);
    }

    #[test]
    fn register_external_skills_copies_directory() {
        let tmp = TempDir::new().unwrap();
        let installer = make_installer(&tmp);

        // Create an external skill source directory.
        let ext = tmp.path().join("my-external-skill");
        std::fs::create_dir_all(&ext).unwrap();
        std::fs::write(ext.join("README.md"), "# External Skill").unwrap();

        let registered = installer.register_external_skills(&[ext]).unwrap();
        assert_eq!(registered, vec!["my-external-skill"]);

        let dest = installer
            .paths
            .skills_dir
            .join("my-external-skill")
            .join("README.md");
        assert!(dest.exists());
        assert_eq!(std::fs::read_to_string(dest).unwrap(), "# External Skill");
    }

    #[test]
    fn register_external_skills_skips_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let installer = make_installer(&tmp);

        let bad_path = tmp.path().join("does-not-exist");
        let registered = installer.register_external_skills(&[bad_path]).unwrap();
        assert!(registered.is_empty());
    }

    #[test]
    fn register_external_skills_skips_existing_without_force() {
        let tmp = TempDir::new().unwrap();
        let installer = make_installer(&tmp);

        let ext = tmp.path().join("dup-skill");
        std::fs::create_dir_all(&ext).unwrap();
        std::fs::write(ext.join("README.md"), "v1").unwrap();

        let r1 = installer.register_external_skills(&[ext.clone()]).unwrap();
        assert_eq!(r1.len(), 1);

        let r2 = installer.register_external_skills(&[ext]).unwrap();
        assert!(r2.is_empty());
    }

    #[test]
    fn register_external_skills_force_overwrites() {
        let tmp = TempDir::new().unwrap();
        let installer = make_installer_force(&tmp);

        let ext = tmp.path().join("overwrite-skill");
        std::fs::create_dir_all(&ext).unwrap();
        std::fs::write(ext.join("README.md"), "v1").unwrap();

        let r1 = installer.register_external_skills(&[ext.clone()]).unwrap();
        assert_eq!(r1.len(), 1);

        std::fs::write(ext.join("README.md"), "v2").unwrap();
        let r2 = installer.register_external_skills(&[ext]).unwrap();
        assert_eq!(r2.len(), 1);

        let content = std::fs::read_to_string(
            installer
                .paths
                .skills_dir
                .join("overwrite-skill")
                .join("README.md"),
        )
        .unwrap();
        assert_eq!(content, "v2");
    }

    #[test]
    fn register_external_skills_multiple_sources() {
        let tmp = TempDir::new().unwrap();
        let installer = make_installer(&tmp);

        let ext_a = tmp.path().join("alpha");
        let ext_b = tmp.path().join("beta");
        std::fs::create_dir_all(&ext_a).unwrap();
        std::fs::write(ext_a.join("README.md"), "alpha").unwrap();
        std::fs::create_dir_all(&ext_b).unwrap();
        std::fs::write(ext_b.join("README.md"), "beta").unwrap();

        let registered = installer.register_external_skills(&[ext_a, ext_b]).unwrap();
        assert_eq!(registered.len(), 2);
        assert!(registered.contains(&"alpha".to_string()));
        assert!(registered.contains(&"beta".to_string()));
    }

    #[test]
    fn full_install_includes_skills() {
        let tmp = TempDir::new().unwrap();
        let installer = make_installer(&tmp);

        // Create the config dir so install() doesn't fail.
        std::fs::create_dir_all(&installer.paths.config_dir).unwrap();

        let result = installer.install();
        assert!(result.success, "errors: {:?}", result.errors);
        assert!(!result.installed_skills.is_empty());
        assert!(
            installer
                .paths
                .skills_dir
                .join("custom-skills-guide")
                .join("README.md")
                .exists()
        );
    }

    #[test]
    fn builtin_skill_templates_not_empty() {
        let templates = Installer::builtin_skill_templates();
        assert!(!templates.is_empty());
        for (name, content) in &templates {
            assert!(!name.is_empty());
            assert!(!content.is_empty());
        }
    }

    #[test]
    fn fresh_install_creates_hooks() {
        let temp = tempfile::tempdir().unwrap();
        let installer = make_installer(&temp);
        let mut result = InstallResult::default();

        installer.install_hooks(&mut result);

        assert!(result.hooks_configured);
        assert!(result.hook_conflicts.is_empty());
        assert!(result.errors.is_empty());

        let settings = read_settings(&installer);
        let hooks = settings.get("hooks").unwrap().as_object().unwrap();

        for event in OMC_HOOK_EVENTS {
            let entries = hooks.get(*event).unwrap().as_array().unwrap();
            assert_eq!(entries.len(), 1, "Expected 1 hook for {event}");
            let cmd = entries[0].get("command").unwrap().as_str().unwrap();
            assert_eq!(cmd, format!("omc-hook {event}"));
            let timeout = entries[0].get("timeout").unwrap().as_u64().unwrap();
            assert_eq!(timeout, HOOK_TIMEOUT_MS);
        }
    }

    #[test]
    fn idempotent_reinstall_does_not_duplicate() {
        let temp = tempfile::tempdir().unwrap();
        let installer = make_installer(&temp);

        let mut result1 = InstallResult::default();
        installer.install_hooks(&mut result1);
        assert!(result1.hooks_configured);

        let mut result2 = InstallResult::default();
        installer.install_hooks(&mut result2);
        assert!(result2.hooks_configured);
        assert!(result2.hook_conflicts.is_empty());

        let settings = read_settings(&installer);
        let hooks = settings.get("hooks").unwrap().as_object().unwrap();

        for event in OMC_HOOK_EVENTS {
            let entries = hooks.get(*event).unwrap().as_array().unwrap();
            assert_eq!(entries.len(), 1, "Duplicate hook found for {event}");
        }
    }

    #[test]
    fn preserves_existing_user_hooks() {
        let temp = tempfile::tempdir().unwrap();
        let installer = make_installer(&temp);

        let existing = serde_json::json!({
            "hooks": {
                "Stop": [
                    {
                        "matcher": "",
                        "command": "my-custom-stop.sh",
                        "timeout": 5000
                    }
                ]
            }
        });
        std::fs::write(
            &installer.paths.settings_file,
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let mut result = InstallResult::default();
        installer.install_hooks(&mut result);

        assert_eq!(result.hook_conflicts.len(), 1);
        assert_eq!(result.hook_conflicts[0].event_type, "Stop");
        assert_eq!(
            result.hook_conflicts[0].existing_command,
            "my-custom-stop.sh"
        );

        let settings = read_settings(&installer);
        let stop_hooks = settings
            .get("hooks")
            .unwrap()
            .get("Stop")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(stop_hooks.len(), 1);
        assert_eq!(
            stop_hooks[0].get("command").unwrap().as_str().unwrap(),
            "my-custom-stop.sh"
        );

        let pre_hooks = settings
            .get("hooks")
            .unwrap()
            .get("PreToolUse")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(pre_hooks.len(), 1);
        assert_eq!(
            pre_hooks[0].get("command").unwrap().as_str().unwrap(),
            "omc-hook PreToolUse"
        );
    }

    #[test]
    fn force_hooks_appends_omc_to_conflicting() {
        let temp = tempfile::tempdir().unwrap();
        let paths = InstallerPaths::from_config_dir(temp.path().to_path_buf());
        let installer = Installer::with_paths(
            paths,
            InstallOptions {
                force_hooks: true,
                ..Default::default()
            },
        );

        let existing = serde_json::json!({
            "hooks": {
                "Stop": [
                    {
                        "matcher": "",
                        "command": "my-custom-stop.sh",
                        "timeout": 5000
                    }
                ]
            }
        });
        std::fs::write(
            &installer.paths.settings_file,
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let mut result = InstallResult::default();
        installer.install_hooks(&mut result);

        assert!(result.hook_conflicts.is_empty());
        assert!(result.hooks_configured);

        let settings = read_settings(&installer);
        let stop_hooks = settings
            .get("hooks")
            .unwrap()
            .get("Stop")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(stop_hooks.len(), 2);
        assert_eq!(
            stop_hooks[0].get("command").unwrap().as_str().unwrap(),
            "my-custom-stop.sh"
        );
        assert_eq!(
            stop_hooks[1].get("command").unwrap().as_str().unwrap(),
            "omc-hook Stop"
        );
    }

    #[test]
    fn missing_settings_file_is_created() {
        let temp = tempfile::tempdir().unwrap();
        let installer = make_installer(&temp);

        assert!(!installer.paths.settings_file.exists());

        let mut result = InstallResult::default();
        installer.install_hooks(&mut result);

        assert!(result.hooks_configured);
        assert!(installer.paths.settings_file.exists());
    }
}
