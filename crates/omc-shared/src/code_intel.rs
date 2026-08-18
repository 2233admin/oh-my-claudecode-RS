//! Thin, read-only adapter for the released `code-intel` CLI.
//!
//! OMC consumes Code Intel's committed artifact query surface. It does not
//! reimplement scanners, indexes, graph providers, or artifact validation.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent_tool::{ToolError, error_codes};
use crate::operation_contract::ArtifactRef;

const DEFAULT_LIMIT: u8 = 20;
const MAX_LIMIT: u8 = 100;
const MAX_QUERY_TEXT: usize = 512;
const MAX_STDOUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 2 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodeIntelQueryRequest {
    pub repo: String,
    #[serde(default)]
    pub artifact_root: Option<String>,
    #[serde(default)]
    pub repo_path: Option<String>,
    #[serde(default)]
    pub artifact_schema: Option<String>,
    #[serde(default)]
    pub artifact_type: Option<String>,
    #[serde(default)]
    pub contains: Option<String>,
    #[serde(default)]
    pub artifact_uri: Option<String>,
    #[serde(default)]
    pub limit: Option<u8>,
    #[serde(default)]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodeIntelQueryPayload {
    pub schema_version: String,
    pub source: String,
    pub operation: String,
    pub read_only: bool,
    pub repository: String,
    pub result: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<ArtifactRef>,
}

pub fn query_code_intel(
    request: &CodeIntelQueryRequest,
) -> Result<CodeIntelQueryPayload, ToolError> {
    validate_request(request)?;
    let env_artifact_root = env::var("CODE_INTEL_ARTIFACT_ROOT").ok();
    let configured_artifact_root = request
        .artifact_root
        .as_deref()
        .or(env_artifact_root.as_deref());
    let artifact_root = resolve_directory(configured_artifact_root, "artifactRoot")?;
    if let Some(repo_path) = request.repo_path.as_deref() {
        resolve_directory(Some(repo_path), "repoPath")?;
    }

    let binary = env::var("OMC_CODE_INTEL_BIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "code-intel".into());
    let query_limit = effective_limit(request);
    let mut command = Command::new(binary);
    command
        .args(["artifact", "query", "--artifact-root"])
        .arg(artifact_root)
        .args(["--repo", &request.repo]);
    if let Some(repo_path) = request.repo_path.as_deref() {
        command.args(["--repo-path", repo_path]);
    }
    if let Some(schema) = request.artifact_schema.as_deref() {
        command.args(["--artifact-schema", schema]);
    }
    if let Some(artifact_type) = request.artifact_type.as_deref() {
        command.args(["--type", artifact_type]);
    }
    if let Some(contains) = request.contains.as_deref() {
        command.args(["--contains", contains]);
    }
    command.args(["--limit", &query_limit.to_string()]);

    let output = command.output().map_err(|error| {
        ToolError::new(
            error_codes::ADAPTER_UNAVAILABLE,
            format!("could not start code-intel read-only adapter: {error}"),
        )
    })?;
    if output.stdout.len() > MAX_STDOUT_BYTES {
        return Err(ToolError::new(
            error_codes::UPSTREAM_RESPONSE_TOO_LARGE,
            "code-intel query response exceeded the OMC adapter limit",
        ));
    }
    if !output.status.success() {
        return Err(ToolError::new(
            error_codes::UPSTREAM_FAILED,
            format!(
                "code-intel artifact query failed (exit {}): {}",
                output
                    .status
                    .code()
                    .map_or_else(|| "unknown".into(), |code| code.to_string()),
                bounded_text(&output.stderr, MAX_STDERR_BYTES)
            ),
        ));
    }
    let mut result: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        ToolError::new(
            error_codes::UPSTREAM_CONTRACT_INVALID,
            format!("code-intel artifact query returned invalid JSON: {error}"),
        )
    })?;
    if let Some(uri) = request.artifact_uri.as_deref() {
        select_artifact_matches(&mut result, uri, query_limit)?;
    }
    let artifact_refs = normalize_artifact_refs(&result)?;

    Ok(CodeIntelQueryPayload {
        schema_version: "omc.code-intel.query.v1".into(),
        source: "code-intel-pipeline".into(),
        operation: if request.artifact_uri.is_some() {
            "artifact.inspect".into()
        } else {
            "artifact.query".into()
        },
        read_only: true,
        repository: request.repo.clone(),
        result,
        artifact_refs,
    })
}

