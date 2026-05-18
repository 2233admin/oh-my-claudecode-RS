use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct MissionTaskCounts {
    pub total: u32,
    pub completed: u32,
    #[serde(rename = "inProgress")]
    pub in_progress: u32,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct Mission {
    pub id: String,
    pub name: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "taskCounts")]
    pub task_counts: Option<MissionTaskCounts>,
    #[serde(rename = "workerCount")]
    pub worker_count: Option<u32>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct MissionBoardState {
    pub missions: Vec<Mission>,
}

pub fn load(cwd: Option<&str>) -> Option<MissionBoardState> {
    let base = cwd.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let path = base.join(".omc").join("state").join("mission-state.json");
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str::<MissionBoardState>(&raw).ok()
}
