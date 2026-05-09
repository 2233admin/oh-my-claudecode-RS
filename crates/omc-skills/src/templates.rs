//! Skill template loading and management

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Frontmatter metadata extracted from skill templates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(rename = "argument-hint")]
    pub argument_hint: Option<String>,
    pub level: Option<String>,
    #[serde(rename = "aliases", default)]
    pub aliases: Vec<String>,
    pub agent: Option<String>,
}

/// A loaded skill template with metadata
#[derive(Debug, Clone)]
pub struct SkillTemplate {
    pub metadata: SkillMetadata,
    pub content: String,
}

/// All available built-in skill names (34 total)
pub const SKILL_NAMES: &[&str] = &[
    "ai-slop-cleaner",
    "ask",
    "autopilot",
    "cancel",
    "ccg",
    "configure-notifications",
    "debug",
    "deep-dive",
    "deep-interview",
    "deepinit",
    "external-context",
    "hud",
    "learner",
    "mcp-setup",
    "omc-doctor",
    "omc-setup",
    "omc-teams",
    "omc-plan",
    "project-session-manager",
    "ralph",
    "ralplan",
    "release",
    "remember",
    "sciomc",
    "self-improve",
    "skill",
    "skillify",
    "team",
    "trace",
    "ultraqa",
    "ultrawork",
    "verify",
    "visual-verdict",
    "wiki",
];

