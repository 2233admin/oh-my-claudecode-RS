use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::types::InboxMessage;

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
/// Uses atomic write: appends via a temp file + rename.
pub fn write_inbox(
    base_dir: &Path,
    team_name: &str,
    worker_name: &str,
    message: &InboxMessage,
) -> Result<()> {
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
    let path = inbox_path(base_dir, team_name, worker_name);
    read_jsonl(&path)
}

/// Clear a worker's inbox after reading (truncate the file).
pub fn clear_inbox(base_dir: &Path, team_name: &str, worker_name: &str) -> Result<()> {
    let path = inbox_path(base_dir, team_name, worker_name);
    if path.exists() {
        fs::write(&path, "")
            .with_context(|| format!("failed to clear inbox: {}", path.display()))?;
    }
    Ok(())
}

/// Append a single JSON line to a file atomically.
/// Reads existing content, writes to a temp file, then renames.
fn append_jsonl(path: &Path, line: &str) -> Result<()> {
    let existing = if path.exists() {
        fs::read_to_string(path).unwrap_or_default()
    } else {
        String::new()
    };

    let tmp_path = path.with_extension("jsonl.tmp");
    let mut content = existing;
    content.push_str(line);
    content.push('\n');

    let mut tmp = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&tmp_path)
        .with_context(|| format!("failed to open tmp file: {}", tmp_path.display()))?;
    tmp.write_all(content.as_bytes())
        .context("failed to write tmp file")?;
    tmp.flush().context("failed to flush tmp file")?;
    drop(tmp);

    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to rename {} -> {}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

/// Read all JSON lines from a file, skipping blank lines.
fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read: {}", path.display()))?;
    let mut messages = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: T = serde_json::from_str(trimmed)
            .with_context(|| format!("failed to parse line {} in {}", i + 1, path.display()))?;
        messages.push(msg);
    }
    Ok(messages)
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
