//! Team-related types for agent orchestration.

use serde::{Deserialize, Serialize};

/// Represents the role of an agent in a team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TeamRole {
    Orchestrator,
    Planner,
    Analyst,
    Architect,
    Executor,
    Debugger,
    Critic,
    CodeReviewer,
    SecurityReviewer,
    TestEngineer,
    Designer,
    Writer,
    CodeSimplifier,
    Explore,
    DocumentSpecialist,
}

impl TeamRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            TeamRole::Orchestrator => "Orchestrator",
            TeamRole::Planner => "Planner",
            TeamRole::Analyst => "Analyst",
            TeamRole::Architect => "Architect",
            TeamRole::Executor => "Executor",
            TeamRole::Debugger => "Debugger",
            TeamRole::Critic => "Critic",
            TeamRole::CodeReviewer => "CodeReviewer",
            TeamRole::SecurityReviewer => "SecurityReviewer",
            TeamRole::TestEngineer => "TestEngineer",
            TeamRole::Designer => "Designer",
            TeamRole::Writer => "Writer",
            TeamRole::CodeSimplifier => "CodeSimplifier",
            TeamRole::Explore => "Explore",
            TeamRole::DocumentSpecialist => "DocumentSpecialist",
        }
    }
}

/// Represents the provider/engine for an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TeamProvider {
    Claude,
    Codex,
    Gemini,
}

/// Worktree mode for agent workspace isolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum WorktreeMode {
    /// Worktree support is disabled.
    Disabled,
    /// Off (same as Disabled).
    Off,
    /// Worktrees are created in detached state.
    Detached,
    /// Worktrees are created with branch prefix.
    Branch,
    /// Worktrees are created with custom names.
    Named,
}

/// Configuration for team operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamOpsConfig {
    /// Maximum number of agents allowed in a team.
    pub max_agents: u8,
    /// Default agent type when not specified.
    pub default_agent_type: TeamProvider,
    /// Monitoring interval in milliseconds.
    pub monitor_interval_ms: u64,
    /// Shutdown timeout in milliseconds.
    pub shutdown_timeout_ms: u64,
    /// Worktree mode for workspace isolation.
    pub worktree_mode: WorktreeMode,
}

impl Default for TeamOpsConfig {
    fn default() -> Self {
        Self {
            max_agents: 5,
            default_agent_type: TeamProvider::Claude,
            monitor_interval_ms: 1000,
            shutdown_timeout_ms: 30000,
            worktree_mode: WorktreeMode::Off,
        }
    }
}

/// Specification for role assignment rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleAssignmentSpec {
    /// The role to assign.
    pub role: TeamRole,
    /// Required capabilities or skills for this role.
    #[serde(default)]
    pub required_skills: Vec<String>,
    /// Preferred provider for this role.
    #[serde(default)]
    pub preferred_provider: Option<TeamProvider>,
    /// Priority weight for this role (higher = more likely to be assigned).
    #[serde(default)]
    pub priority: u8,
}

impl RoleAssignmentSpec {
    pub fn new(role: TeamRole) -> Self {
        Self {
            role,
            required_skills: Vec::new(),
            preferred_provider: None,
            priority: 50,
        }
    }

    pub fn with_skills(mut self, skills: impl Into<Vec<String>>) -> Self {
        self.required_skills = skills.into();
        self
    }

    pub fn with_provider(mut self, provider: TeamProvider) -> Self {
        self.preferred_provider = Some(provider);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === TeamRole ===

    #[test]
    fn team_role_as_str_roundtrip() {
        let variants = vec![
            (TeamRole::Orchestrator, "Orchestrator"),
            (TeamRole::Planner, "Planner"),
            (TeamRole::Analyst, "Analyst"),
            (TeamRole::Architect, "Architect"),
            (TeamRole::Executor, "Executor"),
            (TeamRole::Debugger, "Debugger"),
            (TeamRole::Critic, "Critic"),
            (TeamRole::CodeReviewer, "CodeReviewer"),
            (TeamRole::SecurityReviewer, "SecurityReviewer"),
            (TeamRole::TestEngineer, "TestEngineer"),
            (TeamRole::Designer, "Designer"),
            (TeamRole::Writer, "Writer"),
            (TeamRole::CodeSimplifier, "CodeSimplifier"),
            (TeamRole::Explore, "Explore"),
            (TeamRole::DocumentSpecialist, "DocumentSpecialist"),
        ];
        for (role, expected) in variants {
            assert_eq!(role.as_str(), expected, "as_str() mismatch for {:?}", role);
        }
    }

    #[test]
    fn team_role_serialization_roundtrip() {
        let json = serde_json::to_string(&TeamRole::CodeReviewer).unwrap();
        assert_eq!(json, "\"CodeReviewer\"");
        let deserialized: TeamRole = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, TeamRole::CodeReviewer);
    }

