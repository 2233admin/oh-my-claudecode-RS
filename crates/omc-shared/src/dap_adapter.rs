//! Bounded, explicit-side-effect adapter for an external stdio DAP server.
//!
//! OMC-RS owns the DAP transport and lifecycle boundary only. Debugger-specific
//! launch/attach arguments stay in the caller and the adapter; this module does
//! not download, select, or implement a debugger.

use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::agent_tool::{ToolError, error_codes};

mod client;
#[cfg(test)]
use client::read_message;
use client::{
    DapClient, DapTransportError, required_positive_id, spawn_reader, terminate_with_error,
};

pub const DEBUG_SCHEMA_VERSION: &str = "omc.debug.v1";

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MIN_TIMEOUT_MS: u64 = 5_000;
const MAX_TIMEOUT_MS: u64 = 300_000;
const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_ADAPTER_ARGS: usize = 128;
const MAX_ADAPTER_ARG_BYTES: usize = 64 * 1024;
const MAX_OBSERVED_EVENTS: usize = 256;
const MAX_OUTPUT_EVENTS: usize = 64;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DebugSessionMode {
    Launch,
    Attach,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DebugAction {
    Threads,
    StackTrace,
    Scopes,
    Variables,
    Modules,
    LoadedSources,
    Output,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DebugInspectRequest {
    pub adapter_command: String,
    #[serde(default)]
    pub adapter_args: Vec<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
    pub mode: DebugSessionMode,
    pub action: DebugAction,
    #[serde(default)]
    pub launch_arguments: Option<Value>,
    #[serde(default)]
    pub attach_arguments: Option<Value>,
    #[serde(default)]
    pub thread_id: Option<i64>,
    #[serde(default)]
    pub frame_id: Option<i64>,
    #[serde(default)]
    pub variables_reference: Option<i64>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub allow_side_effects: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DebugOutputEvent {
    pub category: Option<String>,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DebugInspectPayload {
    pub schema_version: String,
    pub operation: String,
    pub adapter_command: String,
    pub working_directory: String,
    pub mode: DebugSessionMode,
    pub action: DebugAction,
    pub capabilities: Value,
    pub result: Value,
    pub observed_events: Vec<String>,
    pub output: Vec<DebugOutputEvent>,
    pub side_effects: Vec<String>,
}

/// Run one explicit launch/attach session and one read-only inspection action.
pub fn inspect_debug(request: &DebugInspectRequest) -> Result<DebugInspectPayload, ToolError> {
    let timeout = validate_request(request)?;
    let working_directory = resolve_working_directory(request.working_directory.as_deref())?;
    let session_arguments = selected_session_arguments(request)?;

    let mut child = Command::new(&request.adapter_command)
        .args(&request.adapter_args)
        .current_dir(&working_directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                ToolError::new(
                    error_codes::ADAPTER_UNAVAILABLE,
                    format!("DAP adapter is not available: {}", request.adapter_command),
                )
            } else {
                ToolError::new(
                    error_codes::UPSTREAM_FAILED,
                    format!("failed to start DAP adapter: {error}"),
                )
            }
        })?;

    let stdin = match child.stdin.take() {
        Some(stdin) => stdin,
        None => return terminate_with_error(&mut child, "DAP adapter stdin was not available"),
    };
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => return terminate_with_error(&mut child, "DAP adapter stdout was not available"),
    };

    let responses = spawn_reader(stdout);
    run_debug_session(
        &mut child,
        stdin,
        responses,
        request,
        &working_directory,
        session_arguments,
        timeout,
    )
}

fn run_debug_session(
    child: &mut Child,
    stdin: ChildStdin,
    responses: Receiver<Result<Value, DapTransportError>>,
    request: &DebugInspectRequest,
    working_directory: &Path,
    session_arguments: Value,
    timeout: Duration,
) -> Result<DebugInspectPayload, ToolError> {
    let mut client = DapClient::new(stdin, responses);
    let result = (|| {
        let initialize = client.request(
            "initialize",
            json!({
                "clientID": "omc-rs",
                "clientName": "omc-rs",
                "adapterID": "omc-external",
                "locale": "en-US",
                "linesStartAt1": true,
                "columnsStartAt1": true,
                "pathFormat": "path",
                "supportsVariableType": true,
                "supportsRunInTerminalRequest": false
            }),
            timeout,
        )?;
        let capabilities = initialize
            .get("capabilities")
            .cloned()
            .unwrap_or_else(|| initialize.clone());
        client.supports_configuration_done = capabilities
            .get("supportsConfigurationDoneRequest")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        client.initialized = true;

        let session_command = match request.mode {
            DebugSessionMode::Launch => "launch",
            DebugSessionMode::Attach => "attach",
        };
        client.request(session_command, session_arguments, timeout)?;

        let action_result = match request.action {
            DebugAction::Threads => client.request("threads", json!({}), timeout)?,
            DebugAction::StackTrace => {
                let thread_id = required_positive_id(request.thread_id, "threadId")?;
                client.request("stackTrace", json!({"threadId": thread_id}), timeout)?
            }
            DebugAction::Scopes => {
                let frame_id = required_positive_id(request.frame_id, "frameId")?;
                client.request("scopes", json!({"frameId": frame_id}), timeout)?
            }
            DebugAction::Variables => client.request(
                "variables",
                json!({
                    "variablesReference": required_positive_id(
                        request.variables_reference,
                        "variablesReference"
                    )?
                }),
                timeout,
            )?,
            DebugAction::Modules => client.request("modules", json!({}), timeout)?,
            DebugAction::LoadedSources => client.request("loadedSources", json!({}), timeout)?,
            DebugAction::Output => {
                client.drain_events(timeout.min(Duration::from_millis(250)))?;
                json!({"outputs": client.output.clone()})
            }
        };

        Ok(DebugInspectPayload {
            schema_version: DEBUG_SCHEMA_VERSION.into(),
            operation: "debug.inspect".into(),
            adapter_command: request.adapter_command.clone(),
            working_directory: working_directory.to_string_lossy().into_owned(),
            mode: request.mode.clone(),
            action: request.action.clone(),
            capabilities,
            result: action_result,
            observed_events: client.observed_events.clone(),
            output: client.output.clone(),
            side_effects: vec![
                "starts an external DAP adapter process".into(),
                match request.mode {
                    DebugSessionMode::Launch => {
                        "launches a debuggee through the external adapter".into()
                    }
                    DebugSessionMode::Attach => {
                        "attaches to an existing debuggee through the external adapter".into()
                    }
                },
                "disconnects and cleans up the adapter session before returning".into(),
            ],
        })
    })();

    // A launch owns the debuggee; an attach must leave it running.
    if client.initialized {
        let _ = client.request(
            "disconnect",
            json!({
                "terminateDebuggee": matches!(request.mode, DebugSessionMode::Launch)
            }),
            timeout.min(Duration::from_secs(2)),
        );
    }
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn validate_request(request: &DebugInspectRequest) -> Result<Duration, ToolError> {
    if request.adapter_command.trim().is_empty() {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            "adapterCommand must not be empty",
        ));
    }
    if request.adapter_args.len() > MAX_ADAPTER_ARGS
        || request
            .adapter_args
            .iter()
            .any(|value| value.len() > MAX_ADAPTER_ARG_BYTES)
    {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            "adapterArgs exceed the bounded adapter argument contract",
        ));
    }
    if !request.allow_side_effects {
        return Err(ToolError::new(
            error_codes::SIDE_EFFECTS_NOT_ALLOWED,
            "allowSideEffects=true is required to launch or attach a DAP session",
        ));
    }
    let timeout_ms = request.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            format!("timeoutMs must be between {MIN_TIMEOUT_MS} and {MAX_TIMEOUT_MS}"),
        ));
    }

    match request.action {
        DebugAction::StackTrace if request.thread_id.is_none() => {
            return Err(ToolError::new(
                error_codes::INVALID_REQUEST,
                "threadId is required for stackTrace",
            ));
        }
        DebugAction::Scopes if request.frame_id.is_none() => {
            return Err(ToolError::new(
                error_codes::INVALID_REQUEST,
                "frameId is required for scopes",
            ));
        }
        DebugAction::Variables if request.variables_reference.is_none() => {
            return Err(ToolError::new(
                error_codes::INVALID_REQUEST,
                "variablesReference is required for variables",
            ));
        }
        _ => {}
    }
    for (name, value) in [
        ("threadId", request.thread_id),
        ("frameId", request.frame_id),
        ("variablesReference", request.variables_reference),
    ] {
        if value.is_some_and(|value| value <= 0) {
            return Err(ToolError::new(
                error_codes::INVALID_REQUEST,
                format!("{name} must be positive"),
            ));
        }
    }
    Ok(Duration::from_millis(timeout_ms))
}

