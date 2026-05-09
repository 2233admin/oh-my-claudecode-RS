//! Codex CLI project initialization.

use crate::adapter::{HostInitReport, HostKind};
use std::path::Path;

/// Initialize a project for Codex CLI.
/// Creates `.codex/` directory structure if not present.
pub fn codex_init_project(root: &Path) -> Result<HostInitReport, String> {
    let mut created = Vec::new();
    let mut unchanged = Vec::new();

    let codex_dir = root.join(".codex");

    // Create .codex/ directory
    if !codex_dir.exists() {
        std::fs::create_dir_all(&codex_dir)
            .map_err(|e| format!("create .codex/: {e}"))?;
        created.push(codex_dir.clone());
    } else {
        unchanged.push(codex_dir.clone());
    }

    // Create config.toml if missing
    let config_path = codex_dir.join("config.toml");
    if !config_path.exists() {
        std::fs::write(&config_path, "# Codex CLI configuration\n")
            .map_err(|e| format!("write config.toml: {e}"))?;
        created.push(config_path);
    } else {
        unchanged.push(config_path);
    }

    // Create agents/ directory
    let agents_dir = codex_dir.join("agents");
    if !agents_dir.exists() {
        std::fs::create_dir_all(&agents_dir)
            .map_err(|e| format!("create agents/: {e}"))?;
        created.push(agents_dir);
    } else {
        unchanged.push(agents_dir);
    }

    // Create skills/ directory
    let skills_dir = codex_dir.join("skills");
    if !skills_dir.exists() {
        std::fs::create_dir_all(&skills_dir)
            .map_err(|e| format!("create skills/: {e}"))?;
        created.push(skills_dir);
    } else {
        unchanged.push(skills_dir);
    }

    // Create hooks.json if missing
    let hooks_path = codex_dir.join("hooks.json");
    if !hooks_path.exists() {
        std::fs::write(&hooks_path, "{}\n")
            .map_err(|e| format!("write hooks.json: {e}"))?;
        created.push(hooks_path);
    } else {
        unchanged.push(hooks_path);
    }

    Ok(HostInitReport {
        host: HostKind::Codex,
        created,
        updated: Vec::new(),
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
        let report = codex_init_project(tmp.path()).unwrap();
        assert!(tmp.path().join(".codex/config.toml").exists());
        assert!(tmp.path().join(".codex/agents").exists());
        assert!(tmp.path().join(".codex/skills").exists());
        assert!(tmp.path().join(".codex/hooks.json").exists());
        assert!(!report.created.is_empty());
    }

    #[test]
    fn init_idempotent() {
        let tmp = TempDir::new().unwrap();
        codex_init_project(tmp.path()).unwrap();
        let report = codex_init_project(tmp.path()).unwrap();
        assert!(report.created.is_empty());
        assert!(!report.unchanged.is_empty());
    }
}
