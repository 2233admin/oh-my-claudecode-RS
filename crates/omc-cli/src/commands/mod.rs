//! CLI command definitions for omc.

use clap::{Parser, Subcommand};

mod agent;
pub use agent::{GoalCommand, ToolCommand};

/// oh-my-claudecode CLI dispatcher.
///
/// Loads and outputs skill instructions for the given subcommand.
#[derive(Parser, Debug)]
#[command(name = "omc", version, about = "oh-my-claudecode CLI dispatcher")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// All available OMC commands.
///
/// Most subcommands map to a skill template. Tool commands use the shared
/// JSON contract directly and do not render a skill.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Expose the stable OMC-RS agent tool contract
    Tool {
        #[command(subcommand)]
        command: ToolCommand,
    },

    /// Create and resume a project-scoped goal ledger
    Goal {
        #[command(subcommand)]
        command: GoalCommand,
    },

    /// Setup OMC for a specific host
    #[command(name = "setup", visible_alias = "omc-setup")]
    OmcSetup {
        /// Target host: claude, codex, or Hermes MCP consumer
        #[arg(long, value_parser = ["claude", "codex", "hermes"])]
        host: Option<String>,

        /// Hermes home directory; defaults to HERMES_HOME or ~/.hermes
        #[arg(long, value_name = "PATH")]
        hermes_home: Option<std::path::PathBuf>,

        /// Force overwrite existing configuration
        #[arg(long, default_value = "false")]
        force: bool,

        /// Additional arguments passed to the template (fallback mode)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Diagnose OMC host configuration and environment
    #[command(name = "doctor", visible_alias = "omc-doctor")]
    OmcDoctor {
        /// Check only one host; omit to check both Claude Code and Codex CLI
        #[arg(long, value_parser = ["claude", "codex"])]
        host: Option<String>,

        /// Emit machine-readable JSON instead of a human report
        #[arg(long)]
        json: bool,

        /// Include the 16 platform capabilities and dependency availability
        #[arg(long)]
        tools: bool,
    },

    /// Start the MCP stdio server
    Mcp,

    /// Run the existing OMC team runtime through the unified CLI
    #[command(name = "team")]
    Team {
        /// Arguments forwarded to the omc-team runtime
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Configure notification preferences
    ConfigureNotifications(SkillArgs),

    /// Display HUD status bar configuration
    Hud(SkillArgs),

    /// List, inspect, or run a specific skill
    Skill(SkillArgs),

    /// Create a new skill from a description
    Skillify(SkillArgs),

    /// Evidence-driven tracing lane
    Trace(SkillArgs),

    /// Verify claims with evidence
    Verify(SkillArgs),

    /// Visual verification of UI/UX changes
    VisualVerdict(SkillArgs),

    /// Generate project wiki documentation
    Wiki(SkillArgs),

    /// Alias for skillify
    Learner(SkillArgs),

    /// Store or recall persistent memories
    Remember(SkillArgs),

    /// Ask an external AI provider
    Ask(SkillArgs),

    /// Autonomous deep research mode
    Autoresearch(SkillArgs),

    /// Cross-context generation
    Ccg(SkillArgs),

    /// Cancel active OMC execution modes
    Cancel(SkillArgs),

    /// Systematic debugging workflow
    Debug(SkillArgs),

    /// Deep-dive analysis into a topic or code area
    DeepDive(SkillArgs),

    /// Deep project initialization
    Deepinit(SkillArgs),

    /// Pull in external context for the current task
    ExternalContext(SkillArgs),

    /// Manage project sessions
    ProjectSessionManager(SkillArgs),

    /// Alias for project-session-manager
    Psm(SkillArgs),

    /// Create a tagged release with notes
    Release(SkillArgs),

    /// Self-improvement and learning loop
    SelfImprove(SkillArgs),

    /// Manage OMC agent teams
    OmcTeams(SkillArgs),

    /// Create a structured implementation plan
    Plan(SkillArgs),

    /// Conduct a deep interview to clarify requirements
    DeepInterview(SkillArgs),

    /// List all available skills
    List,
}

/// Arguments accepted by skill-dispatching commands.
///
/// Everything after the subcommand name is collected as free-form
/// arguments and passed through to the skill as `$ARGUMENTS`.
#[derive(clap::Args, Debug, Clone)]
pub struct SkillArgs {
    /// Arguments forwarded to the skill
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

impl SkillArgs {
    /// Returns the joined argument string for template substitution.
    pub fn joined(&self) -> String {
        self.args.join(" ")
    }
}
