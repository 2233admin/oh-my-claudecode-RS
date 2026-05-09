//! Configuration management for omc

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

pub mod paths;

pub use paths::OmcPaths;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("Failed to get config directory")]
    NoConfigDir,
}

/// Main configuration for oh-my-claudecode-RS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: String,
    pub data_dir: PathBuf,
    pub hooks_dir: PathBuf,
    pub skills_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("omc");

        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            data_dir: data_dir.clone(),
            hooks_dir: data_dir.join("hooks"),
            skills_dir: data_dir.join("skills"),
        }
    }
}

impl Config {
    /// Load config from file
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::config_path()?;
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save config to file
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    fn config_path() -> Result<PathBuf, ConfigError> {
        dirs::config_dir()
            .map(|p| p.join("omc").join("config.json"))
            .ok_or(ConfigError::NoConfigDir)
    }
}
