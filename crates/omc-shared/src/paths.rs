//! Path-segment validation primitives shared across crates.
//!
//! Untrusted strings used to construct filesystem paths must pass
//! [`validate_path_segment`] before being joined to a parent directory.
//! Rejects empty input, path-traversal sequences (`..`), separator chars,
//! and NUL bytes. Mirrors `omc-team::communication::validate_name` so the
//! same defense applies to `omc-interop`, `omc-mcp`, and any crate that
//! cannot depend on `omc-team`.

use anyhow::{Result, bail};

/// Reject any string that would escape its intended parent directory or
/// otherwise be unsafe to embed in a filesystem path.
///
/// `kind` is a free-form label included in error messages
/// (e.g. `"task_id"`, `"session_id"`, `"team"`).
pub fn validate_path_segment(name: &str, kind: &str) -> Result<()> {
    if name.is_empty() {
        bail!("{kind} must not be empty");
    }
    if name.contains("..") || name.contains('/') || name.contains('\\') || name.contains('\0') {
        bail!("{kind} contains invalid characters: {name:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_typical_ids() {
        assert!(validate_path_segment("task-123", "task_id").is_ok());
        assert!(validate_path_segment("session_abc", "session_id").is_ok());
        assert!(validate_path_segment("uuid-a1b2c3", "id").is_ok());
    }

    #[test]
    fn rejects_empty() {
        assert!(validate_path_segment("", "task_id").is_err());
    }

    #[test]
    fn rejects_dot_dot() {
        assert!(validate_path_segment("..", "task_id").is_err());
        assert!(validate_path_segment("..foo", "task_id").is_err());
        assert!(validate_path_segment("foo..bar", "task_id").is_err());
    }

    #[test]
    fn rejects_separators() {
        assert!(validate_path_segment("a/b", "task_id").is_err());
        assert!(validate_path_segment("a\\b", "task_id").is_err());
        assert!(validate_path_segment("/etc/passwd", "task_id").is_err());
    }

    #[test]
    fn rejects_nul_byte() {
        assert!(validate_path_segment("a\0b", "task_id").is_err());
    }
}
