//! Public conformance tests for repository-unknown runtime profiles.

use serde_json::{Value, json};
use std::fs;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after Unix epoch")
        .as_nanos();
    format!("{prefix}-{}-{nanos}", std::process::id())
}

fn run_omc(args: &[&str], project: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omc"))
        .args(args)
        .current_dir(project)
        .env("OMC_HOME", project.join("omc-home"))
        .output()
        .expect("compiled omc binary starts")
}

fn assert_success_json(output: Output, operation: &str) -> Value {
    assert!(
        output.status.success(),
        "{operation} failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("command emits a JSON envelope")
}

#[test]
fn randomized_unknown_profile_completes_public_cli_lifecycle() {
    let project = tempfile::tempdir().expect("temporary project");
    let runtime_id = unique_id("runtime");
    let provider_id = unique_id("provider");
    let model_id = unique_id("model");
    let profile_id = unique_id("profile");
    let profile_path = project.path().join("unknown-profile.json");
    let config_path = project.path().join("consumer-mcp.json");

    let profile = json!({
        "schemaVersion": "omc.profile.v1",
        "id": profile_id,
        "runtime": {
            "id": runtime_id,
            "command": env!("CARGO_BIN_EXE_omc"),
            "args": ["mcp"]
        },
        "provider": { "id": provider_id },
        "model": { "id": model_id, "capabilities": ["tool-calling"] },
        "protocol": { "kind": "mcp-stdio" },
        "permissions": {
            "omc": ["read", "process-spawn"],
            "runtime": ["read", "process-spawn"]
        },
        "setup": {
            "format": "json",
            "path": config_path,
            "registrationPath": ["mcpServers", profile_id]
        }
    });
    fs::write(
        &profile_path,
        serde_json::to_vec_pretty(&profile).expect("profile serializes"),
    )
    .expect("profile fixture writes");

    let profile_arg = profile_path.to_string_lossy();
    let validate = assert_success_json(
        run_omc(
            &["profile", "validate", "--profile", &profile_arg, "--json"],
            project.path(),
        ),
        "validate",
    );
    assert_eq!(validate["data"]["profile"]["id"], profile["id"]);

    let probe = assert_success_json(
        run_omc(
            &["probe", "--profile", &profile_arg, "--json"],
            project.path(),
        ),
        "probe",
    );
    assert_eq!(probe["data"]["ready"], true);

    let setup = assert_success_json(
        run_omc(
            &["setup", "--profile", &profile_arg, "--json"],
            project.path(),
        ),
        "setup",
    );
    assert_eq!(setup["data"]["changed"], true);

    let doctor = assert_success_json(
        run_omc(
            &["doctor", "--profile", &profile_arg, "--json"],
            project.path(),
        ),
        "doctor",
    );
    assert_eq!(doctor["data"]["ready"], true);
}
