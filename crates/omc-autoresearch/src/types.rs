//! Core domain types for the autoresearch system.
//!
//! Derived from the TypeScript `contracts.ts` and `runtime.ts` reference implementation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// All autoresearch errors.
#[derive(Debug, Error)]
pub enum AutoresearchError {
    #[error("contract violation: {0}")]
    Contract(String),

    #[error("evaluator error: {0}")]
    Evaluator(String),

    #[error("run error: {0}")]
    Runtime(String),

    #[error("PRD error: {0}")]
    Prd(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, AutoresearchError>;

// ---------------------------------------------------------------------------
// Keep policy
// ---------------------------------------------------------------------------

/// Whether a candidate is kept based on score improvement or pass-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum KeepPolicy {
    /// Kept only when the evaluator score improves over the last kept score.
    #[default]
    ScoreImprovement,
    /// Kept whenever the evaluator reports pass=true, regardless of score.
    PassOnly,
}

// ---------------------------------------------------------------------------
// Candidate / Decision / Run status
// ---------------------------------------------------------------------------

/// Status of a candidate artifact written by a worker session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    /// Worker produced a code change and wrote a candidate commit.
    Candidate,
    /// Worker determined no change was needed.
    Noop,
    /// Worker requested the run be stopped.
    Abort,
    /// Worker session was interrupted before writing a final artifact.
    Interrupted,
}

/// Decision made by the supervisor after evaluating a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Baseline,
    Keep,
    Discard,
    Noop,
    Ambiguous,
    Abort,
    Interrupted,
    Error,
}

/// High-level status of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Stopped,
    Completed,
    Failed,
}

// ---------------------------------------------------------------------------
// Sandbox contract (from sandbox.md frontmatter)
// ---------------------------------------------------------------------------

/// Parsed evaluator block from sandbox.md frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxEvaluator {
    /// Shell command to execute the evaluator.
    pub command: String,
    /// Output format; must be `"json"` in autoresearch v1.
    pub format: String,
    /// Optional keep policy override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_policy: Option<KeepPolicy>,
}

/// Fully parsed sandbox contract including frontmatter and body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedSandboxContract {
    /// Parsed YAML frontmatter as a flat key-value map.
    pub frontmatter: HashMap<String, serde_json::Value>,
    /// The evaluator block extracted from frontmatter.
    pub evaluator: SandboxEvaluator,
    /// The markdown body after the frontmatter.
    pub body: String,
}

// ---------------------------------------------------------------------------
// Mission contract (from mission.md + sandbox.md)
// ---------------------------------------------------------------------------

/// A fully resolved mission contract: paths, content, and parsed sandbox.
#[derive(Debug, Clone)]
pub struct MissionContract {
    /// Absolute path to the mission directory.
    pub mission_dir: PathBuf,
    /// Absolute path to the git repository root.
    pub repo_root: PathBuf,
    /// Absolute path to mission.md.
    pub mission_file: PathBuf,
    /// Absolute path to sandbox.md.
    pub sandbox_file: PathBuf,
    /// Mission directory path relative to the repo root.
    pub mission_relative_dir: String,
    /// Raw content of mission.md.
    pub mission_content: String,
    /// Raw content of sandbox.md.
    pub sandbox_content: String,
    /// Parsed sandbox contract.
    pub sandbox: ParsedSandboxContract,
    /// URL-safe slug derived from the mission directory name.
    pub mission_slug: String,
}

/// Borrowed reference to a mission contract, for passing to functions
/// without cloning.
#[derive(Debug)]
pub struct MissionContractRef<'a> {
    pub contract: &'a MissionContract,
}

// ---------------------------------------------------------------------------
// Evaluator result
// ---------------------------------------------------------------------------

/// Parsed JSON output from an evaluator invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorResult {
    /// Whether the evaluation passed.
    pub pass: bool,
    /// Optional numeric score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
}

// ---------------------------------------------------------------------------
// Evaluation record
// ---------------------------------------------------------------------------

/// Record of a single evaluator invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRecord {
    pub command: String,
    pub ran_at: String,
    pub status: EvaluationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pass: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
}

/// Evaluator invocation outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationStatus {
    Pass,
    Fail,
    Error,
}

// ---------------------------------------------------------------------------
// Candidate artifact
// ---------------------------------------------------------------------------

/// JSON artifact written by a worker session after one experiment cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateArtifact {
    pub status: CandidateStatus,
    pub candidate_commit: Option<String>,
    pub base_commit: String,
    pub description: String,
    pub notes: Vec<String>,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Research query / result
// ---------------------------------------------------------------------------

