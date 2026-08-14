//! Stable operation, task, event, error, and artifact contracts.
//!
//! These types describe control-plane semantics only. They do not own an
//! Agent loop, a provider call, or a domain tool implementation.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent_tool::ToolError;

pub const TASK_SCHEMA_VERSION: &str = "omc.task.v1";
pub const EVENT_SCHEMA_VERSION: &str = "omc.event.v1";
pub const ARTIFACT_REF_SCHEMA_VERSION: &str = "omc.artifact-ref.v1";
pub const ARTIFACT_URI_PREFIX: &str = "omc://artifact/sha256/";
pub const SUBAGENT_RESULT_SCHEMA_VERSION: &str = "omc.subagent-result.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Dispatched,
    Acknowledged,
    Running,
    Succeeded,
    Failed,
    CancelRequested,
    Cancelled,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskRequest {
    pub schema_version: String,
    pub task_id: String,
    pub correlation_id: String,
    pub subject: String,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub requested_capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl TaskRequest {
    pub fn new(
        task_id: impl Into<String>,
        correlation_id: impl Into<String>,
        subject: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: TASK_SCHEMA_VERSION.into(),
            task_id: task_id.into(),
            correlation_id: correlation_id.into(),
            subject: subject.into(),
            input: Value::Null,
            requested_capabilities: Vec::new(),
            parent_task_id: None,
            idempotency_key: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != TASK_SCHEMA_VERSION {
            return Err("task request schema version is not supported".into());
        }
        if self.task_id.trim().is_empty() || self.correlation_id.trim().is_empty() {
            return Err("task_id and correlation_id must not be empty".into());
        }
        if self.subject.trim().is_empty() {
            return Err("task subject must not be empty".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatus {
    pub schema_version: String,
    pub task_id: String,
    pub correlation_id: String,
    pub state: TaskState,
    pub observed_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<ArtifactRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TypedSubagentResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ToolError>,
}

/// A host-neutral, schema-checked result emitted by a subagent.
///
/// OMC owns the envelope and validation boundary, while the producer owns the
/// named result type and payload fields. This keeps result transport stable
/// without copying a provider or agent runtime into OMC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TypedSubagentResult {
    pub schema_version: String,
    pub result_type: String,
    pub payload: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<ArtifactRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultSchema {
    pub schema_id: String,
    pub required_fields: Vec<String>,
}

impl ResultSchema {
    pub fn new(schema_id: impl Into<String>, required_fields: Vec<String>) -> Self {
        Self {
            schema_id: schema_id.into(),
            required_fields,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if !valid_result_type(&self.schema_id) {
            return Err("result schema_id must be a non-empty portable identifier".into());
        }
        if self.required_fields.iter().any(|field| {
            field.trim().is_empty()
                || field.contains('.')
                || self
                    .required_fields
                    .iter()
                    .filter(|item| *item == field)
                    .count()
                    > 1
        }) {
            return Err("result required fields must be unique, non-empty top-level names".into());
        }
        Ok(())
    }
}

impl TypedSubagentResult {
    pub fn new(
        result_type: impl Into<String>,
        payload: Value,
        artifact_refs: Vec<ArtifactRef>,
    ) -> Result<Self, String> {
        let result = Self {
            schema_version: SUBAGENT_RESULT_SCHEMA_VERSION.into(),
            result_type: result_type.into(),
            payload,
            artifact_refs,
        };
        result.validate_envelope()?;
        Ok(result)
    }

    pub fn validate_envelope(&self) -> Result<(), String> {
        if self.schema_version != SUBAGENT_RESULT_SCHEMA_VERSION {
            return Err("subagent result schema version is not supported".into());
        }
        if !valid_result_type(&self.result_type) {
            return Err("subagent result_type must be a non-empty portable identifier".into());
        }
        if !self.payload.is_object() {
            return Err("subagent result payload must be a JSON object".into());
        }
        for reference in &self.artifact_refs {
            reference.validate()?;
        }
        Ok(())
    }

    pub fn validate_against(&self, schema: &ResultSchema) -> Result<(), String> {
        self.validate_envelope()?;
        schema.validate()?;
        if self.result_type != schema.schema_id {
            return Err("subagent result_type does not match the requested result schema".into());
        }
        let Some(object) = self.payload.as_object() else {
            return Err("subagent result payload must be a JSON object".into());
        };
        if let Some(field) = schema
            .required_fields
            .iter()
            .find(|field| !object.contains_key(*field))
        {
            return Err(format!(
                "subagent result payload is missing required field {field}"
            ));
        }
        Ok(())
    }
}

fn valid_result_type(value: &str) -> bool {
    !value.trim().is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'/' | b'-' | b':')
        })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationEvent {
    pub schema_version: String,
    pub event_id: String,
    pub event_type: String,
    pub correlation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub occurred_at: String,
    pub payload: Value,
}

impl OperationEvent {
    pub fn new(
        event_id: impl Into<String>,
        event_type: impl Into<String>,
        correlation_id: impl Into<String>,
        occurred_at: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            schema_version: EVENT_SCHEMA_VERSION.into(),
            event_id: event_id.into(),
            event_type: event_type.into(),
            correlation_id: correlation_id.into(),
            task_id: None,
            occurred_at: occurred_at.into(),
            payload,
        }
    }

    pub fn from_agent_event(
        event_id: impl Into<String>,
        correlation_id: impl Into<String>,
        task_id: Option<String>,
        occurred_at: impl Into<String>,
        event: &crate::events::AgentEvent,
    ) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_value(event)?;
        let event_type = payload
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("agent.event")
            .to_string();
        Ok(Self {
            schema_version: EVENT_SCHEMA_VERSION.into(),
            event_id: event_id.into(),
            event_type,
            correlation_id: correlation_id.into(),
            task_id,
            occurred_at: occurred_at.into(),
            payload,
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != EVENT_SCHEMA_VERSION {
            return Err("operation event schema version is not supported".into());
        }
        if self.event_id.trim().is_empty()
            || self.event_type.trim().is_empty()
            || self.correlation_id.trim().is_empty()
            || self.occurred_at.trim().is_empty()
        {
            return Err("operation event identity and timestamp fields must not be empty".into());
        }
        Ok(())
    }
}

/// OMC's normalized view of a producer-owned artifact reference.
///
/// The original producer payload remains available to the adapter when
/// needed; this shape gives consumers stable names and provenance fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRef {
    pub schema_version: String,
    pub producer: String,
    pub artifact_schema: String,
    pub artifact_type: String,
    pub path: String,
    pub sha256: String,
    pub consumed_snapshot_identity: String,
    /// Canonical content-addressed URI. Optional for legacy persisted refs;
    /// normalized producer output includes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

impl ArtifactRef {
    pub fn from_code_intel(value: &Value) -> Result<Self, String> {
        let reference = Self {
            schema_version: ARTIFACT_REF_SCHEMA_VERSION.into(),
            producer: "code-intel-pipeline".into(),
            artifact_schema: required_string(value, "artifactSchema")?,
            artifact_type: required_string(value, "type")?,
            path: required_string(value, "path")?,
            sha256: required_string(value, "sha256")?,
            consumed_snapshot_identity: required_string(value, "consumedSnapshotIdentity")?,
            uri: value
                .get("uri")
                .and_then(Value::as_str)
                .map(ToString::to_string),
        };
        reference.validate()?;
        Ok(reference.with_canonical_uri())
    }

    /// Return the stable URI used by OMC consumers to address this artifact.
    pub fn canonical_uri(&self) -> String {
        format!("{ARTIFACT_URI_PREFIX}{}", self.sha256)
    }

    /// Fill the additive URI field while preserving all existing provenance.
    pub fn with_canonical_uri(mut self) -> Self {
        self.uri = Some(self.canonical_uri());
        self
    }

    /// Extract and validate a digest from an OMC artifact URI.
    pub fn digest_from_uri(uri: &str) -> Result<String, String> {
        let digest = uri
            .strip_prefix(ARTIFACT_URI_PREFIX)
            .filter(|value| !value.contains('/') && !value.is_empty())
            .ok_or_else(|| "artifact URI must use omc://artifact/sha256/<digest>".to_string())?;
        if !is_sha256_digest(digest) {
            return Err("artifact URI digest must be a 64-character hexadecimal value".into());
        }
        Ok(digest.to_string())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != ARTIFACT_REF_SCHEMA_VERSION {
            return Err("artifact ref schema version is not supported".into());
        }
        if self.producer.is_empty()
            || self.artifact_schema.is_empty()
            || self.artifact_type.is_empty()
            || self.consumed_snapshot_identity.is_empty()
        {
            return Err("artifact ref contains an empty provenance field".into());
        }
        if self.path.is_empty()
            || self.path.starts_with('/')
            || self.path.starts_with('\\')
            || self.path.contains('\\')
            || self
                .path
                .split('/')
                .any(|component| component.is_empty() || component == "." || component == "..")
        {
            return Err("artifact ref path must be a portable relative path".into());
        }
        if !is_sha256_digest(&self.sha256) {
            return Err("artifact ref sha256 must be a 64-character hexadecimal digest".into());
        }
        if let Some(uri) = &self.uri {
            let digest = Self::digest_from_uri(uri)?;
            if digest != self.sha256 || uri != &self.canonical_uri() {
                return Err("artifact ref URI must match its SHA-256 digest".into());
            }
        }
        Ok(())
    }
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn required_string(value: &Value, field: &str) -> Result<String, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| format!("artifact ref field {field} must be a non-empty string"))
}

#[cfg(test)]
#[path = "operation_contract_tests.rs"]
mod tests;
