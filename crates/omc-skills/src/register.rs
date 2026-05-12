//! Skill registration — links skill source directories into host skills directories.
//!
//! Uses directory symlinks/junctions (platform-specific) with recursive copy as fallback.
//! The `SkillLoader` then discovers skills from the linked structure.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::loader::SkillMetadata;

/// Result of a registration operation.
#[derive(Debug, Default)]
pub struct RegistrationResult {
    /// Source directories that were successfully linked (symlink/junction).
    pub linked: Vec<PathBuf>,
    /// Source directories that were copied (fallback when linking failed).
    pub copied: Vec<PathBuf>,
    /// Link names that already existed (skipped).
    pub skipped: Vec<PathBuf>,
    /// Sources that failed with an error message.
    pub errors: Vec<(PathBuf, String)>,
}

/// Registers skill source directories into a host skills directory.
///
/// Creates symlinks/junctions from `host_skills_dir/<link_name>` -> `source_dir`.
/// Falls back to recursive directory copy when linking fails (e.g., no symlink
/// privileges on Windows).
///
/// The `SkillLoader` with `follow_links(false)` then discovers skills from the
/// linked structure and handles collision resolution by name precedence.
pub struct SkillRegistrar {
    host_skills_dir: PathBuf,
}

impl SkillRegistrar {
    /// Create a new registrar targeting the given host skills directory.
    /// The directory must exist (call `init_project()` first).
    pub fn new(host_skills_dir: impl Into<PathBuf>) -> Self {
        Self {
            host_skills_dir: host_skills_dir.into(),
        }
    }

    /// The host skills directory this registrar targets.
    pub fn host_skills_dir(&self) -> &Path {
        &self.host_skills_dir
    }

    /// Register a skill source directory into the host skills directory.
    ///
    /// Creates a link at `host_skills_dir/<link_name>` pointing to `source_dir`.
    /// If the link already exists, it is skipped (idempotent).
    /// If linking fails, falls back to recursive copy.
    pub fn register(
        &self,
        source_dir: &Path,
        link_name: &str,
    ) -> Result<RegistrationResult, io::Error> {
        let mut result = RegistrationResult::default();
        let link_path = self.host_skills_dir.join(link_name);

        if !source_dir.exists() {
            result.errors.push((
                source_dir.to_path_buf(),
                "source directory does not exist".into(),
            ));
            return Ok(result);
        }

        if link_path.exists() || link_path.symlink_metadata().is_ok() {
            result.skipped.push(link_path);
            return Ok(result);
        }

        match create_link(source_dir, &link_path) {
            Ok(()) => {
                result.linked.push(source_dir.to_path_buf());
            }
            Err(link_err) => {
                tracing::warn!(
                    source = %source_dir.display(),
                    link = %link_path.display(),
                    error = %link_err,
                    "Symlink/junction failed, falling back to copy"
                );
                match copy_dir_recursive(source_dir, &link_path) {
                    Ok(()) => {
                        result.copied.push(source_dir.to_path_buf());
                    }
                    Err(copy_err) => {
                        result.errors.push((
                            source_dir.to_path_buf(),
                            format!("link failed: {link_err}; copy failed: {copy_err}"),
                        ));
                    }
                }
            }
        }

        Ok(result)
    }

    /// Register multiple source directories (batch operation).
    ///
    /// Each entry is `(source_dir, link_name)`.
    pub fn register_all(&self, sources: &[(PathBuf, String)]) -> RegistrationResult {
        let mut combined = RegistrationResult::default();
        for (source, name) in sources {
            match self.register(source, name) {
                Ok(result) => {
                    combined.linked.extend(result.linked);
                    combined.copied.extend(result.copied);
                    combined.skipped.extend(result.skipped);
                    combined.errors.extend(result.errors);
                }
                Err(e) => {
                    combined
                        .errors
                        .push((source.clone(), format!("register failed: {e}")));
                }
            }
        }
        combined
    }

