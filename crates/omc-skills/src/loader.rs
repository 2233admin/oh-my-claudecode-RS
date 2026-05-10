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
    /// Which hosts can load this skill (e.g., ["claude", "codex"]).
    /// Empty or absent means all hosts.
    #[serde(default)]
    pub hosts: Vec<String>,
    /// Protocol version (semver string). Absent means v0 (legacy).
    #[serde(default)]
    pub protocol_version: Option<String>,
}

/// Loaded skill with content
#[derive(Debug, Clone)]
pub struct Skill {
    pub metadata: SkillMetadata,
    pub content: String,
    pub file_path: PathBuf,
    /// Companion files (relative paths from the skill directory).
    /// Only populated for directory-based skills.
    pub companions: Vec<PathBuf>,
    /// Root directory of the skill (parent of SKILL.md for directory skills,
    /// or parent of the .md file for flat-file skills).
    pub skill_dir: PathBuf,
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

    /// Discover all skills in the skills directory.
    ///
    /// Supports two types of skills:
    /// - **Flat-file skills**: a single `.md` file (legacy behavior)
    /// - **Directory skills**: a directory containing `SKILL.md` plus optional companion files
    ///
    /// Uses two-pass WalkDir:
    /// 1. Pass 1: collect directories containing SKILL.md
    /// 2. Pass 2: load flat-file .md skills, skipping files inside directory skills
    pub fn discover_all(&mut self) -> Result<Vec<SkillMetadata>, LoaderError> {
        self.cache.clear();
        self.name_index.clear();

        if !self.skills_dir.exists() {
            return Ok(Vec::new());
        }

        // Pass 1: collect directories containing SKILL.md
        let mut dir_skills: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
        for entry in WalkDir::new(&self.skills_dir)
            .follow_links(false)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_dir() && path.join("SKILL.md").exists() {
                dir_skills.insert(path.to_path_buf());
            }
        }

        // Load directory skills
        for dir in &dir_skills {
            let skill_md = dir.join("SKILL.md");
            match self.load_directory_skill(dir, &skill_md) {
                Ok(skill) => {
                    self.index_skill(&skill);
                    self.cache.insert(skill.metadata.name.clone(), skill);
                }
                Err(e) => {
                    tracing::warn!(
                        skill = %dir.display(),
                        error = %e,
                        "Failed to load directory skill"
                    );
                }
            }
        }

