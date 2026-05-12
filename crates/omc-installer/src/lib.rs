//! omc-installer: Installation, update, and configuration management for oh-my-claudecode
//!
//! Handles installation of OMC agents, commands, skills, and hooks
//! into the Claude Code config directory (~/.claude/).

pub mod config;
pub mod installer;
pub mod updater;

pub use config::{InstallerConfig, InstallerPaths};
pub use installer::{InstallError, InstallOptions, InstallResult, Installer};
pub use updater::{UpdateError, Updater};
