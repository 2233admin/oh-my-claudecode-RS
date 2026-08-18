//! omc-shared: Shared types, config, and state for oh-my-claudecode-RS

pub mod agent_tool;
pub mod capability_catalog;
pub mod code_intel;
pub mod config;
pub mod context_strategy;
pub mod dap_adapter;
pub mod events;
pub mod goal_contract;
pub mod hash_edit;
pub mod lsp_adapter;
pub mod memory;
pub mod operation_contract;
pub mod paths;
pub mod prelude;
pub mod profile;
pub mod profile_negotiation;
pub mod profile_schema;
pub mod resilience;
pub mod routing;
pub mod session_pool;
pub mod shared_memory;
pub mod state;
pub mod team_contract;
pub mod tools;
pub mod types;

pub use agent_tool::PYTHON_SCHEMA_VERSION;
pub use agent_tool::workflow_contract;
pub use agent_tool::workflow_contract::{
    WORKFLOW_SCHEMA_VERSION, WorkflowAdvancePayload, WorkflowAdvanceRequest, WorkflowContext,
    WorkflowDecision, WorkflowStage, advance_workflow,
};
pub use config::{Config, ConfigError, OmcPaths};
pub use goal_contract::{GOAL_SCHEMA_VERSION, GoalCheckpoint, GoalRecord, GoalStatus};
pub use hash_edit::{HASH_EDIT_SCHEMA_VERSION, HashEdit, HashEditResult, LineAnchor};
pub use operation_contract::{
    ARTIFACT_REF_SCHEMA_VERSION, ARTIFACT_URI_PREFIX, EVENT_SCHEMA_VERSION,
    SUBAGENT_RESULT_SCHEMA_VERSION, TASK_SCHEMA_VERSION,
};
pub use profile::{
    DOCTOR_SCHEMA_VERSION, PROBE_SCHEMA_VERSION, PROFILE_SCHEMA_VERSION, SETUP_SCHEMA_VERSION,
    VALIDATION_SCHEMA_VERSION,
};
pub use shared_memory::{MemoryEntry, SharedMemory, SharedMemoryError};
pub use state::{
    AppState, ContextSample, GoalLedger, HudState, SessionInfo, SessionState, StateError,
    StateReader, StateWriter, TeamRunRecord,
};
pub use team_contract::{TEAM_OBSERVABILITY_SCHEMA_VERSION, TeamObservabilityPayload};
