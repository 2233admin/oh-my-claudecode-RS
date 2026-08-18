use std::path::Path;

use omc_shared::capability_catalog::{AvailabilityStatus, capabilities};
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
        && interop.ok;

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
        goals,
        team,
        interop,
    }
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
        ] {
            println!("{name}: {}", if probe.ok { "ready" } else { "issues" });
        }
    }
    Ok(())
}
