use super::*;
use omc_shared::agent_tool::{RouteRequest, ToolResponse, capabilities_payload, route_agent_task};
use omc_shared::operation_contract::{
    OperationEvent, ResultSchema, TaskRequest, TaskState, TaskStatus, TypedSubagentResult,
};
use serde::Deserialize;
use serde_json::json;

fn sample_servers() -> Vec<McpServerDef> {
    vec![
        McpServerDef {
            name: "omc-state".into(),
            command: "omc-mcp".into(),
            args: vec!["--server".into()],
            env: None,
        },
        McpServerDef {
            name: "omc-memory".into(),
            command: "omc-mcp".into(),
            args: vec!["--memory".into()],
            env: Some([("DEBUG".into(), "1".into())].into_iter().collect()),
        },
    ]
}

#[test]
fn claude_mcp_json_format() {
    let json = claude_mcp_json(&sample_servers());
    assert_eq!(json["omc-state"]["command"], "omc-mcp");
    assert_eq!(json["omc-state"]["args"][0], "--server");
    assert_eq!(json["omc-memory"]["env"]["DEBUG"], "1");
}

#[test]
fn codex_mcp_toml_format() {
    let toml_str = codex_mcp_toml(&sample_servers()).unwrap();
    assert!(toml_str.contains("[mcp_servers.omc-state]"));
    assert!(toml_str.contains("command = \"omc-mcp\""));
    assert!(toml_str.contains("[mcp_servers.omc-memory]"));
    assert!(toml_str.contains("DEBUG"));
}

#[test]
fn empty_servers() {
    let json = claude_mcp_json(&[]);
    assert_eq!(json, serde_json::json!({}));
    let toml_str = codex_mcp_toml(&[]).unwrap();
    assert!(toml_str.contains("mcp_servers"));
}

#[test]
fn setup_registers_unified_server_for_both_hosts_and_is_idempotent() {
    let server = omc_server_definition();
    for host in [crate::HostKind::Claude, crate::HostKind::Codex] {
        let tmp = tempfile::tempdir().unwrap();
        crate::create_adapter(host)
            .init_project(tmp.path())
            .expect("host project initializes");

        assert!(ensure_mcp_server(tmp.path(), host, &server).unwrap());
        assert!(!ensure_mcp_server(tmp.path(), host, &server).unwrap());

        match host {
            crate::HostKind::Claude => {
                let config: serde_json::Value = serde_json::from_str(
                    &std::fs::read_to_string(tmp.path().join(".mcp.json")).unwrap(),
                )
                .unwrap();
                assert_eq!(config["mcpServers"]["omc-rs"]["command"], "omc");
                assert_eq!(config["mcpServers"]["omc-rs"]["args"][0], "mcp");
                assert_eq!(
                    std::fs::read_to_string(tmp.path().join(".claude/settings.json")).unwrap(),
                    "{\n}\n"
                );
            }
            crate::HostKind::Codex => {
                let config: toml::Value =
                    std::fs::read_to_string(tmp.path().join(".codex/config.toml"))
                        .unwrap()
                        .parse()
                        .unwrap();
                assert_eq!(
                    config["mcp_servers"]["omc-rs"]["command"],
                    toml::Value::String("omc".into())
                );
                assert_eq!(
                    config["mcp_servers"]["omc-rs"]["args"][0],
                    toml::Value::String("mcp".into())
                );
            }
        }
    }
}

#[test]
fn setup_rejects_conflicting_server_without_overwrite() {
    let tmp = tempfile::tempdir().unwrap();
    crate::create_adapter(crate::HostKind::Claude)
        .init_project(tmp.path())
        .unwrap();
    let path = tmp.path().join(".mcp.json");
    std::fs::write(
        &path,
        r#"{"mcpServers":{"omc-rs":{"command":"other","args":[]}}}"#,
    )
    .unwrap();

    let error = ensure_mcp_server(
        tmp.path(),
        crate::HostKind::Claude,
        &omc_server_definition(),
    )
    .unwrap_err();
    assert!(error.contains("conflicting MCP server"));
    assert!(std::fs::read_to_string(path).unwrap().contains("other"));
}

