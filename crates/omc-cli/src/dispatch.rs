//! Skill dispatch and template loading.

use crate::commands::{Cli, Commands, GoalCommand, SkillArgs, ToolCommand};
use chrono::Utc;
use omc_interop::mcp_bridge::{
    InteropBridgeCliArgs, interop_bridge, interop_bridge_request_from_cli,
};
use omc_interop::read_snapshot;
use omc_mcp::run_stdio;
use omc_python::{
    PythonReplService, PythonSessionError, PythonToolPayload, PythonToolRequest, ReplAction,
};
use omc_shared::agent_tool::{
    RouteRequest, ToolError, ToolResponse, capabilities_payload, normalize_request_id,
    route_agent_task,
};
use omc_shared::code_intel::{CodeIntelQueryRequest, query_code_intel};
use omc_shared::dap_adapter::{DebugInspectRequest, inspect_debug};
use omc_shared::hash_edit::{HashEdit, LineAnchor};
use omc_shared::lsp_adapter::{LspDocumentSymbolsRequest, query_document_symbols};
use omc_shared::operation_contract::{ResultSchema, TypedSubagentResult};
use omc_shared::workflow_contract::{
    WorkflowAdvanceRequest, WorkflowContext, WorkflowStage, advance_workflow,
};
use omc_shared::{GoalCheckpoint, GoalLedger, GoalRecord, OmcPaths};
use omc_team::team_observability;
use serde_json::{Value, json};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

mod tool;
use tool::run_tool;
pub use tool::run_tool_value;
mod host;
#[cfg(test)]
use host::collect_doctor_reports;
use host::{run_doctor, run_setup_host};
mod templates;
use templates::{list_skills, load_template, substitute_arguments};

