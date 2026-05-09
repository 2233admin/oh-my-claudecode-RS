//! Codex CLI readiness check.

use crate::adapter::{HostDoctorReport, HostKind};
use std::path::Path;

/// Full doctor check for Codex CLI.
pub fn codex_doctor(root: &Path) -> HostDoctorReport {
    let mut messages = Vec::new();
    let mut ready = true;

    // Check .codex/ directory
    let codex_dir = root.join(".codex");
    if !codex_dir.exists() {
        messages.push("'.codex/' directory not found — run `omc setup --host codex` first".into());
        ready = false;
    }

    // Check config.toml
    let config = codex_dir.join("config.toml");
    if !config.exists() {
        messages.push("'.codex/config.toml' not found".into());
        ready = false;
    } else {
        messages.push("'.codex/config.toml' found".into());
    }

    // Check agents directory
    let agents_dir = codex_dir.join("agents");
    if agents_dir.exists() {
        let count = std::fs::read_dir(&agents_dir)
            .map(|rd| {
                rd.filter(|e| {
                    e.as_ref()
                        .map(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
                        .unwrap_or(false)
                })
                .count()
            })
            .unwrap_or(0);
        messages.push(format!("agents directory: {count} TOML agent files"));
    }

    // Check hooks.json
    let hooks = codex_dir.join("hooks.json");
    if hooks.exists() {
        messages.push("hooks.json present".into());
    }

    if ready {
        messages.push("Codex CLI host ready".into());
    }

    HostDoctorReport {
        host: HostKind::Codex,
        ready,
        messages,
    }
}

/// Quick readiness check.
pub fn codex_check_ready(root: &Path) -> Result<(), String> {
    let config = root.join(".codex").join("config.toml");
    if !config.exists() {
        return Err("'.codex/config.toml' not found — run `omc setup --host codex` first".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn doctor_missing_dir() {
        let tmp = TempDir::new().unwrap();
        let report = codex_doctor(tmp.path());
        assert!(!report.ready);
    }

    #[test]
    fn doctor_with_config() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".codex")).unwrap();
        std::fs::write(tmp.path().join(".codex/config.toml"), "").unwrap();
        let report = codex_doctor(tmp.path());
        assert!(report.ready);
    }

    #[test]
    fn check_ready_missing() {
        let tmp = TempDir::new().unwrap();
        assert!(codex_check_ready(tmp.path()).is_err());
    }

    #[test]
    fn check_ready_ok() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".codex")).unwrap();
        std::fs::write(tmp.path().join(".codex/config.toml"), "").unwrap();
        assert!(codex_check_ready(tmp.path()).is_ok());
    }
}
