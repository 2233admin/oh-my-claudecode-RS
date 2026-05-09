//! Skill loader module - discovers and loads skills from the filesystem

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

use crate::frontmatter::parse_frontmatter;
use crate::state::SkillStateStore;

/// Skill metadata parsed from YAML frontmatter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// Skill name
    #[serde(default)]
    pub name: String,
    /// Brief description of what the skill does
    #[serde(default)]
    pub description: String,
    /// Hint for required arguments (e.g., "task: string, priority?: number")
    #[serde(default, alias = "argument-hint", alias = "argumentHint")]
    pub argument_hint: Option<String>,
    /// Skill level (e.g., "beginner", "intermediate", "advanced")
    #[serde(default)]
    pub level: Option<String>,
    /// Alternative names that can invoke this skill
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Target agent type (e.g., "researcher", "coder")
    #[serde(default)]
    pub agent: Option<String>,
    /// Preferred model for this skill (e.g., "opus", "sonnet")
    #[serde(default)]
    pub model: Option<String>,
}

/// Loaded skill with content
#[derive(Debug, Clone)]
pub struct Skill {
    pub metadata: SkillMetadata,
    pub content: String,
    pub file_path: PathBuf,
}

/// Errors that can occur during skill loading
#[derive(Error, Debug)]
pub enum LoaderError {
    #[error("Failed to read skill file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Failed to parse frontmatter: {0}")]
    ParseError(String),
    #[error("Skill not found: {0}")]
    NotFound(String),
}

/// Skill loader that discovers and manages skills
#[derive(Debug)]
pub struct SkillLoader {
    skills_dir: PathBuf,
    cache: HashMap<String, Skill>,
    name_index: HashMap<String, String>, // alias/name -> canonical name
}

impl SkillLoader {
    /// Create a new skill loader with the specified skills directory
    pub fn new(skills_dir: impl Into<PathBuf>) -> Self {
        Self {
            skills_dir: skills_dir.into(),
            cache: HashMap::new(),
            name_index: HashMap::new(),
        }
    }

    /// Set the skills directory (builder pattern)
    pub fn with_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.skills_dir = dir.into();
        self
    }

    /// Discover all skills in the skills directory
    pub fn discover_all(&mut self) -> Result<Vec<SkillMetadata>, LoaderError> {
        self.cache.clear();
        self.name_index.clear();

        if !self.skills_dir.exists() {
            return Ok(Vec::new());
        }

        let mut skills = Vec::new();

        for entry in WalkDir::new(&self.skills_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                match self.load_skill(path) {
                    Ok(skill) => {
                        let name = skill.metadata.name.clone();
                        self.name_index.insert(name.clone(), name.clone());
                        for alias in &skill.metadata.aliases {
                            self.name_index.insert(alias.clone(), name.clone());
                        }
                        self.cache.insert(name, skill);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load skill {:?}: {}", path, e);
                    }
                }
            }
        }

        // Collect metadata for all loaded skills
        for skill in self.cache.values() {
            skills.push(skill.metadata.clone());
        }

        Ok(skills)
    }

    /// Load a single skill from a file path
    fn load_skill(&self, path: &Path) -> Result<Skill, LoaderError> {
        let content = std::fs::read_to_string(path)?;
        let metadata =
            parse_frontmatter(&content).map_err(|e| LoaderError::ParseError(e.to_string()))?;

        Ok(Skill {
            metadata,
            content,
            file_path: path.to_path_buf(),
        })
    }

    /// Get a skill by name or alias
    pub fn load(&self, name: &str) -> Result<Skill, LoaderError> {
        let canonical = self
            .name_index
            .get(name)
            .ok_or_else(|| LoaderError::NotFound(name.to_string()))?;

        self.cache
            .get(canonical)
            .cloned()
            .ok_or_else(|| LoaderError::NotFound(name.to_string()))
    }

    /// List all available skills
    pub fn list(&self) -> Vec<SkillMetadata> {
        self.cache.values().map(|s| s.metadata.clone()).collect()
    }

    /// Get skill content with variables substituted from state
    pub fn get_executed_content(
        &self,
        name: &str,
        state: &SkillStateStore,
    ) -> Result<String, LoaderError> {
        let skill = self.load(name)?;
        let mut content = skill.content.clone();

        // Replace {{variable}} placeholders with values from state
        let re = regex::Regex::new(r"\{\{(\w+)\}\}").unwrap();
        content = re
            .replace_all(&content, |caps: &regex::Captures| {
                let var_name = &caps[1];
                state
                    .get(var_name)
                    .unwrap_or_else(|| format!("{{{{{}}}}}", var_name))
            })
            .to_string();

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_discover_all() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        // Create a test skill file
        fs::write(
            skills_dir.join("test-skill.md"),
            r#"---
name: test-skill
description: A test skill
argument_hint: "task: string"
level: beginner
aliases: [test, t]
agent: coder
model: sonnet
---

# Test Skill Content

This is the body of the skill.
"#,
        )
        .unwrap();

        let mut loader = SkillLoader::new(skills_dir);
        let skills = loader.discover_all().unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "test-skill");
        assert_eq!(skills[0].description, "A test skill");
        assert_eq!(skills[0].argument_hint, Some("task: string".to_string()));
        assert_eq!(skills[0].level, Some("beginner".to_string()));
        assert_eq!(skills[0].aliases, vec!["test", "t"]);
        assert_eq!(skills[0].agent, Some("coder".to_string()));
        assert_eq!(skills[0].model, Some("sonnet".to_string()));
    }

    #[test]
    fn test_load_by_alias() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        fs::write(
            skills_dir.join("my-skill.md"),
            r#"---
name: my-skill
description: My skill description
aliases: [alias1, alias2]
---

Content here
"#,
        )
        .unwrap();

        let mut loader = SkillLoader::new(skills_dir);
        loader.discover_all().unwrap();

        // Load by canonical name
        let skill = loader.load("my-skill").unwrap();
        assert_eq!(skill.metadata.name, "my-skill");

        // Load by alias
        let skill = loader.load("alias1").unwrap();
        assert_eq!(skill.metadata.name, "my-skill");

        // Non-existent should fail
        assert!(loader.load("nonexistent").is_err());
    }

    #[test]
    fn test_list() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        fs::write(
            skills_dir.join("skill1.md"),
            "---\nname: skill1\ndescription: First\n---",
        )
        .unwrap();
        fs::write(
            skills_dir.join("skill2.md"),
            "---\nname: skill2\ndescription: Second\n---",
        )
        .unwrap();

        let mut loader = SkillLoader::new(skills_dir);
        loader.discover_all().unwrap();

        let skills = loader.list();
        assert_eq!(skills.len(), 2);
    }
}