fn validate_request(request: &CodeIntelQueryRequest) -> Result<(), ToolError> {
    if request.repo.trim().is_empty()
        || request.repo.contains('/')
        || request.repo.contains('\\')
        || request.repo.contains(':')
        || request.repo.chars().any(char::is_whitespace)
    {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            "repo must be a non-empty repository key without path separators",
        ));
    }
    if let Some(limit) = request.limit
        && !(1..=MAX_LIMIT).contains(&limit)
    {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            "limit must be between 1 and 100",
        ));
    }
    for (name, value) in [
        ("artifactSchema", request.artifact_schema.as_deref()),
        ("artifactType", request.artifact_type.as_deref()),
        ("contains", request.contains.as_deref()),
    ] {
        if value.is_some_and(|item| item.len() > MAX_QUERY_TEXT) {
            return Err(ToolError::new(
                error_codes::INVALID_REQUEST,
                format!("{name} exceeds the {MAX_QUERY_TEXT}-character limit"),
            ));
        }
    }
    if let Some(uri) = request.artifact_uri.as_deref() {
        ArtifactRef::digest_from_uri(uri)
            .map_err(|error| ToolError::new(error_codes::INVALID_REQUEST, error))?;
    }
    Ok(())
}

fn effective_limit(request: &CodeIntelQueryRequest) -> u8 {
    if request.artifact_uri.is_some() {
        MAX_LIMIT
    } else {
        request.limit.unwrap_or(DEFAULT_LIMIT)
    }
}

fn select_artifact_matches(
    result: &mut Value,
    uri: &str,
    requested_limit: u8,
) -> Result<(), ToolError> {
    let digest = ArtifactRef::digest_from_uri(uri)
        .map_err(|error| ToolError::new(error_codes::INVALID_REQUEST, error))?;
    let matches = result
        .get_mut("matches")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            ToolError::new(
                error_codes::ARTIFACT_NOT_FOUND,
                format!("no committed artifact matches URI {uri}"),
            )
        })?;
    let original_len = matches.len();
    let selected: Vec<Value> = matches
        .iter()
        .filter(|item| {
            item.get("artifactRef")
                .and_then(|reference| reference.get("sha256"))
                .and_then(Value::as_str)
                == Some(digest.as_str())
        })
        .cloned()
        .collect();
    if selected.is_empty() {
        let code = if original_len >= usize::from(requested_limit) {
            error_codes::UPSTREAM_QUERY_TRUNCATED
        } else {
            error_codes::ARTIFACT_NOT_FOUND
        };
        return Err(ToolError::new(
            code,
            if code == error_codes::UPSTREAM_QUERY_TRUNCATED {
                format!(
                    "artifact URI {uri} was not found in the bounded {requested_limit}-match query"
                )
            } else {
                format!("no committed artifact matches URI {uri}")
            },
        ));
    }
    *matches = selected;
    Ok(())
}

fn resolve_directory(value: Option<&str>, name: &str) -> Result<PathBuf, ToolError> {
    let raw = match value {
        Some(value) if !value.trim().is_empty() => value,
        _ if name == "artifactRoot" => {
            return Err(ToolError::new(
                error_codes::INVALID_REQUEST,
                "artifactRoot is required, or CODE_INTEL_ARTIFACT_ROOT must be set",
            ));
        }
        _ => {
            return Err(ToolError::new(
                error_codes::INVALID_REQUEST,
                format!("{name} is required"),
            ));
        }
    };
    let path = Path::new(raw);
    if !path.is_dir() {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            format!("{name} must be an existing directory"),
        ));
    }
    Ok(path.to_path_buf())
}

