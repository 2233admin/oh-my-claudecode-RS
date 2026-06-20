use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::types::OutboxMessage;
use super::{append_jsonl, read_jsonl, validate_name};

/// Build the outbox file path: `{base_dir}/.omc/state/team/{team}/outbox/{worker}.jsonl`
fn outbox_path(base_dir: &Path, team_name: &str, worker_name: &str) -> PathBuf {
    base_dir
        .join(".omc")
        .join("state")
        .join("team")
        .join(team_name)
        .join("outbox")
        .join(format!("{worker_name}.jsonl"))
}

/// Write a message to a worker's outbox (worker -> lead).
/// Uses atomic append via O_APPEND.
pub fn write_outbox(
    base_dir: &Path,
    team_name: &str,
    worker_name: &str,
    message: &OutboxMessage,
) -> Result<()> {
    validate_name(team_name, "team")?;
    validate_name(worker_name, "worker")?;
    let path = outbox_path(base_dir, team_name, worker_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create outbox dir: {}", parent.display()))?;
    }
    let line = serde_json::to_string(message).context("failed to serialize outbox message")?;
    append_jsonl(&path, &line)
}

/// Read all messages from a worker's outbox.
pub fn read_outbox(
    base_dir: &Path,
    team_name: &str,
    worker_name: &str,
) -> Result<Vec<OutboxMessage>> {
    validate_name(team_name, "team")?;
    validate_name(worker_name, "worker")?;
    let path = outbox_path(base_dir, team_name, worker_name);
    read_jsonl(&path)
}

/// Rotate the outbox file when it exceeds `max_lines`.
/// Keeps only the most recent `max_lines` messages.
pub fn rotate_outbox(
    base_dir: &Path,
    team_name: &str,
    worker_name: &str,
    max_lines: usize,
) -> Result<()> {
    validate_name(team_name, "team")?;
    validate_name(worker_name, "worker")?;
    let path = outbox_path(base_dir, team_name, worker_name);
    if !path.exists() {
        return Ok(());
    }

    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read: {}", path.display()))?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.len() <= max_lines {
        return Ok(());
    }

    let keep: Vec<&str> = lines[lines.len() - max_lines..].to_vec();
    let tmp_path = path.with_extension("jsonl.tmp");
    {
        let mut tmp = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp_path)
            .with_context(|| format!("failed to open tmp: {}", tmp_path.display()))?;
        for line in &keep {
            tmp.write_all(line.as_bytes()).context("write line")?;
            tmp.write_all(b"\n").context("write newline")?;
        }
        tmp.flush().context("flush")?;
    }
    fs::rename(&tmp_path, &path)
        .with_context(|| format!("rename {} -> {}", tmp_path.display(), path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ts() -> String {
        "2026-05-10T00:00:00Z".into()
    }

    #[test]
    fn write_and_read_outbox_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let msg1 = OutboxMessage::Ready { timestamp: ts() };
        let msg2 = OutboxMessage::TaskComplete {
            task_id: "t1".into(),
            summary: "done".into(),
            timestamp: ts(),
        };

        write_outbox(tmp.path(), "team-a", "worker-1", &msg1).unwrap();
        write_outbox(tmp.path(), "team-a", "worker-1", &msg2).unwrap();

        let messages = read_outbox(tmp.path(), "team-a", "worker-1").unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0], msg1);
        assert_eq!(messages[1], msg2);
    }

    #[test]
    fn read_empty_outbox() {
        let tmp = TempDir::new().unwrap();
        let messages = read_outbox(tmp.path(), "team-a", "ghost").unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn rotate_outbox_keeps_recent() {
        let tmp = TempDir::new().unwrap();
        for i in 0..5 {
            let msg = OutboxMessage::Heartbeat {
                timestamp: format!("2026-05-10T00:00:0{i}Z"),
            };
            write_outbox(tmp.path(), "team-a", "w1", &msg).unwrap();
        }

        let before = read_outbox(tmp.path(), "team-a", "w1").unwrap();
        assert_eq!(before.len(), 5);

        rotate_outbox(tmp.path(), "team-a", "w1", 3).unwrap();

        let after = read_outbox(tmp.path(), "team-a", "w1").unwrap();
        assert_eq!(after.len(), 3);
        // kept the last 3
        assert_eq!(after[0], before[2]);
        assert_eq!(after[1], before[3]);
        assert_eq!(after[2], before[4]);
    }

    #[test]
    fn rotate_outbox_noop_when_under_limit() {
        let tmp = TempDir::new().unwrap();
        let msg = OutboxMessage::Idle { timestamp: ts() };
        write_outbox(tmp.path(), "team-a", "w1", &msg).unwrap();

        rotate_outbox(tmp.path(), "team-a", "w1", 10).unwrap();

        let messages = read_outbox(tmp.path(), "team-a", "w1").unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn rotate_nonexistent_outbox_is_noop() {
        let tmp = TempDir::new().unwrap();
        rotate_outbox(tmp.path(), "team-a", "ghost", 5).unwrap();
    }
}