#[test]
fn forced_setup_replaces_conflict_and_keeps_backup_for_both_hosts() {
    for host in [crate::HostKind::Claude, crate::HostKind::Codex] {
        let tmp = tempfile::tempdir().unwrap();
        crate::create_adapter(host)
            .init_project(tmp.path())
            .unwrap();
        let path = match host {
            crate::HostKind::Claude => tmp.path().join(".mcp.json"),
            crate::HostKind::Codex => tmp.path().join(".codex/config.toml"),
        };
        let original = match host {
            crate::HostKind::Claude => r#"{"mcpServers":{"omc-rs":{"command":"other","args":[]}}}"#,
            crate::HostKind::Codex => "[mcp_servers.omc-rs]\ncommand = \"other\"\nargs = []\n",
        };
        std::fs::write(&path, original).unwrap();

        assert!(
            ensure_mcp_server_with_force(tmp.path(), host, &omc_server_definition(), true).unwrap()
        );
        assert!(std::fs::read_to_string(&path).unwrap().contains("omc"));
        let backup = path.with_extension(format!(
            "{}.bak",
            path.extension().and_then(|value| value.to_str()).unwrap()
        ));
        assert_eq!(std::fs::read_to_string(backup).unwrap(), original);
    }
}

#[test]
fn hermes_setup_is_idempotent_and_preserves_siblings() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.yaml");
    std::fs::write(&path, "provider: openrouter\nmodel: auto\n").unwrap();

    let server = omc_server_definition();
    assert!(ensure_hermes_mcp_server(tmp.path(), &server).unwrap());
    assert!(!ensure_hermes_mcp_server(tmp.path(), &server).unwrap());

    let config: serde_yaml::Value =
        serde_yaml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(config["provider"].as_str(), Some("openrouter"));
    assert_eq!(
        config["mcp_servers"]["omc-rs"]["command"].as_str(),
        Some("omc")
    );
    assert_eq!(
        config["mcp_servers"]["omc-rs"]["args"][0].as_str(),
        Some("mcp")
    );
}

#[test]
fn hermes_setup_rejects_conflict_without_overwrite() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.yaml");
    std::fs::write(
        &path,
        "mcp_servers:\n  omc-rs:\n    command: other\n    args: []\n",
    )
    .unwrap();

    let error = ensure_hermes_mcp_server(tmp.path(), &omc_server_definition()).unwrap_err();
    assert!(error.contains("conflicting MCP server"));
    assert!(
        std::fs::read_to_string(path)
            .unwrap()
            .contains("command: other")
    );
}

#[test]
fn hermes_force_replaces_conflict_and_keeps_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.yaml");
    let original = "provider: local\nmcp_servers:\n  omc-rs:\n    command: other\n    args: []\n";
    std::fs::write(&path, original).unwrap();

    assert!(
        ensure_hermes_mcp_server_with_force(tmp.path(), &omc_server_definition(), true).unwrap()
    );
    let config: serde_yaml::Value =
        serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(config["provider"].as_str(), Some("local"));
    assert_eq!(config["mcp_servers"]["omc-rs"]["command"], "omc");
    assert_eq!(
        std::fs::read_to_string(path.with_extension("yaml.bak")).unwrap(),
        original
    );
}

#[test]
fn same_agent_server_is_registered_for_both_hosts() {
    let servers = [McpServerDef {
        name: "omc-agent-tools".into(),
        command: "omc-mcp".into(),
        args: Vec::new(),
        env: None,
    }];

    let claude = claude_mcp_json(&servers);
    assert_eq!(claude["omc-agent-tools"]["command"], "omc-mcp");

    let codex: toml::Value = codex_mcp_toml(&servers)
        .expect("Codex MCP registration is valid TOML")
        .parse()
        .expect("Codex MCP registration parses");
    assert_eq!(
        codex["mcp_servers"]["omc-agent-tools"]["command"],
        toml::Value::String("omc-mcp".into())
    );
}

