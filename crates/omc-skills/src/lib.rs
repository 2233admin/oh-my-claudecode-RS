//! omc-skills crate: Skills loader and executor for oh-my-claudecode-RS
//!
//! This crate provides functionality to discover, load, and execute skills
//! based on YAML frontmatter metadata.

pub mod executor;
pub mod frontmatter;
pub mod loader;
pub mod state;
pub mod templates;

pub use executor::SkillExecutor;
pub use frontmatter::parse_frontmatter;
pub use loader::SkillLoader;
pub use state::SkillStateStore;

// Re-export types for convenience
pub use loader::SkillMetadata;
