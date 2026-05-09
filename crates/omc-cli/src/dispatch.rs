//! Skill dispatch and template loading.

use crate::commands::{Cli, Commands, SkillArgs};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DispatchError {
    #[error("Skill template not found: {0}")]
    NotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Resolve the canonical skill name for a given command variant.
fn skill_name(cmd: &Commands) -> Option<&'static str> {
    match cmd {
        Commands::OmcSetup(_) => Some("omc-setup"),
        Commands::OmcDoctor(_) => Some("omc-doctor"),
        Commands::ConfigureNotifications(_) => Some("configure-notifications"),
        Commands::Hud(_) => Some("hud"),
        Commands::Skill(_) => Some("skill"),
        Commands::Skillify(_) => Some("skillify"),
        Commands::Trace(_) => Some("trace"),
        Commands::Verify(_) => Some("verify"),
        Commands::VisualVerdict(_) => Some("visual-verdict"),
        Commands::Wiki(_) => Some("wiki"),
        Commands::Learner(_) => Some("learner"),
        Commands::Remember(_) => Some("remember"),
        Commands::Ask(_) => Some("ask"),
        Commands::Autoresearch(_) => Some("autoresearch"),
        Commands::Ccg(_) => Some("ccg"),
        Commands::Cancel(_) => Some("cancel"),
        Commands::Debug(_) => Some("debug"),
        Commands::DeepDive(_) => Some("deep-dive"),
        Commands::Deepinit(_) => Some("deepinit"),
        Commands::ExternalContext(_) => Some("external-context"),
        Commands::ProjectSessionManager(_) => Some("project-session-manager"),
        Commands::Psm(_) => Some("psm"),
        Commands::Release(_) => Some("release"),
        Commands::SelfImprove(_) => Some("self-improve"),
        Commands::OmcTeams(_) => Some("omc-teams"),
        Commands::Plan(_) => Some("plan"),
        Commands::DeepInterview(_) => Some("deep-interview"),
        Commands::List => None,
    }
}

/// Extract the skill args from any command variant.
fn skill_args(cmd: &Commands) -> Option<&SkillArgs> {
    match cmd {
        Commands::OmcSetup(a)
        | Commands::OmcDoctor(a)
        | Commands::ConfigureNotifications(a)
        | Commands::Hud(a)
        | Commands::Skill(a)
        | Commands::Skillify(a)
        | Commands::Trace(a)
        | Commands::Verify(a)
        | Commands::VisualVerdict(a)
        | Commands::Wiki(a)
        | Commands::Learner(a)
        | Commands::Remember(a)
        | Commands::Ask(a)
        | Commands::Autoresearch(a)
        | Commands::Ccg(a)
        | Commands::Cancel(a)
        | Commands::Debug(a)
        | Commands::DeepDive(a)
        | Commands::Deepinit(a)
        | Commands::ExternalContext(a)
        | Commands::ProjectSessionManager(a)
        | Commands::Psm(a)
        | Commands::Release(a)
        | Commands::SelfImprove(a)
        | Commands::OmcTeams(a)
        | Commands::Plan(a)
        | Commands::DeepInterview(a) => Some(a),
        Commands::List => None,
    }
}

/// Main entry point for the CLI.
pub fn run(cli: Cli) -> Result<(), DispatchError> {
    if matches!(&cli.command, Commands::List) {
        list_skills();
        return Ok(());
    }

    let name = skill_name(&cli.command).expect("non-List command must resolve to a skill name");
    let args = skill_args(&cli.command).expect("non-List command must have skill args");

    let template = load_template(name)?;
    let rendered = substitute_arguments(&template, &args.joined());
    print!("{rendered}");

    Ok(())
}

