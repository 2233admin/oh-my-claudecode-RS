use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub mod agent_handle;
pub mod agent_lifecycle;
pub mod agents;
pub mod background;
pub mod communication;
pub mod dispatch;
pub mod fault_tolerance;
pub mod forbidden;
pub mod governance;
pub mod heartbeat;
pub mod idle_nudge;
mod observability;
pub mod phase_controller;
mod runtimes;
pub mod task_graph;
mod trackers;
pub mod usage;
pub mod work_stealing;
pub mod worker_health;
pub use observability::{
    AgentInvocationRecord, AgentSessionRecord, AgentSessionState, CellPlan, ContextBudget,
    ContextGuardAction, ContextGuardDecision, ObservabilityDoctorReport, ObservabilityStartReport,
    ObservabilityTopSnapshot, UsageConfidence, UsageGroupBy, UsageMeasurement, UsageReport,
    UsageRollup, UsageSource, build_cell_plan, context_guard_decision, init_observability,
    load_sessions, mark_session_handoff, observability_doctor, record_team_launch,
    render_resume_packet, top_snapshot, transition_session_state, usage_report,
};
pub use runtimes::{
    RuntimeDoctorReport, RuntimeKind, RuntimeRunRecord, RuntimeStartReport, check_runtime_ready,
    collect_runtime_handoff, find_runtime_record, runtime_doctor, start_runtime,
};
pub use trackers::{
    ClaimReport, DoctorReport, ReadyIssue, RunRecord, TrackerKind, find_run_record,
    find_run_record_by_run_id, github_claim, github_doctor, github_import_task, github_ready,
    import_tracker_task, linear_claim, linear_doctor, linear_import_task, linear_ready, new_run_id,
    save_run_record,
};

const SETTINGS_JSON: &str = ".claude/settings.json";
const WORKTREE_INCLUDE: &str = ".worktreeinclude";
const META_START: &str = "<!-- omc-team-meta";
const META_END: &str = "-->";
const DISCIPLINE_START: &str = "<!-- omc-native-agent-discipline:start -->";
const DISCIPLINE_END: &str = "<!-- omc-native-agent-discipline:end -->";

