use std::path::{Path, PathBuf};

use anyhow::Result;

use super::inbox::write_inbox;
use super::outbox::{read_outbox, write_outbox};
use super::types::{DrainSignal, InboxMessage, OutboxMessage, ShutdownSignal};

/// Routes messages between a lead agent and its worker agents via
/// file-based inbox/outbox channels.
pub struct MessageRouter {
    base_dir: PathBuf,
    team_name: String,
}

impl MessageRouter {
    pub fn new(base_dir: PathBuf, team_name: String) -> Self {
        Self {
            base_dir,
            team_name,
        }
    }

    /// Send a message from the lead to a specific worker's inbox.
    pub fn send_to_worker(&self, worker: &str, msg: InboxMessage) -> Result<()> {
        write_inbox(&self.base_dir, &self.team_name, worker, &msg)
    }

    /// Send a message from a worker to the lead's outbox (per-worker channel).
    pub fn send_to_lead(&self, worker: &str, msg: OutboxMessage) -> Result<()> {
        write_outbox(&self.base_dir, &self.team_name, worker, &msg)
    }

    /// Broadcast a message to all workers' inboxes.
    pub fn broadcast(&self, workers: &[String], msg: InboxMessage) -> Result<()> {
        for worker in workers {
            write_inbox(&self.base_dir, &self.team_name, worker, &msg)?;
        }
        Ok(())
    }

    /// Read all messages from a specific worker's outbox.
    pub fn read_worker_outbox(&self, worker: &str) -> Result<Vec<OutboxMessage>> {
        read_outbox(&self.base_dir, &self.team_name, worker)
    }

    /// Read all outbox messages from all workers, returning (worker_name, message) pairs.
    pub fn read_lead_inbox(&self) -> Result<Vec<(String, OutboxMessage)>> {
        let inbox_dir = self
            .base_dir
            .join(".omc")
            .join("state")
            .join("team")
            .join(&self.team_name)
            .join("outbox");

        if !inbox_dir.exists() {
            return Ok(Vec::new());
        }

        let mut all_messages = Vec::new();
        let entries = std::fs::read_dir(&inbox_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "jsonl") {
                let worker_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let messages: Vec<OutboxMessage> =
                    read_outbox(&self.base_dir, &self.team_name, &worker_name)?;
                for msg in messages {
                    all_messages.push((worker_name.clone(), msg));
                }
            }
        }
        Ok(all_messages)
    }

    /// Write a shutdown signal file for a worker.
    pub fn write_shutdown_signal(&self, worker: &str, signal: &ShutdownSignal) -> Result<()> {
        let path = signal_path(&self.base_dir, &self.team_name, worker, "shutdown");
        atomic_write_json(&path, signal)
    }

    /// Write a drain signal file for a worker.
    pub fn write_drain_signal(&self, worker: &str, signal: &DrainSignal) -> Result<()> {
        let path = signal_path(&self.base_dir, &self.team_name, worker, "drain");
        atomic_write_json(&path, signal)
    }

    /// Check if a shutdown signal exists for a worker. Returns it and deletes the file.
    pub fn check_shutdown_signal(&self, worker: &str) -> Result<Option<ShutdownSignal>> {
        let path = signal_path(&self.base_dir, &self.team_name, worker, "shutdown");
        read_and_consume_signal(&path)
    }
}

/// Build path for signal files: `{base_dir}/.omc/state/team/{team}/signals/{worker}_{kind}.json`
fn signal_path(base_dir: &Path, team_name: &str, worker: &str, kind: &str) -> PathBuf {
    base_dir
        .join(".omc")
        .join("state")
        .join("team")
        .join(team_name)
        .join("signals")
        .join(format!("{worker}_{kind}.json"))
}

/// Write a serializable value as JSON to a file atomically.
fn atomic_write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Read a signal file and delete it (consume-once semantics).
fn read_and_consume_signal<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)?;
    let signal: T = serde_json::from_str(&content)?;
    let _ = std::fs::remove_file(path);
    Ok(Some(signal))
}

