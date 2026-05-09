use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::{Mission, RuntimeKind, StartOptions, TaskCard, slug, unix_timestamp};

const SESSION_RECORD_TYPE: &str = "agent_session";
const INVOCATION_RECORD_TYPE: &str = "agent_invocation";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSessionState {
    Planned,
    Spawned,
    Active,
    Checkpointing,
    Saturated,
    HandoffReady,
    Completed,
    Abandoned,
    Resumable,
}

impl AgentSessionState {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.to_ascii_lowercase().replace('-', "_").as_str() {
            "planned" => Ok(Self::Planned),
            "spawned" => Ok(Self::Spawned),
            "active" => Ok(Self::Active),
            "checkpointing" => Ok(Self::Checkpointing),
            "saturated" => Ok(Self::Saturated),
            "handoff_ready" => Ok(Self::HandoffReady),
            "completed" => Ok(Self::Completed),
            "abandoned" => Ok(Self::Abandoned),
            "resumable" => Ok(Self::Resumable),
            _ => Err(format!("unknown agent session state: {raw}")),
        }
    }

    fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Abandoned)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    Provider,
    Transcript,
    Abtop,
    Xcmd,
    Estimated,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageConfidence {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextGuardAction {
    Continue,
    Checkpoint,
    ResumeBrief,
    StopNewTask,
    ForcedHandoff,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageRollup {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_create_tokens: u64,
    pub cost_usd: Option<f64>,
    pub duration_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageMeasurement {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_create_tokens: u64,
    pub cost_usd: Option<f64>,
    pub source: UsageSource,
    pub confidence: UsageConfidence,
}

impl Default for UsageMeasurement {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_create_tokens: 0,
            cost_usd: None,
            source: UsageSource::Unknown,
            confidence: UsageConfidence::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSessionRecord {
    pub record_type: String,
    pub agent_id: String,
    pub run_id: String,
    pub cell_id: Option<String>,
    pub role: String,
    pub runtime: RuntimeKind,
    pub provider: String,
    pub current_task: String,
    pub state: AgentSessionState,
    pub epoch: u32,
    pub last_resume_brief: Option<String>,
    pub last_handoff: Option<String>,
    pub usage_rollup: UsageRollup,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInvocationRecord {
    pub record_type: String,
    pub invocation_id: String,
    pub agent_id: String,
    pub run_id: String,
    pub cell_id: Option<String>,
    pub runtime: RuntimeKind,
    pub provider: String,
    pub model: Option<String>,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub status: String,
    pub exit_reason: Option<String>,
    pub usage: UsageMeasurement,
    pub context_percent: Option<f32>,
    pub rate_limit: Option<String>,
    pub tool_calls: Vec<ToolInvocation>,
    pub mcp_calls: Vec<ToolInvocation>,
    pub skill_calls: Vec<ToolInvocation>,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub name: String,
    pub target: Option<String>,
    pub duration_ms: Option<u64>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub kind: String,
    pub path: Option<String>,
    pub command: Option<String>,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tool_calls: u32,
    pub max_seconds: u64,
    pub checkpoint_percent: f32,
    pub resume_brief_percent: f32,
    pub stop_new_task_percent: f32,
    pub forced_handoff_percent: f32,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            input_tokens: 64_000,
            output_tokens: 16_000,
            tool_calls: 80,
            max_seconds: 60 * 60 * 2,
            checkpoint_percent: 70.0,
            resume_brief_percent: 85.0,
            stop_new_task_percent: 92.0,
            forced_handoff_percent: 95.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextGuardDecision {
    pub action: ContextGuardAction,
    pub percent: f32,
    pub source: UsageSource,
    pub confidence: UsageConfidence,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellPlan {
    pub total_agents: u8,
    pub lead_role: String,
    pub cells: Vec<CellSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellSpec {
    pub cell_id: String,
    pub agents: Vec<CellAgentSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellAgentSpec {
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservabilityStartReport {
    pub run_id: String,
    pub briefing_path: PathBuf,
    pub session_count: usize,
    pub invocation_count: usize,
    pub cell_plan: CellPlan,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageReport {
    pub group_by: String,
    pub groups: Vec<UsageReportGroup>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageReportGroup {
    pub key: String,
    pub invocation_count: usize,
    pub usage: UsageRollup,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservabilityTopSnapshot {
    pub generated_at: u64,
    pub active_sessions: usize,
    pub total_sessions: usize,
    pub total_invocations: usize,
    pub usage: UsageRollup,
}

#[derive(Debug, Clone, Serialize)]
pub struct ObservabilityDoctorReport {
    pub ok: bool,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageGroupBy {
    Agent,
    Cell,
    Runtime,
    Provider,
}

impl UsageGroupBy {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.to_ascii_lowercase().as_str() {
            "agent" => Ok(Self::Agent),
            "cell" => Ok(Self::Cell),
            "runtime" => Ok(Self::Runtime),
            "provider" => Ok(Self::Provider),
            _ => Err(format!("unknown usage group: {raw}")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Cell => "cell",
            Self::Runtime => "runtime",
            Self::Provider => "provider",
        }
    }
}

#[derive(Debug, Clone)]
struct AgentSpec {
    agent_id: String,
    cell_id: Option<String>,
    role: String,
}

pub fn init_observability(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut touched = Vec::new();
    for dir in [
        ".omc/team/sessions",
        ".omc/team/invocations",
        ".omc/team/whiteboard/entries",
        ".omc/team/runs",
    ] {
        let path = root.join(dir);
        fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        touched.push(path);
    }
    for file in [
        ("facts.md", "# Accepted Facts\n\n"),
        ("decisions.md", "# Accepted Decisions\n\n"),
        ("risks.md", "# Accepted Risks\n\n"),
        ("questions.md", "# Open Questions\n\n"),
        ("handoffs.md", "# Handoffs\n\n"),
    ] {
        let path = root.join(".omc/team/whiteboard").join(file.0);
        if !path.exists() {
            fs::write(&path, file.1).map_err(|e| e.to_string())?;
        }
        touched.push(path);
    }
    let usage = root.join(".omc/team/usage.jsonl");
    if !usage.exists() {
        fs::write(&usage, "").map_err(|e| e.to_string())?;
    }
    touched.push(usage);
    Ok(touched)
}

pub fn record_team_launch(
    root: &Path,
    run_id: &str,
    mission: &Mission,
    task: &TaskCard,
    opts: &StartOptions,
    mission_path: &Path,
    tracker_run_id: Option<&str>,
) -> Result<ObservabilityStartReport, String> {
    init_observability(root)?;
    let run_dir = root.join(".omc/team/runs").join(run_id);
    fs::create_dir_all(&run_dir).map_err(|e| e.to_string())?;
    let cell_plan = build_cell_plan(opts.team_size);
    let briefing_path = run_dir.join("briefing.md");
    fs::write(
        &briefing_path,
        render_briefing(
            run_id,
            mission,
            task,
            opts,
            mission_path,
            tracker_run_id,
            &cell_plan,
        ),
    )
    .map_err(|e| e.to_string())?;
    fs::write(run_dir.join("exit-contract.md"), render_exit_contract())
        .map_err(|e| e.to_string())?;

    let agents = agent_specs(run_id, &cell_plan);
    let now = unix_timestamp();
    let mut invocation_count = 0;
    for spec in &agents {
        let session = AgentSessionRecord {
            record_type: SESSION_RECORD_TYPE.to_string(),
            agent_id: spec.agent_id.clone(),
            run_id: run_id.to_string(),
            cell_id: spec.cell_id.clone(),
            role: spec.role.clone(),
            runtime: opts.runtime,
            provider: provider_for_runtime(opts.runtime).to_string(),
            current_task: task.meta.id.clone(),
            state: AgentSessionState::Planned,
            epoch: 0,
            last_resume_brief: Some(briefing_path.display().to_string()),
            last_handoff: None,
            usage_rollup: UsageRollup::default(),
            created_at: now,
            updated_at: now,
        };
        save_session(root, &session)?;
        let invocation = planned_invocation(&session, now);
        save_invocation(root, &invocation)?;
        append_usage_event(root, &invocation)?;
        invocation_count += 1;
    }

    Ok(ObservabilityStartReport {
        run_id: run_id.to_string(),
        briefing_path,
        session_count: agents.len(),
        invocation_count,
        cell_plan,
    })
}

pub fn build_cell_plan(total_agents: u8) -> CellPlan {
    let total_agents = total_agents.max(1);
    let mut remaining = total_agents.saturating_sub(1);
    let mut cells = Vec::new();
    let roles = ["builder", "reviewer", "verifier"];
    let mut cell_index = 1;
    while remaining > 0 {
        let take = remaining.min(3);
        let agents = roles
            .iter()
            .take(take as usize)
            .map(|role| CellAgentSpec {
                role: (*role).to_string(),
            })
            .collect::<Vec<_>>();
        cells.push(CellSpec {
            cell_id: format!("cell-{cell_index}"),
            agents,
        });
        remaining -= take;
        cell_index += 1;
    }
    CellPlan {
        total_agents,
        lead_role: "lead".to_string(),
        cells,
    }
}

pub fn context_guard_decision(
    percent: f32,
    source: UsageSource,
    confidence: UsageConfidence,
) -> ContextGuardDecision {
    let budget = ContextBudget::default();
    let (action, message) = if percent >= budget.forced_handoff_percent {
        (
            ContextGuardAction::ForcedHandoff,
            "force handoff and resume in a fresh session",
        )
    } else if percent >= budget.stop_new_task_percent {
        (
            ContextGuardAction::StopNewTask,
            "stop claiming new work and only close out current task",
        )
    } else if percent >= budget.resume_brief_percent {
        (
            ContextGuardAction::ResumeBrief,
            "write resume brief before continuing",
        )
    } else if percent >= budget.checkpoint_percent {
        (ContextGuardAction::Checkpoint, "write checkpoint")
    } else {
        (ContextGuardAction::Continue, "continue")
    };
    ContextGuardDecision {
        action,
        percent,
        source,
        confidence,
        message: message.to_string(),
    }
}

pub fn transition_session_state(
    root: &Path,
    agent_id: &str,
    next: AgentSessionState,
) -> Result<AgentSessionRecord, String> {
    let mut session = load_session(root, agent_id)?;
    if !valid_transition(session.state, next) {
        return Err(format!(
            "invalid session transition for {agent_id}: {:?} -> {:?}",
            session.state, next
        ));
    }
    if next == AgentSessionState::Completed {
        validate_exit_contract_for_session(&session)?;
    }
    session.state = next;
    session.epoch += 1;
    session.updated_at = unix_timestamp();
    save_session(root, &session)?;
    Ok(session)
}

pub fn mark_session_handoff(
    root: &Path,
    agent_id: &str,
    handoff_path: &Path,
    resume_brief_path: Option<&Path>,
) -> Result<AgentSessionRecord, String> {
    let mut session = load_session(root, agent_id)?;
    session.last_handoff = Some(handoff_path.display().to_string());
    if let Some(path) = resume_brief_path {
        session.last_resume_brief = Some(path.display().to_string());
    }
    session.state = AgentSessionState::HandoffReady;
    session.epoch += 1;
    session.updated_at = unix_timestamp();
    save_session(root, &session)?;
    Ok(session)
}

pub fn load_sessions(root: &Path) -> Result<Vec<AgentSessionRecord>, String> {
    let dir = root.join(".omc/team/sessions");
    let mut records = Vec::new();
    if !dir.exists() {
        return Ok(records);
    }
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let record: AgentSessionRecord = serde_json::from_str(&raw)
            .map_err(|e| format!("invalid session {}: {e}", path.display()))?;
        if record.record_type == SESSION_RECORD_TYPE {
            records.push(record);
        }
    }
    records.sort_by(|left, right| left.agent_id.cmp(&right.agent_id));
    Ok(records)
}

pub fn render_resume_packet(root: &Path, target: &str) -> Result<String, String> {
    let sessions = load_sessions(root)?;
    let selected = sessions
        .iter()
        .filter(|session| session.agent_id == target || session.run_id == target)
        .cloned()
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(format!("no agent session or run found for {target}"));
    }
    let run_id = selected[0].run_id.clone();
    let run_dir = root.join(".omc/team/runs").join(&run_id);
    fs::create_dir_all(&run_dir).map_err(|e| e.to_string())?;
    let packet = format!(
        "# OMC Resume Packet: {target}\n\n{}\n\n## Sessions\n\n{}\n\n## Whiteboard\n\n{}\n",
        read_optional(run_dir.join("briefing.md")),
        selected
            .iter()
            .map(render_session_resume_line)
            .collect::<Vec<_>>()
            .join("\n"),
        render_whiteboard(root)
    );
    let path = run_dir.join(format!("resume-{}.md", slug(target)));
    fs::write(path, &packet).map_err(|e| e.to_string())?;
    Ok(packet)
}

pub fn usage_report(
    root: &Path,
    run_id: Option<&str>,
    group_by: UsageGroupBy,
) -> Result<UsageReport, String> {
    let invocations = load_invocations(root)?;
    let mut groups: BTreeMap<String, (usize, UsageRollup, Vec<String>)> = BTreeMap::new();
    for invocation in invocations
        .into_iter()
        .filter(|item| run_id.is_none_or(|wanted| item.run_id == wanted))
    {
        let key = match group_by {
            UsageGroupBy::Agent => invocation.agent_id.clone(),
            UsageGroupBy::Cell => invocation
                .cell_id
                .clone()
                .unwrap_or_else(|| "lead".to_string()),
            UsageGroupBy::Runtime => invocation.runtime.as_str().to_string(),
            UsageGroupBy::Provider => invocation.provider.clone(),
        };
        let entry = groups
            .entry(key)
            .or_insert_with(|| (0, UsageRollup::default(), Vec::new()));
        entry.0 += 1;
        entry.1.input_tokens += invocation.usage.input_tokens;
        entry.1.output_tokens += invocation.usage.output_tokens;
        entry.1.cache_read_tokens += invocation.usage.cache_read_tokens;
        entry.1.cache_create_tokens += invocation.usage.cache_create_tokens;
        entry.1.duration_seconds += invocation
            .ended_at
            .unwrap_or(invocation.started_at)
            .saturating_sub(invocation.started_at);
        if let Some(cost) = invocation.usage.cost_usd {
            entry.1.cost_usd = Some(entry.1.cost_usd.unwrap_or(0.0) + cost);
        }
        let source = format!(
            "{:?}/{:?}",
            invocation.usage.source, invocation.usage.confidence
        )
        .to_ascii_lowercase();
        if !entry.2.contains(&source) {
            entry.2.push(source);
        }
    }
    Ok(UsageReport {
        group_by: group_by.as_str().to_string(),
        groups: groups
            .into_iter()
            .map(
                |(key, (invocation_count, usage, sources))| UsageReportGroup {
                    key,
                    invocation_count,
                    usage,
                    sources,
                },
            )
            .collect(),
    })
}

pub fn top_snapshot(root: &Path) -> Result<ObservabilityTopSnapshot, String> {
    let sessions = load_sessions(root)?;
    let usage = usage_report(root, None, UsageGroupBy::Provider)?
        .groups
        .into_iter()
        .fold(UsageRollup::default(), |mut acc, group| {
            acc.input_tokens += group.usage.input_tokens;
            acc.output_tokens += group.usage.output_tokens;
            acc.cache_read_tokens += group.usage.cache_read_tokens;
            acc.cache_create_tokens += group.usage.cache_create_tokens;
            acc.duration_seconds += group.usage.duration_seconds;
            if let Some(cost) = group.usage.cost_usd {
                acc.cost_usd = Some(acc.cost_usd.unwrap_or(0.0) + cost);
            }
            acc
        });
    let active_sessions = sessions
        .iter()
        .filter(|session| !session.state.terminal())
        .count();
    let total_invocations = load_invocations(root)?.len();
    Ok(ObservabilityTopSnapshot {
        generated_at: unix_timestamp(),
        active_sessions,
        total_sessions: sessions.len(),
        total_invocations,
        usage,
    })
}

pub fn observability_doctor(root: &Path) -> ObservabilityDoctorReport {
    let mut messages = Vec::new();
    let mut ok = true;
    for dir in [
        ".omc/team/sessions",
        ".omc/team/invocations",
        ".omc/team/runs",
    ] {
        let path = root.join(dir);
        if path.exists() {
            messages.push(format!("dir ok: {}", path.display()));
        } else {
            ok = false;
            messages.push(format!(
                "missing dir: {} (run `omc-team init`)",
                path.display()
            ));
        }
    }
    if root.join(".omc/team/usage.jsonl").exists() {
        messages.push("usage ledger ok: .omc/team/usage.jsonl".to_string());
    } else {
        ok = false;
        messages.push("missing usage ledger: .omc/team/usage.jsonl".to_string());
    }
    if dirs::home_dir()
        .map(|home| home.join(".x-cmd.root/X").exists())
        .unwrap_or(false)
    {
        messages.push("x-cmd detected: optional usage/session reference available".to_string());
    } else {
        messages.push("x-cmd not detected; OMC observability still works".to_string());
    }
    match Command::new("abtop").arg("--version").output() {
        Ok(output) if output.status.success() => messages.push(format!(
            "abtop detected: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        )),
        _ => {
            messages.push("abtop not detected; optional real-time monitor unavailable".to_string())
        }
    }
    ObservabilityDoctorReport { ok, messages }
}

fn save_session(root: &Path, record: &AgentSessionRecord) -> Result<(), String> {
    let dir = root.join(".omc/team/sessions");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", record.agent_id));
    let rendered = serde_json::to_string_pretty(record).map_err(|e| e.to_string())? + "\n";
    fs::write(path, rendered).map_err(|e| e.to_string())
}

fn load_session(root: &Path, agent_id: &str) -> Result<AgentSessionRecord, String> {
    let path = root
        .join(".omc/team/sessions")
        .join(format!("{agent_id}.json"));
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read session {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("invalid session {}: {e}", path.display()))
}

fn save_invocation(root: &Path, record: &AgentInvocationRecord) -> Result<(), String> {
    let dir = root.join(".omc/team/invocations");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", record.invocation_id));
    let rendered = serde_json::to_string_pretty(record).map_err(|e| e.to_string())? + "\n";
    fs::write(path, rendered).map_err(|e| e.to_string())
}

fn load_invocations(root: &Path) -> Result<Vec<AgentInvocationRecord>, String> {
    let dir = root.join(".omc/team/invocations");
    let mut records = Vec::new();
    if !dir.exists() {
        return Ok(records);
    }
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let record: AgentInvocationRecord = serde_json::from_str(&raw)
            .map_err(|e| format!("invalid invocation {}: {e}", path.display()))?;
        if record.record_type == INVOCATION_RECORD_TYPE {
            records.push(record);
        }
    }
    records.sort_by_key(|record| record.started_at);
    Ok(records)
}

fn append_usage_event(root: &Path, invocation: &AgentInvocationRecord) -> Result<(), String> {
    let path = root.join(".omc/team/usage.jsonl");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let line = serde_json::to_string(invocation).map_err(|e| e.to_string())?;
    let mut old = fs::read_to_string(&path).unwrap_or_default();
    old.push_str(&line);
    old.push('\n');
    fs::write(path, old).map_err(|e| e.to_string())
}

fn planned_invocation(session: &AgentSessionRecord, now: u64) -> AgentInvocationRecord {
    AgentInvocationRecord {
        record_type: INVOCATION_RECORD_TYPE.to_string(),
        invocation_id: format!("{}-epoch-{}", session.agent_id, session.epoch),
        agent_id: session.agent_id.clone(),
        run_id: session.run_id.clone(),
        cell_id: session.cell_id.clone(),
        runtime: session.runtime,
        provider: session.provider.clone(),
        model: None,
        started_at: now,
        ended_at: None,
        status: "planned".to_string(),
        exit_reason: None,
        usage: UsageMeasurement::default(),
        context_percent: None,
        rate_limit: None,
        tool_calls: Vec::new(),
        mcp_calls: Vec::new(),
        skill_calls: Vec::new(),
        evidence: Vec::new(),
    }
}

fn validate_exit_contract_for_session(session: &AgentSessionRecord) -> Result<(), String> {
    let Some(handoff) = &session.last_handoff else {
        return Err(format!(
            "session {} cannot complete without a handoff",
            session.agent_id
        ));
    };
    if !Path::new(handoff).exists() {
        return Err(format!(
            "session {} handoff path does not exist: {handoff}",
            session.agent_id
        ));
    }
    Ok(())
}

fn valid_transition(from: AgentSessionState, to: AgentSessionState) -> bool {
    if from == to {
        return true;
    }
    match from {
        AgentSessionState::Planned => matches!(
            to,
            AgentSessionState::Spawned | AgentSessionState::Abandoned
        ),
        AgentSessionState::Spawned => matches!(
            to,
            AgentSessionState::Active | AgentSessionState::Abandoned | AgentSessionState::Resumable
        ),
        AgentSessionState::Active => matches!(
            to,
            AgentSessionState::Checkpointing
                | AgentSessionState::Saturated
                | AgentSessionState::HandoffReady
                | AgentSessionState::Completed
                | AgentSessionState::Abandoned
                | AgentSessionState::Resumable
        ),
        AgentSessionState::Checkpointing => matches!(
            to,
            AgentSessionState::Active
                | AgentSessionState::Saturated
                | AgentSessionState::HandoffReady
                | AgentSessionState::Resumable
                | AgentSessionState::Abandoned
        ),
        AgentSessionState::Saturated => matches!(
            to,
            AgentSessionState::HandoffReady
                | AgentSessionState::Resumable
                | AgentSessionState::Abandoned
        ),
        AgentSessionState::HandoffReady => matches!(
            to,
            AgentSessionState::Completed
                | AgentSessionState::Resumable
                | AgentSessionState::Abandoned
        ),
        AgentSessionState::Resumable => matches!(
            to,
            AgentSessionState::Spawned | AgentSessionState::Active | AgentSessionState::Abandoned
        ),
        AgentSessionState::Completed | AgentSessionState::Abandoned => false,
    }
}

fn agent_specs(run_id: &str, plan: &CellPlan) -> Vec<AgentSpec> {
    let mut specs = vec![AgentSpec {
        agent_id: format!("{run_id}-lead"),
        cell_id: None,
        role: plan.lead_role.clone(),
    }];
    for cell in &plan.cells {
        for agent in &cell.agents {
            specs.push(AgentSpec {
                agent_id: format!("{run_id}-{}-{}", cell.cell_id, agent.role),
                cell_id: Some(cell.cell_id.clone()),
                role: agent.role.clone(),
            });
        }
    }
    specs
}

fn provider_for_runtime(runtime: RuntimeKind) -> &'static str {
    match runtime {
        RuntimeKind::Claude => "claude-code",
        RuntimeKind::Fsc => "fsc",
        RuntimeKind::Kohaku => "kohaku",
    }
}

fn render_briefing(
    run_id: &str,
    mission: &Mission,
    task: &TaskCard,
    opts: &StartOptions,
    mission_path: &Path,
    tracker_run_id: Option<&str>,
    cell_plan: &CellPlan,
) -> String {
    format!(
        r#"# OMC Team Briefing: {team}

Run: `{run_id}`
Tracker run: `{tracker_run}`
Runtime: `{runtime}`
Mission path: `{mission_path}`

## Task

- ID: {id}
- Title: {title}
- Risk: {risk}

## Context Budget Protocol

- 70% context: write checkpoint.
- 85% context: write resume brief.
- 92% context: stop claiming new work; only close out current task.
- 95% context: force handoff and continue in a fresh session.
- If context percent is unavailable, estimate from token/window and mark it low confidence.

## Cell Plan

{cell_plan}

## Ownership

{ownership}

## Acceptance

{acceptance}

## Verification

{verification}

## Mission Prompt

{prompt}
"#,
        team = mission.team_name,
        tracker_run = tracker_run_id.unwrap_or("none"),
        runtime = opts.runtime.as_str(),
        mission_path = mission_path.display(),
        id = task.meta.id,
        title = task.meta.title,
        risk = task.meta.risk,
        ownership = bullets(&task.meta.ownership),
        acceptance = bullets(&task.meta.acceptance),
        verification = bullets(&task.meta.verification),
        cell_plan = render_cell_plan(cell_plan),
        prompt = mission.prompt.trim()
    )
}

fn render_exit_contract() -> &'static str {
    r#"# Subagent Exit Contract

Before a subagent can be considered completed, it must leave:

- `handoff.md`: what changed, files touched, validation, risks, follow-ups.
- `resume_brief.md`: compact next-session briefing.
- `evidence.json`: file/command/test evidence.
- `usage.json`: token, time, cost, source, confidence.
- `next_action.md`: first concrete next step if resumed.

Without a handoff, OMC may only mark the session `abandoned` or `resumable`, never `completed`.
"#
}

fn render_cell_plan(plan: &CellPlan) -> String {
    let mut lines = vec![format!("- lead: {}", plan.lead_role)];
    for cell in &plan.cells {
        lines.push(format!(
            "- {}: {}",
            cell.cell_id,
            cell.agents
                .iter()
                .map(|agent| agent.role.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    lines.join("\n")
}

fn render_session_resume_line(session: &AgentSessionRecord) -> String {
    format!(
        "- `{}` role={} cell={} state={:?} epoch={} handoff={} resume={}",
        session.agent_id,
        session.role,
        session.cell_id.as_deref().unwrap_or("lead"),
        session.state,
        session.epoch,
        session.last_handoff.as_deref().unwrap_or("none"),
        session.last_resume_brief.as_deref().unwrap_or("none")
    )
}

fn render_whiteboard(root: &Path) -> String {
    [
        "facts.md",
        "decisions.md",
        "risks.md",
        "questions.md",
        "handoffs.md",
    ]
    .iter()
    .map(|file| read_optional(root.join(".omc/team/whiteboard").join(file)))
    .filter(|content| !content.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n\n")
}

fn read_optional(path: PathBuf) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn bullets(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MissionKind, TaskMetadata};

    #[test]
    fn cell_plan_groups_sixteen_agents_into_five_cells() {
        let plan = build_cell_plan(16);
        assert_eq!(plan.total_agents, 16);
        assert_eq!(plan.cells.len(), 5);
        assert!(plan.cells.iter().all(|cell| cell.agents.len() == 3));
        assert_eq!(plan.cells[0].agents[0].role, "builder");
        assert_eq!(plan.cells[0].agents[1].role, "reviewer");
        assert_eq!(plan.cells[0].agents[2].role, "verifier");
    }

    #[test]
    fn context_guard_thresholds_escalate() {
        assert_eq!(
            context_guard_decision(69.0, UsageSource::Estimated, UsageConfidence::Low).action,
            ContextGuardAction::Continue
        );
        assert_eq!(
            context_guard_decision(70.0, UsageSource::Estimated, UsageConfidence::Low).action,
            ContextGuardAction::Checkpoint
        );
        assert_eq!(
            context_guard_decision(85.0, UsageSource::Estimated, UsageConfidence::Low).action,
            ContextGuardAction::ResumeBrief
        );
        assert_eq!(
            context_guard_decision(92.0, UsageSource::Estimated, UsageConfidence::Low).action,
            ContextGuardAction::StopNewTask
        );
        assert_eq!(
            context_guard_decision(95.0, UsageSource::Estimated, UsageConfidence::Low).action,
            ContextGuardAction::ForcedHandoff
        );
    }

    #[test]
    fn completed_session_requires_handoff() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).unwrap();
        let session = AgentSessionRecord {
            record_type: SESSION_RECORD_TYPE.to_string(),
            agent_id: "agent-1".to_string(),
            run_id: "run-1".to_string(),
            cell_id: None,
            role: "lead".to_string(),
            runtime: RuntimeKind::Claude,
            provider: "claude-code".to_string(),
            current_task: "LOCAL-1".to_string(),
            state: AgentSessionState::HandoffReady,
            epoch: 0,
            last_resume_brief: None,
            last_handoff: None,
            usage_rollup: UsageRollup::default(),
            created_at: 1,
            updated_at: 1,
        };
        save_session(&root, &session).unwrap();
        let err =
            transition_session_state(&root, "agent-1", AgentSessionState::Completed).unwrap_err();
        assert!(err.contains("without a handoff"));

        let handoff = root.join("handoff.md");
        fs::write(&handoff, "done").unwrap();
        mark_session_handoff(&root, "agent-1", &handoff, None).unwrap();
        transition_session_state(&root, "agent-1", AgentSessionState::Completed).unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn launch_writes_sessions_invocations_and_briefing() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).unwrap();
        let task = task_card();
        let mission = Mission {
            id: task.meta.id.clone(),
            team_name: "local-1".to_string(),
            prompt: "mission body".to_string(),
        };
        let opts = StartOptions {
            team_size: 4,
            ..StartOptions::new(MissionKind::Implementation)
        };
        let mission_path = root.join(".omc/team/missions/local-1.md");
        fs::create_dir_all(mission_path.parent().unwrap()).unwrap();
        fs::write(&mission_path, "mission body").unwrap();

        let report =
            record_team_launch(&root, "run-1", &mission, &task, &opts, &mission_path, None)
                .unwrap();
        assert_eq!(report.session_count, 4);
        assert_eq!(load_sessions(&root).unwrap().len(), 4);
        assert_eq!(load_invocations(&root).unwrap().len(), 4);
        assert!(report.briefing_path.exists());
        let resume = render_resume_packet(&root, "run-1").unwrap();
        assert!(resume.contains("OMC Resume Packet"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn usage_report_preserves_source_and_confidence() {
        let root = unique_temp_dir();
        fs::create_dir_all(&root).unwrap();
        init_observability(&root).unwrap();
        let invocation = AgentInvocationRecord {
            record_type: INVOCATION_RECORD_TYPE.to_string(),
            invocation_id: "inv-1".to_string(),
            agent_id: "agent-1".to_string(),
            run_id: "run-1".to_string(),
            cell_id: Some("cell-1".to_string()),
            runtime: RuntimeKind::Claude,
            provider: "claude-code".to_string(),
            model: Some("claude".to_string()),
            started_at: 10,
            ended_at: Some(20),
            status: "completed".to_string(),
            exit_reason: None,
            usage: UsageMeasurement {
                input_tokens: 10,
                output_tokens: 5,
                cache_read_tokens: 2,
                cache_create_tokens: 1,
                cost_usd: Some(0.01),
                source: UsageSource::Transcript,
                confidence: UsageConfidence::Medium,
            },
            context_percent: Some(12.0),
            rate_limit: None,
            tool_calls: Vec::new(),
            mcp_calls: Vec::new(),
            skill_calls: Vec::new(),
            evidence: Vec::new(),
        };
        save_invocation(&root, &invocation).unwrap();
        let report = usage_report(&root, Some("run-1"), UsageGroupBy::Agent).unwrap();
        assert_eq!(report.groups[0].usage.input_tokens, 10);
        assert!(
            report.groups[0]
                .sources
                .contains(&"transcript/medium".to_string())
        );
        let _ = fs::remove_dir_all(root);
    }

    fn task_card() -> TaskCard {
        TaskCard {
            meta: TaskMetadata {
                id: "LOCAL-1".to_string(),
                title: "Session governance".to_string(),
                agent_ready: true,
                risk: "medium".to_string(),
                ownership: vec!["crates/omc-team".to_string()],
                acceptance: vec!["sessions are durable".to_string()],
                verification: vec!["cargo test -p omc-team".to_string()],
                source: None,
                linear_id: None,
                tracker: None,
                github_repo: None,
                github_issue_number: None,
            },
            body: "Implement session governance.".to_string(),
        }
    }

    fn unique_temp_dir() -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "omc-observability-test-{}",
            crate::unix_timestamp_nanos()
        ));
        dir
    }
}
