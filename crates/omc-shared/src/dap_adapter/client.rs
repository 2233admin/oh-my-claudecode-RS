use super::*;

pub(super) fn required_positive_id(value: Option<i64>, name: &str) -> Result<i64, ToolError> {
    value.filter(|value| *value > 0).ok_or_else(|| {
        ToolError::new(
            error_codes::INVALID_REQUEST,
            format!("{name} is required and must be positive"),
        )
    })
}

pub(super) fn terminate_with_error<T>(child: &mut Child, message: &str) -> Result<T, ToolError> {
    let _ = child.kill();
    let _ = child.wait();
    Err(ToolError::new(error_codes::UPSTREAM_FAILED, message))
}

pub(super) struct DapClient {
    stdin: ChildStdin,
    responses: Receiver<Result<Value, DapTransportError>>,
    next_sequence: i64,
    pub(super) initialized: bool,
    pub(super) supports_configuration_done: bool,
    configuration_done_sent: bool,
    pub(super) observed_events: Vec<String>,
    pub(super) output: Vec<DebugOutputEvent>,
}

impl DapClient {
    pub(super) fn new(
        stdin: ChildStdin,
        responses: Receiver<Result<Value, DapTransportError>>,
    ) -> Self {
        Self {
            stdin,
            responses,
            next_sequence: 1,
            initialized: false,
            supports_configuration_done: false,
            configuration_done_sent: false,
            observed_events: Vec::new(),
            output: Vec::new(),
        }
    }

    pub(super) fn request(
        &mut self,
        command: &str,
        arguments: Value,
        timeout: Duration,
    ) -> Result<Value, ToolError> {
        let request_sequence = self.next_sequence;
        self.next_sequence += 1;
        write_message(
            &mut self.stdin,
            &json!({
                "seq": request_sequence,
                "type": "request",
                "command": command,
                "arguments": arguments
            }),
        )?;

        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ToolError::new(
                    error_codes::DEBUG_TIMEOUT,
                    format!("DAP timed out waiting for {command}"),
                ));
            }
            let message = self
                .responses
                .recv_timeout(remaining)
                .map_err(|error| match error {
                    mpsc::RecvTimeoutError::Timeout => ToolError::new(
                        error_codes::DEBUG_TIMEOUT,
                        format!("DAP timed out waiting for {command}"),
                    ),
                    mpsc::RecvTimeoutError::Disconnected => ToolError::new(
                        error_codes::UPSTREAM_FAILED,
                        "DAP adapter closed its output before responding",
                    ),
                })?
                .map_err(map_transport_error)?;

            let message_type = message.get("type").and_then(Value::as_str).ok_or_else(|| {
                ToolError::new(
                    error_codes::UPSTREAM_CONTRACT_INVALID,
                    "DAP message omitted type",
                )
            })?;
            match message_type {
                "event" => self.handle_event(&message)?,
                "request" => self.reject_reverse_request(&message)?,
                "response" => {
                    if message.get("request_seq").and_then(Value::as_i64) != Some(request_sequence)
                    {
                        continue;
                    }
                    if message.get("success").and_then(Value::as_bool) == Some(false) {
                        return Err(ToolError::new(
                            error_codes::UPSTREAM_FAILED,
                            message
                                .get("message")
                                .and_then(Value::as_str)
                                .unwrap_or("DAP request failed"),
                        ));
                    }
                    return Ok(message.get("body").cloned().unwrap_or(Value::Null));
                }
                other => {
                    return Err(ToolError::new(
                        error_codes::UPSTREAM_CONTRACT_INVALID,
                        format!("unsupported DAP message type: {other}"),
                    ));
                }
            }
        }
    }

    fn handle_event(&mut self, message: &Value) -> Result<(), ToolError> {
        let event = message
            .get("event")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ToolError::new(
                    error_codes::UPSTREAM_CONTRACT_INVALID,
                    "DAP event omitted event name",
                )
            })?;
        if self.observed_events.len() < MAX_OBSERVED_EVENTS {
            self.observed_events.push(event.into());
        }
        if event == "output"
            && self.output.len() < MAX_OUTPUT_EVENTS
            && let Some(body) = message.get("body")
        {
            let output = body
                .get("output")
                .and_then(Value::as_str)
                .unwrap_or_default();
            self.output.push(DebugOutputEvent {
                category: body
                    .get("category")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                output: truncate_string(output, MAX_OUTPUT_BYTES),
            });
        }
        if event == "initialized"
            && self.supports_configuration_done
            && !self.configuration_done_sent
        {
            let sequence = self.next_sequence;
            self.next_sequence += 1;
            write_message(
                &mut self.stdin,
                &json!({
                    "seq": sequence,
                    "type": "request",
                    "command": "configurationDone",
                    "arguments": {}
                }),
            )?;
            self.configuration_done_sent = true;
        }
        Ok(())
    }

    fn reject_reverse_request(&mut self, message: &Value) -> Result<(), ToolError> {
        let request_sequence = message.get("seq").and_then(Value::as_i64).ok_or_else(|| {
            ToolError::new(
                error_codes::UPSTREAM_CONTRACT_INVALID,
                "DAP reverse request omitted seq",
            )
        })?;
        let command = message
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        write_message(
            &mut self.stdin,
            &json!({
                "seq": sequence,
                "type": "response",
                "request_seq": request_sequence,
                "command": command,
                "success": false,
                "message": "OMC-RS does not support DAP reverse requests"
            }),
        )
    }

    pub(super) fn drain_events(&mut self, timeout: Duration) -> Result<(), ToolError> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(());
            }
            let message = match self.responses.recv_timeout(remaining) {
                Ok(message) => message.map_err(map_transport_error)?,
                Err(mpsc::RecvTimeoutError::Timeout) => return Ok(()),
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            };
            match message.get("type").and_then(Value::as_str) {
                Some("event") => self.handle_event(&message)?,
                Some("request") => self.reject_reverse_request(&message)?,
                Some("response") => {}
                Some(other) => {
                    return Err(ToolError::new(
                        error_codes::UPSTREAM_CONTRACT_INVALID,
                        format!("unsupported DAP message type: {other}"),
                    ));
                }
                None => {
                    return Err(ToolError::new(
                        error_codes::UPSTREAM_CONTRACT_INVALID,
                        "DAP message omitted type",
                    ));
                }
            }
        }
    }
}

