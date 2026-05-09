use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Paths to Claude Code configuration directories.
#[derive(Debug, Clone)]
pub struct InstallerPaths {
    /// Root config directory (e.g. ~/.claude)
    pub config_dir: PathBuf,
    /// Agents directory (<config_dir>/agents)
    pub agents_dir: PathBuf,
    /// Skills directory (<config_dir>/skills)
    pub skills_dir: PathBuf,
    /// Hooks directory (<config_dir>/hooks)
    pub hooks_dir: PathBuf,
    /// HUD directory (<config_dir>/hud)
    pub hud_dir: PathBuf,
    /// settings.json path
    pub settings_file: PathBuf,
    /// Version metadata file path
    pub version_file: PathBuf,
}

impl InstallerPaths {
    /// Create paths from the default Claude config directory (~/.claude).
    pub fn default_config() -> Option<Self> {
        let config_dir = dirs::home_dir()?.join(".claude");
        Some(Self::from_config_dir(config_dir))
    }

    /// Create paths from a custom config directory.
    pub fn from_config_dir(config_dir: PathBuf) -> Self {
        Self {
            agents_dir: config_dir.join("agents"),
            skills_dir: config_dir.join("skills"),
            hooks_dir: config_dir.join("hooks"),
            hud_dir: config_dir.join("hud"),
            settings_file: config_dir.join("settings.json"),
            version_file: config_dir.join(".omc-version.json"),
            config_dir,
        }
    }
}

/// Persisted OMC version metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMetadata {
    pub version: String,
    #[serde(rename = "installedAt")]
    pub installed_at: String,
    #[serde(rename = "installMethod")]
    pub install_method: String,
    #[serde(rename = "lastCheckAt")]
    pub last_check_at: String,
}

/// OMC runtime configuration (stored in .omc-config.json).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstallerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hud_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_binary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_completed: Option<String>,
}

impl InstallerConfig {
    /// Load config from disk, returning default if the file doesn't exist.
    pub fn load(config_dir: &std::path::Path) -> Self {
        let config_path = config_dir.join(".omc-config.json");
        match std::fs::read_to_string(&config_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Persist config to disk.
    pub fn save(&self, config_dir: &std::path::Path) -> Result<(), std::io::Error> {
        let config_path = config_dir.join(".omc-config.json");
        let content = serde_json::to_string_pretty(self).unwrap_or_default();
        std::fs::write(config_path, content)
    }

    /// Whether the HUD statusline is enabled (default: true).
    pub fn is_hud_enabled(&self) -> bool {
        self.hud_enabled.unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths_from_config_dir() {
        let temp_dir = tempdir().unwrap();
        let config_dir = temp_dir.path().join(".claude");
        let paths = InstallerPaths::from_config_dir(config_dir.clone());
        assert_eq!(paths.agents_dir, config_dir.join("agents"));
        assert_eq!(paths.skills_dir, config_dir.join("skills"));
        assert_eq!(paths.hooks_dir, config_dir.join("hooks"));
        assert_eq!(paths.hud_dir, config_dir.join("hud"));
        assert_eq!(
            paths.settings_file,
            config_dir.join("settings.json")
        );
    }

    #[test]
    fn default_hud_enabled() {
        let config = InstallerConfig::default();
        assert!(config.is_hud_enabled());
    }
}
