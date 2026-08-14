//! Stable, host-neutral contract for the OMC-RS agent tool surface.
//!
//! This module deliberately exposes routing semantics rather than provider
//! model IDs. Hermes, Sentinel, and other hosts can consume the same result
//! without depending on OMC's internal provider configuration.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::routing::{ComplexityTier, ModelType, RoutingConfig, RoutingContext, route_task};

pub const TOOL_SCHEMA_VERSION: &str = "omc.tool.v1";
pub const PYTHON_SCHEMA_VERSION: &str = "omc.python.v1";
pub const DEBUG_SCHEMA_VERSION: &str = crate::dap_adapter::DEBUG_SCHEMA_VERSION;
pub const TEAM_OBSERVABILITY_SCHEMA_VERSION: &str =
    crate::team_contract::TEAM_OBSERVABILITY_SCHEMA_VERSION;
pub const INTEROP_SNAPSHOT_SCHEMA_VERSION: &str = "omc.interop.snapshot.v1";
pub const INTEROP_BRIDGE_SCHEMA_VERSION: &str = "omc.interop.bridge.v1";

/// Host-neutral decision contract for clarify/plan/execute/verify workflows.
/// Hosts and the existing team runtime provide evidence; this module never
/// starts an agent or persists lifecycle state.
#[path = "workflow_contract.rs"]
pub mod workflow_contract;

pub mod error_codes {
    pub const INVALID_REQUEST: &str = "invalid_request";
    pub const ADAPTER_UNAVAILABLE: &str = "adapter_unavailable";
    pub const UPSTREAM_FAILED: &str = "upstream_failed";
    pub const UPSTREAM_CONTRACT_INVALID: &str = "upstream_contract_invalid";
    pub const UPSTREAM_RESPONSE_TOO_LARGE: &str = "upstream_response_too_large";
    pub const UPSTREAM_QUERY_TRUNCATED: &str = "upstream_query_truncated";
    pub const ARTIFACT_NOT_FOUND: &str = "artifact_not_found";
    pub const STALE_EDIT: &str = "stale_edit";
    pub const EDIT_FAILED: &str = "edit_failed";
    pub const SIDE_EFFECTS_NOT_ALLOWED: &str = "side_effects_not_allowed";
    pub const DEBUG_TIMEOUT: &str = "debug_timeout";
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl ToolError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResponse<T> {
    pub schema_version: String,
    pub request_id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ToolError>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: BTreeMap<String, Value>,
}

impl<T> ToolResponse<T> {
    pub fn success(request_id: impl Into<String>, data: T) -> Self {
        Self {
            schema_version: TOOL_SCHEMA_VERSION.to_string(),
            request_id: request_id.into(),
            ok: true,
            data: Some(data),
            error: None,
            meta: BTreeMap::new(),
        }
    }

