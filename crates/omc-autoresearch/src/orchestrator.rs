//! Autoresearch orchestrator: iteration loop and coordination.
//!
//! This module is a skeleton. Full implementation will manage the
//! run loop -- launching worker sessions, processing candidates,
//! evaluating, and deciding keep/discard until the run terminates.

use crate::runtime;
use crate::types::{DecisionStatus, MissionContract, Result};
use std::path::Path;
use tracing::info;

/// Maximum number of trailing noops before the run is considered stalled.
pub const MAX_TRAILING_NOOPS: usize = 3;

/// Configuration for an orchestrator run.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum iterations before the run is force-stopped.
    pub max_iterations: usize,
    /// Maximum runtime in milliseconds (0 = unlimited).
    pub max_runtime_ms: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            max_runtime_ms: 0,
        }
    }
}

/// Orchestrates a single autoresearch run.
///
/// Skeleton: the full implementation will:
/// 1. Prepare the runtime (worktree, baseline, manifests).
/// 2. Loop: launch worker -> read candidate -> evaluate -> decide.
/// 3. Finalize the run with a terminal status.
pub async fn run(
    contract: &MissionContract,
    project_root: &Path,
    worktree_path: &Path,
    config: &OrchestratorConfig,
) -> Result<OrchestratorResult> {
    info!(
        mission = %contract.mission_slug,
        max_iterations = config.max_iterations,
        "starting autoresearch orchestrator (skeleton)"
    );

    let _prepared = runtime::prepare_runtime(contract, project_root, worktree_path).await?;

    // TODO: implement the iteration loop
    // loop {
    //     1. launch worker session
    //     2. wait for worker to write candidate artifact
    //     3. runtime::process_candidate(contract, &mut manifest, project_root).await?
    //     4. check termination conditions (abort, max iterations, trailing noops, deadline)
    // }

    Ok(OrchestratorResult {
        status: OrchestratorStatus::Completed,
        iterations: 0,
        final_decision: DecisionStatus::Baseline,
    })
}

/// Status of an orchestrator run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrchestratorStatus {
    Completed,
    Stopped,
    Failed,
    MaxIterationsReached,
}

/// Result returned by the orchestrator after a run completes.
#[derive(Debug, Clone)]
pub struct OrchestratorResult {
    pub status: OrchestratorStatus,
    pub iterations: usize,
    pub final_decision: DecisionStatus,
}
