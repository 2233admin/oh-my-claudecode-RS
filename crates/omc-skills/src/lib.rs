//! omc-skills crate: Skills loader and executor for oh-my-claudecode-RS
//!
//! This crate provides functionality to discover, load, and execute skills
//! based on YAML frontmatter metadata.

pub mod bootstrap;
pub mod executor;
pub mod frontmatter;
pub mod loader;
pub mod register;
pub mod state;
pub mod templates;

pub use bootstrap::{bootstrap_omc_dir, bootstrap_omc_skills};
pub use executor::SkillExecutor;
pub use frontmatter::parse_frontmatter;
pub use loader::SkillLoader;
pub use register::{RegistrationResult, SkillRegistrar};
pub use state::SkillStateStore;

// Re-export types for convenience
pub use loader::SkillMetadata;
