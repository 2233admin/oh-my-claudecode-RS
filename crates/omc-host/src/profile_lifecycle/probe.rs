//! Bounded, read-only MCP stdio handshake.

use super::ResolvedProfile;
use omc_shared::profile::{
    CapabilityEvidence, ContractIssue, EvidenceSource, PROBE_SCHEMA_VERSION, ProbeReport,
    ProtocolDescriptor,
};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

pub fn probe_stdio(resolved: &ResolvedProfile, timeout: Duration) -> ProbeReport {
    if !matches!(resolved.profile.protocol, ProtocolDescriptor::McpStdio) {
        return failed(resolved, "unsupported_protocol", "profile is not MCP stdio");
    }
    let mut command = Command::new(&resolved.profile.runtime.command);
    command
        .args(&resolved.profile.runtime.args)
        .envs(&resolved.profile.runtime.environment)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return failed(resolved, "spawn_failed", error.to_string()),
    };
    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        return failed(resolved, "transport_failed", "runtime stdin unavailable");
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        return failed(resolved, "transport_failed", "runtime stdout unavailable");
    };
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut lines = BufReader::new(stdout).lines();
        let first = lines.next().transpose();
        let second = lines.next().transpose();
        let _ = sender.send((first, second));
    });
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"omc-profile-probe","version":"1"}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    ];
    for request in requests {
        if writeln!(stdin, "{request}")
            .and_then(|_| stdin.flush())
            .is_err()
        {
            let _ = child.kill();
            return failed(resolved, "transport_failed", "cannot write MCP request");
        }
    }
    let response = receiver.recv_timeout(timeout);
    let _ = child.kill();
    let _ = child.wait();
    match response {
        Ok((Ok(Some(initialize)), Ok(Some(tools)))) => evaluate(resolved, &initialize, &tools),
        Ok((Err(error), _)) | Ok((_, Err(error))) => {
            failed(resolved, "transport_failed", error.to_string())
        }
        Ok(_) => failed(
            resolved,
            "handshake_incomplete",
            "runtime exited before handshake",
        ),
        Err(_) => failed(resolved, "probe_timeout", "MCP handshake exceeded timeout"),
    }
}

fn evaluate(resolved: &ResolvedProfile, initialize: &str, tools: &str) -> ProbeReport {
    let init: Value = match serde_json::from_str(initialize) {
        Ok(value) => value,
        Err(error) => return failed(resolved, "invalid_response", error.to_string()),
    };
    let listed: Value = match serde_json::from_str(tools) {
        Ok(value) => value,
        Err(error) => return failed(resolved, "invalid_response", error.to_string()),
    };
    if init.get("result").is_none() || listed.get("result").is_none() {
        return failed(
            resolved,
            "handshake_rejected",
            "MCP response contains no result",
        );
    }
    let count = listed["result"]["tools"]
        .as_array()
        .map_or(0, std::vec::Vec::len);
    ProbeReport {
        schema_version: PROBE_SCHEMA_VERSION.into(),
        profile_id: resolved.profile.id.clone(),
        ready: true,
        provenance: resolved.provenance.clone(),
        evidence: vec![CapabilityEvidence {
            capability: "tool-calling".into(),
            available: true,
            source: EvidenceSource::Probed,
            detail: Some(format!(
                "MCP initialize succeeded; {count} tools discovered"
            )),
        }],
        issues: Vec::new(),
    }
}

fn failed(
    resolved: &ResolvedProfile,
    code: impl Into<String>,
    message: impl Into<String>,
) -> ProbeReport {
    ProbeReport {
        schema_version: PROBE_SCHEMA_VERSION.into(),
        profile_id: resolved.profile.id.clone(),
        ready: false,
        provenance: resolved.provenance.clone(),
        evidence: Vec::new(),
        issues: vec![ContractIssue {
            code: code.into(),
            path: "$.runtime".into(),
            message: message.into(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omc_shared::profile::{
        ModelDescriptor, PermissionPolicy, Profile, ProfileSource, ProtocolDescriptor, Provenance,
        ProviderDescriptor, RuntimeDescriptor,
    };
    use std::collections::BTreeMap;

    #[test]
    fn missing_runtime_is_typed_not_ready() {
        let resolved = ResolvedProfile {
            profile: Profile {
                schema_version: "omc.profile.v1".into(),
                id: "missing".into(),
                runtime: RuntimeDescriptor {
                    id: "missing".into(),
                    command: "definitely-no-such-omc-runtime".into(),
                    args: Vec::new(),
                    environment: BTreeMap::new(),
                },
                provider: ProviderDescriptor {
                    id: "p".into(),
                    endpoint: None,
                },
                model: ModelDescriptor {
                    id: "m".into(),
                    capabilities: Vec::new(),
                    probeable: false,
                },
                protocol: ProtocolDescriptor::McpStdio,
                permissions: PermissionPolicy::default(),
                dependencies: Vec::new(),
                setup: None,
                extensions: BTreeMap::new(),
            },
            provenance: Provenance {
                source: ProfileSource::Explicit,
                schema_version: "omc.profile.v1".into(),
                digest: "a".repeat(64),
                catalog_version: None,
                location: None,
            },
        };
        let report = probe_stdio(&resolved, Duration::from_millis(50));
        assert!(!report.ready);
        assert_eq!(report.issues[0].code, "spawn_failed");
    }
}
