//! Claude Code readiness check.

use crate::adapter::{HostDoctorReport, HostKind};
use std::path::Path;

/// Full doctor check for Claude Code.
pub fn claude_doctor(root: &Path) -> HostDoctorReport {
    let mut messages = Vec::new();
    let mut ready = true;

    // Check if .claude directory exists
    let claude_dir = root.join(".claude");
    if !claude_dir.exists() {
        messages
            .push("'.claude/' directory not found — run `omc setup --host claude` first".into());
        ready = false;
    }

    // Check if settings.json exists
    let settings = claude_dir.join("settings.json");
    if !settings.exists() {
        messages.push("'.claude/settings.json' not found".into());
        ready = false;
    } else {
        messages.push("'.claude/settings.json' found".into());
    }

    // Check if agents directory exists
    let agents_dir = claude_dir.join("agents");
    if agents_dir.exists() {
        let count = std::fs::read_dir(&agents_dir)
            .map_or(0, |rd| {
                rd.filter(|e| {
                    e.as_ref()
                        .is_ok_and(|e| e.path().extension().is_some())
                })
                .count()
            });
        messages.push(format!("agents directory: {count} agent files"));
    }

    // Check if skills directory exists
    let skills_dir = claude_dir.join("skills");
    if skills_dir.exists() {
        messages.push("skills directory present".into());
    }

    if ready {
        messages.push("Claude Code host ready".into());
    }

    HostDoctorReport {
        host: HostKind::Claude,
        ready,
        messages,
    }
}

/// Quick readiness check.
pub fn claude_check_ready(root: &Path) -> Result<(), String> {
    let settings = root.join(".claude").join("settings.json");
    if !settings.exists() {
        return Err("'.claude/settings.json' not found — run `omc setup` first".into());
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
        let report = claude_doctor(tmp.path());
        assert!(!report.ready);
        assert!(report.messages.iter().any(|m| m.contains("not found")));
    }

    #[test]
    fn doctor_with_settings() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".claude")).unwrap();
        std::fs::write(tmp.path().join(".claude/settings.json"), "{}").unwrap();
        let report = claude_doctor(tmp.path());
        assert!(report.ready);
    }

    #[test]
    fn check_ready_missing() {
        let tmp = TempDir::new().unwrap();
        assert!(claude_check_ready(tmp.path()).is_err());
    }

    #[test]
    fn check_ready_ok() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".claude")).unwrap();
        std::fs::write(tmp.path().join(".claude/settings.json"), "{}").unwrap();
        assert!(claude_check_ready(tmp.path()).is_ok());
    }
}
