//! Hash-anchored, atomic source edits.
//!
//! The edit is intentionally narrow: callers name one contiguous line range
//! and provide the hash for every current line. Any stale anchor aborts before
//! the file is written.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

pub const HASH_EDIT_SCHEMA_VERSION: &str = "omc.hash-edit.v1";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LineAnchor {
    pub line: usize,
    pub sha256: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HashEdit {
    pub schema_version: String,
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub anchors: Vec<LineAnchor>,
    pub replacement: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_file_sha256: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HashEditResult {
    pub schema_version: String,
    pub path: String,
    pub applied: bool,
    pub file_sha256_before: String,
    pub file_sha256_after: String,
}

#[derive(Debug, Error)]
pub enum HashEditError {
    #[error("invalid hash edit: {0}")]
    Invalid(String),
    #[error("hash edit target is outside the project root: {0}")]
    OutsideRoot(String),
    #[error("hash edit target does not exist: {0}")]
    MissingTarget(String),
    #[error("stale line anchor at line {line}: expected {expected}, found {actual}")]
    StaleAnchor {
        line: usize,
        expected: String,
        actual: String,
    },
    #[error("stale file digest: expected {expected}, found {actual}")]
    StaleFile { expected: String, actual: String },
    #[error("failed to read hash edit target: {0}")]
    Read(#[from] std::io::Error),
}

impl HashEdit {
    pub fn new(
        path: impl Into<String>,
        start_line: usize,
        end_line: usize,
        anchors: Vec<LineAnchor>,
        replacement: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: HASH_EDIT_SCHEMA_VERSION.into(),
            path: path.into(),
            start_line,
            end_line,
            anchors,
            replacement: replacement.into(),
            expected_file_sha256: None,
        }
    }

    pub fn apply(&self, root: &Path) -> Result<HashEditResult, HashEditError> {
        self.validate()?;
        let target = resolve_target(root, &self.path)?;
        let original = fs::read_to_string(&target)?;
        let before = digest(original.as_bytes());
        if let Some(expected) = &self.expected_file_sha256 {
            validate_digest(expected, "expected_file_sha256")?;
            if expected != &before {
                return Err(HashEditError::StaleFile {
                    expected: expected.clone(),
                    actual: before,
                });
            }
        }

        let newline = if original.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let lines: Vec<&str> = original.lines().collect();
        for anchor in &self.anchors {
            let actual_line = lines.get(anchor.line - 1).ok_or_else(|| {
                HashEditError::Invalid(format!("anchor line {} is out of range", anchor.line))
            })?;
            let actual = digest(actual_line.as_bytes());
            if actual != anchor.sha256 {
                return Err(HashEditError::StaleAnchor {
                    line: anchor.line,
                    expected: anchor.sha256.clone(),
                    actual,
                });
            }
        }

        let replacement_lines: Vec<&str> = if self.replacement.is_empty() {
            Vec::new()
        } else {
            self.replacement.split('\n').collect()
        };
        let mut output_lines: Vec<&str> = Vec::new();
        output_lines.extend_from_slice(&lines[..self.start_line - 1]);
        output_lines.extend(replacement_lines);
        output_lines.extend_from_slice(&lines[self.end_line..]);
        let mut updated = output_lines.join(newline);
        if original.ends_with(newline) && !updated.ends_with(newline) {
            updated.push_str(newline);
        }

        let tmp = target.with_extension("omc-hash-edit.tmp");
        fs::write(&tmp, updated.as_bytes())?;
        if let Err(error) = fs::rename(&tmp, &target) {
            let _ = fs::remove_file(&tmp);
            return Err(error.into());
        }
        Ok(HashEditResult {
            schema_version: HASH_EDIT_SCHEMA_VERSION.into(),
            path: self.path.clone(),
            applied: true,
            file_sha256_before: before,
            file_sha256_after: digest(updated.as_bytes()),
        })
    }

    fn validate(&self) -> Result<(), HashEditError> {
        if self.schema_version != HASH_EDIT_SCHEMA_VERSION {
            return Err(HashEditError::Invalid("unsupported schema version".into()));
        }
        if self.path.is_empty()
            || self.path.starts_with('/')
            || self.path.starts_with('\\')
            || self.path.contains('\\')
            || self
                .path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(HashEditError::Invalid(
                "path must be a portable relative path".into(),
            ));
        }
        if self.start_line == 0 || self.end_line < self.start_line {
            return Err(HashEditError::Invalid(
                "line range must be 1-based and contiguous".into(),
            ));
        }
        if self.anchors.len() != self.end_line - self.start_line + 1
            || self.anchors.iter().enumerate().any(|(offset, anchor)| {
                anchor.line != self.start_line + offset || !is_digest(&anchor.sha256)
            })
        {
            return Err(HashEditError::Invalid(
                "anchors must cover the full contiguous line range".into(),
            ));
        }
        if let Some(expected) = &self.expected_file_sha256 {
            validate_digest(expected, "expected_file_sha256")?;
        }
        Ok(())
    }
}

fn resolve_target(root: &Path, path: &str) -> Result<PathBuf, HashEditError> {
    let root = root.canonicalize()?;
    let target = root.join(path);
    if !target.exists() {
        return Err(HashEditError::MissingTarget(path.into()));
    }
    let target = target.canonicalize()?;
    if !target.starts_with(&root) {
        return Err(HashEditError::OutsideRoot(path.into()));
    }
    Ok(target)
}

fn is_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_digest(value: &str, field: &str) -> Result<(), HashEditError> {
    if is_digest(value) {
        Ok(())
    } else {
        Err(HashEditError::Invalid(format!(
            "{field} must be a SHA-256 digest"
        )))
    }
}

pub fn line_sha256(line: &str) -> String {
    digest(line.as_bytes())
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_matching_anchors_and_preserves_newline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("src.rs");
        fs::write(&path, "one\r\ntwo\r\nthree\r\n").unwrap();
        let edit = HashEdit::new(
            "src.rs",
            2,
            2,
            vec![LineAnchor {
                line: 2,
                sha256: line_sha256("two"),
            }],
            "changed",
        );
        let result = edit.apply(dir.path()).unwrap();
        assert!(result.applied);
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "one\r\nchanged\r\nthree\r\n"
        );
    }

    #[test]
    fn rejects_stale_anchor_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("src.rs");
        fs::write(&path, "one\ntwo\n").unwrap();
        let edit = HashEdit::new(
            "src.rs",
            1,
            1,
            vec![LineAnchor {
                line: 1,
                sha256: line_sha256("old"),
            }],
            "changed",
        );
        assert!(matches!(
            edit.apply(dir.path()),
            Err(HashEditError::StaleAnchor { .. })
        ));
        assert_eq!(fs::read_to_string(path).unwrap(), "one\ntwo\n");
    }

    #[test]
    fn rejects_traversal() {
        let edit = HashEdit::new("../secret", 1, 1, vec![], "x");
        assert!(edit.validate().is_err());
    }

    #[test]
    fn empty_replacement_removes_lines_without_leading_newline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("src.rs");
        fs::write(&path, "one\ntwo\nthree\n").unwrap();
        let edit = HashEdit::new(
            "src.rs",
            1,
            1,
            vec![LineAnchor {
                line: 1,
                sha256: line_sha256("one"),
            }],
            "",
        );
        edit.apply(dir.path()).unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "two\nthree\n");
    }
}