fn normalize_artifact_refs(result: &Value) -> Result<Vec<ArtifactRef>, ToolError> {
    result
        .get("matches")
        .and_then(Value::as_array)
        .map(|matches| {
            matches
                .iter()
                .map(|item| {
                    item.get("artifactRef").ok_or_else(|| {
                        ToolError::new(
                            error_codes::UPSTREAM_CONTRACT_INVALID,
                            "code-intel match is missing artifactRef",
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_else(|| Ok(Vec::new()))?
        .into_iter()
        .map(|reference| {
            ArtifactRef::from_code_intel(reference).map_err(|error| {
                ToolError::new(
                    error_codes::UPSTREAM_CONTRACT_INVALID,
                    format!("code-intel returned an invalid artifact ref: {error}"),
                )
            })
        })
        .collect()
}

fn bounded_text(bytes: &[u8], max_bytes: usize) -> String {
    let text = String::from_utf8_lossy(bytes).trim().to_string();
    if text.len() <= max_bytes {
        text
    } else {
        let clipped: String = text.chars().take(max_bytes).collect();
        format!("{clipped}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_rejects_unsafe_repository_key() {
        let request = CodeIntelQueryRequest {
            repo: "../secret".into(),
            artifact_root: Some(".".into()),
            repo_path: None,
            artifact_schema: None,
            artifact_type: None,
            contains: None,
            artifact_uri: None,
            limit: None,
            request_id: None,
        };
        assert_eq!(
            query_code_intel(&request).unwrap_err().code,
            error_codes::INVALID_REQUEST
        );
    }

    #[test]
    fn request_accepts_normalized_query() {
        let request = CodeIntelQueryRequest {
            repo: "code-intel-pipeline".into(),
            artifact_root: Some(".".into()),
            repo_path: None,
            artifact_schema: Some("agent-code-slice-ranking.v1".into()),
            artifact_type: Some("code_evidence.agent_slice".into()),
            contains: None,
            artifact_uri: None,
            limit: Some(10),
            request_id: Some("r1".into()),
        };
        assert!(validate_request(&request).is_ok());
    }

    #[test]
    fn request_rejects_invalid_artifact_uri() {
        let request = CodeIntelQueryRequest {
            repo: "code-intel-pipeline".into(),
            artifact_root: Some(".".into()),
            repo_path: None,
            artifact_schema: None,
            artifact_type: None,
            contains: None,
            artifact_uri: Some("omc://artifact/sha256/not-a-digest".into()),
            limit: None,
            request_id: None,
        };
        assert_eq!(
            query_code_intel(&request).unwrap_err().code,
            error_codes::INVALID_REQUEST
        );
    }

    #[test]
    fn artifact_uri_selection_filters_verified_matches() {
        let digest = "a".repeat(64);
        let other = "b".repeat(64);
        let mut result = serde_json::json!({
            "matches": [
                {"artifactRef": {"sha256": other}},
                {"artifactRef": {"sha256": digest}}
            ]
        });
        select_artifact_matches(&mut result, &format!("omc://artifact/sha256/{digest}"), 100)
            .unwrap();
        assert_eq!(result["matches"].as_array().unwrap().len(), 1);
        assert_eq!(result["matches"][0]["artifactRef"]["sha256"], digest);
    }

    #[test]
    fn artifact_uri_selection_fails_closed_on_bounded_miss() {
        let digest = "a".repeat(64);
        let mut result = serde_json::json!({
            "matches": (0..100)
                .map(|index| serde_json::json!({"artifactRef": {"sha256": format!("{index:064x}")}}))
                .collect::<Vec<_>>()
        });
        let error =
            select_artifact_matches(&mut result, &format!("omc://artifact/sha256/{digest}"), 100)
                .unwrap_err();
        assert_eq!(error.code, error_codes::UPSTREAM_QUERY_TRUNCATED);
    }
}