/// Load all embedded templates
pub fn get_templates() -> HashMap<String, SkillTemplate> {
    let mut templates = HashMap::new();

    templates.insert(
        "ai-slop-cleaner".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "ai-slop-cleaner".to_string(),
                description: "Clean AI-generated code slop with a regression-safe, deletion-first workflow and optional reviewer-only mode".to_string(),
                argument_hint: None,
                level: Some("3".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/ai-slop-cleaner.md").to_string(),
        },
    );

    templates.insert(
        "ask".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "ask".to_string(),
                description: "Process-first advisor routing for Claude, Codex, or Gemini via `omc ask`, with artifact capture and no raw CLI assembly".to_string(),
                argument_hint: None,
                level: None,
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/ask.md").to_string(),
        },
    );

    templates.insert(
        "autopilot".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "autopilot".to_string(),
                description: "Full autonomous execution from idea to working code".to_string(),
                argument_hint: Some("<product idea or task description>".to_string()),
                level: Some("4".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/autopilot.md").to_string(),
        },
    );

    templates.insert(
        "cancel".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "cancel".to_string(),
                description: "Cancel any active OMC mode (autopilot, ralph, ultrawork, ultraqa, swarm, ultrapilot, pipeline, team)".to_string(),
                argument_hint: Some("[--force|--all]".to_string()),
                level: Some("2".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/cancel.md").to_string(),
        },
    );

    templates.insert(
        "ccg".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "ccg".to_string(),
                description: "Claude-Codex-Gemini tri-model orchestration via /ask codex + /ask gemini, then Claude synthesizes results".to_string(),
                argument_hint: None,
                level: Some("5".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/ccg.md").to_string(),
        },
    );

    templates.insert(
        "configure-notifications".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "configure-notifications".to_string(),
                description: "Configure notification integrations (Telegram, Discord, Slack) via natural language".to_string(),
                argument_hint: None,
                level: Some("2".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/configure-notifications.md").to_string(),
        },
    );

    templates.insert(
        "debug".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "debug".to_string(),
                description: "Diagnose the current OMC session or repo state using logs, traces, state, and focused reproduction".to_string(),
                argument_hint: None,
                level: None,
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/debug.md").to_string(),
        },
    );

    templates.insert(
        "deep-dive".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "deep-dive".to_string(),
                description: "2-stage pipeline: trace (causal investigation) -> deep-interview (requirements crystallization) with 3-point injection".to_string(),
                argument_hint: Some("<problem or exploration target>".to_string()),
                level: None,
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/deep-dive.md").to_string(),
        },
    );

    templates.insert(
        "deep-interview".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "deep-interview".to_string(),
                description: "Socratic deep interview with mathematical ambiguity gating before explicit execution approval".to_string(),
                argument_hint: Some("[--quick|--standard|--deep] [--autoresearch] <idea or vague description>".to_string()),
                level: Some("3".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/deep-interview.md").to_string(),
        },
    );

    templates.insert(
        "deepinit".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "deepinit".to_string(),
                description:
                    "Deep codebase initialization with hierarchical AGENTS.md documentation"
                        .to_string(),
                argument_hint: None,
                level: Some("4".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/deepinit.md").to_string(),
        },
    );

    templates.insert(
        "external-context".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "external-context".to_string(),
                description: "Invoke parallel document-specialist agents for external web searches and documentation lookup".to_string(),
                argument_hint: Some("<search query or topic>".to_string()),
                level: Some("4".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/external-context.md").to_string(),
        },
    );

    templates.insert(
        "hud".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "hud".to_string(),
                description: "Configure HUD display options (layout, presets, display elements)"
                    .to_string(),
                argument_hint: Some("[setup|minimal|focused|full|status]".to_string()),
                level: Some("2".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/hud.md").to_string(),
        },
    );

    templates.insert(
        "learner".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "learner".to_string(),
                description: "Extract a learned skill from the current conversation".to_string(),
                argument_hint: None,
                level: Some("7".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/learner.md").to_string(),
        },
    );

    templates.insert(
        "mcp-setup".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "mcp-setup".to_string(),
                description: "Configure popular MCP servers for enhanced agent capabilities"
                    .to_string(),
                argument_hint: None,
                level: Some("2".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/mcp-setup.md").to_string(),
        },
    );

    templates.insert(
        "omc-doctor".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "omc-doctor".to_string(),
                description: "Diagnose and fix oh-my-claudecode installation issues".to_string(),
                argument_hint: None,
                level: Some("3".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/omc-doctor.md").to_string(),
        },
    );

    templates.insert(
        "omc-setup".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "omc-setup".to_string(),
                description: "Install or refresh oh-my-claudecode for plugin, npm, and local-dev setups from the canonical setup flow".to_string(),
                argument_hint: None,
                level: Some("2".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/omc-setup.md").to_string(),
        },
    );

    templates.insert(
        "omc-teams".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "omc-teams".to_string(),
                description: "CLI-team runtime for claude, codex, or gemini workers in tmux panes when you need process-based parallel execution".to_string(),
                argument_hint: None,
                level: Some("4".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/omc-teams.md").to_string(),
        },
    );

    templates.insert(
        "omc-plan".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "omc-plan".to_string(),
                description: "Strategic planning with optional interview workflow".to_string(),
                argument_hint: Some("[--direct|--consensus|--review] [--interactive] [--deliberate] <task description>".to_string()),
                level: Some("4".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/plan.md").to_string(),
        },
    );

    templates.insert(
        "project-session-manager".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "project-session-manager".to_string(),
                description: "Worktree-first dev environment manager for issues, PRs, and features with optional tmux sessions".to_string(),
                argument_hint: None,
                level: Some("2".to_string()),
                aliases: vec!["psm".to_string()],
                agent: None,
            },
            content: include_str!("templates/project-session-manager.md").to_string(),
        },
    );

    templates.insert(
        "ralph".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "ralph".to_string(),
                description: "Self-referential loop until task completion with configurable verification reviewer".to_string(),
                argument_hint: Some("[--no-deslop] [--critic=architect|critic|codex] <task description>".to_string()),
                level: Some("4".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/ralph.md").to_string(),
        },
    );

    templates.insert(
        "ralplan".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "ralplan".to_string(),
                description: "Consensus planning entrypoint that auto-gates vague ralph/autopilot/team requests before execution".to_string(),
                argument_hint: Some("[--interactive] [--deliberate] [--architect codex] [--critic codex] <task description>".to_string()),
                level: Some("4".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/ralplan.md").to_string(),
        },
    );

    templates.insert(
        "release".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "release".to_string(),
                description: "Generic release assistant — analyzes repo release rules, caches them in .omc/RELEASE_RULE.md, then guides the release".to_string(),
                argument_hint: None,
                level: Some("3".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/release.md").to_string(),
        },
    );

    templates.insert(
        "remember".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "remember".to_string(),
                description: "Review reusable project knowledge and decide what belongs in project memory, notepad, or durable docs".to_string(),
                argument_hint: None,
                level: None,
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/remember.md").to_string(),
        },
    );

    templates.insert(
        "sciomc".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "sciomc".to_string(),
                description: "Orchestrate parallel scientist agents for comprehensive analysis with AUTO mode".to_string(),
                argument_hint: Some("<research goal>".to_string()),
                level: Some("4".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/sciomc.md").to_string(),
        },
    );

    templates.insert(
        "self-improve".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "self-improve".to_string(),
                description:
                    "Autonomous evolutionary code improvement engine with tournament selection"
                        .to_string(),
                argument_hint: None,
                level: Some("4".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/self-improve.md").to_string(),
        },
    );

    templates.insert(
        "skill".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "skill".to_string(),
                description: "Manage local skills - list, add, remove, search, edit, setup wizard"
                    .to_string(),
                argument_hint: Some("<command> [args]".to_string()),
                level: Some("2".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/skill.md").to_string(),
        },
    );

    templates.insert(
        "skillify".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "skillify".to_string(),
                description: "Turn a repeatable workflow from the current session into a reusable OMC skill draft".to_string(),
                argument_hint: None,
                level: None,
                aliases: vec!["learner".to_string()],
                agent: None,
            },
            content: include_str!("templates/skillify.md").to_string(),
        },
    );

    templates.insert(
        "team".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "team".to_string(),
                description:
                    "N coordinated agents on shared task list using Claude Code native teams"
                        .to_string(),
                argument_hint: Some("[N:agent-type] [ralph] <task description>".to_string()),
                level: Some("4".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/team.md").to_string(),
        },
    );

    templates.insert(
        "trace".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "trace".to_string(),
                description: "Evidence-driven tracing lane that orchestrates competing tracer hypotheses in Claude built-in team mode".to_string(),
                argument_hint: Some("<observation to trace>".to_string()),
                level: Some("2".to_string()),
                aliases: vec![],
                agent: Some("tracer".to_string()),
            },
            content: include_str!("templates/trace.md").to_string(),
        },
    );

    templates.insert(
        "ultraqa".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "ultraqa".to_string(),
                description: "QA cycling workflow - test, verify, fix, repeat until goal met"
                    .to_string(),
                argument_hint: Some(
                    "[--tests|--build|--lint|--typecheck|--custom <pattern>] [--interactive]"
                        .to_string(),
                ),
                level: Some("3".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/ultraqa.md").to_string(),
        },
    );

    templates.insert(
        "ultrawork".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "ultrawork".to_string(),
                description: "Parallel execution engine for high-throughput task completion"
                    .to_string(),
                argument_hint: Some("<task description with parallel work items>".to_string()),
                level: Some("4".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/ultrawork.md").to_string(),
        },
    );

    templates.insert(
        "verify".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "verify".to_string(),
                description: "Verify that a change really works before you claim completion"
                    .to_string(),
                argument_hint: None,
                level: None,
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/verify.md").to_string(),
        },
    );

    templates.insert(
        "visual-verdict".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "visual-verdict".to_string(),
                description: "Structured visual QA verdict for screenshot-to-reference comparisons"
                    .to_string(),
                argument_hint: None,
                level: Some("2".to_string()),
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/visual-verdict.md").to_string(),
        },
    );

    templates.insert(
        "wiki".to_string(),
        SkillTemplate {
            metadata: SkillMetadata {
                name: "wiki".to_string(),
                description: "LLM Wiki — persistent markdown knowledge base that compounds across sessions (Karpathy model)".to_string(),
                argument_hint: None,
                level: None,
                aliases: vec![],
                agent: None,
            },
            content: include_str!("templates/wiki.md").to_string(),
        },
    );

    templates
}

/// Get a specific template by name
pub fn get_template(name: &str) -> Option<SkillTemplate> {
    get_templates().get(name).cloned()
}

/// List all available skill names
pub fn list_skill_names() -> Vec<&'static str> {
    SKILL_NAMES.to_vec()
}

static TEMPLATES: Lazy<HashMap<String, SkillTemplate>> = Lazy::new(get_templates);

/// Get a template by name (cached)
pub fn template(name: &str) -> Option<&'static SkillTemplate> {
    TEMPLATES.get(name)
}
