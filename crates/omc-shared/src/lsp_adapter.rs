//! Bounded, read-only LSP adapter.
//!
//! Long-running consumers may reuse a small project-scoped pool; the direct
//! function remains a one-shot fallback. This is not a language-server
//! registry or a second agent runtime.

use std::fmt::{self, Write as FmtWrite};
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::agent_tool::{ToolError, error_codes};

mod transport;
#[cfg(test)]
use transport::read_message;
use transport::{
    LspTransportError, path_to_file_uri, receive_response, spawn_reader, write_notification,
    write_request,
};

const RUST_ANALYZER_COMMAND: &str = "rust-analyzer";
const DEFAULT_TIMEOUT_MS: u64 = 20_000;
const MIN_TIMEOUT_MS: u64 = 5_000;
const MAX_TIMEOUT_MS: u64 = 60_000;
const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_PROJECT_SESSIONS: usize = 4;
const PROJECT_SESSION_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LspDocumentSymbolsRequest {
    #[serde(default)]
    pub working_directory: Option<String>,
    pub file: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LspDocumentSymbolsPayload {
    pub operation: String,
    pub server: String,
    pub working_directory: String,
    pub file: String,
    pub uri: String,
    pub result: Value,
    pub server_process_id: u32,
    pub session_reused: bool,
    pub side_effects: Vec<String>,
}

mod session;
pub use session::LspProjectPool;

pub fn query_document_symbols(
    request: &LspDocumentSymbolsRequest,
) -> Result<LspDocumentSymbolsPayload, ToolError> {
    let timeout = validate_timeout(request.timeout_ms)?;
    let (root, file, relative_file) = resolve_source_file(request)?;
    let contents = fs::read_to_string(&file).map_err(|error| {
        ToolError::new(
            error_codes::INVALID_REQUEST,
            format!("cannot read Rust source file: {error}"),
        )
    })?;
    let uri = path_to_file_uri(&file);

    let (result, server_process_id) = run_document_symbols(&root, &uri, &contents, timeout)?;
    Ok(LspDocumentSymbolsPayload {
        operation: "lsp.document_symbols".into(),
        server: RUST_ANALYZER_COMMAND.into(),
        working_directory: root.to_string_lossy().into_owned(),
        file: relative_file,
        uri,
        result,
        server_process_id,
        session_reused: false,
        side_effects: Vec::new(),
    })
}

fn validate_timeout(timeout_ms: Option<u64>) -> Result<Duration, ToolError> {
    let timeout_ms = timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            format!("timeoutMs must be between {MIN_TIMEOUT_MS} and {MAX_TIMEOUT_MS}"),
        ));
    }
    Ok(Duration::from_millis(timeout_ms))
}

fn resolve_source_file(
    request: &LspDocumentSymbolsRequest,
) -> Result<(PathBuf, PathBuf, String), ToolError> {
    if request.file.trim().is_empty() {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            "file must not be empty",
        ));
    }

    let root_input = request.working_directory.as_deref().unwrap_or(".");
    let root = Path::new(root_input).canonicalize().map_err(|error| {
        ToolError::new(
            error_codes::INVALID_REQUEST,
            format!("workingDirectory is not a readable directory: {error}"),
        )
    })?;
    if !root.is_dir() {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            "workingDirectory must be a directory",
        ));
    }

    let requested_file = Path::new(&request.file);
    if requested_file.is_absolute() {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            "file must be project-relative",
        ));
    }

    let file = root.join(requested_file).canonicalize().map_err(|error| {
        ToolError::new(
            error_codes::INVALID_REQUEST,
            format!("file is not readable: {error}"),
        )
    })?;
    if !file.starts_with(&root) {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            "file must remain inside workingDirectory",
        ));
    }
    if !file.is_file() {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            "file must be a regular file",
        ));
    }
    if file.extension().and_then(|value| value.to_str()) != Some("rs") {
        return Err(ToolError::new(
            error_codes::INVALID_REQUEST,
            "lsp_document_symbols currently supports Rust files only",
        ));
    }

    let relative = file
        .strip_prefix(&root)
        .map_err(|_| ToolError::new(error_codes::INVALID_REQUEST, "file is outside root"))?
        .to_string_lossy()
        .replace('\\', "/");
    Ok((root, file, relative))
}

fn run_document_symbols(
    root: &Path,
    uri: &str,
    contents: &str,
    timeout: Duration,
) -> Result<(Value, u32), ToolError> {
    let mut child = Command::new(RUST_ANALYZER_COMMAND)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                ToolError::new(
                    error_codes::ADAPTER_UNAVAILABLE,
                    "rust-analyzer is not available on PATH",
                )
            } else {
                ToolError::new(
                    error_codes::UPSTREAM_FAILED,
                    format!("failed to start rust-analyzer: {error}"),
                )
            }
        })?;

    let mut stdin = match child.stdin.take() {
        Some(stdin) => stdin,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ToolError::new(
                error_codes::UPSTREAM_FAILED,
                "rust-analyzer stdin was not available",
            ));
        }
    };
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ToolError::new(
                error_codes::UPSTREAM_FAILED,
                "rust-analyzer stdout was not available",
            ));
        }
    };
    let responses = spawn_reader(stdout);

    let result = run_lsp_session(&mut stdin, &responses, root, uri, contents, timeout);

    // This adapter is intentionally one-shot. Killing after the exit
    // notification bounds cleanup even when a server is still indexing.
    let _ = write_notification(&mut stdin, "exit", Value::Null);
    let _ = child.kill();
    let _ = child.wait();
    result.map(|value| (value, child.id()))
}

fn run_lsp_session(
    stdin: &mut ChildStdin,
    responses: &Receiver<Result<Value, LspTransportError>>,
    root: &Path,
    uri: &str,
    contents: &str,
    timeout: Duration,
) -> Result<Value, ToolError> {
    initialize_lsp(stdin, responses, root, timeout)?;
    write_notification(
        stdin,
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": uri,
                "languageId": "rust",
                "version": 1,
                "text": contents
            }
        }),
    )?;
    write_request(
        stdin,
        2,
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": uri}}),
    )?;
    receive_response(responses, 2, timeout)
}

fn initialize_lsp(
    stdin: &mut ChildStdin,
    responses: &Receiver<Result<Value, LspTransportError>>,
    root: &Path,
    timeout: Duration,
) -> Result<(), ToolError> {
    write_request(
        stdin,
        1,
        "initialize",
        json!({
            "processId": Value::Null,
            "clientInfo": {"name": "omc-rs", "version": env!("CARGO_PKG_VERSION")},
            "rootUri": path_to_file_uri(root),
            "workspaceFolders": [{"uri": path_to_file_uri(root), "name": "omc-rs-workspace"}],
            "capabilities": {
                "workspace": {"workspaceFolders": true},
                "textDocument": {"documentSymbol": {"hierarchicalDocumentSymbolSupport": true}}
            },
            "initializationOptions": {},
            "trace": "off"
        }),
    )?;
    let initialized = receive_response(responses, 1, timeout)?;
    if initialized
        .get("capabilities")
        .and_then(Value::as_object)
        .is_none()
    {
        return Err(ToolError::new(
            error_codes::UPSTREAM_CONTRACT_INVALID,
            "rust-analyzer initialize response omitted capabilities",
        ));
    }

    write_notification(stdin, "initialized", json!({}))?;
    Ok(())
}

#[cfg(test)]
#[path = "lsp_adapter_tests.rs"]
mod tests;
