use super::*;

use crate::session_pool::{BoundedSessionPool, SessionPoolError};

/// Process-local project pool used by long-running consumers such as MCP.
pub struct LspProjectPool {
    sessions: Mutex<BoundedSessionPool<PathBuf, LspSession>>,
}
struct LspSession {
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<Result<Value, LspTransportError>>,
    next_request_id: u64,
    document_versions: std::collections::HashMap<String, i64>,
}

impl LspSession {
    fn open(root: &Path, timeout: Duration) -> Result<Self, ToolError> {
        let mut child = spawn_lsp(root)?;
        let stdin = child.stdin.take().ok_or_else(|| {
            ToolError::new(
                error_codes::UPSTREAM_FAILED,
                "rust-analyzer stdin was not available",
            )
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ToolError::new(
                error_codes::UPSTREAM_FAILED,
                "rust-analyzer stdout was not available",
            )
        })?;
        let responses = spawn_reader(stdout);
        let mut session = Self {
            child,
            stdin,
            responses,
            next_request_id: 1,
            document_versions: std::collections::HashMap::new(),
        };
        initialize_lsp(&mut session.stdin, &session.responses, root, timeout)?;
        session.next_request_id = 2;
        Ok(session)
    }

    fn document_symbols(
        &mut self,
        uri: &str,
        contents: &str,
        timeout: Duration,
    ) -> Result<Value, ToolError> {
        let version = self.document_versions.entry(uri.to_string()).or_insert(0);
        *version += 1;
        if *version == 1 {
            write_notification(
                &mut self.stdin,
                "textDocument/didOpen",
                json!({
                    "textDocument": {"uri": uri, "languageId": "rust", "version": version, "text": contents}
                }),
            )?;
        } else {
            write_notification(
                &mut self.stdin,
                "textDocument/didChange",
                json!({
                    "textDocument": {"uri": uri, "version": version},
                    "contentChanges": [{"text": contents}]
                }),
            )?;
        }
        let request_id = self.next_request_id;
        self.next_request_id += 1;
        write_request(
            &mut self.stdin,
            request_id,
            "textDocument/documentSymbol",
            json!({"textDocument": {"uri": uri}}),
        )?;
        receive_response(&self.responses, request_id, timeout)
    }
}

impl Drop for LspSession {
    fn drop(&mut self) {
        let request_id = self.next_request_id;
        let _ = write_request(&mut self.stdin, request_id, "shutdown", Value::Null);
        let _ = receive_response(&self.responses, request_id, Duration::from_millis(500));
        let _ = write_notification(&mut self.stdin, "exit", Value::Null);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn query_with_pool(
    sessions: &Mutex<BoundedSessionPool<PathBuf, LspSession>>,
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
    let mut pool = sessions.lock().map_err(|_| {
        ToolError::new(
            error_codes::UPSTREAM_FAILED,
            "LSP project session pool was poisoned",
        )
    })?;
    let reused = pool.contains_key(&root);
    let session = pool
        .get_or_try_insert_with(root.clone(), || LspSession::open(&root, timeout))
        .map_err(|error| match error {
            SessionPoolError::Capacity { limit } => ToolError::new(
                error_codes::UPSTREAM_FAILED,
                format!("LSP project session capacity reached ({limit})"),
            ),
            SessionPoolError::Open(error) => error,
        })?;
    let result = session.document_symbols(&uri, &contents, timeout)?;
    Ok(LspDocumentSymbolsPayload {
        operation: "lsp.document_symbols".into(),
        server: RUST_ANALYZER_COMMAND.into(),
        working_directory: root.to_string_lossy().into_owned(),
        file: relative_file,
        uri,
        result,
        server_process_id: session.child.id(),
        session_reused: reused,
        side_effects: Vec::new(),
    })
}

fn spawn_lsp(root: &Path) -> Result<Child, ToolError> {
    Command::new(RUST_ANALYZER_COMMAND)
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
        })
}

impl Default for LspProjectPool {
    fn default() -> Self {
        Self::new()
    }
}

impl LspProjectPool {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(BoundedSessionPool::new(
                MAX_PROJECT_SESSIONS,
                PROJECT_SESSION_TTL,
            )),
        }
    }

    pub fn query_document_symbols(
        &self,
        request: &LspDocumentSymbolsRequest,
    ) -> Result<LspDocumentSymbolsPayload, ToolError> {
        query_with_pool(&self.sessions, request)
    }
}
