use super::*;

#[test]
fn capabilities_tool_returns_contract_json() {
    let result = AgentCapabilitiesTool.handle(serde_json::json!({
        "requestId": "test-1"
    }));
    assert_eq!(result.is_error, None);
    assert!(result.content[0].text.contains("omc.tool.v1"));
    assert!(result.content[0].text.contains("agent_route"));
}

#[test]
fn route_tool_rejects_missing_task() {
    let result = AgentRouteTool.handle(serde_json::json!({
        "requestId": "test-2"
    }));
    assert_eq!(result.is_error, Some(true));
    assert!(result.content[0].text.contains("invalid_request"));
}

#[test]
fn code_intel_tool_rejects_missing_repository() {
    let result = CodeIntelArtifactQueryTool.handle(serde_json::json!({
        "requestId": "test-3"
    }));
    assert_eq!(result.is_error, Some(true));
    assert!(result.content[0].text.contains("invalid_request"));
}

#[test]
fn lsp_tool_rejects_missing_file_without_starting_server() {
    let result = LspDocumentSymbolsTool.handle(serde_json::json!({
        "requestId": "test-lsp-1",
        "workingDirectory": "."
    }));
    assert_eq!(result.is_error, Some(true));
    assert!(result.content[0].text.contains("invalid_request"));
}

#[test]
fn lsp_tool_definition_is_read_only_by_contract() {
    let definition = LspDocumentSymbolsTool.definition();
    assert_eq!(definition.name, "lsp_document_symbols");
    assert!(definition.input_schema.properties.contains_key("file"));
    assert_eq!(
        definition.input_schema.properties["timeoutMs"].minimum,
        Some(5000)
    );
}

#[test]
fn debug_tool_requires_explicit_side_effect_opt_in() {
    let result = DebugInspectTool.handle(serde_json::json!({
        "adapterCommand": "missing-dap-adapter",
        "mode": "launch",
        "action": "threads",
        "launchArguments": {"program": "target"},
        "requestId": "debug-test"
    }));
    assert_eq!(result.is_error, Some(true));
    assert!(result.content[0].text.contains("side_effects_not_allowed"));
}

#[test]
fn debug_tool_definition_exposes_only_read_actions() {
    let definition = DebugInspectTool.definition();
    assert_eq!(definition.name, "debug_inspect");
    assert!(
        definition.input_schema.properties["action"]
            .r#enum
            .as_ref()
            .expect("debug action enum")
            .contains(&"threads".into())
    );
    assert!(
        !definition.input_schema.properties["action"]
            .r#enum
            .as_ref()
            .expect("debug action enum")
            .contains(&"continue".into())
    );
}

#[test]
fn workflow_tool_advances_only_from_supplied_evidence() {
    let result = WorkflowAdvanceTool.handle(serde_json::json!({
        "currentStage": "planning",
        "allTasksAssigned": true,
        "planApproved": true,
        "requestId": "workflow-test"
    }));
    assert_eq!(result.is_error, None);
    let text = &result.content[0].text;
    assert!(text.contains("omc.workflow.v1"));
    assert!(text.contains("executing"));
}

#[test]
fn workflow_tool_rejects_unknown_stage() {
    let result = WorkflowAdvanceTool.handle(serde_json::json!({
        "currentStage": "not-a-stage",
        "requestId": "workflow-invalid"
    }));
    assert_eq!(result.is_error, Some(true));
    assert!(result.content[0].text.contains("invalid_request"));
}

#[test]
fn result_tool_validates_required_fields() {
    let result = SubagentResultValidateTool.handle(serde_json::json!({
        "resultType": "omc.agent.findings.v1",
        "payload": {"summary": "done"},
        "requiredFields": ["summary"],
        "requestId": "test-4"
    }));
    assert_eq!(result.is_error, None);
    assert!(result.content[0].text.contains("omc.subagent-result.v1"));
}
