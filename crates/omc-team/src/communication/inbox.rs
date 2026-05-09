use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::types::InboxMessage;
use super::{append_jsonl, read_jsonl, validate_name};

/// Build the inbox file path: `{base_dir}/.omc/state/team/{team}/inbox/{worker}.jsonl`
fn inbox_path(base_dir: &Path, team_name: &str, worker_name: &str) -> PathBuf {
    base_dir
        .join(".omc")
        .join("state")
        .join("team")
        .join(team_name)
        .join("inbox")
        .join(format!("{worker_name}.jsonl"))
}

/// Write a message to a worker's inbox (lead -> worker).
/// Uses atomic append via O_APPEND.
pub fn write_inbox(
    base_dir: &Path,
    team_name: &str,
    worker_name: &str,
    message: &InboxMessage,
) -> Result<()> {
    validate_name(team_name, "team")?;
    validate_name(worker_name, "worker")?;
    let path = inbox_path(base_dir, team_name, worker_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create inbox dir: {}", parent.display()))?;
    }
    let line = serde_json::to_string(message).context("failed to serialize inbox message")?;
    append_jsonl(&path, &line)
}

/// Read all messages from a worker's inbox.
pub fn read_inbox(
    base_dir: &Path,
    team_name: &str,
    worker_name: &str,
) -> Result<Vec<InboxMessage>> {
    validate_name(team_name, "team")?;
    validate_name(worker_name, "worker")?;
    let path = inbox_path(base_dir, team_name, worker_name);
    read_jsonl(&path)
}

/// Clear a worker's inbox after reading (truncate the file).
pub fn clear_inbox(base_dir: &Path, team_name: &str, worker_name: &str) -> Result<()> {
    validate_name(team_name, "team")?;
    validate_name(worker_name, "worker")?;
    let path = inbox_path(base_dir, team_name, worker_name);
    if path.exists() {
        fs::write(&path, "")
            .with_context(|| format!("failed to clear inbox: {}", path.display()))?;
    }
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
    fn write_and_read_inbox_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let msg1 = InboxMessage::Message {
            content: "hello".into(),
            timestamp: ts(),
        };
        let msg2 = InboxMessage::Context {
            content: "ctx data".into(),
            timestamp: ts(),
        };

        write_inbox(tmp.path(), "team-a", "worker-1", &msg1).unwrap();
        write_inbox(tmp.path(), "team-a", "worker-1", &msg2).unwrap();

        let messages = read_inbox(tmp.path(), "team-a", "worker-1").unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0], msg1);
        assert_eq!(messages[1], msg2);
    }

    #[test]
    fn read_empty_inbox() {
        let tmp = TempDir::new().unwrap();
        let messages = read_inbox(tmp.path(), "team-a", "nonexistent").unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn clear_inbox_removes_messages() {
        let tmp = TempDir::new().unwrap();
        let msg = InboxMessage::Message {
            content: "test".into(),
            timestamp: ts(),
        };
        write_inbox(tmp.path(), "team-a", "worker-1", &msg).unwrap();

        let before = read_inbox(tmp.path(), "team-a", "worker-1").unwrap();
        assert_eq!(before.len(), 1);

        clear_inbox(tmp.path(), "team-a", "worker-1").unwrap();

        let after = read_inbox(tmp.path(), "team-a", "worker-1").unwrap();
        assert!(after.is_empty());
    }

    #[test]
    fn clear_nonexistent_inbox_is_noop() {
        let tmp = TempDir::new().unwrap();
        clear_inbox(tmp.path(), "team-a", "ghost").unwrap();
    }

    #[test]
    fn separate_workers_have_separate_inboxes() {
        let tmp = TempDir::new().unwrap();
        let msg_a = InboxMessage::Message {
            content: "for w1".into(),
            timestamp: ts(),
        };
        let msg_b = InboxMessage::Message {
            content: "for w2".into(),
            timestamp: ts(),
        };

        write_inbox(tmp.path(), "team-a", "worker-1", &msg_a).unwrap();
        write_inbox(tmp.path(), "team-a", "worker-2", &msg_b).unwrap();

        let inbox_1 = read_inbox(tmp.path(), "team-a", "worker-1").unwrap();
        let inbox_2 = read_inbox(tmp.path(), "team-a", "worker-2").unwrap();
        assert_eq!(inbox_1.len(), 1);
        assert_eq!(inbox_2.len(), 1);
        assert_eq!(inbox_1[0], msg_a);
        assert_eq!(inbox_2[0], msg_b);
    }
}