#[derive(Debug, Error)]
pub enum DispatchError {
    #[error("Skill template not found: {0}")]
    NotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Goal error: {0}")]
    Goal(String),
    #[error("State error: {0}")]
    State(#[from] omc_shared::state::StateError),
    #[error("Host configuration error: {0}")]
    Host(String),
    #[error("Team runtime error: {0}")]
    Team(String),
}

/// Resolve the canonical skill name for a given command variant.
fn skill_name(cmd: &Commands) -> Option<&'static str> {
    match cmd {
        Commands::Tool { .. } => None,
        Commands::Goal { .. } => None,
        Commands::OmcSetup { .. } => Some("omc-setup"),
        Commands::OmcDoctor { .. } => None,
        Commands::Mcp => None,
        Commands::Team { .. } => None,
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
fn skill_args(cmd: &Commands) -> Option<SkillArgs> {
    match cmd {
        Commands::Tool { .. } => None,
        Commands::Goal { .. } => None,
        Commands::OmcDoctor { .. } => None,
        Commands::Mcp => None,
        Commands::Team { .. } => None,
        Commands::OmcSetup { args, .. } => Some(SkillArgs { args: args.clone() }),
        Commands::ConfigureNotifications(a)
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
        | Commands::DeepInterview(a) => Some(a.clone()),
        Commands::List => None,
    }
}

/// Main entry point for the CLI.
pub fn run(cli: Cli) -> Result<(), DispatchError> {
    if let Commands::Tool { command } = &cli.command {
        return run_tool(command);
    }

    if matches!(&cli.command, Commands::Mcp) {
        run_stdio()?;
        return Ok(());
    }

    if let Commands::Team { args } = &cli.command {
        return run_team(args);
    }

    if let Commands::Goal { command } = &cli.command {
        let root = std::env::current_dir().map_err(DispatchError::Io)?;
        return run_goal(command, &root);
    }

    if let Commands::OmcDoctor { host, json } = &cli.command {
        let root = std::env::current_dir().map_err(DispatchError::Io)?;
        return run_doctor(&root, host.as_deref(), *json);
    }

    if matches!(&cli.command, Commands::List) {
        list_skills();
        return Ok(());
    }

    // Intercept `omc setup --host <host>` for real setup logic
    if let Commands::OmcSetup {
        host: Some(host),
        force,
        hermes_home,
        ..
    } = &cli.command
    {
        let root = std::env::current_dir().map_err(DispatchError::Io)?;
        run_setup_host(&root, host, *force, hermes_home.as_deref())?;
        return Ok(());
    }

    // Default: load template and print
    let name = skill_name(&cli.command).expect("non-List command must resolve to a skill name");
    let args = skill_args(&cli.command).expect("non-List command must have skill args");

    let template = load_template(name)?;
    let rendered = substitute_arguments(&template, &args.joined());
    print!("{rendered}");

    Ok(())
}

/// Forward team operations to the existing runtime binary.
///
/// Keeping this as a process bridge preserves omc-team's lifecycle and
/// runtime implementation while making it consumable from the single `omc`
/// entrypoint. It deliberately does not create a second scheduler.
fn run_team(args: &[String]) -> Result<(), DispatchError> {
    let command = resolve_team_command();
    let status = Command::new(&command)
        .args(args)
        .status()
        .map_err(DispatchError::Io)?;
    if status.success() {
        Ok(())
    } else {
        Err(DispatchError::Team(format!(
            "{} exited with {status}",
            command.display()
        )))
    }
}

fn resolve_team_command() -> PathBuf {
    let names = if cfg!(windows) {
        ["omc-team.exe", "omc-team"]
    } else {
        ["omc-team", "omc-team.exe"]
    };

    if let Ok(executable) = std::env::current_exe() {
        let mut directory = executable.parent();
        for _ in 0..=2 {
            if let Some(dir) = directory {
                for name in names {
                    let candidate = dir.join(name);
                    if candidate.is_file() {
                        return candidate;
                    }
                }
                directory = dir.parent();
            }
        }
    }

    PathBuf::from(names[0])
}

fn run_goal(command: &GoalCommand, root: &Path) -> Result<(), DispatchError> {
    let ledger = GoalLedger::new(OmcPaths::new_with_root(root.join(".omc")));
    let now = || Utc::now().to_rfc3339();

    match command {
        GoalCommand::Create {
            id,
            objective,
            owner,
            task_id,
        } => {
            let mut goal = GoalRecord::new(id, objective, now());
            goal.owner = owner.clone();
            if let Some(task_id) = task_id {
                goal.attach_task(task_id).map_err(DispatchError::Goal)?;
            }
            ledger.create(&goal)?;
            print_goal(&goal)?;
        }
        GoalCommand::List => {
            println!("{}", serde_json::to_string_pretty(&ledger.list()?)?);
        }
        GoalCommand::Show { id } => {
            print_goal(&ledger.load(id)?)?;
        }
        GoalCommand::Start { id } => {
            let mut goal = ledger.load(id)?;
            goal.start(now()).map_err(DispatchError::Goal)?;
            ledger.save(&goal)?;
            print_goal(&goal)?;
        }
        GoalCommand::Block { id, reason } => {
            let mut goal = ledger.load(id)?;
            goal.block(reason, now()).map_err(DispatchError::Goal)?;
            ledger.save(&goal)?;
            print_goal(&goal)?;
        }
        GoalCommand::Checkpoint {
            id,
            checkpoint_id,
            summary,
        } => {
            let mut goal = ledger.load(id)?;
            goal.add_checkpoint(GoalCheckpoint {
                checkpoint_id: checkpoint_id.clone(),
                summary: summary.clone(),
                recorded_at: now(),
                artifact_refs: Vec::new(),
            })
            .map_err(DispatchError::Goal)?;
            goal.updated_at = now();
            ledger.save(&goal)?;
            print_goal(&goal)?;
        }
        GoalCommand::Complete { id } => {
            let mut goal = ledger.load(id)?;
            goal.complete(now()).map_err(DispatchError::Goal)?;
            ledger.save(&goal)?;
            print_goal(&goal)?;
        }
    }

    Ok(())
}

fn print_goal(goal: &GoalRecord) -> Result<(), DispatchError> {
    println!("{}", serde_json::to_string_pretty(goal)?);
    Ok(())
}

#[cfg(test)]
#[path = "dispatch/tests.rs"]
mod tests;
