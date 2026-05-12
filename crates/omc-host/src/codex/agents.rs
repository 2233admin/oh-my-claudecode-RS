//! Codex CLI agent definition generation (TOML format).

use crate::adapter::HostKind;
use crate::types::{AgentGenOptions, AgentRole, GeneratedAgentFile};
use std::path::PathBuf;

/// Generate a `.codex/agents/<name>.toml` file.
pub fn generate_codex_agent(
    role: &AgentRole,
    _opts: &AgentGenOptions,
) -> Result<GeneratedAgentFile, String> {
    if let Some(ref hosts) = role.hosts
        && !hosts.contains(&HostKind::Codex)
    {
        return Err(format!("agent '{}' is not available for Codex", role.name));
    }

    let mut toml_map = toml::map::Map::new();
    toml_map.insert("name".into(), toml::Value::String(role.name.clone()));
    toml_map.insert(
        "description".into(),
        toml::Value::String(role.description.clone()),
    );

    if let Some(effort) = role.reasoning_effort {
        let s = match effort {
            crate::types::ReasoningEffort::Low => "low",
            crate::types::ReasoningEffort::Medium => "medium",
            crate::types::ReasoningEffort::High => "high",
        };
        toml_map.insert("reasoning_effort".into(), toml::Value::String(s.into()));
    }

    if let Some(posture) = role.posture {
        let s = match posture {
            crate::types::Posture::Default => "default",
            crate::types::Posture::Conservative => "conservative",
            crate::types::Posture::Exploratory => "exploratory",
            crate::types::Posture::FrontierOrchestrator => "frontier-orchestrator",
            crate::types::Posture::DeepWorker => "deep-worker",
            crate::types::Posture::FastLane => "fast-lane",
        };
        toml_map.insert("posture".into(), toml::Value::String(s.into()));
    }

    if let Some(ref model) = role.model_class {
        toml_map.insert("model_class".into(), toml::Value::String(model.clone()));
    }

    if let Some(ref routing) = role.routing_role {
        toml_map.insert("routing_role".into(), toml::Value::String(routing.clone()));
    }

    toml_map.insert(
        "system_prompt".into(),
        toml::Value::String(role.system_prompt.clone()),
    );

    let content =
        toml::to_string_pretty(&toml::Value::Table(toml_map)).map_err(|e| e.to_string())?;

    let filename = format!("{}.toml", role.name);

    Ok(GeneratedAgentFile {
        relative_path: PathBuf::from(".codex").join("agents").join(&filename),
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Posture, ReasoningEffort};

    fn sample_role() -> AgentRole {
        AgentRole {
            name: "planner".into(),
            description: "Split missions into tasks".into(),
            system_prompt: "You are a planner.".into(),
            reasoning_effort: Some(ReasoningEffort::High),
            posture: Some(Posture::Conservative),
            model_class: Some("o3".into()),
            routing_role: Some("planner".into()),
            hosts: None,
        }
    }

    #[test]
    fn generates_toml_with_all_fields() {
        let file = generate_codex_agent(&sample_role(), &AgentGenOptions::default()).unwrap();
        assert!(file.content.contains("name = \"planner\""));
        assert!(file.content.contains("reasoning_effort = \"high\""));
        assert!(file.content.contains("posture = \"conservative\""));
        assert!(file.content.contains("model_class = \"o3\""));
        assert!(file.content.contains("routing_role = \"planner\""));
    }

    #[test]
    fn path_is_correct() {
        let file = generate_codex_agent(&sample_role(), &AgentGenOptions::default()).unwrap();
        assert_eq!(
            file.relative_path,
            PathBuf::from(".codex/agents/planner.toml")
        );
    }

    #[test]
    fn minimal_role() {
        let role = AgentRole {
            name: "simple".into(),
            description: "A simple agent".into(),
            system_prompt: "Do stuff.".into(),
            reasoning_effort: None,
            posture: None,
            model_class: None,
            routing_role: None,
            hosts: None,
        };
        let file = generate_codex_agent(&role, &AgentGenOptions::default()).unwrap();
        assert!(file.content.contains("name = \"simple\""));
        assert!(!file.content.contains("reasoning_effort"));
    }

    #[test]
    fn host_filter_blocks_claude_only() {
        let mut role = sample_role();
        role.hosts = Some(vec![HostKind::Claude]);
        assert!(generate_codex_agent(&role, &AgentGenOptions::default()).is_err());
    }

    #[test]
    fn all_posture_variants_serialize() {
        for posture in [
            Posture::Default,
            Posture::Conservative,
            Posture::Exploratory,
            Posture::FrontierOrchestrator,
            Posture::DeepWorker,
            Posture::FastLane,
        ] {
            let mut role = sample_role();
            role.posture = Some(posture);
            let file = generate_codex_agent(&role, &AgentGenOptions::default()).unwrap();
            assert!(file.content.contains("posture"));
        }
    }
}
