pub mod inbox;
pub mod message_router;
pub mod outbox;
pub mod types;

pub use inbox::{clear_inbox, read_inbox, write_inbox};
pub use message_router::MessageRouter;
pub use outbox::{read_outbox, rotate_outbox, write_outbox};
pub use types::{DrainSignal, InboxMessage, OutboxMessage, ShutdownSignal};

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

/// Validate a team/worker name to prevent path traversal attacks.
pub(crate) fn validate_name(name: &str, kind: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("{kind} name must not be empty");
    }
    if name.contains("..") || name.contains('/') || name.contains('\\') || name.contains('\0') {
        anyhow::bail!("{kind} name contains invalid characters: {name:?}");
    }
    Ok(())
}

/// Append a single JSON line to a file. Safe for concurrent writers because
/// `writeln!` on a file opened with `O_APPEND` is atomic for writes under
/// PIPE_BUF (4096 bytes on Linux, 1024 on Windows).
pub(crate) fn append_jsonl(path: &Path, line: &str) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", line)?;
    file.flush()?;
    Ok(())
}

/// Read all JSON lines from a file, skipping blank lines.
pub(crate) fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read: {}", path.display()))?;
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