    /// Remove a registered link or copy. Does NOT delete the source.
    ///
    /// If the link is a symlink/junction, removes only the link.
    /// If it's a copied directory, removes the copy.
    /// If it doesn't exist, returns Ok (idempotent).
    pub fn unregister(&self, link_name: &str) -> Result<(), io::Error> {
        let link_path = self.host_skills_dir.join(link_name);

        if !link_path.exists() && link_path.symlink_metadata().is_err() {
            return Ok(());
        }

        // Check if it's a symlink/junction (metadata without following).
        let meta = link_path.symlink_metadata()?;
        if meta.is_symlink() || meta.file_type().is_symlink() {
            remove_symlink_or_junction(&link_path)?;
        } else if meta.is_dir() {
            // It's a copy — remove the entire directory
            fs::remove_dir_all(&link_path)?;
        } else {
            fs::remove_file(&link_path)?;
        }

        Ok(())
    }

    /// List currently registered entries in the host skills directory.
    pub fn list_registered(&self) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        if let Ok(read_dir) = fs::read_dir(&self.host_skills_dir) {
            for entry in read_dir.filter_map(std::result::Result::ok) {
                entries.push(entry.path());
            }
        }
        entries
    }

    /// Generate a Codex `skills.toml` manifest from skill metadata.
    ///
    /// The manifest lists all skills with their name, description, and path
    /// relative to the `.codex/` directory.
    pub fn generate_codex_manifest(&self, skills: &[SkillMetadata]) -> String {
        let mut toml = String::new();
        toml.push_str("# Auto-generated by omc setup --host codex\n");
        toml.push_str("# Do not edit manually.\n\n");

        for skill in skills {
            toml.push_str("[[skills]]\n");
            let _ = writeln!(toml, "name = \"{}\"", escape_toml_string(&skill.name));
            let _ = writeln!(
                toml,
                "description = \"{}\"",
                escape_toml_string(&skill.description)
            );
            // Path is relative to .codex/ — for directory skills, use skill_dir;
            // for flat-file skills, use the .md file path.
            // Since we register directories, the path is skills/<link_name>/SKILL.md
            // or just the skill name for flat files.
            let _ = writeln!(
                toml,
                "path = \"skills/{}\"",
                escape_toml_string(&skill.name)
            );
            toml.push('\n');
        }

        toml
    }
}

/// Escape a string for TOML value embedding.
fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Create a platform-specific link from `source` to `link_path`.
///
/// - Unix: `std::os::unix::fs::symlink`
/// - Windows: `std::os::windows::fs::symlink_dir` (requires Developer Mode or elevation)
///
/// Returns an error if linking is not supported or fails.
fn create_link(source: &Path, link_path: &Path) -> Result<(), io::Error> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, link_path)
    }

    #[cfg(windows)]
    {
        // First ensure the parent directory exists
        if let Some(parent) = link_path.parent() {
            fs::create_dir_all(parent)?;
        }
        std::os::windows::fs::symlink_dir(source, link_path)
    }

    #[cfg(not(any(unix, windows)))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symlink not supported on this platform",
        ))
    }
}

/// Recursively copy a directory from `source` to `dest`.
///
/// Creates `dest` if it doesn't exist. Copies all files and subdirectories.
fn copy_dir_recursive(source: &Path, dest: &Path) -> Result<(), io::Error> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &dest_path)?;
        } else {
            fs::copy(&source_path, &dest_path)?;
        }
    }

    Ok(())
}

