use super::*;
use std::io::Write as _;
use std::net::TcpListener;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tempfile::tempdir;

fn signed(catalog: &Catalog, key: &str) -> SignedCatalog {
    let value = serde_json::to_value(catalog).unwrap();
    let digest = sha256_hex(&serde_json::to_vec(&value).unwrap());
    SignedCatalog {
        signature: keyed_signature(key, &digest).unwrap(),
        digest,
        signature_algorithm: "hmac-sha256-v1".into(),
        catalog: value,
    }
}

#[test]
fn offline_baseline_contains_all_metadata_dimensions() {
    let catalog = CatalogManager::bundled().unwrap();
    assert!(
        !catalog.profiles.is_empty()
            && !catalog.providers.is_empty()
            && !catalog.models.is_empty()
            && !catalog.dependencies.is_empty()
    );
}

#[test]
fn corrupt_or_untrusted_refresh_preserves_active_catalog() {
    let dir = tempdir().unwrap();
    let manager = CatalogManager::new(dir.path());
    let source = dir.path().join("source.json");
    fs::write(&source, "not-json").unwrap();
    assert!(manager.refresh_file(&source, &source, "key").is_err());
    assert_eq!(manager.active().unwrap().1.source, "bundled");
}

#[test]
fn digest_mismatch_and_incompatible_range_are_rejected() {
    let dir = tempdir().unwrap();
    let manager = CatalogManager::new(dir.path().join("state"));
    let source = dir.path().join("source.json");
    let mut catalog = CatalogManager::bundled().unwrap();
    let mut envelope = signed(&catalog, "key");
    envelope.digest = "bad".into();
    fs::write(&source, serde_json::to_vec(&envelope).unwrap()).unwrap();
    assert!(matches!(
        manager.refresh_file(&source, &source, "key"),
        Err(CatalogError::Integrity(_))
    ));
    catalog.compatible_omc.min_inclusive = "99.0.0".into();
    fs::write(
        &source,
        serde_json::to_vec(&signed(&catalog, "key")).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        manager.refresh_file(&source, &source, "key"),
        Err(CatalogError::Incompatible(_))
    ));
}

#[test]
fn activation_and_rollback_are_atomic_from_the_readers_view() {
    let dir = tempdir().unwrap();
    let manager = CatalogManager::new(dir.path().join("state"));
    let source = dir.path().join("source.json");
    let mut first = CatalogManager::bundled().unwrap();
    first.catalog_version = "1.1.0".into();
    fs::write(&source, serde_json::to_vec(&signed(&first, "key")).unwrap()).unwrap();
    manager.refresh_file(&source, &source, "key").unwrap();
    let mut second = first.clone();
    second.catalog_version = "1.2.0".into();
    fs::write(
        &source,
        serde_json::to_vec(&signed(&second, "key")).unwrap(),
    )
    .unwrap();
    manager.refresh_file(&source, &source, "key").unwrap();
    assert_eq!(manager.rollback().unwrap().catalog_version, "1.1.0");
}

#[test]
fn interrupted_or_corrupt_active_pointer_recovers_previous() {
    let dir = tempdir().unwrap();
    let state = dir.path().join("state");
    let manager = CatalogManager::new(&state);
    let source = dir.path().join("source.json");
    let mut first = CatalogManager::bundled().unwrap();
    first.catalog_version = "1.1.0".into();
    fs::write(&source, serde_json::to_vec(&signed(&first, "key")).unwrap()).unwrap();
    manager.refresh_file(&source, &source, "key").unwrap();
    let mut second = first.clone();
    second.catalog_version = "1.2.0".into();
    fs::write(
        &source,
        serde_json::to_vec(&signed(&second, "key")).unwrap(),
    )
    .unwrap();
    manager.refresh_file(&source, &source, "key").unwrap();
    fs::write(state.join("active.json"), "interrupted").unwrap();
    assert_eq!(manager.active().unwrap().0.catalog_version, "1.1.0");
}

#[test]
fn missing_active_completes_valid_staged_activation() {
    let dir = tempdir().unwrap();
    let state = dir.path().join("state");
    fs::create_dir_all(&state).unwrap();
    let manager = CatalogManager::new(&state);
    let mut previous = CatalogManager::bundled().unwrap();
    previous.catalog_version = "1.1.0".into();
    let mut staged = previous.clone();
    staged.catalog_version = "1.2.0".into();
    fs::write(
        state.join("previous.json"),
        serde_json::to_vec_pretty(&previous).unwrap(),
    )
    .unwrap();
    fs::write(
        state.join("staged.json"),
        serde_json::to_vec_pretty(&staged).unwrap(),
    )
    .unwrap();

    let (recovered, status) = manager.active().unwrap();
    assert_eq!(recovered.catalog_version, "1.2.0");
    assert_eq!(status.source, "recovered-staged");
    assert!(state.join("active.json").is_file());
    assert!(!state.join("staged.json").exists());
}

