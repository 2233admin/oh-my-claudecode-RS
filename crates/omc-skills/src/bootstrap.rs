use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Bootstrap the `.omc/skills/` directory structure.
/// Creates `<root>/.omc/skills/` if it doesn't exist.
/// Called during `omc setup` or on first skill discovery.
pub fn bootstrap_omc_skills(root: &Path) -> Result<PathBuf, io::Error> {
    let omc_skills = root.join(".omc").join("skills");
    if !omc_skills.exists() {
        fs::create_dir_all(&omc_skills)?;
        tracing::info!(path = %omc_skills.display(), "Created .omc/skills directory");
    }
    Ok(omc_skills)
}

/// Bootstrap the full `.omc/` directory structure (all subdirs).
pub fn bootstrap_omc_dir(root: &Path) -> Result<(), io::Error> {
    let omc = root.join(".omc");
    for subdir in &[
        "skills",
        "team",
        "team/missions",
        "team/runs",
        "team/whiteboard",
    ] {
        let path = omc.join(subdir);
        if !path.exists() {
            fs::create_dir_all(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn bootstrap_skills_creates_directory() {
        let tmp = TempDir::new().unwrap();
        let result = bootstrap_omc_skills(tmp.path()).unwrap();
        assert!(result.exists());
        assert_eq!(result, tmp.path().join(".omc").join("skills"));
    }

    #[test]
    fn bootstrap_skills_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let first = bootstrap_omc_skills(tmp.path()).unwrap();
        let second = bootstrap_omc_skills(tmp.path()).unwrap();
        assert_eq!(first, second);
        assert!(first.exists());
    }

    #[test]
    fn bootstrap_skills_returns_correct_path() {
        let tmp = TempDir::new().unwrap();
        let path = bootstrap_omc_skills(tmp.path()).unwrap();
        assert_eq!(path, tmp.path().join(".omc").join("skills"));
    }

    #[test]
    fn bootstrap_omc_dir_creates_all_subdirs() {
        let tmp = TempDir::new().unwrap();
        bootstrap_omc_dir(tmp.path()).unwrap();
        let omc = tmp.path().join(".omc");
        assert!(omc.join("skills").exists());
        assert!(omc.join("team").exists());
        assert!(omc.join("team").join("missions").exists());
        assert!(omc.join("team").join("runs").exists());
        assert!(omc.join("team").join("whiteboard").exists());
    }

    #[test]
    fn bootstrap_omc_dir_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        bootstrap_omc_dir(tmp.path()).unwrap();
        bootstrap_omc_dir(tmp.path()).unwrap();
        let omc = tmp.path().join(".omc");
        assert!(omc.join("skills").exists());
        assert!(omc.join("team").exists());
    }
}
