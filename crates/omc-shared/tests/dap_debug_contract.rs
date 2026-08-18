//! Real stdio DAP and host-consumer contract coverage.

use std::fs;
use std::path::Path;

use omc_shared::agent_tool::ToolResponse;
use omc_shared::dap_adapter::{DebugAction, DebugInspectRequest, DebugSessionMode, inspect_debug};
use serde::Deserialize;
use serde_json::json;
use tempfile::tempdir;

const DAP_FIXTURE: &str = r#"
import json
import subprocess
import sys

def receive():
    content_length = None
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        if line in (b"\r\n", b"\n"):
            break
        name, value = line.decode("ascii").split(":", 1)
        if name.lower() == "content-length":
            content_length = int(value.strip())
    if content_length is None:
        raise RuntimeError("missing Content-Length")
    return json.loads(sys.stdin.buffer.read(content_length).decode("utf-8"))

sequence = 1
target = None

def send(message):
    global sequence
    if "seq" not in message:
        message["seq"] = sequence
        sequence += 1
    body = json.dumps(message, separators=(",", ":")).encode("utf-8")
    sys.stdout.buffer.write(("Content-Length: %d\r\n\r\n" % len(body)).encode("ascii"))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()

while True:
    request = receive()
    if request is None:
        break
    command = request.get("command")
    request_seq = request.get("seq")
    if command == "initialize":
        send({"type": "response", "request_seq": request_seq, "command": command,
              "success": True, "body": {"capabilities": {}}})
    elif command in ("launch", "attach"):
        arguments = request.get("arguments") or {}
        target_command = arguments.get("targetCommand")
        target_args = arguments.get("targetArgs") or []
        if target_command:
            target = subprocess.Popen([target_command] + target_args,
                                       cwd=arguments.get("targetCwd") or None,
                                       stdout=subprocess.DEVNULL,
                                       stderr=subprocess.DEVNULL)
        send({"type": "event", "event": "initialized"})
        send({"type": "event", "event": "output",
              "body": {"category": "console", "output": "fixture target launched\n"}})
        send({"type": "response", "request_seq": request_seq, "command": command,
              "success": True, "body": {}})
    elif command == "threads":
        thread_id = target.pid if target is not None else 1
        send({"type": "response", "request_seq": request_seq, "command": command,
              "success": True, "body": {"threads": [{"id": thread_id, "name": "fixture-target"}]}})
    elif command == "disconnect":
        arguments = request.get("arguments") or {}
        if target is not None and arguments.get("terminateDebuggee"):
            target.terminate()
            target.wait(timeout=2)
        send({"type": "response", "request_seq": request_seq, "command": command,
              "success": True, "body": {}})
        break
    else:
        send({"type": "response", "request_seq": request_seq, "command": command,
              "success": True, "body": {}})
"#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostDebugPayload {
    schema_version: String,
    operation: String,
    action: DebugAction,
    result: serde_json::Value,
    observed_events: Vec<String>,
    side_effects: Vec<String>,
}

#[test]
fn external_stdio_dap_launches_target_and_host_consumes_read_only_result() {
    let root = tempdir().expect("temporary DAP root");
    let script = root.path().join("dap_fixture.py");
    fs::write(&script, DAP_FIXTURE).expect("DAP fixture is written");
    let adapter_command = std::env::var("OMC_PYTHON_COMMAND").unwrap_or_else(|_| "python".into());
    let target_command = std::env::current_exe().expect("test target exists");

    let request = DebugInspectRequest {
        adapter_command,
        adapter_args: vec!["-u".into(), script.to_string_lossy().into_owned()],
        working_directory: Some(root.path().to_string_lossy().into_owned()),
        mode: DebugSessionMode::Launch,
        action: DebugAction::Threads,
        launch_arguments: Some(json!({
            "targetCommand": target_command,
            "targetArgs": ["--help"],
            "targetCwd": root.path()
        })),
        attach_arguments: None,
        thread_id: None,
        frame_id: None,
        variables_reference: None,
        timeout_ms: Some(10_000),
        allow_side_effects: true,
    };

    let payload = inspect_debug(&request).expect("external DAP session succeeds");
    assert_eq!(payload.schema_version, "omc.debug.v1");
    assert_eq!(payload.operation, "debug.inspect");
    assert_eq!(payload.result["threads"][0]["name"], "fixture-target");
    assert!(
        payload
            .observed_events
            .iter()
            .any(|event| event == "initialized")
    );
    assert_eq!(payload.output[0].output, "fixture target launched\n");

    let encoded = serde_json::to_value(ToolResponse::success("hermes-debug", payload))
        .expect("debug response serializes");
    let host_data = encoded["data"].clone();
    let host_view: HostDebugPayload =
        serde_json::from_value(host_data).expect("consumer-owned debug view parses");
    assert_eq!(host_view.schema_version, "omc.debug.v1");
    assert_eq!(host_view.operation, "debug.inspect");
    assert_eq!(host_view.action, DebugAction::Threads);
    assert_eq!(host_view.result["threads"][0]["name"], "fixture-target");
    assert!(!host_view.side_effects.is_empty());
    assert!(
        host_view
            .observed_events
            .iter()
            .any(|event| event == "output")
    );
}

#[test]
fn debug_request_rejects_attach_without_selected_target_arguments() {
    let request = DebugInspectRequest {
        adapter_command: "not-used".into(),
        adapter_args: Vec::new(),
        working_directory: Some(Path::new(".").to_string_lossy().into_owned()),
        mode: DebugSessionMode::Attach,
        action: DebugAction::Threads,
        launch_arguments: None,
        attach_arguments: None,
        thread_id: None,
        frame_id: None,
        variables_reference: None,
        timeout_ms: Some(5_000),
        allow_side_effects: true,
    };
    let error = inspect_debug(&request).expect_err("target args are required");
    assert_eq!(error.code, "invalid_request");
}
