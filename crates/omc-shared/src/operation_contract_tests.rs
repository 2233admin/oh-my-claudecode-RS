use super::*;

#[test]
fn task_and_event_contracts_use_stable_versions() {
    let request = TaskRequest::new("task-1", "corr-1", "inspect repository");
    let event = OperationEvent::new(
        "event-1",
        "task.queued",
        "corr-1",
        "2026-08-12T00:00:00Z",
        serde_json::json!({"state":"queued"}),
    );
    assert_eq!(request.schema_version, TASK_SCHEMA_VERSION);
    assert!(request.validate().is_ok());
    assert_eq!(event.schema_version, EVENT_SCHEMA_VERSION);
    assert!(event.validate().is_ok());
    assert_eq!(
        serde_json::to_value(event).unwrap()["eventType"],
        "task.queued"
    );
}

#[test]
fn agent_event_is_wrapped_with_correlation_and_task_identity() {
    let event = crate::events::AgentEvent::Progress {
        message: "halfway".into(),
        percent: Some(0.5),
    };
    let wrapped = OperationEvent::from_agent_event(
        "event-2",
        "corr-2",
        Some("task-2".into()),
        "2026-08-12T00:00:00Z",
        &event,
    )
    .unwrap();
    assert_eq!(wrapped.event_type, "progress");
    assert_eq!(wrapped.task_id.as_deref(), Some("task-2"));
}

#[test]
fn code_intel_artifact_ref_is_normalized_and_validated() {
    let reference = ArtifactRef::from_code_intel(&serde_json::json!({
        "artifactSchema": "agent-code-slice-ranking.v1",
        "type": "code_evidence.agent_slice",
        "path": "objects/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "consumedSnapshotIdentity": "snapshot-1"
    }))
    .unwrap();
    assert_eq!(reference.schema_version, ARTIFACT_REF_SCHEMA_VERSION);
    assert_eq!(reference.producer, "code-intel-pipeline");
}

#[test]
fn artifact_ref_rejects_path_traversal_and_bad_digest() {
    let mut reference = ArtifactRef {
        schema_version: ARTIFACT_REF_SCHEMA_VERSION.into(),
        producer: "code-intel-pipeline".into(),
        artifact_schema: "schema".into(),
        artifact_type: "type".into(),
        path: "objects/../secret".into(),
        sha256: "bad".into(),
        consumed_snapshot_identity: "snapshot".into(),
        uri: None,
    };
    assert!(reference.validate().is_err());
    reference.path = "objects/sha256/a".into();
    reference.sha256 = "a".repeat(64);
    assert!(reference.validate().is_ok());
}

#[test]
fn code_intel_ref_has_canonical_uri_and_rejects_mismatch() {
    let digest = "a".repeat(64);
    let reference = ArtifactRef::from_code_intel(&serde_json::json!({
        "artifactSchema": "schema",
        "type": "type",
        "path": format!("objects/sha256/{digest}"),
        "sha256": digest,
        "consumedSnapshotIdentity": "snapshot"
    }))
    .unwrap();
    assert_eq!(
        reference.uri.as_deref(),
        Some(
            "omc://artifact/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        )
    );
    assert_eq!(
        ArtifactRef::digest_from_uri(reference.uri.as_deref().unwrap()).unwrap(),
        "a".repeat(64)
    );

    let mut mismatched = reference;
    mismatched.uri = Some(format!("{ARTIFACT_URI_PREFIX}{}", "b".repeat(64)));
    assert!(mismatched.validate().is_err());

    let mut upstream_mismatch = serde_json::json!({
        "artifactSchema": "schema",
        "type": "type",
        "path": format!("objects/sha256/{}", "a".repeat(64)),
        "sha256": "a".repeat(64),
        "consumedSnapshotIdentity": "snapshot",
    });
    upstream_mismatch["uri"] =
        serde_json::Value::String(format!("{ARTIFACT_URI_PREFIX}{}", "b".repeat(64)));
    assert!(ArtifactRef::from_code_intel(&upstream_mismatch).is_err());
}

#[test]
fn legacy_artifact_ref_can_be_normalized_without_losing_provenance() {
    let reference = ArtifactRef {
        schema_version: ARTIFACT_REF_SCHEMA_VERSION.into(),
        producer: "producer".into(),
        artifact_schema: "schema".into(),
        artifact_type: "type".into(),
        path: "objects/sha256/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .into(),
        sha256: "a".repeat(64),
        consumed_snapshot_identity: "snapshot".into(),
        uri: None,
    }
    .with_canonical_uri();
    assert!(reference.validate().is_ok());
    assert_eq!(reference.producer, "producer");
    assert!(reference.uri.is_some());
}