/// Load a skill template by name from the skills directory.
///
/// Search order:
/// 1. `OMC_SKILLS_DIR` environment variable
/// 2. `<crate_parent>/crates/omc-skills/src/templates/<name>.md`
/// 3. `~/.omc/skills/<name>/SKILL.md`
fn load_template(name: &str) -> Result<String, DispatchError> {
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

fn template_search_paths(name: &str) -> Vec<PathBuf> {
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
fn find_workspace_root(from: &Path) -> Option<PathBuf> {
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
fn omc_home() -> Option<PathBuf> {
    if let Ok(home) = std::env::var(OMC_HOME) {
        return Some(PathBuf::from(home));
    }
    dirs::home_dir().map(|h| h.join(".omc"))
}

/// Replace `$ARGUMENTS` placeholders in a template with the user's arguments.
fn substitute_arguments(template: &str, arguments: &str) -> String {
    template
        .replace("{{ARGUMENTS}}", arguments)
        .replace("$ARGUMENTS", arguments)
}

/// List all discoverable skills by scanning the template directories.
fn list_skills() {
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
static OMC_SKILLS_DIR: &str = "OMC_SKILLS_DIR";

fn template_search_roots() -> Vec<PathBuf> {
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
fn extract_description(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_frontmatter_description(&content)
}

/// Parse the `description` field from YAML frontmatter delimited by `---`.
fn parse_frontmatter_description(content: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_arguments_dollar() {
        let template = "Run with:\n```text\n$ARGUMENTS\n```\n";
        let result = substitute_arguments(template, "hello world");
        assert_eq!(result, "Run with:\n```text\nhello world\n```\n");
    }

    #[test]
    fn test_substitute_arguments_braces() {
        let template = "Task: {{ARGUMENTS}}";
        let result = substitute_arguments(template, "hello world");
        assert_eq!(result, "Task: hello world");
    }

    #[test]
    fn test_substitute_arguments_both_formats() {
        let template = "$ARGUMENTS and {{ARGUMENTS}}";
        let result = substitute_arguments(template, "test");
        assert_eq!(result, "test and test");
    }

    #[test]
    fn test_substitute_no_placeholder() {
        let template = "No placeholder here";
        let result = substitute_arguments(template, "args");
        assert_eq!(result, "No placeholder here");
    }

    #[test]
    fn test_substitute_empty_arguments() {
        let template = "Args: $ARGUMENTS";
        let result = substitute_arguments(template, "");
        assert_eq!(result, "Args: ");
    }

    #[test]
    fn test_parse_frontmatter_description() {
        let content = r#"---
description: "A test skill"
name: test
---

# Content"#;
        let desc = parse_frontmatter_description(content);
        assert_eq!(desc, Some("A test skill".to_string()));
    }

    #[test]
    fn test_parse_frontmatter_no_description() {
        let content = r#"---
name: test
---

# Content"#;
        let desc = parse_frontmatter_description(content);
        assert_eq!(desc, None);
    }

    #[test]
    fn test_parse_frontmatter_empty() {
        let content = "no frontmatter here";
        let desc = parse_frontmatter_description(content);
        assert_eq!(desc, None);
    }

    #[test]
    fn test_skill_args_joined() {
        let args = SkillArgs {
            args: vec!["hello".into(), "world".into()],
        };
        assert_eq!(args.joined(), "hello world");
    }

    #[test]
    fn test_skill_args_empty() {
        let args = SkillArgs { args: vec![] };
        assert_eq!(args.joined(), "");
    }

    #[test]
    fn test_skill_names_unique() {
        // Verify all commands map to distinct skill names (except aliases)
        let commands = vec![
            "omc-setup",
            "omc-doctor",
            "configure-notifications",
            "hud",
            "skill",
            "skillify",
            "trace",
            "verify",
            "visual-verdict",
            "wiki",
            "learner",
            "remember",
            "ask",
            "autoresearch",
            "ccg",
            "cancel",
            "debug",
            "deep-dive",
            "deepinit",
            "external-context",
            "project-session-manager",
            "psm",
            "release",
            "self-improve",
            "omc-teams",
            "plan",
            "deep-interview",
        ];
        let mut sorted = commands.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            commands.len(),
            sorted.len(),
            "duplicate skill names detected"
        );
    }
}
