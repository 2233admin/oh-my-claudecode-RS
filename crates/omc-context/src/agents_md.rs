//! AGENTS.md Management
//!
//! Discovers, parses, and manages AGENTS.md files for project and user-level
//! agent configuration. AGENTS.md files define agent behavior, rules, and
//! context that should be injected when working in a project.
//!
//! Ported from oh-my-claudecode's agents module.

use dashmap::DashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum AgentsMdError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
}

/// Parsed AGENTS.md section.
#[derive(Debug, Clone)]
pub struct AgentsMdSection {
    pub heading: String,
    pub level: u8,
    pub content: String,
}

/// Parsed AGENTS.md file.
#[derive(Debug, Clone)]
pub struct AgentsMdFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub sections: Vec<AgentsMdSection>,
    pub raw_content: String,
    pub is_global: bool,
}

/// Manages AGENTS.md file discovery and parsing.
#[derive(Clone)]
pub struct AgentsMdManager {
    cache: Arc<DashMap<PathBuf, AgentsMdFile>>,
}

impl Default for AgentsMdManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentsMdManager {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
        }
    }

    /// Find and parse all AGENTS.md files for a given project path.
    /// Searches from the file's directory up to the project root and user home.
    pub async fn find_agents_files(&self, project_root: &Path) -> Vec<AgentsMdFile> {
        let mut files = Vec::new();

        // Project-level AGENTS.md
        let project_agents = project_root.join("AGENTS.md");
        if let Ok(Some(file)) = self
            .load_or_cache(&project_agents, project_root, false)
            .await
        {
            files.push(file);
        }

        // .claude/AGENTS.md
        let claude_agents = project_root.join(".claude").join("AGENTS.md");
        if let Ok(Some(file)) = self
            .load_or_cache(&claude_agents, project_root, false)
            .await
        {
            files.push(file);
        }

        // User-level AGENTS.md
        if let Some(home) = dirs::home_dir() {
            let user_agents = home.join("AGENTS.md");
            if let Ok(Some(file)) = self.load_or_cache(&user_agents, &home, true).await {
                files.push(file);
            }

            let user_claude_agents = home.join(".claude").join("AGENTS.md");
            if let Ok(Some(file)) = self.load_or_cache(&user_claude_agents, &home, true).await {
                files.push(file);
            }
        }

        files
    }

    /// Get the merged content of all AGENTS.md files for a project.
    pub async fn get_merged_content(&self, project_root: &Path) -> String {
        let files = self.find_agents_files(project_root).await;
        if files.is_empty() {
            return String::default();
        }

        let mut parts = Vec::default();
        for file in &files {
            parts.push(format!(
                "[AGENTS.md: {}]\n{}",
                file.relative_path, file.raw_content
            ));
        }
        parts.join("\n\n---\n\n")
    }

    /// Parse an AGENTS.md file into sections.
    pub fn parse(content: &str) -> Vec<AgentsMdSection> {
        let mut sections = Vec::default();
        let mut current_heading = String::default();
        let mut current_level: u8 = 0;
        let mut current_content = String::default();

        for line in content.lines() {
            if let Some((level, heading)) = parse_heading(line) {
                if !current_heading.is_empty() {
                    sections.push(AgentsMdSection {
                        heading: current_heading.clone(),
                        level: current_level,
                        content: current_content.trim().to_string(),
                    });
                }
                current_heading = heading;
                current_level = level;
                current_content.clear();
            } else {
                current_content.push_str(line);
                current_content.push('\n');
            }
        }

        if !current_heading.is_empty() {
            sections.push(AgentsMdSection {
                heading: current_heading,
                level: current_level,
                content: current_content.trim().to_string(),
            });
        }

        sections
    }

    /// Clear the file cache.
    pub async fn clear_cache(&self) {
        self.cache.clear();
    }

    async fn load_or_cache(
        &self,
        path: &Path,
        root: &Path,
        is_global: bool,
    ) -> Result<Option<AgentsMdFile>, AgentsMdError> {
        // Check cache first
        if let Some(file) = self.cache.get(path) {
            return Ok(Some(file.clone()));
        }

        if !path.exists() {
            return Ok(None);
        }

        let raw_content = tokio::fs::read_to_string(path).await?;
        let relative_path = pathdiff::diff_paths(path, root).map_or_else(|| path.to_string_lossy().to_string(), |p| p.to_string_lossy().to_string());

        let sections = Self::parse(&raw_content);
        let file = AgentsMdFile {
            path: path.to_path_buf(),
            relative_path,
            sections,
            raw_content,
            is_global,
        };

        self.cache.insert(path.to_path_buf(), file.clone());
        Ok(Some(file))
    }
}

/// Parse a markdown heading line, returning (level, text).
fn parse_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('#') {
        return None;
    }

    let level = trimmed.chars().take_while(|&c| c == '#').count() as u8;
    if level == 0 || level > 6 {
        return None;
    }

    let heading = trimmed[level as usize..].trim().to_string();
    if heading.is_empty() {
        return None;
    }

    Some((level, heading))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_heading() {
        assert_eq!(parse_heading("# Title"), Some((1, "Title".into())));
        assert_eq!(
            parse_heading("## Sub Heading"),
            Some((2, "Sub Heading".into()))
        );
        assert_eq!(parse_heading("Not a heading"), None);
        assert_eq!(parse_heading("#"), None);
    }

    #[test]
    fn test_parse_agents_md() {
        let content = "# Rules\n\nDo X.\n\n## Sub\n\nDo Y.\n";
        let sections = AgentsMdManager::parse(content);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].heading, "Rules");
        assert_eq!(sections[0].level, 1);
        assert_eq!(sections[1].heading, "Sub");
        assert_eq!(sections[1].level, 2);
        assert!(sections[1].content.contains("Do Y."));
    }

    #[tokio::test]
    async fn test_find_agents_files_nonexistent() {
        let manager = AgentsMdManager::default();
        let tmp = tempfile::tempdir().unwrap();
        let files = manager.find_agents_files(tmp.path()).await;
        // May find user-level files, but no project-level ones
        assert!(files.iter().all(|f| f.is_global));
    }

    #[tokio::test]
    async fn test_find_agents_files_with_project_agents() {
        let tmp = tempfile::tempdir().unwrap();
        let agents_path = tmp.path().join("AGENTS.md");
        std::fs::write(&agents_path, "# Test\nContent here\n").unwrap();

        let manager = AgentsMdManager::default();
        let files = manager.find_agents_files(tmp.path()).await;
        let project_files: Vec<_> = files.iter().filter(|f| !f.is_global).collect();
        assert_eq!(project_files.len(), 1);
        assert_eq!(project_files[0].sections[0].heading, "Test");
    }
}
