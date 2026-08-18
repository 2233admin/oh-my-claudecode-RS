use super::*;

fn request() -> DebugInspectRequest {
    serde_json::from_value(json!({
        "adapterCommand": "dap-fixture",
        "mode": "launch",
        "action": "threads",
        "launchArguments": {"program": "target"},
        "allowSideEffects": true
    }))
    .expect("debug request parses")
}

#[test]
fn timeout_is_bounded() {
    assert!(
        validate_request(&DebugInspectRequest {
            timeout_ms: Some(MIN_TIMEOUT_MS - 1),
            ..request()
        })
        .is_err()
    );
    assert!(
        validate_request(&DebugInspectRequest {
            timeout_ms: Some(MIN_TIMEOUT_MS),
            ..request()
        })
        .is_ok()
    );
    assert!(
        validate_request(&DebugInspectRequest {
            timeout_ms: Some(MAX_TIMEOUT_MS + 1),
            ..request()
        })
        .is_err()
    );
}

#[test]
fn launch_requires_explicit_side_effect_opt_in_and_target() {
    let mut request = request();
    request.allow_side_effects = false;
    assert_eq!(
        validate_request(&request).unwrap_err().code,
        error_codes::SIDE_EFFECTS_NOT_ALLOWED
    );

    request.allow_side_effects = true;
    request.launch_arguments = None;
    assert!(selected_session_arguments(&request).is_err());
}

#[test]
fn dap_headers_are_bounded_and_utf8_json_is_decoded() {
    let body = br#"{"type":"event","event":"initialized"}"#;
    let input = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut bytes = input.into_bytes();
    bytes.extend_from_slice(body);
    let message = read_message(&mut bytes.as_slice())
        .expect("DAP framing parses")
        .expect("DAP message exists");
    assert_eq!(message["event"], "initialized");

    let oversized = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_BYTES + 1);
    assert!(matches!(
        read_message(&mut oversized.as_bytes()),
        Err(DapTransportError::ResponseTooLarge(_))
    ));
}