        // Pass 2: load flat-file .md skills (skip files inside directory skills)
        for entry in WalkDir::new(&self.skills_dir)
            .follow_links(false)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }

            // Skip files inside directory skills
            if dir_skills.iter().any(|dir| path.starts_with(dir)) {
                continue;
            }

            match self.load_flat_skill(path) {
                Ok(skill) => {
                    self.index_skill(&skill);
                    self.cache.insert(skill.metadata.name.clone(), skill);
                }
                Err(e) => {
                    tracing::warn!(
                        skill = %path.display(),
                        error = %e,
                        "Failed to load flat-file skill"
                    );
                }
            }
        }

        Ok(self.cache.values().map(|s| s.metadata.clone()).collect())
    }

    /// Index a skill's name and aliases into name_index
    fn index_skill(&mut self, skill: &Skill) {
        let name = skill.metadata.name.clone();
        self.name_index.insert(name.clone(), name.clone());
        for alias in &skill.metadata.aliases {
            self.name_index.insert(alias.clone(), name.clone());
        }
    }

    /// Load a directory-based skill (directory containing SKILL.md + companions)
    fn load_directory_skill(&self, dir: &Path, skill_md: &Path) -> Result<Skill, LoaderError> {
        let content = std::fs::read_to_string(skill_md)?;
        let metadata =
            parse_frontmatter(&content).map_err(|e| LoaderError::ParseError(e.to_string()))?;

        // Collect companion files (all files except SKILL.md)
        let mut companions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path == skill_md {
                    continue;
                }
                if path.is_file()
                    && let Ok(rel) = path.strip_prefix(dir)
                {
                    companions.push(rel.to_path_buf());
                }
            }
        }

        Ok(Skill {
            metadata,
            content,
            file_path: skill_md.to_path_buf(),
            companions,
            skill_dir: dir.to_path_buf(),
        })
    }

    /// Load a flat-file skill (single .md file)
    fn load_flat_skill(&self, path: &Path) -> Result<Skill, LoaderError> {
        let content = std::fs::read_to_string(path)?;
        let metadata =
            parse_frontmatter(&content).map_err(|e| LoaderError::ParseError(e.to_string()))?;

        Ok(Skill {
            metadata,
            content,
            file_path: path.to_path_buf(),
            companions: vec![],
            skill_dir: path.parent().unwrap_or(path).to_path_buf(),
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

    /// Read a companion file from a directory-based skill.
    /// `relative_path` is relative to the skill directory.
    pub fn read_companion(
        &self,
        skill_name: &str,
        relative_path: &str,
    ) -> Result<String, LoaderError> {
        let skill = self.load(skill_name)?;
        let companion_path = skill.skill_dir.join(relative_path);

        // Validate the path doesn't escape the skill directory
        if !companion_path.starts_with(&skill.skill_dir) {
            return Err(LoaderError::ParseError(format!(
                "Companion path escapes skill directory: {}",
                relative_path
            )));
        }

        // Check max size (1MB default)
        if let Ok(meta) = std::fs::metadata(&companion_path) {
            const MAX_COMPANION_SIZE: u64 = 1024 * 1024; // 1MB
            if meta.len() > MAX_COMPANION_SIZE {
                return Err(LoaderError::ParseError(format!(
                    "Companion file exceeds 1MB limit: {} ({} bytes)",
                    relative_path,
                    meta.len()
                )));
            }
        }

        std::fs::read_to_string(&companion_path).map_err(LoaderError::ReadError)
    }

    /// List skills filtered by host. If `host` is None, returns all skills.
    /// Skills with empty `hosts` field are available to all hosts.
    pub fn list_for_host(&self, host: Option<&str>) -> Vec<SkillMetadata> {
        self.cache
            .values()
            .filter(|s| {
                if let Some(host_name) = host {
                    s.metadata.hosts.is_empty() || s.metadata.hosts.iter().any(|h| h == host_name)
                } else {
                    true
                }
            })
            .map(|s| s.metadata.clone())
            .collect()
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

        let skill = loader.load("my-skill").unwrap();
        assert_eq!(skill.metadata.name, "my-skill");

        let skill = loader.load("alias1").unwrap();
        assert_eq!(skill.metadata.name, "my-skill");

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

    #[test]
    fn test_directory_skill_discovery() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        // Create a directory skill
        let dir_skill = skills_dir.join("tdd");
        fs::create_dir_all(&dir_skill).unwrap();
        fs::write(
            dir_skill.join("SKILL.md"),
            "---\nname: tdd\ndescription: Test-driven development\n---\n\n# TDD\n\nDo TDD.",
        )
        .unwrap();
        fs::write(
            dir_skill.join("tests.md"),
            "# Test guidelines\n\nWrite tests first.",
        )
        .unwrap();
        fs::write(
            dir_skill.join("mocking.md"),
            "# Mocking\n\nMock at boundaries.",
        )
        .unwrap();

        // Also create a flat-file skill
        fs::write(
            skills_dir.join("simple.md"),
            "---\nname: simple\ndescription: Simple skill\n---\n\nSimple.",
        )
        .unwrap();

        let mut loader = SkillLoader::new(skills_dir);
        let skills = loader.discover_all().unwrap();

        assert_eq!(skills.len(), 2);

        // Directory skill should have companions
        let tdd = loader.load("tdd").unwrap();
        assert_eq!(tdd.companions.len(), 2);
        assert!(
            tdd.companions
                .iter()
                .any(|c| c.to_str() == Some("tests.md"))
        );
        assert!(
            tdd.companions
                .iter()
                .any(|c| c.to_str() == Some("mocking.md"))
        );
        assert_eq!(tdd.skill_dir, dir_skill);

        // Flat-file skill should have no companions
        let simple = loader.load("simple").unwrap();
        assert!(simple.companions.is_empty());
    }

    #[test]
    fn test_read_companion() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        let dir_skill = skills_dir.join("my-skill");
        fs::create_dir_all(&dir_skill).unwrap();
        fs::write(
            dir_skill.join("SKILL.md"),
            "---\nname: my-skill\ndescription: Test\n---\n\nContent.",
        )
        .unwrap();
        fs::write(dir_skill.join("helper.md"), "# Helper\n\nHelpful content.").unwrap();

        let mut loader = SkillLoader::new(skills_dir);
        loader.discover_all().unwrap();

        let content = loader.read_companion("my-skill", "helper.md").unwrap();
        assert_eq!(content, "# Helper\n\nHelpful content.");

        // Path traversal should fail
        assert!(loader.read_companion("my-skill", "../other.md").is_err());
    }

    #[test]
    fn test_host_filtering() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        // Skill for all hosts (empty hosts)
        fs::write(
            skills_dir.join("universal.md"),
            "---\nname: universal\ndescription: Works everywhere\n---",
        )
        .unwrap();

        // Skill only for claude
        fs::write(
            skills_dir.join("claude-only.md"),
            "---\nname: claude-only\ndescription: Claude only\nhosts: [claude]\n---",
        )
        .unwrap();

        // Skill only for codex
        fs::write(
            skills_dir.join("codex-only.md"),
            "---\nname: codex-only\ndescription: Codex only\nhosts: [codex]\n---",
        )
        .unwrap();

        let mut loader = SkillLoader::new(skills_dir);
        loader.discover_all().unwrap();

        // All skills visible without host filter
        assert_eq!(loader.list().len(), 3);

        // Claude sees universal + claude-only
        let claude_skills = loader.list_for_host(Some("claude"));
        assert_eq!(claude_skills.len(), 2);
        assert!(claude_skills.iter().any(|s| s.name == "universal"));
        assert!(claude_skills.iter().any(|s| s.name == "claude-only"));

        // Codex sees universal + codex-only
        let codex_skills = loader.list_for_host(Some("codex"));
        assert_eq!(codex_skills.len(), 2);
        assert!(codex_skills.iter().any(|s| s.name == "universal"));
        assert!(codex_skills.iter().any(|s| s.name == "codex-only"));
    }

    #[test]
    fn test_directory_skill_not_loaded_as_flat() {
        let temp_dir = TempDir::new().unwrap();
        let skills_dir = temp_dir.path();

        // Create directory skill with companion .md files
        let dir_skill = skills_dir.join("my-skill");
        fs::create_dir_all(&dir_skill).unwrap();
        fs::write(
            dir_skill.join("SKILL.md"),
            "---\nname: my-skill\ndescription: Dir skill\n---",
        )
        .unwrap();
        fs::write(
            dir_skill.join("README.md"),
            "# README\n\nThis should NOT be loaded as a separate skill.",
        )
        .unwrap();

        let mut loader = SkillLoader::new(skills_dir);
        loader.discover_all().unwrap();

        // Only 1 skill loaded (the directory skill), not the README.md
        assert_eq!(loader.list().len(), 1);
        assert!(loader.load("my-skill").is_ok());
    }
}
