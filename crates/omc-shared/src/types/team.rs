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