#[derive(Debug, Deserialize)]
struct ConsumerEnvelope {
    schema_version: String,
    request_id: String,
    ok: bool,
    data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConsumerEvent {
    schema_version: String,
    event_id: String,
    event_type: String,
    correlation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConsumerResult {
    schema_version: String,
    result_type: String,
    payload: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConsumerTaskStatus {
    schema_version: String,
    task_id: String,
    correlation_id: String,
    state: String,
    result: Option<ConsumerResult>,
}

fn consume_tool_response<T: serde::Serialize>(response: ToolResponse<T>) {
    let encoded = serde_json::to_value(response).expect("tool response is serializable");
    let envelope: ConsumerEnvelope =
        serde_json::from_value(encoded).expect("consumer envelope parses");
    assert_eq!(envelope.schema_version, "omc.tool.v1");
    assert!(!envelope.request_id.is_empty());
    assert!(envelope.ok);
    assert!(envelope.data.is_some());
}

fn assert_agent_registration(host: crate::HostKind) {
    let registration = crate::create_adapter(host)
        .generate_mcp_registration(&[McpServerDef {
            name: "omc-agent-tools".into(),
            command: "omc-mcp".into(),
            args: Vec::new(),
            env: None,
        }])
        .expect("agent MCP registration is generated");

    match host {
        crate::HostKind::Claude => {
            assert_eq!(registration["omc-agent-tools"]["command"], "omc-mcp");
        }
        crate::HostKind::Codex => {
            let toml_text = registration["toml"]
                .as_str()
                .expect("Codex registration carries TOML text");
            let config: toml::Value = toml_text.parse().expect("Codex registration parses");
            assert_eq!(
                config["mcp_servers"]["omc-agent-tools"]["command"],
                toml::Value::String("omc-mcp".into())
            );
        }
    }
}

fn consume_one_agent_contract() {
    let capabilities = ToolResponse::success("host-capabilities", capabilities_payload());
    consume_tool_response(capabilities);

    let route = route_agent_task(&RouteRequest {
        task: "implement a small repository change and verify it".into(),
        agent_type: Some("executor".into()),
        previous_failures: None,
    });
    consume_tool_response(ToolResponse::success("host-route", route.clone()));

    let task = TaskRequest::new("task-1", "corr-1", "implement and verify");
    task.validate().expect("task request is valid");
    let before = OperationEvent::new(
        "event-before",
        "task.dispatched",
        "corr-1",
        "2026-08-12T15:00:00Z",
        json!({"surface": route.recommended_surface}),
    );
    before.validate().expect("pre-execution event is valid");
    let before_view: ConsumerEvent = serde_json::from_value(serde_json::to_value(before).unwrap())
        .expect("consumer parses pre-event");
    assert_eq!(before_view.schema_version, "omc.event.v1");
    assert_eq!(before_view.event_id, "event-before");
    assert_eq!(before_view.event_type, "task.dispatched");
    assert_eq!(before_view.correlation_id, "corr-1");

    let schema = ResultSchema::new("omc.agent.change.v1", vec!["summary".into()]);
    let result = TypedSubagentResult::new(
        schema.schema_id.clone(),
        json!({"summary":"change verified"}),
        Vec::new(),
    )
    .expect("typed result is valid");
    result
        .validate_against(&schema)
        .expect("typed result matches consumer schema");
    let after = OperationEvent::new(
        "event-after",
        "task.succeeded",
        "corr-1",
        "2026-08-12T15:01:00Z",
        json!({"resultType": result.result_type}),
    );
    after.validate().expect("post-execution event is valid");

    let status = TaskStatus {
        schema_version: "omc.task.v1".into(),
        task_id: task.task_id,
        correlation_id: task.correlation_id,
        state: TaskState::Succeeded,
        observed_at: "2026-08-12T15:01:00Z".into(),
        artifact_refs: Vec::new(),
        result: Some(result),
        error: None,
    };
    let mut status_view: ConsumerTaskStatus =
        serde_json::from_value(serde_json::to_value(status).unwrap())
            .expect("consumer parses task status");
    assert_eq!(status_view.schema_version, "omc.task.v1");
    assert_eq!(status_view.task_id, "task-1");
    assert_eq!(status_view.correlation_id, "corr-1");
    assert_eq!(status_view.state, "succeeded");
    let result_view = status_view.result.take().expect("status carries result");
    assert_eq!(result_view.schema_version, "omc.subagent-result.v1");
    assert_eq!(result_view.result_type, "omc.agent.change.v1");
    assert_eq!(result_view.payload["summary"], "change verified");

    let after_view: ConsumerEvent = serde_json::from_value(serde_json::to_value(after).unwrap())
        .expect("consumer parses post-event");
    assert_eq!(after_view.event_type, "task.succeeded");
    assert_eq!(after_view.correlation_id, "corr-1");
}

#[test]
fn both_hosts_consume_one_agent_contract() {
    for host in [crate::HostKind::Claude, crate::HostKind::Codex] {
        assert_agent_registration(host);
        consume_one_agent_contract();
    }
}
