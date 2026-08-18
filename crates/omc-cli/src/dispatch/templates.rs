use super::*;

/// Load a skill template by name from the skills directory.
///
/// Search order:
/// 1. `OMC_SKILLS_DIR` environment variable
/// 2. `<crate_parent>/crates/omc-skills/src/templates/<name>.md`
/// 3. `~/.omc/skills/<name>/SKILL.md`
pub(super) fn load_template(name: &str) -> Result<String, DispatchError> {
    let candidates = template_search_paths(name);

    for path in &candidates {
        if path.exists() {
            return std::fs::read_to_string(path).map_err(Into::into);
        }
    }

    Err(DispatchError::NotFound(format!(
        "{name} (searched: {})",
        candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

/// Build the ordered list of paths to check for a skill template.
static OMC_SKILLS_DIR: &str = "OMC_SKILLS_DIR";
static OMC_HOME: &str = "OMC_HOME";

pub(super) fn template_search_paths(name: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. Explicit environment override
    if let Ok(dir) = std::env::var(OMC_SKILLS_DIR) {
        paths.push(PathBuf::from(dir).join(format!("{name}.md")));
    }

    // 2. Sibling crate templates directory (dev / repo layout)
    if let Ok(exe) = std::env::current_exe() {
        // Walk up from target/<profile>/build/omc-cli-*/out or target/<profile>/
        // to find the workspace root, then look in crates/omc-skills/src/templates/
        if let Some(ws) = find_workspace_root(&exe) {
            paths.push(
                ws.join("crates/omc-skills/src/templates")
                    .join(format!("{name}.md")),
            );
        }
    }

    // Also try relative to CWD (useful during development)
    if let Ok(cwd) = std::env::current_dir() {
        paths.push(
            cwd.join("crates/omc-skills/src/templates")
                .join(format!("{name}.md")),
        );
        // Also check sibling project
        paths.push(
            cwd.join("../oh-my-claudecode-RS/crates/omc-skills/src/templates")
                .join(format!("{name}.md")),
        );
    }

    // 3. Installed OMC home directory
    if let Some(home) = omc_home() {
        paths.push(home.join("skills").join(name).join("SKILL.md"));
    }

    paths
}

/// Attempt to find the workspace root by looking for Cargo.toml with `[workspace]`.
pub(super) fn find_workspace_root(from: &Path) -> Option<PathBuf> {
    let mut dir = from.to_path_buf();
    loop {
        if !dir.pop() {
            break;
        }
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists()
            && let Ok(content) = std::fs::read_to_string(&cargo_toml)
            && content.contains("[workspace]")
        {
            return Some(dir);
        }
    }
    None
}

/// Resolve the OMC home directory.
pub(super) fn omc_home() -> Option<PathBuf> {
    if let Ok(home) = std::env::var(OMC_HOME) {
        return Some(PathBuf::from(home));
    }
    dirs::home_dir().map(|h| h.join(".omc"))
}

/// Replace `$ARGUMENTS` placeholders in a template with the user's arguments.
pub(super) fn substitute_arguments(template: &str, arguments: &str) -> String {
    template
        .replace("{{ARGUMENTS}}", arguments)
        .replace("$ARGUMENTS", arguments)
}

/// List all discoverable skills by scanning the template directories.
pub(super) fn list_skills() {
    let mut skills = BTreeMap::new();

    // Scan templates directory
    let search_roots = template_search_roots();
    for root in &search_roots {
        if root.is_dir() {
            for entry in std::fs::read_dir(root).into_iter().flatten() {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md")
                    && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                {
                    let desc = extract_description(&path);
                    skills.entry(stem.to_string()).or_insert(desc);
                }
            }
        }
    }

    if skills.is_empty() {
        println!("No skills found. Set OMC_SKILLS_DIR or install skills to ~/.omc/skills/");
        return;
    }

    println!("{:<30} Description", "Skill");
    println!("{:<30} -----------", "-----");
    for (name, desc) in &skills {
        let desc_str = desc.as_deref().unwrap_or("");
        println!("{name:<30} {desc_str}");
    }
}

/// Get directories to scan for skill listing.
pub(super) fn template_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(dir) = std::env::var(OMC_SKILLS_DIR) {
        roots.push(PathBuf::from(dir));
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(ws) = find_workspace_root(&exe)
    {
        roots.push(ws.join("crates/omc-skills/src/templates"));
    }

    if let Ok(cwd) = std::env::current_dir() {
        let dev_path = cwd.join("crates/omc-skills/src/templates");
        if dev_path.is_dir() {
            roots.push(dev_path);
        }
    }

    if let Some(home) = omc_home() {
        roots.push(home.join("skills"));
    }

    roots
}

/// Extract the description from a skill template's YAML frontmatter.
pub(super) fn extract_description(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_frontmatter_description(&content)
}

/// Parse the `description` field from YAML frontmatter delimited by `---`.
pub(super) fn parse_frontmatter_description(content: &str) -> Option<String> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_first = &trimmed[3..];
    let end = after_first.find("---")?;
    let frontmatter = &after_first[..end];

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("description:") {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    None
}