fn truncate_string(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

fn write_message(stdin: &mut ChildStdin, message: &Value) -> Result<(), ToolError> {
    let body = serde_json::to_vec(message).map_err(|error| {
        ToolError::new(
            error_codes::UPSTREAM_CONTRACT_INVALID,
            format!("cannot encode DAP request: {error}"),
        )
    })?;
    write!(stdin, "Content-Length: {}\r\n\r\n", body.len()).map_err(|error| {
        ToolError::new(
            error_codes::UPSTREAM_FAILED,
            format!("cannot write DAP headers: {error}"),
        )
    })?;
    stdin.write_all(&body).map_err(|error| {
        ToolError::new(
            error_codes::UPSTREAM_FAILED,
            format!("cannot write DAP body: {error}"),
        )
    })?;
    stdin.flush().map_err(|error| {
        ToolError::new(
            error_codes::UPSTREAM_FAILED,
            format!("cannot flush DAP request: {error}"),
        )
    })
}

pub(super) fn spawn_reader(
    stdout: impl Read + Send + 'static,
) -> Receiver<Result<Value, DapTransportError>> {
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

pub(super) fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>, DapTransportError> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).map_err(DapTransportError::Io)?;
        if bytes == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(DapTransportError::InvalidHeader(line.trim().into()));
        };
        if name.eq_ignore_ascii_case("Content-Length") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| DapTransportError::InvalidHeader(line.trim().into()))?,
            );
        }
    }
    let length = content_length.ok_or(DapTransportError::MissingContentLength)?;
    if length > MAX_MESSAGE_BYTES {
        return Err(DapTransportError::ResponseTooLarge(length));
    }
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .map_err(DapTransportError::Io)?;
    serde_json::from_slice(&body).map_err(DapTransportError::Json)
}

fn map_transport_error(error: DapTransportError) -> ToolError {
    match error {
        DapTransportError::ResponseTooLarge(length) => ToolError::new(
            error_codes::UPSTREAM_RESPONSE_TOO_LARGE,
            format!("DAP response exceeds {MAX_MESSAGE_BYTES} bytes: {length}"),
        ),
        other => ToolError::new(error_codes::UPSTREAM_FAILED, other.to_string()),
    }
}

#[derive(Debug)]
pub(super) enum DapTransportError {
    Io(io::Error),
    Json(serde_json::Error),
    InvalidHeader(String),
    MissingContentLength,
    ResponseTooLarge(usize),
}

impl fmt::Display for DapTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "DAP I/O error: {error}"),
            Self::Json(error) => write!(formatter, "invalid DAP JSON response: {error}"),
            Self::InvalidHeader(header) => write!(formatter, "invalid DAP header: {header}"),
            Self::MissingContentLength => {
                formatter.write_str("DAP response omitted Content-Length")
            }
            Self::ResponseTooLarge(length) => write!(formatter, "DAP response too large: {length}"),
        }
    }
}