#[cfg(test)]
mod tests {
    use super::super::inbox::read_inbox;
    use super::*;
    use tempfile::TempDir;

    fn ts() -> String {
        "2026-05-10T00:00:00Z".into()
    }

    #[test]
    fn send_and_read_worker_outbox() {
        let tmp = TempDir::new().unwrap();
        let router = MessageRouter::new(tmp.path().to_path_buf(), "team-a".into());

        let msg = OutboxMessage::Ready { timestamp: ts() };
        router.send_to_lead("w1", msg.clone()).unwrap();

        let messages = router.read_worker_outbox("w1").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0], msg);
    }

    #[test]
    fn broadcast_to_multiple_workers() {
        let tmp = TempDir::new().unwrap();
        let router = MessageRouter::new(tmp.path().to_path_buf(), "team-a".into());

        let msg = InboxMessage::Context {
            content: "shared context".into(),
            timestamp: ts(),
        };
        let workers = vec!["w1".into(), "w2".into(), "w3".into()];
        router.broadcast(&workers, msg.clone()).unwrap();

        for w in &workers {
            let inbox = read_inbox(tmp.path(), "team-a", w).unwrap();
            assert_eq!(inbox.len(), 1);
            assert_eq!(inbox[0], msg);
        }
    }

    #[test]
    fn read_lead_inbox_aggregates_all_workers() {
        let tmp = TempDir::new().unwrap();
        let router = MessageRouter::new(tmp.path().to_path_buf(), "team-a".into());

        let msg1 = OutboxMessage::Heartbeat { timestamp: ts() };
        let msg2 = OutboxMessage::Idle { timestamp: ts() };

        router.send_to_lead("w1", msg1.clone()).unwrap();
        router.send_to_lead("w2", msg2.clone()).unwrap();

        let all = router.read_lead_inbox().unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|(w, m)| w == "w1" && *m == msg1));
        assert!(all.iter().any(|(w, m)| w == "w2" && *m == msg2));
    }

    #[test]
    fn read_lead_inbox_empty() {
        let tmp = TempDir::new().unwrap();
        let router = MessageRouter::new(tmp.path().to_path_buf(), "team-a".into());
        let all = router.read_lead_inbox().unwrap();
        assert!(all.is_empty());
    }

    #[test]
    fn shutdown_signal_write_and_check() {
        let tmp = TempDir::new().unwrap();
        let router = MessageRouter::new(tmp.path().to_path_buf(), "team-a".into());

        let signal = ShutdownSignal {
            request_id: "r1".into(),
            reason: "timeout".into(),
            timestamp: ts(),
        };
        router.write_shutdown_signal("w1", &signal).unwrap();

        let checked = router.check_shutdown_signal("w1").unwrap();
        assert_eq!(checked, Some(signal));

        // Second check should return None (consumed)
        let again = router.check_shutdown_signal("w1").unwrap();
        assert_eq!(again, None);
    }

    #[test]
    fn drain_signal_write_and_read() {
        let tmp = TempDir::new().unwrap();
        let router = MessageRouter::new(tmp.path().to_path_buf(), "team-a".into());

        let signal = DrainSignal {
            request_id: "d1".into(),
            reason: "scaling down".into(),
            timestamp: ts(),
        };
        router.write_drain_signal("w1", &signal).unwrap();

        let path = signal_path(tmp.path(), "team-a", "w1", "drain");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        let read_back: DrainSignal = serde_json::from_str(&content).unwrap();
        assert_eq!(read_back, signal);
    }

    #[test]
    fn check_missing_shutdown_returns_none() {
        let tmp = TempDir::new().unwrap();
        let router = MessageRouter::new(tmp.path().to_path_buf(), "team-a".into());
        let result = router.check_shutdown_signal("ghost").unwrap();
        assert_eq!(result, None);
    }
}