/// A user-facing research query to kick off an autoresearch run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchQuery {
    /// Free-text description of the research mission.
    pub mission_text: String,
    /// Shell command for the evaluator.
    pub evaluator_command: String,
    /// Where the evaluator command came from.
    pub evaluator_source: EvaluatorSource,
    /// Confidence in the inferred evaluator (0.0 -- 1.0).
    pub confidence: f64,
    /// Optional keep policy override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_policy: Option<KeepPolicy>,
    /// URL-safe slug for the mission.
    pub slug: String,
    /// Whether the setup is ready to launch a run.
    pub ready_to_launch: bool,
    /// Clarification question when launch is blocked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clarification_question: Option<String>,
    /// Signals detected from the repository that informed the setup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_signals: Option<Vec<String>>,
}

/// Source of the evaluator command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluatorSource {
    /// User explicitly provided the command.
    User,
    /// Command was inferred from the repo.
    Inferred,
}

/// Result of a completed research run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchResult {
    pub run_id: String,
    pub status: RunStatus,
    pub stop_reason: Option<String>,
    pub iteration: usize,
    pub last_kept_commit: String,
    pub last_kept_score: Option<f64>,
    pub completed_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Run manifest
// ---------------------------------------------------------------------------

/// Persistent manifest for an autoresearch run, stored as JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub schema_version: u32,
    pub run_id: String,
    pub run_tag: String,
    pub mission_dir: PathBuf,
    pub mission_file: PathBuf,
    pub sandbox_file: PathBuf,
    pub repo_root: PathBuf,
    pub worktree_path: PathBuf,
    pub mission_slug: String,
    pub branch_name: String,
    pub baseline_commit: String,
    pub last_kept_commit: String,
    pub last_kept_score: Option<f64>,
    pub latest_candidate_commit: Option<String>,
    pub results_file: PathBuf,
    pub instructions_file: PathBuf,
    pub manifest_file: PathBuf,
    pub ledger_file: PathBuf,
    pub latest_evaluator_file: PathBuf,
    pub candidate_file: PathBuf,
    pub evaluator: SandboxEvaluator,
    pub keep_policy: KeepPolicy,
    pub status: RunStatus,
    pub stop_reason: Option<String>,
    pub iteration: usize,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Ledger entry
// ---------------------------------------------------------------------------

/// A single entry in the iteration ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub iteration: usize,
    pub kind: LedgerEntryKind,
    pub decision: DecisionStatus,
    pub decision_reason: String,
    pub candidate_status: LedgerCandidateStatus,
    pub base_commit: String,
    pub candidate_commit: Option<String>,
    pub kept_commit: String,
    pub keep_policy: KeepPolicy,
    pub evaluator: Option<EvaluationRecord>,
    pub created_at: String,
    pub notes: Vec<String>,
    pub description: String,
}

/// Kind of ledger entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerEntryKind {
    Baseline,
    Iteration,
}

/// Candidate status as recorded in a ledger entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerCandidateStatus {
    Candidate,
    Noop,
    Abort,
    Interrupted,
    Baseline,
}

// ---------------------------------------------------------------------------
// Setup handoff
// ---------------------------------------------------------------------------

/// Handoff payload from the setup agent to the runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupHandoff {
    pub mission_text: String,
    pub evaluator_command: String,
    pub evaluator_source: EvaluatorSource,
    pub confidence: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_policy: Option<KeepPolicy>,
    pub slug: String,
    pub ready_to_launch: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clarification_question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_signals: Option<Vec<String>>,
}

/// Minimum confidence threshold for inferred evaluators.
pub const SETUP_CONFIDENCE_THRESHOLD: f64 = 0.8;

// ---------------------------------------------------------------------------
// Decision
// ---------------------------------------------------------------------------

/// The outcome of one supervisor decision cycle.
#[derive(Debug, Clone)]
pub struct Decision {
    pub decision: DecisionStatus,
    pub reason: String,
    pub keep: bool,
    pub evaluator: Option<EvaluationRecord>,
    pub notes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Prepared runtime
// ---------------------------------------------------------------------------

/// All paths and identifiers produced when preparing an autoresearch run.
#[derive(Debug, Clone)]
pub struct PreparedRuntime {
    pub run_id: String,
    pub run_tag: String,
    pub run_dir: PathBuf,
    pub instructions_file: PathBuf,
    pub manifest_file: PathBuf,
    pub ledger_file: PathBuf,
    pub latest_evaluator_file: PathBuf,
    pub results_file: PathBuf,
    pub state_file: PathBuf,
    pub candidate_file: PathBuf,
    pub repo_root: PathBuf,
    pub worktree_path: PathBuf,
    pub task_description: String,
}