fn selected_session_arguments(request: &DebugInspectRequest) -> Result<Value, ToolError> {
    let (selected, forbidden, name) = match request.mode {
        DebugSessionMode::Launch => (
            request.launch_arguments.as_ref(),
            request.attach_arguments.as_ref(),
            "launchArguments",
        ),
        DebugSessionMode::Attach => (
            request.attach_arguments.as_ref(),
            request.launch_arguments.as_ref(),
            "attachArguments",
        ),
    };
    if forbidden.is_some() {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            format!(
                "{} is not valid for the selected debug mode",
                if name == "launchArguments" {
                    "attachArguments"
                } else {
                    "launchArguments"
                }
            ),
        ));
    }
    let Some(value) = selected else {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            format!("{name} is required to identify the debug target"),
        ));
    };
    if !value.is_object() || value.as_object().is_some_and(serde_json::Map::is_empty) {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            format!("{name} must be a non-empty JSON object"),
        ));
    }
    Ok(value.clone())
}

fn resolve_working_directory(input: Option<&str>) -> Result<PathBuf, ToolError> {
    let root = input.unwrap_or(".");
    let path = Path::new(root).canonicalize().map_err(|error| {
        ToolError::new(
            error_codes::INVALID_REQUEST,
            format!("workingDirectory is not a readable directory: {error}"),
        )
    })?;
    if !path.is_dir() {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            "workingDirectory must be a directory",
        ));
    }
    Ok(path)
}

#[cfg(test)]
#[path = "dap_adapter_tests.rs"]
mod tests;