    pub fn failure(request_id: impl Into<String>, error: ToolError) -> Self {
        Self {
            schema_version: TOOL_SCHEMA_VERSION.to_string(),
            request_id: request_id.into(),
            ok: false,
            data: None,
            error: Some(error),
            meta: BTreeMap::new(),
        }
    }
}

pub fn normalize_request_id(request_id: Option<&str>, fallback: &str) -> String {
    request_id
        .filter(|value| !value.trim().is_empty())
        .map_or_else(|| fallback.to_string(), ToString::to_string)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Capability {
    pub name: String,
    pub description: String,
    pub kind: String,
    pub side_effects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilitiesPayload {
    pub product: String,
    pub protocol: String,
    pub contracts: Vec<String>,
    pub capabilities: Vec<Capability>,
}

pub fn capabilities_payload() -> CapabilitiesPayload {
    CapabilitiesPayload {
        product: "omc-rs".into(),
        protocol: TOOL_SCHEMA_VERSION.into(),
        contracts: vec![
            TOOL_SCHEMA_VERSION.into(),
            crate::operation_contract::TASK_SCHEMA_VERSION.into(),
            crate::operation_contract::EVENT_SCHEMA_VERSION.into(),
            crate::operation_contract::ARTIFACT_REF_SCHEMA_VERSION.into(),
            crate::goal_contract::GOAL_SCHEMA_VERSION.into(),
            crate::operation_contract::SUBAGENT_RESULT_SCHEMA_VERSION.into(),
            crate::hash_edit::HASH_EDIT_SCHEMA_VERSION.into(),
            crate::workflow_contract::WORKFLOW_SCHEMA_VERSION.into(),
            PYTHON_SCHEMA_VERSION.into(),
            DEBUG_SCHEMA_VERSION.into(),
            TEAM_OBSERVABILITY_SCHEMA_VERSION.into(),
            INTEROP_SNAPSHOT_SCHEMA_VERSION.into(),
            INTEROP_BRIDGE_SCHEMA_VERSION.into(),
        ],
        capabilities: vec![
            Capability {
                name: "agent_capabilities".into(),
                description: "List the stable OMC-RS capability contract.".into(),
                kind: "discovery".into(),
                side_effects: vec![],
            },
            Capability {
                name: "agent_route".into(),
                description: "Route a task by complexity without exposing provider model IDs."
                    .into(),
                kind: "routing".into(),
                side_effects: vec![],
            },
            Capability {
                name: "code_intel_artifact_query".into(),
                description:
                    "Read committed Code Intel artifacts through its released query surface; results include normalized artifact provenance and canonical URIs, with bounded exact inspection by artifact URI.".into(),
                kind: "repository_intelligence".into(),
                side_effects: vec![],
            },
            Capability {
                name: "lsp_document_symbols".into(),
                description: "Read Rust document symbols through a bounded one-shot rust-analyzer adapter.".into(),
                kind: "language_intelligence".into(),
                side_effects: vec![],
            },
            Capability {
                name: "python_repl".into(),
                description: "Execute explicit Python cells in a process-local persistent session; the host must opt into code side effects.".into(),
                kind: "code_execution".into(),
                side_effects: vec![
                    "executes local Python code".into(),
                    "may read/write files, spawn processes, or use network according to the code".into(),
                ],
            },
            Capability {
                name: "debug_inspect".into(),
                description: "Inspect an explicit launch or attach session through an external stdio DAP adapter; only read-only DAP actions are exposed.".into(),
                kind: "debugging".into(),
                side_effects: vec![
                    "starts an external DAP adapter process".into(),
                    "launches or attaches a debuggee when the host opts into side effects".into(),
                ],
            },
            Capability {
                name: "team_observability".into(),
                description: "Read sessions, aggregate usage, or health from the existing omc-team runtime without starting or mutating a team.".into(),
                kind: "orchestration_observability".into(),
                side_effects: vec![],
            },
            Capability {
                name: "interop_snapshot".into(),
                description: "Read a bounded, host-neutral snapshot of existing OMC/OMX interop state without starting or mutating either runtime.".into(),
                kind: "orchestration_interop".into(),
                side_effects: vec![],
            },
            Capability {
                name: "interop_bridge".into(),
                description: "Send one explicit OMC/OMX task or message through a gated durable bridge; it never starts workers or owns a task lifecycle.".into(),
                kind: "orchestration_interop".into(),
                side_effects: vec!["writes a task or message record when active interop flags and allowSideEffects=true are both present".into()],
            },
            Capability {
                name: "workflow_advance".into(),
                description: "Advance a host-neutral clarify-plan-execute-verify workflow only when supplied evidence permits it.".into(),
                kind: "orchestration".into(),
                side_effects: vec![],
            },
            Capability {
                name: "subagent_result_validate".into(),
                description: "Validate a typed subagent result against a small named schema.".into(),
                kind: "result_validation".into(),
                side_effects: vec![],
            },
            Capability {
                name: "hash_edit".into(),
                description: "Apply a contiguous line edit only when every supplied SHA-256 anchor matches.".into(),
                kind: "source_edit".into(),
                side_effects: vec!["writes one validated project file atomically".into()],
            },
            Capability {
                name: "state_*".into(),
                description: "Read and write OMC mode state through the existing state tools."
                    .into(),
                kind: "state".into(),
                side_effects: vec!["writes .omc/state when using state_write".into()],
            },
            Capability {
                name: "goal_*".into(),
                description: "Create, resume, checkpoint, block, and complete project goals through the durable OMC ledger.".into(),
                kind: "goal_state".into(),
                side_effects: vec!["writes .omc/state/goals when using goal tools".into()],
            },
            Capability {
                name: "project_memory_*".into(),
                description: "Read and write project memory through the existing memory tools."
                    .into(),
                kind: "memory".into(),
                side_effects: vec!["writes .omc/project-memory.json when using write tools".into()],
            },
            Capability {
                name: "notepad_*".into(),
                description: "Read and write the shared OMC notepad through existing tools.".into(),
                kind: "memory".into(),
                side_effects: vec!["writes .omc/notepad.md when using write tools".into()],
            },
        ],
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RouteRequest {
    pub task: String,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub previous_failures: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RoutePayload {
    pub tier: ComplexityTier,
    pub model_role: ModelType,
    pub recommended_surface: String,
    pub confidence: f64,
    pub reasons: Vec<String>,
    pub escalated: bool,
}

pub fn route_agent_task(request: &RouteRequest) -> RoutePayload {
    let context = RoutingContext {
        task_prompt: request.task.clone(),
        agent_type: request.agent_type.clone(),
        previous_failures: request.previous_failures,
        ..Default::default()
    };
    let decision = route_task(&context, &RoutingConfig::default());
    let recommended_surface = match decision.tier {
        ComplexityTier::Low => "caller-native",
        ComplexityTier::Medium => "caller-native-or-single-agent",
        ComplexityTier::High => "omc-team",
    };

    RoutePayload {
        tier: decision.tier,
        model_role: decision.model_type,
        recommended_surface: recommended_surface.into(),
        confidence: decision.confidence,
        reasons: decision.reasons,
        escalated: decision.escalated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_catalog_is_host_neutral() {
        let payload = capabilities_payload();
        assert_eq!(payload.product, "omc-rs");
        assert!(payload.contracts.iter().any(|item| item == "omc.task.v1"));
        assert!(payload.contracts.iter().any(|item| item == "omc.goal.v1"));
        assert!(
            payload
                .contracts
                .iter()
                .any(|item| item == PYTHON_SCHEMA_VERSION)
        );
        assert!(
            payload
                .contracts
                .iter()
                .any(|item| item == "omc.workflow.v1")
        );
        assert!(
            payload
                .capabilities
                .iter()
                .any(|item| item.name == "agent_route")
        );
        assert!(
            payload
                .capabilities
                .iter()
                .any(|item| item.name == "code_intel_artifact_query")
        );
        assert!(
            payload
                .capabilities
                .iter()
                .any(|item| item.name == "lsp_document_symbols")
        );
        assert!(
            payload
                .contracts
                .iter()
                .any(|item| item == INTEROP_SNAPSHOT_SCHEMA_VERSION)
        );
        assert!(
            payload
                .capabilities
                .iter()
                .any(|item| item.name == "interop_snapshot")
        );
        assert!(
            payload
                .contracts
                .iter()
                .any(|item| item == INTEROP_BRIDGE_SCHEMA_VERSION)
        );
        assert!(
            payload
                .capabilities
                .iter()
                .any(|item| item.name == "interop_bridge")
        );
        assert!(
            payload
                .capabilities
                .iter()
                .any(|item| item.name == "workflow_advance")
        );
        assert!(
            payload
                .capabilities
                .iter()
                .any(|item| item.name == "python_repl")
        );
        assert!(
            payload
                .capabilities
                .iter()
                .any(|item| item.name == "goal_*")
        );
        assert!(
            !serde_json::to_string(&payload)
                .unwrap()
                .contains("claude-sonnet")
        );
    }

    #[test]
    fn simple_task_stays_with_caller() {
        let result = route_agent_task(&RouteRequest {
            task: "find the config file".into(),
            agent_type: None,
            previous_failures: None,
        });
        assert_eq!(result.tier, ComplexityTier::Low);
        assert_eq!(result.recommended_surface, "caller-native");
    }

    #[test]
    fn architecture_task_recommends_team_surface() {
        let result = route_agent_task(&RouteRequest {
            task: "redesign the system architecture across the repository".into(),
            agent_type: Some("architect".into()),
            previous_failures: Some(1),
        });
        assert_eq!(result.tier, ComplexityTier::High);
        assert_eq!(result.recommended_surface, "omc-team");
    }

    #[test]
    fn response_has_stable_error_shape() {
        let response: ToolResponse<Value> = ToolResponse::failure(
            "req-1",
            ToolError::new("invalid_request", "task is required"),
        );
        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["schema_version"], TOOL_SCHEMA_VERSION);
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "invalid_request");
    }
}