#[derive(Debug, Default)]
pub struct InitReport {
    pub created: Vec<PathBuf>,
    pub updated: Vec<PathBuf>,
    pub unchanged: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionKind {
    Implementation,
    Research,
    Review,
}

#[derive(Debug, Clone)]
pub struct StartOptions {
    pub kind: MissionKind,
    pub runtime: RuntimeKind,
    pub team_size: u8,
    pub mode: String,
    pub repo: Option<String>,
    pub team: Option<String>,
    pub tracker: Option<TrackerKind>,
    pub security_review: bool,
    pub test_review: bool,
}

impl StartOptions {
    pub fn new(kind: MissionKind) -> Self {
        Self {
            kind,
            runtime: RuntimeKind::Claude,
            team_size: 3,
            mode: match kind {
                MissionKind::Implementation => "implementation".to_string(),
                MissionKind::Research => "research".to_string(),
                MissionKind::Review => "review".to_string(),
            },
            repo: None,
            team: None,
            tracker: None,
            security_review: false,
            test_review: false,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(3..=16).contains(&self.team_size) {
            return Err("team size must be between 3 and 16 for omc-team v0.4".to_string());
        }
        if self.runtime == RuntimeKind::Kohaku && self.team_size > 5 {
            return Err(
                "kohaku runtime currently supports team size 3-5; use claude or fsc for larger teams"
                    .to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskMetadata {
    pub id: String,
    pub title: String,
    pub agent_ready: bool,
    pub risk: String,
    pub ownership: Vec<String>,
    pub acceptance: Vec<String>,
    pub verification: Vec<String>,
    pub source: Option<String>,
    pub linear_id: Option<String>,
    pub tracker: Option<String>,
    pub github_repo: Option<String>,
    pub github_issue_number: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct TaskCard {
    pub meta: TaskMetadata,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct Mission {
    pub id: String,
    pub team_name: String,
    pub prompt: String,
}

#[derive(Debug, Clone)]
pub struct PreparedMission {
    pub mission: Mission,
    pub task: TaskCard,
    pub run_record: Option<RunRecord>,
}

#[derive(Debug, Clone, Copy)]
pub enum HookKind {
    TaskCreated,
    TaskCompleted,
    TeammateIdle,
}

impl HookKind {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "task-created" | "TaskCreated" => Ok(Self::TaskCreated),
            "task-completed" | "TaskCompleted" => Ok(Self::TaskCompleted),
            "teammate-idle" | "TeammateIdle" => Ok(Self::TeammateIdle),
            _ => Err(format!("unknown hook kind: {raw}")),
        }
    }
}

pub fn init_project(root: &Path) -> Result<InitReport, String> {
    let mut report = InitReport::default();
    ensure_dir(root.join(".claude/agents"), &mut report)?;
    ensure_dir(root.join(".claude/hooks"), &mut report)?;
    ensure_dir(root.join(".omc/team/missions"), &mut report)?;
    ensure_dir(root.join(".omc/team/imports"), &mut report)?;
    ensure_dir(root.join(".omc/team/fsc"), &mut report)?;
    ensure_dir(root.join(".omc/team/kohaku"), &mut report)?;
    ensure_dir(root.join(".omc/team/sessions"), &mut report)?;
    ensure_dir(root.join(".omc/team/invocations"), &mut report)?;
    ensure_dir(root.join(".omc/team/whiteboard/entries"), &mut report)?;
    ensure_dir(root.join(".omc/team/runs"), &mut report)?;

    upsert_file(
        root.join(".claude/agents/omc-planner.md"),
        agent_planner(),
        &mut report,
    )?;
    upsert_file(
        root.join(".claude/agents/omc-executor.md"),
        agent_executor(),
        &mut report,
    )?;
    upsert_file(
        root.join(".claude/agents/omc-reviewer.md"),
        agent_reviewer(),
        &mut report,
    )?;
    upsert_file(
        root.join(".claude/agents/omc-security-auditor.md"),
        agent_security(),
        &mut report,
    )?;
    upsert_file(
        root.join(".claude/agents/omc-linear-reporter.md"),
        agent_linear_reporter(),
        &mut report,
    )?;
    upsert_agents_md(root, &mut report)?;
    upsert_claude_md(root, &mut report)?;
    upsert_file(
        root.join(WORKTREE_INCLUDE),
        ".env\n.env.local\n.claude/settings.local.json\n",
        &mut report,
    )?;
    upsert_gitignore(root, &mut report)?;
    upsert_settings(root, &mut report)?;
    for path in init_observability(root)? {
        if !report.created.contains(&path)
            && !report.updated.contains(&path)
            && !report.unchanged.contains(&path)
        {
            report.unchanged.push(path);
        }
    }
    Ok(report)
}

static CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS: &str = "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS";

pub fn check_claude_ready() -> Result<(), String> {
    if env::var(CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS)
        .ok()
        .as_deref()
        != Some("1")
    {
        return Err(
            "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 is required. Run: set CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 or add it to .claude/settings.json"
                .to_string(),
        );
    }

    let output = Command::new("claude")
        .arg("--version")
        .output()
        .map_err(|_| {
            "Claude Code CLI not found. Install Claude Code v2.1.32+ and ensure `claude` is on PATH"
                .to_string()
        })?;

    let version = String::from_utf8_lossy(&output.stdout);
    let version = if version.trim().is_empty() {
        String::from_utf8_lossy(&output.stderr).to_string()
    } else {
        version.to_string()
    };
    if !version_at_least(&version, (2, 1, 32)) {
        return Err(format!(
            "Claude Code v2.1.32+ is required for agent teams; detected: {}",
            version.trim()
        ));
    }
    Ok(())
}

pub fn mission_for_local_or_issue(
    root: &Path,
    target: &str,
    opts: StartOptions,
) -> Result<Mission, String> {
    Ok(prepare_start_mission(root, target, opts)?.mission)
}

pub fn prepare_start_mission(
    root: &Path,
    target: &str,
    opts: StartOptions,
) -> Result<PreparedMission, String> {
    let tracker = opts.tracker.or_else(|| infer_tracker(target));
    let (task, run_record) = if let Some(tracker) = tracker {
        let imported = import_tracker_task(
            root,
            target,
            tracker,
            opts.repo.as_deref(),
            opts.team.as_deref(),
        )?;
        ensure_ready(&imported.card)?;
        let run_id = new_run_id(&imported.issue_ref);
        let lease_comment_id = trackers::claim_specific_issue(&imported, &run_id)?;
        let team_name = slug(&imported.issue_ref);
        let record = RunRecord {
            run_id,
            tracker,
            repo_or_team: imported.repo_or_team,
            issue_ref: imported.issue_ref,
            issue_id: imported.issue_id,
            team_name,
            mission_path: String::default(),
            started_at: unix_timestamp(),
            lease_comment_id: Some(lease_comment_id),
            start_comment_id: None,
            handoff_comment_id: None,
            last_known_state: Some("claimed".to_string()),
        };
        (imported.card, Some(record))
    } else {
        let path = root.join(target);
        let task = parse_task_card(
            &fs::read_to_string(&path).map_err(|e| format!("failed to read task file: {e}"))?,
        )?;
        ensure_ready(&task)?;
        (task, None)
    };

    let mission = Mission {
        id: task.meta.id.clone(),
        team_name: run_record
            .as_ref()
            .map_or_else(|| slug(&task.meta.id), |record| record.team_name.clone()),
        prompt: if opts.runtime == RuntimeKind::Claude {
            implementation_prompt(&task, &opts)
        } else {
            runtime_adapter_prompt(&task, &opts)
        },
    };
    Ok(PreparedMission {
        mission,
        task,
        run_record,
    })
}

pub fn finalize_start_record(
    root: &Path,
    mut record: RunRecord,
    mission_path: &Path,
) -> Result<RunRecord, String> {
    record.mission_path = mission_path.display().to_string();
    record.start_comment_id = trackers::mark_started(&record)?;
    record.last_known_state = Some("started".to_string());
    save_run_record(root, &record)?;
    Ok(record)
}

pub fn mission_for_research(topic: &str, opts: StartOptions) -> Mission {
    let id = format!("research-{}", slug(topic));
    Mission {
        team_name: slug(&id),
        id,
        prompt: research_prompt(topic, &opts),
    }
}

pub fn mission_for_review(target: &str, opts: StartOptions) -> Mission {
    let id = format!("review-{}", slug(target));
    Mission {
        team_name: slug(&id),
        id,
        prompt: review_prompt(target, opts),
    }
}

pub fn write_mission(root: &Path, mission: &Mission) -> Result<PathBuf, String> {
    let dir = root.join(".omc/team/missions");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.md", mission.team_name));
    fs::write(&path, &mission.prompt).map_err(|e| format!("failed to write mission: {e}"))?;
    Ok(path)
}

pub fn import_linear_issue(root: &Path, issue_id: &str) -> Result<PathBuf, String> {
    let imported = linear_import_task(root, issue_id, None)?;
    Ok(root.join(format!(
        ".omc/team/imports/{}.md",
        slug(&imported.issue_ref)
    )))
}

pub fn collect_handoff(root: &Path, team_name: &str) -> Result<String, String> {
    let mut candidates = vec![root.join(format!(".claude/tasks/{team_name}"))];
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(format!(".claude/tasks/{team_name}")));
    }

    let mut sections = Vec::new();
    for dir in candidates {
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_file() {
                let body = fs::read_to_string(&path).unwrap_or_default();
                sections.push(format!("## {}\n\n{}", path.display(), body.trim()));
            }
        }
    }

    let generated_at = unix_timestamp().to_string();
    let summary = if sections.is_empty() {
        format!(
            "# OMC Team Handoff: {team_name}\n\nGenerated: {generated_at}\n\nNo Claude team task files were found. Ask the lead to clean up the team and paste final results here."
        )
    } else {
        format!(
            "# OMC Team Handoff: {team_name}\n\nGenerated: {generated_at}\n\n{}",
            sections.join("\n\n")
        )
    };

    let dir = root.join(".omc/team/handoffs");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    fs::write(dir.join(format!("{team_name}.md")), &summary).map_err(|e| e.to_string())?;
    Ok(summary)
}

pub fn post_handoff_to_tracker(
    root: &Path,
    team_name: &str,
    summary: &str,
    tracker: TrackerKind,
    done: bool,
) -> Result<RunRecord, String> {
    let mut record = find_run_record(root, team_name, Some(tracker))?;
    record.handoff_comment_id = trackers::post_handoff(&record, summary, done)?;
    record.last_known_state = Some(if done { "done" } else { "review" }.to_string());
    save_run_record(root, &record)?;
    Ok(record)
}

pub fn post_runtime_handoff_to_tracker(
    root: &Path,
    target: &str,
    summary: &str,
    runtime: RuntimeKind,
    tracker: TrackerKind,
    done: bool,
) -> Result<RunRecord, String> {
    let runtime_record = find_runtime_record(root, target, Some(runtime))?;
    let mut record = if let Some(run_id) = &runtime_record.tracker_run_id {
        find_run_record_by_run_id(root, run_id, Some(tracker))?
    } else if let Some(team_name) = &runtime_record.tracker_team_name {
        find_run_record(root, team_name, Some(tracker))?
    } else {
        return Err(format!(
            "runtime run {} has no linked {} tracker record",
            runtime_record.run_id,
            tracker.as_str()
        ));
    };
    record.handoff_comment_id = trackers::post_handoff(&record, summary, done)?;
    record.last_known_state = Some(if done { "done" } else { "review" }.to_string());
    save_run_record(root, &record)?;
    Ok(record)
}

pub fn handle_hook(kind: HookKind, stdin_json: &str) -> Result<(), String> {
    let raw = stdin_json.to_lowercase();
    match kind {
        HookKind::TaskCreated => {
            require_words(&raw, &["ownership", "acceptance", "verification"]).map_err(|missing| {
                format!(
                    "OMC Team gate: task creation requires ownership, acceptance, and verification. Missing: {}",
                    missing.join(", ")
                )
            })
        }
        HookKind::TaskCompleted => {
            require_words(&raw, &["tests", "handoff"]).map_err(|missing| {
                format!(
                    "OMC Team gate: task completion requires tests and handoff evidence. Missing: {}",
                    missing.join(", ")
                )
            })
        }
        HookKind::TeammateIdle => {
            if raw.contains("completed") || raw.contains("handoff") {
                Ok(())
            } else {
                Err("OMC Team gate: teammate is idle without completion or handoff evidence; continue working or report a blocker.".to_string())
            }
        }
    }
}

pub fn parse_task_card(content: &str) -> Result<TaskCard, String> {
    let start = content
        .find(META_START)
        .ok_or("task card is missing omc-team JSON metadata")?;
    let after_start = start + META_START.len();
    let end = content[after_start..]
        .find(META_END)
        .map(|idx| after_start + idx)
        .ok_or("task card metadata is missing closing marker")?;
    let meta_raw = content[after_start..end].trim();
    let meta: TaskMetadata =
        serde_json::from_str(meta_raw).map_err(|e| format!("invalid task metadata JSON: {e}"))?;
    let body = content[end + META_END.len()..].trim().to_string();
    Ok(TaskCard { meta, body })
}

pub fn render_task_card(card: &TaskCard) -> String {
    let meta = serde_json::to_string_pretty(&card.meta).expect("task metadata serializes");
    format!("{META_START}\n{meta}\n{META_END}\n\n{}\n", card.body.trim())
}

fn ensure_ready(task: &TaskCard) -> Result<(), String> {
    if !task.meta.agent_ready {
        return Err(format!("task {} is not agent_ready", task.meta.id));
    }
    if task.meta.ownership.is_empty() {
        return Err(format!("task {} is missing ownership", task.meta.id));
    }
    if task.meta.acceptance.is_empty() {
        return Err(format!("task {} is missing acceptance", task.meta.id));
    }
    if task.meta.verification.is_empty() {
        return Err(format!("task {} is missing verification", task.meta.id));
    }
    Ok(())
}

fn infer_tracker(target: &str) -> Option<TrackerKind> {
    if trackers::looks_like_github_issue_ref(target) {
        Some(TrackerKind::GitHub)
    } else if looks_like_linear_id(target) {
        Some(TrackerKind::Linear)
    } else {
        None
    }
}

fn ensure_dir(path: PathBuf, report: &mut InitReport) -> Result<(), String> {
    if path.exists() {
        report.unchanged.push(path);
    } else {
        fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        report.created.push(path);
    }
    Ok(())
}

fn upsert_file(path: PathBuf, content: &str, report: &mut InitReport) -> Result<(), String> {
    if path.exists() {
        let old = fs::read_to_string(&path).unwrap_or_default();
        if old == content {
            report.unchanged.push(path);
        } else {
            fs::write(&path, content).map_err(|e| e.to_string())?;
            report.updated.push(path);
        }
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, content).map_err(|e| e.to_string())?;
        report.created.push(path);
    }
    Ok(())
}

fn upsert_gitignore(root: &Path, report: &mut InitReport) -> Result<(), String> {
    let path = root.join(".gitignore");
    let required = [".claude/worktrees/", ".omc/team/"];
    let mut content = fs::read_to_string(&path).unwrap_or_default();
    let mut changed = false;
    for line in required {
        if !content.lines().any(|existing| existing.trim() == line) {
            if !content.ends_with('\n') && !content.is_empty() {
                content.push('\n');
            }
            content.push_str(line);
            content.push('\n');
            changed = true;
        }
    }
    if changed || !path.exists() {
        fs::write(&path, content).map_err(|e| e.to_string())?;
        report.updated.push(path);
    } else {
        report.unchanged.push(path);
    }
    Ok(())
}

fn upsert_claude_md(root: &Path, report: &mut InitReport) -> Result<(), String> {
    upsert_agent_discipline_doc(root.join("CLAUDE.md"), report)
}

fn upsert_agents_md(root: &Path, report: &mut InitReport) -> Result<(), String> {
    upsert_agent_discipline_doc(root.join("AGENTS.md"), report)
}

fn upsert_agent_discipline_doc(path: PathBuf, report: &mut InitReport) -> Result<(), String> {
    let block = format!(
        "{DISCIPLINE_START}\n{}\n{DISCIPLINE_END}\n",
        native_agent_discipline().trim()
    );
    if !path.exists() {
        fs::write(&path, format!("{block}\n")).map_err(|e| e.to_string())?;
        report.created.push(path);
        return Ok(());
    }

    let old = fs::read_to_string(&path).unwrap_or_default();
    let next = if let Some(start) = old.find(DISCIPLINE_START) {
        let after_start = start + DISCIPLINE_START.len();
        let Some(end_rel) = old[after_start..].find(DISCIPLINE_END) else {
            return Err(format!(
                "{} has an OMC discipline start marker without an end marker",
                path.display()
            ));
        };
        let end = after_start + end_rel + DISCIPLINE_END.len();
        format!("{}{}{}", &old[..start], block, &old[end..])
    } else {
        let mut content = old.clone();
        if !content.ends_with('\n') && !content.is_empty() {
            content.push('\n');
        }
        if !content.ends_with("\n\n") && !content.is_empty() {
            content.push('\n');
        }
        content.push_str(&block);
        content.push('\n');
        content
    };

    if next == old {
        report.unchanged.push(path);
    } else {
        fs::write(&path, next).map_err(|e| e.to_string())?;
        report.updated.push(path);
    }
    Ok(())
}

fn upsert_settings(root: &Path, report: &mut InitReport) -> Result<(), String> {
    let path = root.join(SETTINGS_JSON);
    let mut value: Value = if path.exists() {
        serde_json::from_str(&fs::read_to_string(&path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("invalid {}: {e}", path.display()))?
    } else {
        json!({})
    };

    ensure_object_field(&mut value, "env")?;
    ensure_object_field(&mut value, "hooks")?;
    value["env"]["CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS"] = json!("1");
    value["hooks"]["TaskCreated"] = json!([{
        "matcher": "*",
        "hooks": [{"type": "command", "command": "omc-team hook task-created"}]
    }]);
    value["hooks"]["TaskCompleted"] = json!([{
        "matcher": "*",
        "hooks": [{"type": "command", "command": "omc-team hook task-completed"}]
    }]);
    value["hooks"]["TeammateIdle"] = json!([{
        "matcher": "*",
        "hooks": [{"type": "command", "command": "omc-team hook teammate-idle"}]
    }]);

    let mut rendered = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    rendered.push('\n');
    upsert_file(path, &rendered, report)
}

fn ensure_object_field(value: &mut Value, field: &str) -> Result<(), String> {
    if value.get(field).is_none() || value.get(field) == Some(&Value::Null) {
        value[field] = json!({});
    }
    if !value[field].is_object() {
        return Err(format!("{field} must be a JSON object in Claude settings"));
    }
    Ok(())
}

fn native_agent_discipline() -> &'static str {
    r#"## OMC Native Agent Discipline

This project uses OMC's built-in agent discipline, inspired by Karpathy-style guidance for reducing common LLM coding mistakes. Source inspiration: https://github.com/forrestchang/andrej-karpathy-skills

### Think Before Coding

- State important assumptions before acting.
- Ask when ambiguity changes the implementation or risk.
- Surface meaningful tradeoffs instead of silently choosing.
- Push back when the requested path is more complex than needed.

### Simplicity First

- Build the smallest change that satisfies the task.
- Do not add speculative features, configurability, or abstractions.
- Avoid new layers for one-off behavior.
- If the solution is growing without clear payoff, simplify before continuing.

### Surgical Changes

- Every changed line must trace to the task.
- Match existing style and local patterns.
- Do not refactor, reformat, or delete unrelated code.
- Clean up only unused code introduced by your own change.
- Mention unrelated issues separately instead of fixing them opportunistically.

### Goal-Driven Execution

- Define acceptance and verification before implementation.
- Prefer a failing or focused test before a risky fix.
- Keep looping until the stated verification passes or a blocker is explicit.
- Final handoff must include changed files, verification run, residual risk, and follow-ups.

### GitHub Contract Discipline

- Before creating issues or pull requests, read repository templates and CONTRIBUTING guidance.
- Preserve template headings, checklists, and required fields.
- Fill issue and PR bodies through generated body files, then lint them before calling GitHub.
- PRs must explain scope, tests, linked issues, and any intentional omissions.

### Tooling Boundary

- Prefer OMC native commands and adapters for team orchestration, tracker updates, sessions, usage, and handoff.
- Do not require x-cmd or x-cmd skills for normal project work.
- Treat x-cmd as an optional toolbox only when a task explicitly benefits from it.
- Do not use x-cmd as a hidden tracker, scheduler, memory layer, or source of team truth.
"#
}

fn native_agent_discipline_prompt() -> &'static str {
    r#"OMC Native Agent Discipline:
- Think before coding: state assumptions, ask on meaningful ambiguity, and surface tradeoffs.
- Simplicity first: choose the smallest implementation that satisfies the mission.
- Surgical changes: every changed line must trace to this mission; preserve unrelated code and user edits.
- Goal-driven execution: define verification early, loop until it passes, and report evidence.
- GitHub contract discipline: follow repository templates and CONTRIBUTING guidance for issues and PRs.
- Tooling boundary: use OMC native adapters first; x-cmd is optional and must not become the tracker, scheduler, memory layer, or team truth.
"#
}

fn implementation_prompt(task: &TaskCard, opts: &StartOptions) -> String {
    let team_size = opts.team_size;
    format!(
        r#"Create a Claude Code agent team for this implementation mission.

{discipline}

Use {team_size} total agents unless Claude Code constrains the team size. Use official agent team task list, mailbox, and teammate coordination. Require plan approval before any teammate edits files. All coding teammates must use git worktree isolation. Do not create an alternate task tracker.

Team structure:
- Lead: coordinate, split work, keep the task list current, wait for teammates before synthesizing.
- Planner: split the mission into tasks with ownership, acceptance, and verification.
- Executors: self-claim unblocked tasks from the shared task list.
- Reviewer: validate correctness, test evidence, and integration risk.
- Security/Audit: join if the risk is high or sensitive files are touched.

Cell coordination:
- If more than 5 agents are requested, do not run a flat chat. Use one lead and cells of builder/reviewer/verifier.
- Cell members may discuss inside the cell. Cross-cell communication must go through task list updates, mailbox summaries, whiteboard facts, or cell handoffs.
- The lead should read accepted facts, cell handoffs, and unresolved questions instead of raw transcripts.

Session, usage, and context contract:
- Treat every teammate run as ephemeral. Before completion, leave handoff.md, resume_brief.md, evidence.json, usage.json, and next_action.md content in the team task artifacts.
- At 70% context write a checkpoint. At 85% write a resume brief. At 92% stop claiming new work. At 95% force handoff and resume in a fresh session.
- Token, cost, context, and rate-limit data must include source and confidence when reported. Prefer provider data, then transcript parsing, then observer data, then estimates.

Task:
- ID: {id}
- Title: {title}
- Risk: {risk}

Ownership:
{ownership}

Acceptance:
{acceptance}

Verification:
{verification}

Context:
{body}

Completion requirements:
- Each teammate must leave a handoff with changed files, tests run, risks, and follow-ups.
- Each teammate must leave a resume brief that lets a fresh session continue without raw transcript replay.
- The lead must synthesize a final handoff suitable for GitHub/Linear.
- If this maps to Linear or GitHub, include the external issue ID in the final summary.
"#,
        discipline = native_agent_discipline_prompt(),
        id = task.meta.id,
        title = task.meta.title,
        risk = task.meta.risk,
        ownership = bullets(&task.meta.ownership),
        acceptance = bullets(&task.meta.acceptance),
        verification = bullets(&task.meta.verification),
        body = task.body
    )
}

fn runtime_adapter_prompt(task: &TaskCard, opts: &StartOptions) -> String {
    let runtime = opts.runtime.as_str();
    format!(
        r#"Prepare an OMC runtime mission for the local `{runtime}` adapter.

{discipline}

This is not a Claude Code official agent team launch. OMC is using `{runtime}` as a local execution backend while GitHub/Linear remain visibility adapters only.

Runtime contract:
- Do not use GitHub or Linear as the runtime task list.
- Do not create upstream FSC or KohakuTerrarium pull requests from this run.
- Keep tracker updates to lease/start/handoff visibility only.
- Collect artifacts under `.omc/team/{runtime}/` and leave a final handoff.
- Every runtime worker invocation must leave enough handoff/resume/usage evidence for OMC to continue even if the native session disappears.
- Follow the context budget protocol: checkpoint at 70%, resume brief at 85%, stop new work at 92%, force handoff at 95%.

Task:
- ID: {id}
- Title: {title}
- Risk: {risk}

Ownership:
{ownership}

Acceptance:
{acceptance}

Verification:
{verification}

Context:
{body}
"#,
        discipline = native_agent_discipline_prompt(),
        id = task.meta.id,
        title = task.meta.title,
        risk = task.meta.risk,
        ownership = bullets(&task.meta.ownership),
        acceptance = bullets(&task.meta.acceptance),
        verification = bullets(&task.meta.verification),
        body = task.body
    )
}

fn research_prompt(topic: &str, opts: &StartOptions) -> String {
    format!(
        r#"Create a Claude Code agent team for a research mission.

{discipline}

Use {team_size} teammates. Default to read-only research and review. Do not edit repo files unless I explicitly approve a follow-up implementation plan.
Treat each teammate as ephemeral: leave a compact resume brief, evidence, unresolved questions, and usage/source confidence in the final artifacts.

Roles:
- Lead synthesizes findings and challenges weak conclusions.
- Researcher A investigates the strongest implementation path.
- Researcher B investigates risks, limitations, and alternatives.
- Reviewer checks assumptions against project docs and official documentation.

Topic:
{topic}

Final output:
- Decision-ready summary.
- Concrete options.
- Risks and unknowns.
- Recommended next implementation slice.
"#,
        discipline = native_agent_discipline_prompt(),
        team_size = opts.team_size,
        topic = topic
    )
}

fn review_prompt(target: &str, opts: StartOptions) -> String {
    let mut lenses = vec![
        "correctness and regressions",
        "maintainability and scope control",
        "test coverage and verification",
    ];
    if opts.security_review {
        lenses.push("security implications");
    }
    if opts.test_review {
        lenses.push("test reliability and missing scenarios");
    }
    format!(
        r#"Create a Claude Code agent team to review {target}.

{discipline}

Use {team_size} teammates. Each teammate must focus on a distinct review lens and report findings with severity and evidence. The lead must synthesize duplicate findings and produce a final review.
Treat each reviewer session as ephemeral: leave a compact resume brief, evidence, unresolved questions, and usage/source confidence in the final artifacts.

Review lenses:
{lenses}

Rules:
- Do not make code changes during review unless I explicitly approve a fix pass.
- Use existing tests and project docs as the source of truth.
- If this is a PR, include the PR identifier in the final handoff.
"#,
        discipline = native_agent_discipline_prompt(),
        team_size = opts.team_size,
        lenses = bullets(
            &lenses
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
        )
    )
}

fn bullets(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn require_words(raw: &str, words: &[&str]) -> Result<(), Vec<String>> {
    let missing = words
        .iter()
        .filter(|word| !raw.contains(**word))
        .map(|word| (*word).to_string())
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

pub(crate) fn version_at_least(raw: &str, min: (u32, u32, u32)) -> bool {
    let Some(version) = raw
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find(|part| part.matches('.').count() >= 2)
    else {
        return false;
    };
    let parts = version
        .split('.')
        .take(3)
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect::<Vec<_>>();
    (parts[0], parts[1], parts[2]) >= min
}

fn looks_like_linear_id(raw: &str) -> bool {
    let mut parts = raw.split('-');
    let Some(prefix) = parts.next() else {
        return false;
    };
    let Some(number) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && !prefix.is_empty()
        && prefix.chars().all(|c| c.is_ascii_alphabetic())
        && number.chars().all(|c| c.is_ascii_digit())
}

pub(crate) fn slug(raw: &str) -> String {
    let mut out = String::default();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn agent_planner() -> &'static str {
    r#"---
name: omc-planner
description: Split OMC Team missions into ownership-safe tasks with acceptance and verification.
---

You are the planning teammate for OMC Team. Produce task slices that avoid file conflicts. Every task must include ownership, acceptance, verification, and dependencies when relevant.

Follow OMC Native Agent Discipline: state assumptions, prefer the simplest viable task split, keep ownership surgical, and define verification before implementation.
"#
}

fn agent_executor() -> &'static str {
    r#"---
name: omc-executor
description: Implement one ownership-bounded task in a Claude Code agent team.
---

You implement only the task you claimed. Stay inside ownership boundaries, preserve user changes, run verification, and leave a handoff with changed files, tests, risks, and follow-ups.

Follow OMC Native Agent Discipline: minimize code, avoid speculative abstractions, touch only task-relevant lines, and keep looping until verification passes or the blocker is explicit.
"#
}

fn agent_reviewer() -> &'static str {
    r#"---
name: omc-reviewer
description: Review OMC Team implementation work for correctness, regressions, and verification quality.
---

You review completed teammate work. Lead with concrete findings and evidence. Verify acceptance criteria and test results before recommending completion.

Follow OMC Native Agent Discipline: challenge assumptions, flag unnecessary complexity, protect unrelated code, and require clear verification evidence.
"#
}

fn agent_security() -> &'static str {
    r#"---
name: omc-security-auditor
description: Audit high-risk OMC Team tasks for security and unsafe automation behavior.
---

You inspect security-sensitive changes, secrets handling, command execution, auth, data boundaries, and destructive operations. Report severity and exact evidence.

Follow OMC Native Agent Discipline: state risk assumptions, avoid broad rewrites, keep findings scoped, and require concrete mitigation or explicit residual risk.
"#
}

