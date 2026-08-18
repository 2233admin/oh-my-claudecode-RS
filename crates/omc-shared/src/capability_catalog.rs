//! Single source of truth for the stable OMC-RS capability surface.

use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::agent_tool::Capability;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityStatus {
    Available,
    Unavailable,
    Conditional,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DependencyAvailability {
    pub name: String,
    pub resolved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityAvailability {
    pub status: AvailabilityStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<DependencyAvailability>,
}

struct Descriptor {
    name: &'static str,
    description: &'static str,
    kind: &'static str,
    side_effects: &'static [&'static str],
    mcp_tools: &'static [&'static str],
    dependency: Dependency,
}

#[derive(Clone, Copy)]
enum Dependency {
    None,
    Command(&'static str),
    EnvCommand {
        variable: &'static str,
        fallback: &'static str,
    },
    EnvFlags(&'static [&'static str]),
    Python,
    TeamBinary,
    CallerSupplied(&'static str),
}

const DESCRIPTORS: &[Descriptor] = &[
    descriptor(
        "agent_capabilities",
        "List the stable OMC-RS capability contract.",
        "discovery",
        &[],
        &["agent_capabilities"],
        Dependency::None,
    ),
    descriptor(
        "agent_route",
        "Route a task by complexity without exposing provider model IDs.",
        "routing",
        &[],
        &["agent_route"],
        Dependency::None,
    ),
    descriptor(
        "code_intel_artifact_query",
        "Read committed Code Intel artifacts through its released query surface.",
        "repository_intelligence",
        &[],
        &["code_intel_artifact_query"],
        Dependency::EnvCommand {
            variable: "OMC_CODE_INTEL_BIN",
            fallback: "code-intel",
        },
    ),
    descriptor(
        "lsp_document_symbols",
        "Read Rust document symbols through a bounded rust-analyzer adapter.",
        "language_intelligence",
        &[],
        &["lsp_document_symbols"],
        Dependency::Command("rust-analyzer"),
    ),
    descriptor(
        "python_repl",
        "Execute explicit Python cells in a process-local persistent session.",
        "code_execution",
        &[
            "executes local Python code",
            "may read/write files, spawn processes, or use network according to the code",
        ],
        &["python_repl"],
        Dependency::Python,
    ),
    descriptor(
        "debug_inspect",
        "Inspect an explicit launch or attach session through an external stdio DAP adapter.",
        "debugging",
        &[
            "starts an external DAP adapter process",
            "launches or attaches a debuggee when the host opts into side effects",
        ],
        &["debug_inspect"],
        Dependency::CallerSupplied("DAP adapter command"),
    ),
    descriptor(
        "team_observability",
        "Read sessions, aggregate usage, or health from the existing omc-team runtime.",
        "orchestration_observability",
        &[],
        &["team_observability"],
        Dependency::TeamBinary,
    ),
    descriptor(
        "interop_snapshot",
        "Read a bounded, host-neutral snapshot of existing OMC/OMX interop state.",
        "orchestration_interop",
        &[],
        &["interop_snapshot"],
        Dependency::None,
    ),
    descriptor(
        "interop_bridge",
        "Send one explicit OMC/OMX task or message through a gated durable bridge.",
        "orchestration_interop",
        &[
            "writes a task or message record when active interop flags and allowSideEffects=true are both present",
        ],
        &["interop_bridge"],
        Dependency::EnvFlags(&[
            "OMX_OMC_INTEROP_MODE=active",
            "OMX_OMC_INTEROP_ENABLED=1",
            "OMC_INTEROP_TOOLS_ENABLED=1",
        ]),
    ),
    descriptor(
        "workflow_advance",
        "Advance a host-neutral clarify-plan-execute-verify workflow from supplied evidence.",
        "orchestration",
        &[],
        &["workflow_advance"],
        Dependency::None,
    ),
    descriptor(
        "subagent_result_validate",
        "Validate a typed subagent result against a small named schema.",
        "result_validation",
        &[],
        &["subagent_result_validate"],
        Dependency::None,
    ),
    descriptor(
        "hash_edit",
        "Apply a contiguous line edit only when every supplied SHA-256 anchor matches.",
        "source_edit",
        &["writes one validated project file atomically"],
        &["hash_edit"],
        Dependency::None,
    ),
    descriptor(
        "state_*",
        "Read and write OMC mode state through the existing state tools.",
        "state",
        &["writes .omc/state when using state_write"],
        &[
            "state_read",
            "state_write",
            "state_clear",
            "state_list_active",
            "state_get_status",
        ],
        Dependency::None,
    ),
    descriptor(
        "goal_*",
        "Create, resume, checkpoint, block, and complete project goals through the durable OMC ledger.",
        "goal_state",
        &["writes .omc/state/goals when using goal tools"],
        &[
            "goal_create",
            "goal_list",
            "goal_get",
            "goal_start",
            "goal_block",
            "goal_checkpoint",
            "goal_complete",
        ],
        Dependency::None,
    ),
    descriptor(
        "project_memory_*",
        "Read and write project memory through the existing memory tools.",
        "memory",
        &["writes .omc/project-memory.json when using write tools"],
        &[
            "project_memory_read",
            "project_memory_write",
            "project_memory_add_note",
            "project_memory_add_directive",
        ],
        Dependency::None,
    ),
    descriptor(
        "notepad_*",
        "Read and write the shared OMC notepad through existing tools.",
        "memory",
        &["writes .omc/notepad.md when using write tools"],
        &[
            "notepad_read",
            "notepad_write_priority",
            "notepad_write_working",
            "notepad_write_manual",
        ],
        Dependency::None,
    ),
];

const fn descriptor(
    name: &'static str,
    description: &'static str,
    kind: &'static str,
    side_effects: &'static [&'static str],
    mcp_tools: &'static [&'static str],
    dependency: Dependency,
) -> Descriptor {
    Descriptor {
        name,
        description,
        kind,
        side_effects,
        mcp_tools,
        dependency,
    }
}

