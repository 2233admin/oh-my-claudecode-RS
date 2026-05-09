//! omc-xcmd: x-cmd integration for OMC-RS
//!
//! Provides access to x-cmd skills, tools, and status.

pub mod executor;
pub mod skills;

use std::path::PathBuf;

/// x-cmd root directory
pub fn xcmd_root() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".x-cmd.root"))
}

/// Check if x-cmd is installed
pub fn is_installed() -> bool {
    xcmd_root().map(|p| p.exists()).unwrap_or(false)
}

/// x-cmd skills directory
pub fn skills_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".agents/skills"))
}

/// x-cmd agents directory
pub fn agents_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".agents"))
}

/// Get x-cmd version from VERSION file
pub fn get_version() -> Option<String> {
    xcmd_root()
        .and_then(|p| std::fs::read_to_string(p.join("VERSION")).ok())
        .map(|v| v.trim().to_string())
}