#[test]
fn missing_active_falls_back_to_previous_when_staged_is_invalid() {
    let dir = tempdir().unwrap();
    let state = dir.path().join("state");
    fs::create_dir_all(&state).unwrap();
    let manager = CatalogManager::new(&state);
    let mut previous = CatalogManager::bundled().unwrap();
    previous.catalog_version = "1.1.0".into();
    fs::write(
        state.join("previous.json"),
        serde_json::to_vec_pretty(&previous).unwrap(),
    )
    .unwrap();
    fs::write(state.join("staged.json"), "interrupted").unwrap();

    let (recovered, status) = manager.active().unwrap();
    assert_eq!(recovered.catalog_version, "1.1.0");
    assert_eq!(status.source, "recovered-previous");
}

#[test]
fn executable_directives_and_same_major_removal_fail_closed() {
    let mut value: Value = serde_json::from_str(BUNDLED_CATALOG).unwrap();
    value["profiles"][0]["installScript"] = Value::String("curl | sh".into());
    assert!(serde_json::from_value::<Catalog>(value).is_err());
    let previous = CatalogManager::bundled().unwrap();
    let mut next = previous.clone();
    next.dependencies.clear();
    assert!(check_same_major_compatible(&previous, &next).is_err());

    let mut mismatched = previous;
    mismatched.profiles[0].profile =
        Some(crate::builtin_profiles::bundled_profiles(Path::new(".hermes"))["codex"].clone());
    assert!(matches!(
        validate_catalog(&mismatched, env!("CARGO_PKG_VERSION")),
        Err(CatalogError::Schema(_))
    ));
}

#[test]
fn untrusted_http_source_is_rejected_before_network_io() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let observed = requests.clone();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        if listener.accept().is_ok() {
            observed.fetch_add(1, Ordering::SeqCst);
        }
    });
    let dir = tempdir().unwrap();
    let manager = CatalogManager::new(dir.path());
    let uri = format!("http://{address}/private.json");
    assert!(matches!(
        manager.refresh_source(&uri, "key"),
        Err(CatalogError::UntrustedSource(_))
    ));
    handle.join().unwrap();
    assert_eq!(requests.load(Ordering::SeqCst), 0);
    assert_eq!(manager.active().unwrap().1.source, "bundled");
}

#[test]
fn registered_local_source_keeps_path_compatibility() {
    let dir = tempdir().unwrap();
    let manager = CatalogManager::new(dir.path().join("state"));
    let source = dir.path().join("catalog.json");
    let mut catalog = CatalogManager::bundled().unwrap();
    catalog.catalog_version = "1.1.0".into();
    fs::write(
        &source,
        serde_json::to_vec(&signed(&catalog, "key")).unwrap(),
    )
    .unwrap();
    manager.trust_source(source.to_str().unwrap()).unwrap();
    assert_eq!(
        manager
            .refresh_source(source.to_str().unwrap(), "key")
            .unwrap()
            .catalog_version,
        "1.1.0"
    );
}

#[test]
fn untrusted_missing_local_source_is_rejected_before_file_io() {
    let dir = tempdir().unwrap();
    let manager = CatalogManager::new(dir.path().join("state"));
    let source = dir.path().join("missing-catalog.json");
    assert!(matches!(
        manager.refresh_source(source.to_str().unwrap(), "key"),
        Err(CatalogError::UntrustedSource(_))
    ));
    assert_eq!(manager.active().unwrap().1.source, "bundled");
}

#[test]
fn trusted_local_source_can_be_registered_before_file_exists() {
    let dir = tempdir().unwrap();
    let manager = CatalogManager::new(dir.path().join("state"));
    let source = dir.path().join("catalog.json");
    manager.trust_source(source.to_str().unwrap()).unwrap();

    let mut catalog = CatalogManager::bundled().unwrap();
    catalog.catalog_version = "1.1.0".into();
    fs::write(
        &source,
        serde_json::to_vec(&signed(&catalog, "key")).unwrap(),
    )
    .unwrap();

    assert_eq!(
        manager
            .refresh_source(source.to_str().unwrap(), "key")
            .unwrap()
            .catalog_version,
        "1.1.0"
    );
}

#[test]
fn registered_http_source_downloads_verifies_and_activates() {
    let mut catalog = CatalogManager::bundled().unwrap();
    catalog.catalog_version = "1.1.0".into();
    let body = serde_json::to_vec(&signed(&catalog, "key")).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).unwrap();
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        ).unwrap();
        socket.write_all(&body).unwrap();
    });
    let dir = tempdir().unwrap();
    let manager = CatalogManager::new(dir.path());
    let uri = format!("http://{address}/catalog.json");
    manager.trust_source(&uri).unwrap();
    let status = manager.refresh_source(&uri, "key").unwrap();
    handle.join().unwrap();
    assert_eq!(status.catalog_version, "1.1.0");
    assert_eq!(manager.active().unwrap().0.catalog_version, "1.1.0");
}