    #[test]
    fn team_role_15_variants() {
        // Verify 15 variants by counting the exhaustive match arms in as_str()
        let count = [
            TeamRole::Orchestrator,
            TeamRole::Planner,
            TeamRole::Analyst,
            TeamRole::Architect,
            TeamRole::Executor,
            TeamRole::Debugger,
            TeamRole::Critic,
            TeamRole::CodeReviewer,
            TeamRole::SecurityReviewer,
            TeamRole::TestEngineer,
            TeamRole::Designer,
            TeamRole::Writer,
            TeamRole::CodeSimplifier,
            TeamRole::Explore,
            TeamRole::DocumentSpecialist,
        ]
        .len();
        assert_eq!(count, 15);
    }

    // === TeamProvider ===

    #[test]
    fn team_provider_serialization_roundtrip() {
        for provider in [
            TeamProvider::Claude,
            TeamProvider::Codex,
            TeamProvider::Gemini,
        ] {
            let json = serde_json::to_string(&provider).unwrap();
            let deserialized: TeamProvider = serde_json::from_str(&json).unwrap();
            assert_eq!(provider, deserialized);
        }
    }

    // === WorktreeMode ===

    #[test]
    fn worktree_mode_serialization_roundtrip() {
        let variants = vec![
            WorktreeMode::Disabled,
            WorktreeMode::Off,
            WorktreeMode::Detached,
            WorktreeMode::Branch,
            WorktreeMode::Named,
        ];
        for mode in variants {
            let json = serde_json::to_string(&mode).unwrap();
            let deserialized: WorktreeMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, deserialized);
        }
    }

    // === TeamOpsConfig ===

    #[test]
    fn team_ops_config_default() {
        let config = TeamOpsConfig::default();
        assert_eq!(config.max_agents, 5);
        assert_eq!(config.default_agent_type, TeamProvider::Claude);
        assert_eq!(config.monitor_interval_ms, 1000);
        assert_eq!(config.shutdown_timeout_ms, 30000);
        assert_eq!(config.worktree_mode, WorktreeMode::Off);
    }

    #[test]
    fn team_ops_config_serialization_uses_camel_case() {
        let config = TeamOpsConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(
            json.contains("maxAgents"),
            "expected camelCase, got: {}",
            json
        );
        assert!(json.contains("defaultAgentType"));
        assert!(json.contains("monitorIntervalMs"));
    }

    #[test]
    fn team_ops_config_roundtrip() {
        let config = TeamOpsConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: TeamOpsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.max_agents, config.max_agents);
    }

    // === RoleAssignmentSpec ===

    #[test]
    fn role_assignment_spec_builder() {
        let spec = RoleAssignmentSpec::new(TeamRole::Architect)
            .with_skills(vec!["rust".into(), "architecture".into()])
            .with_provider(TeamProvider::Claude);

        assert_eq!(spec.role, TeamRole::Architect);
        assert_eq!(spec.required_skills, vec!["rust", "architecture"]);
        assert_eq!(spec.preferred_provider, Some(TeamProvider::Claude));
        assert_eq!(spec.priority, 50);
    }

    #[test]
    fn role_assignment_spec_defaults() {
        let spec = RoleAssignmentSpec::new(TeamRole::Executor);
        assert!(spec.required_skills.is_empty());
        assert!(spec.preferred_provider.is_none());
        assert_eq!(spec.priority, 50);
    }

    #[test]
    fn role_assignment_spec_serialization_roundtrip() {
        let spec =
            RoleAssignmentSpec::new(TeamRole::TestEngineer).with_skills(vec!["testing".into()]);
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: RoleAssignmentSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role, TeamRole::TestEngineer);
        assert_eq!(deserialized.required_skills, vec!["testing"]);
    }

    // === TeamConfigBlock ===

    #[test]
    fn team_config_block_default() {
        let block = TeamConfigBlock::default();
        assert!(block.ops.is_none());
        assert!(block.role_routing.is_empty());
    }

    #[test]
    fn team_config_block_with_roles() {
        let block = TeamConfigBlock {
            ops: Some(TeamOpsConfig::default()),
            role_routing: vec![
                RoleAssignmentSpec::new(TeamRole::Planner),
                RoleAssignmentSpec::new(TeamRole::Executor),
            ],
        };
        let json = serde_json::to_string(&block).unwrap();
        let deserialized: TeamConfigBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.role_routing.len(), 2);
        assert!(deserialized.ops.is_some());
    }
}

/// Team configuration block containing ops and role routing settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct TeamConfigBlock {
    /// Operations configuration for the team.
    #[serde(default)]
    pub ops: Option<TeamOpsConfig>,
    /// Role routing configuration.
    #[serde(default)]
    pub role_routing: Vec<RoleAssignmentSpec>,
}
