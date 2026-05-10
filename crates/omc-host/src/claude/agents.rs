//! Claude Code agent definition generation (markdown with YAML frontmatter).

use crate::adapter::HostKind;
use crate::types::{AgentGenOptions, AgentRole, GeneratedAgentFile};
use std::path::PathBuf;

/// Generate a `.claude/agents/<name>.md` file.
pub fn generate_claude_agent(
    role: &AgentRole,
    _opts: &AgentGenOptions,
) -> Result<GeneratedAgentFile, String> {
    // Check host filter
    if let Some(ref hosts) = role.hosts
        && !hosts.contains(&HostKind::Claude)
    {
        return Err(format!("agent '{}' is not available for Claude", role.name));
    }

    let mut frontmatter = String::new();
    frontmatter.push_str("---\n");
    frontmatter.push_str(&format!("name: {}\n", role.name));
    frontmatter.push_str(&format!("description: {}\n", role.description));
    if let Some(ref routing) = role.routing_role {
        frontmatter.push_str(&format!("routing_role: {routing}\n"));
    }
    frontmatter.push_str("---\n\n");
    frontmatter.push_str(&role.system_prompt);

    let filename = format!("{}.md", role.name);

    Ok(GeneratedAgentFile {
        relative_path: PathBuf::from(".claude").join("agents").join(&filename),
        content: frontmatter,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ReasoningEffort;

    fn sample_role() -> AgentRole {
        AgentRole {
            name: "planner".into(),
            description: "Split missions into tasks".into(),
            system_prompt: "You are a planner.".into(),
            reasoning_effort: Some(ReasoningEffort::High),
            posture: None,
            model_class: None,
            routing_role: Some("planner".into()),
            hosts: None,
        }
    }

    #[test]
    fn generates_markdown_with_frontmatter() {
        let file = generate_claude_agent(&sample_role(), &AgentGenOptions::default()).unwrap();
        assert!(file.content.starts_with("---\n"));
        assert!(file.content.contains("name: planner"));
        assert!(
            file.content
                .contains("description: Split missions into tasks")
        );
        assert!(file.content.contains("routing_role: planner"));
        assert!(file.content.contains("You are a planner."));
    }

    #[test]
    fn path_is_correct() {
        let file = generate_claude_agent(&sample_role(), &AgentGenOptions::default()).unwrap();
        assert_eq!(
            file.relative_path,
            PathBuf::from(".claude/agents/planner.md")
        );
    }

    #[test]
    fn host_filter_blocks_codex_only_agent() {
        let mut role = sample_role();
        role.hosts = Some(vec![HostKind::Codex]);
        assert!(generate_claude_agent(&role, &AgentGenOptions::default()).is_err());
    }

    #[test]
    fn host_filter_allows_claude_agent() {
        let mut role = sample_role();
        role.hosts = Some(vec![HostKind::Claude, HostKind::Codex]);
        assert!(generate_claude_agent(&role, &AgentGenOptions::default()).is_ok());
    }
}
