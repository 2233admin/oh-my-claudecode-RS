use super::*;

pub(super) fn write_request(
    stdin: &mut ChildStdin,
    id: u64,
    method: &str,
    params: Value,
) -> Result<(), ToolError> {
    write_message(
        stdin,
        &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
    )
}

pub(super) fn write_notification(
    stdin: &mut ChildStdin,
    method: &str,
    params: Value,
) -> Result<(), ToolError> {
    write_message(
        stdin,
        &json!({"jsonrpc": "2.0", "method": method, "params": params}),
    )
}

fn write_message(stdin: &mut ChildStdin, message: &Value) -> Result<(), ToolError> {
    let body = serde_json::to_vec(message).map_err(|error| {
        ToolError::new(
            error_codes::UPSTREAM_CONTRACT_INVALID,
            format!("cannot encode LSP request: {error}"),
        )
    })?;
    write!(stdin, "Content-Length: {}\r\n\r\n", body.len()).map_err(|error| {
        ToolError::new(
            error_codes::UPSTREAM_FAILED,
            format!("cannot write LSP headers: {error}"),
        )
    })?;
    stdin.write_all(&body).map_err(|error| {
        ToolError::new(
            error_codes::UPSTREAM_FAILED,
            format!("cannot write LSP body: {error}"),
        )
    })?;
    stdin.flush().map_err(|error| {
        ToolError::new(
            error_codes::UPSTREAM_FAILED,
            format!("cannot flush LSP request: {error}"),
        )
    })
}

pub(super) fn receive_response(
    responses: &Receiver<Result<Value, LspTransportError>>,
    expected_id: u64,
    timeout: Duration,
) -> Result<Value, ToolError> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ToolError::new(
                error_codes::UPSTREAM_FAILED,
                format!("rust-analyzer timed out waiting for response id {expected_id}"),
            ));
        }
        let message = responses
            .recv_timeout(remaining)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => ToolError::new(
                    error_codes::UPSTREAM_FAILED,
                    format!("rust-analyzer timed out waiting for response id {expected_id}"),
                ),
                mpsc::RecvTimeoutError::Disconnected => ToolError::new(
                    error_codes::UPSTREAM_FAILED,
                    "rust-analyzer closed its output before responding",
                ),
            })?;
        let message = message.map_err(map_transport_error)?;
        if message.get("id") != Some(&json!(expected_id)) {
            continue;
        }
        if let Some(error) = message.get("error") {
            return Err(ToolError::new(
                error_codes::UPSTREAM_FAILED,
                format!("rust-analyzer returned an error: {error}"),
            ));
        }
        return message.get("result").cloned().ok_or_else(|| {
            ToolError::new(
                error_codes::UPSTREAM_CONTRACT_INVALID,
                "rust-analyzer response omitted result",
            )
        });
    }
}

pub(super) fn spawn_reader(
    stdout: impl Read + Send + 'static,
) -> Receiver<Result<Value, LspTransportError>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            match read_message(&mut reader) {
                Ok(Some(message)) => {
                    if sender.send(Ok(message)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = sender.send(Err(error));
                    break;
                }
            }
        }
    });
    receiver
}

pub(super) fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>, LspTransportError> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).map_err(LspTransportError::Io)?;
        if bytes == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(LspTransportError::InvalidHeader(line.trim().into()));
        };
        if name.eq_ignore_ascii_case("Content-Length") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| LspTransportError::InvalidHeader(line.trim().into()))?,
            );
        }
    }
    let length = content_length.ok_or(LspTransportError::MissingContentLength)?;
    if length > MAX_MESSAGE_BYTES {
        return Err(LspTransportError::ResponseTooLarge(length));
    }
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .map_err(LspTransportError::Io)?;
    serde_json::from_slice(&body).map_err(LspTransportError::Json)
}

fn map_transport_error(error: LspTransportError) -> ToolError {
    match error {
        LspTransportError::ResponseTooLarge(length) => ToolError::new(
            error_codes::UPSTREAM_RESPONSE_TOO_LARGE,
            format!("LSP response exceeds {MAX_MESSAGE_BYTES} bytes: {length}"),
        ),
        other => ToolError::new(error_codes::UPSTREAM_FAILED, other.to_string()),
    }
}

#[derive(Debug)]
pub(super) enum LspTransportError {
    Io(io::Error),
    Json(serde_json::Error),
    InvalidHeader(String),
    MissingContentLength,
    ResponseTooLarge(usize),
}

impl fmt::Display for LspTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "LSP I/O error: {error}"),
            Self::Json(error) => write!(formatter, "invalid LSP JSON response: {error}"),
            Self::InvalidHeader(header) => write!(formatter, "invalid LSP header: {header}"),
            Self::MissingContentLength => {
                formatter.write_str("LSP response omitted Content-Length")
            }
            Self::ResponseTooLarge(length) => write!(formatter, "LSP response too large: {length}"),
        }
    }
}

pub(super) fn path_to_file_uri(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let normalized = normalized.strip_prefix("//?/").unwrap_or(&normalized);
    let encoded = percent_encode_path(normalized);
    if cfg!(windows) {
        format!("file:///{encoded}")
    } else {
        format!("file://{encoded}")
    }
}

fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/' | b':') {
            encoded.push(*byte as char);
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}
