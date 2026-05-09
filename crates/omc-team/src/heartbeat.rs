use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatData {
    pub worker_id: String,
    pub team_name: String,
    pub timestamp: String,
    pub status: String,
    pub current_task_id: Option<String>,
    pub pid: Option<u32>,
}

pub struct HeartbeatManager {
    base_dir: PathBuf,
    max_age_ms: u64,
}

impl HeartbeatManager {
    pub fn new(base_dir: PathBuf, max_age_ms: u64) -> Self {
        Self {
            base_dir,
            max_age_ms,
        }
    }

    fn heartbeat_dir(&self, team: &str) -> PathBuf {
        self.base_dir
            .join(".omc/state/team")
            .join(team)
            .join("heartbeat")
    }

    fn heartbeat_path(&self, team: &str, worker: &str) -> PathBuf {
        self.heartbeat_dir(team).join(format!("{worker}.json"))
    }

    pub fn write_heartbeat(&self, data: &HeartbeatData) -> anyhow::Result<()> {
        let dir = self.heartbeat_dir(&data.team_name);
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", data.worker_id));
        let json = serde_json::to_string_pretty(data)?;
        fs::write(&path, json)?;
        Ok(())
    }

    pub fn read_heartbeat(
        &self,
        team: &str,
        worker: &str,
    ) -> anyhow::Result<Option<HeartbeatData>> {
        let path = self.heartbeat_path(team, worker);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        let data: HeartbeatData = serde_json::from_str(&content)?;
        Ok(Some(data))
    }

    pub fn list_heartbeats(&self, team: &str) -> anyhow::Result<Vec<HeartbeatData>> {
        let dir = self.heartbeat_dir(team);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut results = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json")
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(data) = serde_json::from_str::<HeartbeatData>(&content)
            {
                results.push(data);
            }
        }
        Ok(results)
    }

    pub fn is_alive(&self, team: &str, worker: &str) -> anyhow::Result<bool> {
        let Some(data) = self.read_heartbeat(team, worker)? else {
            return Ok(false);
        };
        let hb_ms = parse_timestamp_ms(&data.timestamp).unwrap_or(0);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Ok(now_ms.saturating_sub(hb_ms) <= self.max_age_ms)
    }

    pub fn delete_heartbeat(&self, team: &str, worker: &str) -> anyhow::Result<()> {
        let path = self.heartbeat_path(team, worker);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn cleanup_team(&self, team: &str) -> anyhow::Result<()> {
        let dir = self.heartbeat_dir(team);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }
}

fn parse_timestamp_ms(ts: &str) -> Option<u64> {
    // Try RFC 3339
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
        return Some(dt.timestamp_millis() as u64);
    }
    // Try unix seconds
    if let Ok(secs) = ts.parse::<u64>() {
        return Some(secs * 1000);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_dir() -> PathBuf {
        let mut dir = env::temp_dir();
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.push(format!("omc-heartbeat-test-{ts}"));
        dir
    }

    fn make_data(team: &str, worker: &str) -> HeartbeatData {
        HeartbeatData {
            worker_id: worker.to_string(),
            team_name: team.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            status: "active".to_string(),
            current_task_id: None,
            pid: None,
        }
    }

    #[test]
    fn write_read_roundtrip() {
        let base = temp_dir();
        let mgr = HeartbeatManager::new(base.clone(), 60_000);
        let data = make_data("alpha", "w1");

        mgr.write_heartbeat(&data).unwrap();
        let read = mgr.read_heartbeat("alpha", "w1").unwrap().unwrap();

        assert_eq!(read.worker_id, "w1");
        assert_eq!(read.team_name, "alpha");
        assert_eq!(read.status, "active");

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn read_missing_returns_none() {
        let base = temp_dir();
        let mgr = HeartbeatManager::new(base.clone(), 60_000);

        assert!(mgr.read_heartbeat("alpha", "ghost").unwrap().is_none());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn is_alive_fresh_heartbeat() {
        let base = temp_dir();
        let mgr = HeartbeatManager::new(base.clone(), 60_000);
        let data = make_data("alpha", "w1");

        mgr.write_heartbeat(&data).unwrap();
        assert!(mgr.is_alive("alpha", "w1").unwrap());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn is_alive_stale_heartbeat() {
        let base = temp_dir();
        let mgr = HeartbeatManager::new(base.clone(), 1000);
        let mut data = make_data("alpha", "w1");
        // Set timestamp to 5 seconds ago
        data.timestamp = (chrono::Utc::now() - chrono::Duration::seconds(5)).to_rfc3339();

        mgr.write_heartbeat(&data).unwrap();
        assert!(!mgr.is_alive("alpha", "w1").unwrap());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn list_heartbeats_multiple_workers() {
        let base = temp_dir();
        let mgr = HeartbeatManager::new(base.clone(), 60_000);

        mgr.write_heartbeat(&make_data("alpha", "w1")).unwrap();
        mgr.write_heartbeat(&make_data("alpha", "w2")).unwrap();
        mgr.write_heartbeat(&make_data("beta", "w3")).unwrap();

        let alpha_hbs = mgr.list_heartbeats("alpha").unwrap();
        assert_eq!(alpha_hbs.len(), 2);

        let beta_hbs = mgr.list_heartbeats("beta").unwrap();
        assert_eq!(beta_hbs.len(), 1);

        let empty = mgr.list_heartbeats("gamma").unwrap();
        assert!(empty.is_empty());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn delete_and_cleanup() {
        let base = temp_dir();
        let mgr = HeartbeatManager::new(base.clone(), 60_000);

        mgr.write_heartbeat(&make_data("alpha", "w1")).unwrap();
        mgr.write_heartbeat(&make_data("alpha", "w2")).unwrap();

        mgr.delete_heartbeat("alpha", "w1").unwrap();
        assert!(mgr.read_heartbeat("alpha", "w1").unwrap().is_none());
        assert!(mgr.read_heartbeat("alpha", "w2").unwrap().is_some());

        mgr.cleanup_team("alpha").unwrap();
        assert!(mgr.list_heartbeats("alpha").unwrap().is_empty());

        let _ = fs::remove_dir_all(base);
    }
}