pub fn capabilities() -> Vec<Capability> {
    static CATALOG: OnceLock<Vec<Capability>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            DESCRIPTORS
                .iter()
                .map(|item| Capability {
                    name: item.name.into(),
                    description: item.description.into(),
                    kind: item.kind.into(),
                    side_effects: item
                        .side_effects
                        .iter()
                        .map(|value| (*value).into())
                        .collect(),
                    mcp_tools: item.mcp_tools.iter().map(|value| (*value).into()).collect(),
                    availability: availability(item.dependency),
                })
                .collect()
        })
        .clone()
}

pub fn mcp_tool_names() -> Vec<&'static str> {
    DESCRIPTORS
        .iter()
        .flat_map(|item| item.mcp_tools.iter().copied())
        .collect()
}

fn availability(dependency: Dependency) -> CapabilityAvailability {
    match dependency {
        Dependency::None => ready(Vec::new()),
        Dependency::Command(command) => command_availability(command, &[command]),
        Dependency::EnvCommand { variable, fallback } => {
            let configured = env::var(variable).ok();
            command_availability(fallback, &[configured.as_deref().unwrap_or(fallback)])
        }
        Dependency::EnvFlags(flags) => {
            let dependencies = flags
                .iter()
                .map(|flag| {
                    let (name, expected) = flag.split_once('=').unwrap_or((flag, ""));
                    DependencyAvailability {
                        name: (*flag).into(),
                        resolved: env::var(name).is_ok_and(|value| value == expected),
                        path: None,
                    }
                })
                .collect::<Vec<_>>();
            if dependencies.iter().all(|item| item.resolved) {
                ready(dependencies)
            } else {
                CapabilityAvailability {
                    status: AvailabilityStatus::Conditional,
                    reason: Some("interop bridge activation flags are not all enabled".into()),
                    dependencies,
                }
            }
        }
        Dependency::Python => {
            let configured = env::var("OMC_PYTHON_COMMAND").ok();
            let candidates = configured.as_deref().map_or_else(
                || {
                    if cfg!(windows) {
                        vec!["python", "python3"]
                    } else {
                        vec!["python3", "python"]
                    }
                },
                |command| vec![command],
            );
            command_availability("python", &candidates)
        }
        Dependency::TeamBinary => {
            let adjacent = env::current_exe().ok().and_then(|path| {
                let name = if cfg!(windows) {
                    "omc-team.exe"
                } else {
                    "omc-team"
                };
                path.parent()
                    .map(|parent| parent.join(name))
                    .filter(|candidate| candidate.is_file())
            });
            adjacent.map_or_else(
                || command_availability("omc-team", &["omc-team"]),
                |path| {
                    ready(vec![DependencyAvailability {
                        name: "omc-team".into(),
                        resolved: true,
                        path: Some(path.display().to_string()),
                    }])
                },
            )
        }
        Dependency::CallerSupplied(name) => CapabilityAvailability {
            status: AvailabilityStatus::Conditional,
            reason: Some(format!("{name} is resolved per request")),
            dependencies: vec![DependencyAvailability {
                name: name.into(),
                resolved: false,
                path: None,
            }],
        },
    }
}

fn command_availability(name: &str, candidates: &[&str]) -> CapabilityAvailability {
    let resolved = candidates
        .iter()
        .find_map(|candidate| resolve_command(candidate));
    let dependency = DependencyAvailability {
        name: name.into(),
        resolved: resolved.is_some(),
        path: resolved.as_ref().map(|path| path.display().to_string()),
    };
    if resolved.is_some() {
        ready(vec![dependency])
    } else {
        CapabilityAvailability {
            status: AvailabilityStatus::Unavailable,
            reason: Some(format!("{name} was not found on PATH")),
            dependencies: vec![dependency],
        }
    }
}

fn ready(dependencies: Vec<DependencyAvailability>) -> CapabilityAvailability {
    CapabilityAvailability {
        status: AvailabilityStatus::Available,
        reason: None,
        dependencies,
    }
}

fn resolve_command(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.components().count() > 1 && path.is_file() {
        return Some(path.to_path_buf());
    }
    let suffixes: Vec<&str> = if cfg!(windows) {
        vec!["", ".exe", ".cmd", ".bat"]
    } else {
        vec![""]
    };
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .find_map(|dir| {
            suffixes
                .iter()
                .map(|suffix| dir.join(format!("{command}{suffix}")))
                .find(|candidate| candidate.is_file())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn catalog_has_sixteen_unique_capabilities_and_tools() {
        let capabilities = capabilities();
        assert_eq!(capabilities.len(), 16);
        assert_eq!(
            capabilities
                .iter()
                .map(|item| &item.name)
                .collect::<BTreeSet<_>>()
                .len(),
            16
        );
        let tools = mcp_tool_names();
        assert_eq!(tools.len(), 32);
        assert_eq!(tools.iter().collect::<BTreeSet<_>>().len(), 32);
    }
}
