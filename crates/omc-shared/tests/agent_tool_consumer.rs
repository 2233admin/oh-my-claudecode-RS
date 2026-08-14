//! Host-side contract fixtures.
//!
//! These tests intentionally deserialize into consumer-owned views instead of
//! OMC-RS types. Hermes, Sentinel, and other hosts should only depend on the
//! versioned JSON envelope and machine-readable error fields.

use omc_shared::agent_tool::{CapabilitiesPayload, ToolError, ToolResponse, capabilities_payload};
use omc_shared::lsp_adapter::LspDocumentSymbolsPayload;
use omc_shared::operation_contract::{ArtifactRef, ResultSchema, TypedSubagentResult};
use omc_shared::workflow_contract::{WorkflowAdvancePayload, WorkflowDecision, WorkflowStage};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct HostResponse {
    schema_version: String,
    request_id: String,
    ok: bool,
    data: Option<serde_json::Value>,
    error: Option<HostError>,
}

#[derive(Debug, Deserialize)]
struct HostError {
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostArtifactRef {
    schema_version: String,
    producer: String,
    sha256: String,
    uri: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostLspPayload {
    operation: String,
    server: String,
    file: String,
    result: serde_json::Value,
    side_effects: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostWorkflowPayload {
    schema_version: String,
    current_stage: WorkflowStage,
    next_stage: Option<WorkflowStage>,
    decision: WorkflowDecision,
    reason: String,
}

#[test]
fn host_consumes_success_without_omc_internal_types() {
    let response = ToolResponse::success("hermes-smoke", capabilities_payload());
    let encoded = serde_json::to_value(response).expect("tool response is serializable");
    let host_view: HostResponse = serde_json::from_value(encoded).expect("host view parses");

    assert_eq!(host_view.schema_version, "omc.tool.v1");
    assert_eq!(host_view.request_id, "hermes-smoke");
    assert!(host_view.ok);
    assert!(host_view.error.is_none());

    let data = host_view.data.expect("success response has data");
    assert_eq!(data["product"], "omc-rs");
    assert_eq!(data["protocol"], "omc.tool.v1");
    assert!(data["capabilities"].is_array());
}

#[test]
fn host_consumes_machine_readable_failure() {
    let response: ToolResponse<CapabilitiesPayload> = ToolResponse::failure(
        "sentinel-smoke",
        ToolError::new("invalid_request", "repo is required"),
    );
    let encoded = serde_json::to_value(response).expect("tool response is serializable");
    let host_view: HostResponse = serde_json::from_value(encoded).expect("host view parses");

    assert_eq!(host_view.schema_version, "omc.tool.v1");
    assert_eq!(host_view.request_id, "sentinel-smoke");
    assert!(!host_view.ok);
    assert!(host_view.data.is_none());

    let error = host_view.error.expect("failure response has error");
    assert_eq!(error.code, "invalid_request");
    assert_eq!(error.message, "repo is required");
}

#[test]
fn host_consumes_typed_result_without_omc_internal_types() {
    let schema = ResultSchema::new("omc.agent.findings.v1", vec!["summary".into()]);
    let result = TypedSubagentResult::new(
        schema.schema_id.clone(),
        serde_json::json!({"summary":"done"}),
        Vec::new(),
    )
    .expect("typed result envelope is valid");
    result
        .validate_against(&schema)
        .expect("consumer schema accepts result");
    let encoded = serde_json::to_value(result).expect("typed result is serializable");
    assert_eq!(encoded["schemaVersion"], "omc.subagent-result.v1");
    assert_eq!(encoded["resultType"], "omc.agent.findings.v1");
    assert_eq!(encoded["payload"]["summary"], "done");
}

#[test]
fn host_consumes_canonical_artifact_uri_without_omc_internal_types() {
    let digest = "a".repeat(64);
    let reference = ArtifactRef::from_code_intel(&serde_json::json!({
        "artifactSchema": "agent-code-slice-ranking.v1",
        "type": "code_evidence.agent_slice",
        "path": format!("objects/sha256/{digest}"),
        "sha256": digest,
        "consumedSnapshotIdentity": "snapshot-1"
    }))
    .expect("artifact reference is normalized");
    let encoded = serde_json::to_value(reference).expect("artifact reference is serializable");
    let host_view: HostArtifactRef =
        serde_json::from_value(encoded).expect("host view parses canonical URI");

    assert_eq!(host_view.schema_version, "omc.artifact-ref.v1");
    assert_eq!(host_view.producer, "code-intel-pipeline");
    assert_eq!(host_view.sha256, "a".repeat(64));
    assert_eq!(
        host_view.uri,
        "omc://artifact/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
}

#[test]
fn host_consumes_lsp_result_without_omc_internal_types() {
    let payload = LspDocumentSymbolsPayload {
        operation: "lsp.document_symbols".into(),
        server: "rust-analyzer".into(),
        working_directory: "C:/work/omc-rs".into(),
        file: "src/lib.rs".into(),
        uri: "file:///C:/work/omc-rs/src/lib.rs".into(),
        result: serde_json::json!([{"name":"main","kind":12}]),
        side_effects: Vec::new(),
    };
    let encoded = serde_json::to_value(ToolResponse::success("hermes-lsp", payload))
        .expect("LSP response is serializable");
    let host_view: HostResponse = serde_json::from_value(encoded).expect("host view parses");
    let data: HostLspPayload = serde_json::from_value(host_view.data.expect("LSP data exists"))
        .expect("host LSP view parses");

    assert_eq!(data.operation, "lsp.document_symbols");
    assert_eq!(data.server, "rust-analyzer");
    assert_eq!(data.file, "src/lib.rs");
    assert_eq!(data.result[0]["name"], "main");
    assert!(data.side_effects.is_empty());
}

#[test]
fn host_consumes_workflow_result_without_runtime_types() {
    let payload = WorkflowAdvancePayload {
        schema_version: "omc.workflow.v1".into(),
        current_stage: WorkflowStage::Planning,
        next_stage: Some(WorkflowStage::Executing),
        decision: WorkflowDecision::Advance,
        reason: "planning -> executing".into(),
    };
    let encoded = serde_json::to_value(ToolResponse::success("sentinel-workflow", payload))
        .expect("workflow response is serializable");
    let host_view: HostResponse = serde_json::from_value(encoded).expect("host view parses");
    let data: HostWorkflowPayload =
        serde_json::from_value(host_view.data.expect("workflow data exists"))
            .expect("host workflow view parses");

    assert_eq!(data.schema_version, "omc.workflow.v1");
    assert_eq!(data.current_stage, WorkflowStage::Planning);
    assert_eq!(data.next_stage, Some(WorkflowStage::Executing));
    assert_eq!(data.decision, WorkflowDecision::Advance);
    assert_eq!(data.reason, "planning -> executing");
}
