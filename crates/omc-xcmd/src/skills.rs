//! Skills listing and searching for x-cmd

use std::path::{Path, PathBuf};
use serde::Serialize;

/// Skill information
#[derive(Debug, Serialize)]
pub struct Skill {
    pub name: String,
    pub path: PathBuf,
    pub description: Option<String>,
}

/// List all x-cmd skills
pub fn list_skills() -> Vec<Skill> {
    let skills_dir = match super::skills_dir() {
        Some(p) => p,
        None => return vec![],
    };
    
    if !skills_dir.exists() {
        return vec![];
    }
    
    let mut skills = vec![];
    if let Ok(entries) = std::fs::read_dir(&skills_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                
                let description = read_skill_description(&path);
                skills.push(Skill { name, path, description });
            }
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// Read skill description from SKILL.md
fn read_skill_description(path: &Path) -> Option<String> {
    let skill_md = path.join("SKILL.md");
    if skill_md.exists() {
        std::fs::read_to_string(&skill_md).ok()
            .and_then(|content| {
                content.lines()
                    .find(|l| l.trim().starts_with("description:"))
                    .map(|l| l.replace("description:", "").trim().to_string())
            })
    } else {
        None
    }
}

/// Search skills by name or description
pub fn search_skills(term: &str) -> Vec<Skill> {
    let term = term.to_lowercase();
    list_skills().into_iter()
        .filter(|s| {
            s.name.to_lowercase().contains(&term) ||
            s.description.as_ref()
                .map(|d| d.to_lowercase().contains(&term))
                .unwrap_or(false)
        })
        .collect()
}

/// Get skill count
pub fn skill_count() -> usize {
    list_skills().len()
}
