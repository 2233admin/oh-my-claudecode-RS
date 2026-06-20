//! Claude Code project initialization.

use crate::adapter::{HostInitReport, HostKind};
use std::path::Path;

/// Initialize a project for Claude Code.
/// Creates `.claude/` directory structure with empty settings if not present.
pub fn claude_init_project(root: &Path) -> Result<HostInitReport, String> {
    let mut created = Vec::new();
    let updated = Vec::new();
    let mut unchanged = Vec::new();

    let claude_dir = root.join(".claude");

    // Create .claude/ directory
    if !claude_dir.exists() {
        std::fs::create_dir_all(&claude_dir).map_err(|e| format!("create .claude/: {e}"))?;
        created.push(claude_dir.clone());
    } else {
        unchanged.push(claude_dir.clone());
    }

    // Create settings.json if missing
    let settings_path = claude_dir.join("settings.json");
    if !settings_path.exists() {
        std::fs::write(&settings_path, "{\n}\n")
            .map_err(|e| format!("write settings.json: {e}"))?;
        created.push(settings_path);
    } else {
        unchanged.push(settings_path);
    }

    // Create agents/ directory
    let agents_dir = claude_dir.join("agents");
    if !agents_dir.exists() {
        std::fs::create_dir_all(&agents_dir).map_err(|e| format!("create agents/: {e}"))?;
        created.push(agents_dir);
    } else {
        unchanged.push(agents_dir);
    }

    // Create skills/ directory
    let skills_dir = claude_dir.join("skills");
    if !skills_dir.exists() {
        std::fs::create_dir_all(&skills_dir).map_err(|e| format!("create skills/: {e}"))?;
        created.push(skills_dir);
    } else {
        unchanged.push(skills_dir);
    }

    Ok(HostInitReport {
        host: HostKind::Claude,
        created,
        updated,
        unchanged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn init_creates_structure() {
        let tmp = TempDir::new().unwrap();
        let report = claude_init_project(tmp.path()).unwrap();
        assert!(tmp.path().join(".claude/settings.json").exists());
        assert!(tmp.path().join(".claude/agents").exists());
        assert!(tmp.path().join(".claude/skills").exists());
        assert!(!report.created.is_empty());
    }

    #[test]
    fn init_idempotent() {
        let tmp = TempDir::new().unwrap();
        claude_init_project(tmp.path()).unwrap();
        let report = claude_init_project(tmp.path()).unwrap();
        assert!(report.created.is_empty());
        assert!(!report.unchanged.is_empty());
    }
}
