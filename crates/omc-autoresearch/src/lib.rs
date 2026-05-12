//! omc-autoresearch: Autoresearch runtime, orchestrator, and PRD management
//!
//! This crate provides the core types and runtime for the autoresearch loop:
//! - `types` -- core domain types (queries, contracts, results, status enums)
//! - `runtime` -- run lifecycle, evaluation, candidate processing
//! - `orchestrator` -- iteration loop and coordination
//! - `prd` -- PRD loading, parsing, and validation

pub mod orchestrator;
pub mod prd;
pub mod runtime;
pub mod types;

pub use types::{
    AutoresearchError, CandidateArtifact, CandidateStatus, DecisionStatus, EvaluationRecord,
    KeepPolicy, MissionContract, MissionContractRef, ParsedSandboxContract, ResearchQuery,
    ResearchResult, RunManifest, RunStatus, SandboxEvaluator, SetupHandoff,
};
