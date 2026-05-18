use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const MAX_STATE_AGE: Duration = Duration::from_secs(2 * 3600);

#[derive(Debug, Default, Deserialize, Clone)]
pub struct RalphState {
    pub active: bool,
    pub iteration: u32,
    #[serde(rename = "maxIterations")]
    pub max_iterations: u32,
    #[serde(rename = "prdMode")]
    pub prd_mode: Option<bool>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct UltraworkState {
    pub active: bool,
    #[serde(rename = "reinforcementCount")]
    pub reinforcement_count: u32,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct AutopilotState {
    pub active: bool,
    pub phase: Option<String>,
    pub iteration: Option<u32>,
    #[serde(rename = "maxIterations")]
    pub max_iterations: Option<u32>,
    #[serde(rename = "tasksCompleted")]
    pub tasks_completed: Option<u32>,
    #[serde(rename = "tasksTotal")]
    pub tasks_total: Option<u32>,
    #[serde(rename = "filesCreated")]
    pub files_created: Option<u32>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct BackgroundTask {
    pub id: String,
    pub description: Option<String>,
    #[serde(rename = "agentType")]
    pub agent_type: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct HudFileState {
    #[serde(rename = "backgroundTasks", default)]
    pub background_tasks: Vec<BackgroundTask>,
    #[serde(rename = "sessionStartTimestamp")]
    pub session_start_timestamp: Option<String>,
    #[serde(rename = "lastPromptTimestamp")]
    pub last_prompt_timestamp: Option<String>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct PrdState {
    #[serde(rename = "currentStoryId")]
    pub current_story_id: Option<String>,
    pub completed: u32,
    pub total: u32,
}

#[derive(Debug, Default)]
pub struct OmcState {
    pub ralph: Option<RalphState>,
    pub ultrawork: Option<UltraworkState>,
    pub autopilot: Option<AutopilotState>,
    pub hud: Option<HudFileState>,
    pub prd: Option<PrdState>,
}

impl OmcState {
    pub fn load(cwd: Option<&str>, session_id: Option<&str>) -> Self {
        let base = cwd.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));

        let ralph = load_state_with_fallbacks::<RalphState>(&base, session_id, "ralph-state");
        let ultrawork =
            load_state_with_fallbacks::<UltraworkState>(&base, session_id, "ultrawork-state");
        let autopilot =
            load_state_with_fallbacks::<AutopilotState>(&base, session_id, "autopilot-state");
        let hud = load_state_with_fallbacks::<HudFileState>(&base, session_id, "hud-state");

        // PRD: check {cwd}/prd.json, then {cwd}/.omc/prd.json
        let prd = read_state_file::<PrdState>(&base.join("prd.json"))
            .or_else(|| read_state_file::<PrdState>(&base.join(".omc").join("prd.json")));

        OmcState {
            ralph,
            ultrawork,
            autopilot,
            hud,
            prd,
        }
    }
}

/// Try session-scoped path, then global, then legacy for a given state name.
fn load_state_with_fallbacks<T>(
    base: &Path,
    session_id: Option<&str>,
    name: &str,
) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    let filename = format!("{name}.json");

    // Session-scoped: {base}/.omc/state/sessions/{session_id}/{name}.json
    if let Some(sid) = session_id {
        let path = base
            .join(".omc")
            .join("state")
            .join("sessions")
            .join(sid)
            .join(&filename);
        if let Some(v) = read_state_file::<T>(&path) {
            return Some(v);
        }
    }

    // Global: {base}/.omc/state/{name}.json
    let global = base.join(".omc").join("state").join(&filename);
    if let Some(v) = read_state_file::<T>(&global) {
        return Some(v);
    }

    // Legacy: {base}/.omc/{name}.json
    let legacy = base.join(".omc").join(&filename);
    read_state_file::<T>(&legacy)
}

fn read_state_file<T>(path: &Path) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    let metadata = std::fs::metadata(path).ok()?;

    // Stale check: if mtime > MAX_STATE_AGE ago, treat as absent
    let mtime = metadata.modified().ok()?;
    let age = SystemTime::now().duration_since(mtime).unwrap_or(Duration::ZERO);
    if age > MAX_STATE_AGE {
        return None;
    }

    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<T>(&raw).ok()
}
