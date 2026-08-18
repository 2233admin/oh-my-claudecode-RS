//! Universal diagnosis based on installed configuration and live evidence.

use super::{ResolvedProfile, probe_stdio};
use omc_shared::profile::{
    CapabilityEvidence, ContractIssue, DOCTOR_SCHEMA_VERSION, DoctorReport, EvidenceSource,
    RepairGuidance,
};
use std::path::Path;
use std::time::Duration;

pub fn doctor_profile(
    resolved: &ResolvedProfile,
    project_root: &Path,
    timeout: Duration,
) -> DoctorReport {
    let mut evidence = Vec::new();
    let mut issues = Vec::new();
    if let Some(setup) = &resolved.profile.setup {
        let destination = if setup.path.is_absolute() {
            setup.path.clone()
        } else {
            project_root.join(&setup.path)
        };
        let configured = destination.is_file();
        evidence.push(CapabilityEvidence {
            capability: "configured".into(),
            available: configured,
            source: EvidenceSource::Probed,
            detail: Some(destination.display().to_string()),
        });
        if !configured {
            issues.push(ContractIssue {
                code: "not_configured".into(),
                path: "$.setup.path".into(),
                message: format!("{} does not exist; run setup", destination.display()),
            });
        }
    }
    for dependency in &resolved.profile.dependencies {
        let selected = dependency
            .commands
            .iter()
            .find(|command| command_available(command));
        let available = selected.is_some()
            || matches!(
                dependency.kind,
                omc_shared::profile::DependencyKind::Optional
                    | omc_shared::profile::DependencyKind::CallerSupplied
            );
        evidence.push(CapabilityEvidence {
            capability: format!("dependency:{}", dependency.id),
            available,
            source: EvidenceSource::Probed,
            detail: selected
                .cloned()
                .or_else(|| dependency.version_requirement.clone()),
        });
        if !available {
            issues.push(ContractIssue {
                code: "dependency_missing".into(),
                path: format!("$.dependencies.{}", dependency.id),
                message: format!(
                    "no compatible command found: {}",
                    dependency.commands.join(", ")
                ),
            });
        }
    }
    let probe = probe_stdio(resolved, timeout);
    evidence.extend(probe.evidence);
    issues.extend(probe.issues);
    DoctorReport {
        schema_version: DOCTOR_SCHEMA_VERSION.into(),
        profile_id: resolved.profile.id.clone(),
        ready: issues.is_empty(),
        provenance: resolved.provenance.clone(),
        evidence,
        issues,
        repairs: repair_guidance(resolved, project_root),
    }
}

fn repair_guidance(resolved: &ResolvedProfile, project_root: &Path) -> Vec<RepairGuidance> {
    let mut repairs = Vec::new();
    if let Some(setup) = &resolved.profile.setup {
        let destination = if setup.path.is_absolute() {
            setup.path.clone()
        } else {
            project_root.join(&setup.path)
        };
        if !destination.is_file() {
            repairs.push(RepairGuidance {
                code: "not_configured".into(),
                summary: format!(
                    "Register profile '{}' in the consumer configuration",
                    resolved.profile.id
                ),
                command: resolved
                    .provenance
                    .location
                    .as_ref()
                    .map(|path| format!("omc setup --profile \"{}\"", path.display())),
                automatic: false,
            });
        }
    }
    for dependency in &resolved.profile.dependencies {
        if !dependency
            .commands
            .iter()
            .any(|command| command_available(command))
            && !matches!(
                dependency.kind,
                omc_shared::profile::DependencyKind::Optional
                    | omc_shared::profile::DependencyKind::CallerSupplied
            )
        {
            repairs.push(RepairGuidance {
                code: "dependency_missing".into(),
                summary: format!(
                    "Install '{}' from its trusted distribution channel, then verify it",
                    dependency.id
                ),
                command: dependency
                    .commands
                    .first()
                    .map(|command| format!("{command} --version")),
                automatic: false,
            });
        }
    }
    repairs
}

fn command_available(command: &str) -> bool {
    std::process::Command::new(command)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile_lifecycle::{ProfileRef, ResolutionContext, resolve_profile};
    use std::collections::BTreeMap;
    use std::fs;

    #[test]
    fn absent_config_and_runtime_are_actionable() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("profile.json");
        fs::write(&path, serde_json::to_vec(&serde_json::json!({
            "schemaVersion":"omc.profile.v1", "id":"broken",
            "runtime":{"id":"r","command":"definitely-missing-runtime"},
            "provider":{"id":"p"}, "model":{"id":"m"},
            "protocol":{"kind":"mcp-stdio"},
            "setup":{"format":"json","path":"consumer.json","registrationPath":["servers","broken"]}
        })).unwrap()).unwrap();
        let resolved = resolve_profile(
            &ProfileRef::Explicit(path),
            &ResolutionContext {
                project_root: root.path().into(),
                user_home: root.path().into(),
                organization_catalog: None,
                built_ins: BTreeMap::new(),
            },
        )
        .unwrap();
        let report = doctor_profile(&resolved, root.path(), Duration::from_millis(50));
        assert!(!report.ready);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "not_configured")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "spawn_failed")
        );
        assert!(report.repairs.iter().all(|repair| !repair.automatic));
        assert!(
            report
                .repairs
                .iter()
                .any(|repair| repair.code == "not_configured")
        );
    }
}