fn agent_linear_reporter() -> &'static str {
    r#"---
name: omc-linear-reporter
description: Prepare Linear/GitHub handoff comments from OMC Team results.
---

You turn teammate handoffs into concise Linear/GitHub updates: status, changed files, tests, risks, blockers, and next action.

Follow OMC Native Agent Discipline and repository contract discipline: preserve issue/PR templates, include verification, and avoid overstating completion.
"#
}

pub(crate) fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
fn unix_timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_markdown_json_task_card() {
        let card = render_task_card(&TaskCard {
            meta: TaskMetadata {
                id: "LOCAL-1".to_string(),
                title: "Test".to_string(),
                agent_ready: true,
                risk: "low".to_string(),
                ownership: vec!["src/lib.rs".to_string()],
                acceptance: vec!["works".to_string()],
                verification: vec!["cargo test".to_string()],
                source: None,
                linear_id: None,
                tracker: None,
                github_repo: None,
                github_issue_number: None,
            },
            body: "Body".to_string(),
        });

        let parsed = parse_task_card(&card).unwrap();
        assert_eq!(parsed.meta.id, "LOCAL-1");
        assert_eq!(parsed.body, "Body");
    }

    #[test]
    fn readiness_gate_rejects_missing_verification() {
        let task = TaskCard {
            meta: TaskMetadata {
                id: "LOCAL-2".to_string(),
                title: "Bad".to_string(),
                agent_ready: true,
                risk: "low".to_string(),
                ownership: vec!["src/lib.rs".to_string()],
                acceptance: vec!["works".to_string()],
                verification: vec![],
                source: None,
                linear_id: None,
                tracker: None,
                github_repo: None,
                github_issue_number: None,
            },
            body: String::default(),
        };
        assert!(ensure_ready(&task).unwrap_err().contains("verification"));
    }

    #[test]
    fn hook_task_created_blocks_missing_fields() {
        let err = handle_hook(HookKind::TaskCreated, r#"{"task":"no ownership"}"#).unwrap_err();
        assert!(err.contains("acceptance"));
        assert!(err.contains("verification"));
    }

    #[test]
    fn version_parser_accepts_minimum() {
        assert!(version_at_least("Claude Code v2.1.32", (2, 1, 32)));
        assert!(version_at_least("claude 2.2.0", (2, 1, 32)));
        assert!(!version_at_least("claude 2.1.31", (2, 1, 32)));
    }

    #[test]
    fn init_project_writes_expected_artifacts() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).unwrap();
        init_project(&root).unwrap();

        assert!(root.join(".claude/settings.json").exists());
        assert!(root.join(".claude/agents/omc-planner.md").exists());
        assert!(root.join(".worktreeinclude").exists());
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents.contains(DISCIPLINE_START));
        assert!(agents.contains("OMC Native Agent Discipline"));
        assert!(agents.contains("Do not require x-cmd"));
        let claude = fs::read_to_string(root.join("CLAUDE.md")).unwrap();
        assert!(claude.contains(DISCIPLINE_START));
        assert!(claude.contains("OMC Native Agent Discipline"));
        assert!(claude.contains("Think Before Coding"));
        let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert!(ignore.contains(".claude/worktrees/"));
        assert!(ignore.contains(".omc/team/"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn init_project_preserves_existing_claude_md() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("AGENTS.md"),
            "# Existing Agent Rules\n\nUse OMC.\n",
        )
        .unwrap();
        fs::write(root.join("CLAUDE.md"), "# Existing Rules\n\nKeep me.\n").unwrap();

        init_project(&root).unwrap();
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents.contains("# Existing Agent Rules"));
        assert!(agents.contains("Use OMC."));
        assert_eq!(agents.matches(DISCIPLINE_START).count(), 1);
        let claude = fs::read_to_string(root.join("CLAUDE.md")).unwrap();
        assert!(claude.contains("# Existing Rules"));
        assert!(claude.contains("Keep me."));
        assert_eq!(claude.matches(DISCIPLINE_START).count(), 1);

        init_project(&root).unwrap();
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert_eq!(agents.matches(DISCIPLINE_START).count(), 1);
        let claude = fs::read_to_string(root.join("CLAUDE.md")).unwrap();
        assert_eq!(claude.matches(DISCIPLINE_START).count(), 1);

        let _ = fs::remove_dir_all(root);
    }

    fn unique_temp_dir() -> PathBuf {
        let mut dir = env::temp_dir();
        dir.push(format!("omc-team-test-{}", unix_timestamp_nanos()));
        dir
    }
}
