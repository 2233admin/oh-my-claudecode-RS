use std::path::Path;

use omc_host::profile_lifecycle::{
    ProfileRef, ResolutionContext, resolve_profile, validate_profile,
};
use omc_shared::capability_catalog::{AvailabilityStatus, capabilities};
use omc_shared::profile::OmcPermission;
use omc_shared::{GoalLedger, GoalStatus, OmcPaths};
use serde::Serialize;
use serde_json::Value;

use super::{DispatchError, collect_doctor_reports};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StatusReport {
    pub schema_version: &'static str,
    pub overall: &'static str,
    pub project_root: String,
    pub capabilities: CapabilitySummary,
    pub hosts: Vec<omc_host::HostDoctorReport>,
    pub runtime_profile: Probe,
    pub catalog: Probe,
    pub dependencies: Probe,
    pub goals: Probe,
    pub team: Probe,
    pub interop: Probe,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CapabilitySummary {
    pub total: usize,
    pub mcp_tools: usize,
    pub available: usize,
    pub conditional: usize,
    pub unavailable: usize,
}

#[derive(Debug, Serialize)]
pub(super) struct Probe {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Probe {
    fn data<T: Serialize>(value: T) -> Self {
        match serde_json::to_value(value) {
            Ok(data) => Self {
                ok: true,
                data: Some(data),
                error: None,
            },
            Err(error) => Self {
                ok: false,
                data: None,
                error: Some(error.to_string()),
            },
        }
    }

    fn result<T: Serialize, E: ToString>(result: Result<T, E>) -> Self {
        match result {
            Ok(value) => match serde_json::to_value(value) {
                Ok(data) => Self {
                    ok: true,
                    data: Some(data),
                    error: None,
                },
                Err(error) => Self {
                    ok: false,
                    data: None,
                    error: Some(error.to_string()),
                },
            },
            Err(error) => Self {
                ok: false,
                data: None,
                error: Some(error.to_string()),
            },
        }
    }
}

pub(super) fn build_status(root: &Path) -> StatusReport {
    let active_profile = std::env::var_os("OMC_PROFILE").map(std::path::PathBuf::from);
    build_status_with_profile(root, active_profile.as_deref())
}

pub(super) fn build_status_with_profile(
    root: &Path,
    active_profile: Option<&Path>,
) -> StatusReport {
    let catalog = capabilities();
    let available = catalog
        .iter()
        .filter(|item| item.availability.status == AvailabilityStatus::Available)
        .count();
    let conditional = catalog
        .iter()
        .filter(|item| item.availability.status == AvailabilityStatus::Conditional)
        .count();
    let unavailable = catalog.len() - available - conditional;
    let hosts = collect_doctor_reports(root, None).unwrap_or_default();
    let runtime_profile = profile_status(root, active_profile);
    let (catalog_probe, dependency_probe) = catalog_status(root);
    let goals = match GoalLedger::new(OmcPaths::new_with_root(root.join(".omc"))).list() {
        Ok(goals) => Probe::data(serde_json::json!({
            "total": goals.len(),
            "active": goals.iter().filter(|goal| goal.status == GoalStatus::Active).count(),
            "blocked": goals.iter().filter(|goal| goal.status == GoalStatus::Blocked).count(),
        })),
        Err(error) => Probe::result::<Value, _>(Err(error)),
    };
    let team = match omc_team::team_observability(root, "doctor") {
        Ok(payload) => {
            let ok = payload
                .data
                .get("ok")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Probe {
                ok,
                data: Some(payload.data),
                error: None,
            }
        }
        Err(error) => Probe::result::<Value, _>(Err(error)),
    };
    let interop = match omc_interop::read_snapshot(&root.to_string_lossy(), Some(10)) {
        Ok(snapshot) => Probe::data(serde_json::json!({
            "mode": snapshot.interop_mode,
            "directWriteEnabled": snapshot.direct_write_enabled,
            "taskCount": snapshot.normalized_task_count,
            "messageCount": snapshot.shared_message_count,
            "omxTeamCount": snapshot.omx_teams.len(),
        })),
        Err(error) => Probe::result::<Value, _>(Err(error)),
    };
    let healthy = unavailable == 0
        && hosts.iter().all(|host| host.ready)
        && goals.ok
        && team.ok
        && interop.ok
        && catalog_probe.ok
        && dependency_probe.ok;

    StatusReport {
        schema_version: "omc.status.v1",
        overall: if healthy { "ready" } else { "degraded" },
        project_root: root.display().to_string(),
        capabilities: CapabilitySummary {
            total: catalog.len(),
            mcp_tools: catalog.iter().map(|item| item.mcp_tools.len()).sum(),
            available,
            conditional,
            unavailable,
        },
        hosts,
        runtime_profile,
        catalog: catalog_probe,
        dependencies: dependency_probe,
        goals,
        team,
        interop,
    }
}

fn catalog_status(root: &Path) -> (Probe, Probe) {
    let catalog_root = std::env::var_os("OMC_CATALOG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join(".omc/catalogs"));
    match omc_host::catalog::CatalogManager::new(catalog_root).active() {
        Ok((catalog, status)) => {
            let dependency_data = catalog
                .dependencies
                .iter()
                .map(omc_host::catalog::dependency_evidence)
                .collect::<Vec<_>>();
            (Probe::data(status), Probe::data(dependency_data))
        }
        Err(error) => {
            let message = error.to_string();
            let guidance = serde_json::json!({
                "state": "not-ready",
                "repair": "Run `omc catalog status`; restore the bundled baseline or use `omc catalog rollback`. Catalog metadata never installs executables automatically."
            });
            (
                Probe {
                    ok: false,
                    data: Some(guidance.clone()),
                    error: Some(message.clone()),
                },
                Probe {
                    ok: false,
                    data: Some(guidance),
                    error: Some(message),
                },
            )
        }
    }
}

fn profile_status(root: &Path, active_profile: Option<&Path>) -> Probe {
    let Some(path) = active_profile else {
        return Probe::data(serde_json::json!({
            "state": "unconfigured",
            "repair": "Set OMC_PROFILE to a profile file, then run `omc profile validate --profile <path>`"
        }));
    };
    let context = ResolutionContext {
        project_root: root.to_path_buf(),
        user_home: OmcPaths::new().home,
        organization_catalog: Some(
            std::env::var_os("OMC_ORG_PROFILE_DIR")
                .or_else(|| std::env::var_os("OMC_CATALOG_HOME"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| root.join(".omc/catalogs")),
        ),
        built_ins: omc_host::builtin_profiles::bundled_profiles(
            &omc_host::mcp_reg::resolve_hermes_home(None),
        ),
    };
    let resolved = match resolve_profile(&ProfileRef::Explicit(path.to_path_buf()), &context) {
        Ok(resolved) => resolved,
        Err(error) => return Probe::result::<Value, _>(Err(error)),
    };
    let validation = validate_profile(resolved.clone());
    if !validation.valid {
        return Probe {
            ok: false,
            data: serde_json::to_value(validation).ok(),
            error: Some("active profile is invalid; run `omc profile validate`".into()),
        };
    }
    let omc_permissions = resolved
        .profile
        .permissions
        .omc
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<OmcPermission>>();
    let runtime_permissions = resolved
        .profile
        .permissions
        .runtime
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<OmcPermission>>();
    let effective_permissions = omc_permissions
        .intersection(&runtime_permissions)
        .copied()
        .collect::<Vec<_>>();
    let evidence_source = if resolved.provenance.catalog_version.is_some() {
        "cataloged"
    } else {
        "declared"
    };
    Probe::data(serde_json::json!({
        "state": "resolved",
        "activeProfile": resolved.profile.id,
        "runtime": resolved.profile.runtime.id,
        "providerEvidence": {"id": resolved.profile.provider.id, "source": evidence_source},
        "modelEvidence": {"id": resolved.profile.model.id, "capabilities": resolved.profile.model.capabilities, "source": evidence_source},
        "protocolEvidence": {"protocol": resolved.profile.protocol, "source": evidence_source},
        "effectivePermissions": effective_permissions,
        "dependencies": resolved.profile.dependencies,
        "catalogVersion": resolved.provenance.catalog_version,
        "provenance": resolved.provenance,
    }))
}

pub(super) fn run_status(root: &Path, json: bool) -> Result<(), DispatchError> {
    let status = build_status(root);
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("OMC Status [{}]", status.overall.to_uppercase());
        println!("Project: {}", status.project_root);
        println!(
            "Capabilities: {}/{} available, {} conditional, {} unavailable ({} MCP tools)",
            status.capabilities.available,
            status.capabilities.total,
            status.capabilities.conditional,
            status.capabilities.unavailable,
            status.capabilities.mcp_tools
        );
        if let Some(data) = &status.runtime_profile.data {
            println!(
                "Runtime profile: {}",
                data.get("activeProfile")
                    .or_else(|| data.get("state"))
                    .and_then(Value::as_str)
                    .unwrap_or("issues")
            );
        }
        println!(
            "Hosts: {}",
            status
                .hosts
                .iter()
                .map(|host| format!(
                    "{}={}",
                    host.host,
                    if host.ready { "ready" } else { "issues" }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
        for (name, probe) in [
            ("Goals", &status.goals),
            ("Team", &status.team),
            ("Interop", &status.interop),
            ("Catalog", &status.catalog),
            ("Dependencies", &status.dependencies),
        ] {
            println!("{name}: {}", if probe.ok { "ready" } else { "issues" });
        }
    }
    Ok(())
}