/// Remove a symlink or junction without touching the target directory.
///
/// On Windows, tries `fs::remove_file` first (works for directory symlinks),
/// then falls back to `fs::remove_dir` (works for junctions).
/// On Unix, uses `fs::remove_file`.
fn remove_symlink_or_junction(link_path: &Path) -> Result<(), io::Error> {
    // On all platforms, remove_file removes symlinks (file and directory)
    // without following them.
    if fs::remove_file(link_path).is_ok() {
        return Ok(());
    }

    // Fallback: remove_dir works for junctions on Windows
    fs::remove_dir(link_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::SkillLoader;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_skill(dir: &Path, name: &str, description: &str) {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {description}\n---\n\nContent."),
        )
        .unwrap();
    }

    fn create_flat_skill(dir: &Path, name: &str, description: &str) {
        fs::write(
            dir.join(format!("{name}.md")),
            format!("---\nname: {name}\ndescription: {description}\n---\n\nContent."),
        )
        .unwrap();
    }

    #[test]
    fn test_register_creates_link() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&host).unwrap();
        create_test_skill(&source, "my-skill", "A test skill");

        let registrar = SkillRegistrar::new(&host);
        let result = registrar.register(&source, "my-skills").unwrap();

        assert_eq!(result.linked.len(), 1);
        assert!(result.copied.is_empty());
        assert!(result.errors.is_empty());
        assert!(host.join("my-skills").exists());
    }

    #[test]
    fn test_register_idempotent() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&host).unwrap();

        let registrar = SkillRegistrar::new(&host);

        let r1 = registrar.register(&source, "my-skills").unwrap();
        assert_eq!(r1.linked.len() + r1.copied.len(), 1);

        let r2 = registrar.register(&source, "my-skills").unwrap();
        assert_eq!(r2.skipped.len(), 1);
        assert!(r2.linked.is_empty());
        assert!(r2.copied.is_empty());
    }

    #[test]
    fn test_register_nonexistent_source() {
        let temp = TempDir::new().unwrap();
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&host).unwrap();

        let registrar = SkillRegistrar::new(&host);
        let result = registrar
            .register(&temp.path().join("nonexistent"), "missing")
            .unwrap();

        assert_eq!(result.errors.len(), 1);
        assert!(result.errors[0].1.contains("does not exist"));
    }

    #[test]
    fn test_unregister_removes_link() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&host).unwrap();

        let registrar = SkillRegistrar::new(&host);
        registrar.register(&source, "my-skills").unwrap();
        assert!(host.join("my-skills").exists());

        registrar.unregister("my-skills").unwrap();
        assert!(!host.join("my-skills").exists());
        // Source must survive
        assert!(source.exists());
    }

    #[test]
    fn test_unregister_nonexistent() {
        let temp = TempDir::new().unwrap();
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&host).unwrap();

        let registrar = SkillRegistrar::new(&host);
        // Should not error on missing link
        registrar.unregister("nonexistent").unwrap();
    }

    #[test]
    fn test_unregister_does_not_delete_source() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&host).unwrap();
        create_test_skill(&source, "keep-me", "Should survive");

        let registrar = SkillRegistrar::new(&host);
        registrar.register(&source, "src").unwrap();
        registrar.unregister("src").unwrap();

        assert!(source.exists());
        assert!(source.join("keep-me").join("SKILL.md").exists());
    }

    #[test]
    fn test_list_registered() {
        let temp = TempDir::new().unwrap();
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&host).unwrap();

        let registrar = SkillRegistrar::new(&host);

        for i in 0..3 {
            let src = temp.path().join(format!("src{i}"));
            fs::create_dir_all(&src).unwrap();
            registrar.register(&src, &format!("skill-{i}")).unwrap();
        }

        let entries = registrar.list_registered();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_register_multiple_sources() {
        let temp = TempDir::new().unwrap();
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&host).unwrap();

        let sources: Vec<(PathBuf, String)> = (0..3)
            .map(|i| {
                let src = temp.path().join(format!("src{i}"));
                fs::create_dir_all(&src).unwrap();
                (src, format!("link-{i}"))
            })
            .collect();

        let registrar = SkillRegistrar::new(&host);
        let result = registrar.register_all(&sources);

        assert_eq!(result.linked.len() + result.copied.len(), 3);
        assert!(result.errors.is_empty());
        assert_eq!(registrar.list_registered().len(), 3);
    }

    #[test]
    fn test_codex_manifest_generation() {
        let temp = TempDir::new().unwrap();
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&host).unwrap();

        let skills = vec![
            SkillMetadata {
                name: "tdd".to_string(),
                description: "Test-driven development".to_string(),
                argument_hint: None,
                level: None,
                aliases: vec![],
                agent: None,
                model: None,
                hosts: vec![],
                protocol_version: None,
            },
            SkillMetadata {
                name: "diagnose".to_string(),
                description: "Bug diagnosis loop".to_string(),
                argument_hint: None,
                level: None,
                aliases: vec![],
                agent: None,
                model: None,
                hosts: vec![],
                protocol_version: None,
            },
        ];

        let registrar = SkillRegistrar::new(&host);
        let manifest = registrar.generate_codex_manifest(&skills);

        assert!(manifest.contains("[[skills]]"));
        assert!(manifest.contains("name = \"tdd\""));
        assert!(manifest.contains("description = \"Test-driven development\""));
        assert!(manifest.contains("path = \"skills/tdd\""));
        assert!(manifest.contains("name = \"diagnose\""));
    }

    #[test]
    fn test_codex_manifest_empty() {
        let temp = TempDir::new().unwrap();
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&host).unwrap();

        let registrar = SkillRegistrar::new(&host);
        let manifest = registrar.generate_codex_manifest(&[]);

        assert!(manifest.contains("# Auto-generated"));
        assert!(!manifest.contains("[[skills]]"));
    }

    #[test]
    fn test_codex_manifest_escapes_special_chars() {
        let temp = TempDir::new().unwrap();
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&host).unwrap();

        let skills = vec![SkillMetadata {
            name: "test".to_string(),
            description: "A \"quoted\" skill with\nnewlines".to_string(),
            argument_hint: None,
            level: None,
            aliases: vec![],
            agent: None,
            model: None,
            hosts: vec![],
            protocol_version: None,
        }];

        let registrar = SkillRegistrar::new(&host);
        let manifest = registrar.generate_codex_manifest(&skills);

        assert!(manifest.contains(r#"description = "A \"quoted\" skill with\nnewlines""#));
    }

    #[test]
    fn test_link_preserves_directory_structure() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&host).unwrap();

        create_test_skill(&source, "skill-a", "First skill");
        create_test_skill(&source, "skill-b", "Second skill");
        create_flat_skill(&source, "flat-skill", "A flat skill");

        let registrar = SkillRegistrar::new(&host);
        registrar.register(&source, "all-skills").unwrap();

        // Verify structure is accessible through the link
        let linked = host.join("all-skills");
        assert!(linked.join("skill-a").join("SKILL.md").exists());
        assert!(linked.join("skill-b").join("SKILL.md").exists());
        assert!(linked.join("flat-skill.md").exists());
    }

    #[test]
    fn test_end_to_end_register_then_discover() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&host).unwrap();

        create_test_skill(&source, "alpha", "Alpha skill");
        create_test_skill(&source, "beta", "Beta skill");

        let registrar = SkillRegistrar::new(&host);

        // Register individual skill directories (not the parent) because
        // SkillLoader uses follow_links(false) and won't traverse into
        // symlinked directories on all platforms.
        let alpha_dir = source.join("alpha");
        let beta_dir = source.join("beta");
        registrar.register(&alpha_dir, "alpha").unwrap();
        registrar.register(&beta_dir, "beta").unwrap();

        // SkillLoader should discover skills from the registered dirs
        let mut loader = SkillLoader::new(&host);
        let skills = loader.discover_all().unwrap();

        assert_eq!(skills.len(), 2);
        assert!(skills.iter().any(|s| s.name == "alpha"));
        assert!(skills.iter().any(|s| s.name == "beta"));
    }

    #[test]
    fn test_registration_result_tracks_status() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&host).unwrap();

        let registrar = SkillRegistrar::new(&host);

        // First register: should link or copy
        let r1 = registrar.register(&source, "link1").unwrap();
        assert_eq!(r1.linked.len() + r1.copied.len(), 1);
        assert!(r1.skipped.is_empty());
        assert!(r1.errors.is_empty());

        // Second register same name: should skip
        let r2 = registrar.register(&source, "link1").unwrap();
        assert!(r2.linked.is_empty());
        assert!(r2.copied.is_empty());
        assert_eq!(r2.skipped.len(), 1);

        // Nonexistent source: should error
        let r3 = registrar
            .register(&temp.path().join("nope"), "link2")
            .unwrap();
        assert!(r3.linked.is_empty());
        assert_eq!(r3.errors.len(), 1);
    }

    #[test]
    fn test_register_with_special_characters_in_name() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let host = temp.path().join("host-skills");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&host).unwrap();

        create_test_skill(&source, "my-cool_skill", "Has hyphens and underscores");

        let registrar = SkillRegistrar::new(&host);
        let result = registrar.register(&source, "my-cool_skill").unwrap();

        assert_eq!(result.linked.len() + result.copied.len(), 1);
        assert!(host.join("my-cool_skill").exists());
    }
}
