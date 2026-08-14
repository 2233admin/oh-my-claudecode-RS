use super::*;

#[test]
fn timeout_has_a_bounded_contract() {
    let error = validate_timeout(Some(MIN_TIMEOUT_MS - 1)).unwrap_err();
    assert_eq!(error.code, error_codes::INVALID_REQUEST);
    assert!(validate_timeout(Some(MIN_TIMEOUT_MS)).is_ok());
    assert!(validate_timeout(Some(MAX_TIMEOUT_MS)).is_ok());
    assert!(validate_timeout(Some(MAX_TIMEOUT_MS + 1)).is_err());
}

#[test]
fn file_uri_preserves_windows_drive_and_escapes_spaces() {
    let uri = path_to_file_uri(Path::new(r"C:\work dir\src\lib.rs"));
    assert_eq!(uri, "file:///C:/work%20dir/src/lib.rs");
}

#[test]
fn file_uri_removes_windows_verbatim_prefix() {
    let uri = path_to_file_uri(Path::new(r"\\?\D:\work\src\lib.rs"));
    assert_eq!(uri, "file:///D:/work/src/lib.rs");
}

#[test]
fn oversized_lsp_message_is_rejected_before_allocation() {
    let input = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_BYTES + 1);
    let error = read_message(&mut input.as_bytes()).unwrap_err();
    assert!(matches!(error, LspTransportError::ResponseTooLarge(_)));
}
