//! Autoresearch runtime: run lifecycle, evaluation, and candidate processing.
//!
//! This module is a skeleton. Full implementation mirrors the TypeScript
//! `runtime.ts` -- prepare, seed baseline, process candidates, finalize.

use crate::types::{
    AutoresearchError, CandidateArtifact, CandidateStatus, Decision, DecisionStatus,
    EvaluationRecord, EvaluationStatus, EvaluatorResult, KeepPolicy, MissionContract,
    PreparedRuntime, Result, RunManifest,
};
use std::path::Path;
use tracing::debug;

// ---------------------------------------------------------------------------
// Run tag / ID helpers
// ---------------------------------------------------------------------------

/// Build a run tag from the current timestamp (ISO 8601 compacted).
pub fn build_run_tag() -> String {
    let now = chrono::Utc::now();
    now.format("%Y%m%dT%H%M%SZ").to_string()
}

/// Build a run ID from a mission slug and run tag.
pub fn build_run_id(mission_slug: &str, run_tag: &str) -> String {
    format!("{}-{}", mission_slug.to_lowercase(), run_tag.to_lowercase())
}

// ---------------------------------------------------------------------------
// Evaluator invocation
// ---------------------------------------------------------------------------

/// Parse raw JSON output from an evaluator command.
pub fn parse_evaluator_result(raw: &str) -> Result<EvaluatorResult> {
    let parsed: serde_json::Value = serde_json::from_str(raw)?;
    let obj = parsed
        .as_object()
        .ok_or_else(|| AutoresearchError::Evaluator("output must be a JSON object".into()))?;

    let pass = obj
        .get("pass")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| AutoresearchError::Evaluator("pass (boolean) is required".into()))?;

    let score = obj.get("score").and_then(|v| v.as_f64());

    Ok(EvaluatorResult { pass, score })
}

/// Maximum byte length accepted for an evaluator command. Anything larger is
/// almost certainly accidental (or hostile) and is rejected before reaching
/// the shell.
const MAX_EVALUATOR_COMMAND_BYTES: usize = 8192;

/// Validate an evaluator command before handing it to `sh -c`.
///
/// `contract.sandbox.evaluator.command` is treated as a *trusted* input
/// authored by the mission operator. The shell escape hatch is intentional
/// (evaluators routinely use pipes / redirects / `&&` chains). This check is
/// the cheap defense-in-depth layer: it rejects clearly malformed inputs
/// (empty, NUL byte, runaway length) without touching the shell semantics
/// the operator relies on.
fn validate_evaluator_command(cmd: &str) -> Result<()> {
    if cmd.trim().is_empty() {
        return Err(AutoresearchError::Runtime(
            "evaluator command must not be empty".into(),
        ));
    }
    if cmd.contains('\0') {
        return Err(AutoresearchError::Runtime(
            "evaluator command contains NUL byte".into(),
        ));
    }
    if cmd.len() > MAX_EVALUATOR_COMMAND_BYTES {
        return Err(AutoresearchError::Runtime(format!(
            "evaluator command too long: {} bytes > {} max",
            cmd.len(),
            MAX_EVALUATOR_COMMAND_BYTES
        )));
    }
    Ok(())
}

/// Run the evaluator command and return a record.
///
/// Skeleton: spawns the command and captures output. Full implementation will
/// handle timeouts, exit-code analysis, and ledger integration.
///
/// SAFETY: `contract.sandbox.evaluator.command` is executed via `sh -c` to
/// preserve shell features (pipes, redirects, chained commands) that
/// real-world evaluators rely on. The string is treated as trusted input
/// from the mission contract; [`validate_evaluator_command`] applies the
/// minimal sanity checks before invocation. Do not relax that validation
/// or accept evaluator commands from untrusted sources without a stronger
/// sandbox.
pub fn run_evaluator(contract: &MissionContract, worktree_path: &Path) -> Result<EvaluationRecord> {
    let cmd = &contract.sandbox.evaluator.command;
    validate_evaluator_command(cmd)?;
    debug!(command = %cmd, worktree = %worktree_path.display(), "running evaluator");

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(worktree_path)
        .output()
        .map_err(|e| AutoresearchError::Runtime(format!("failed to spawn evaluator: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let ran_at = chrono::Utc::now().to_rfc3339();

    if !output.status.success() {
        return Ok(EvaluationRecord {
            command: cmd.clone(),
            ran_at,
            status: EvaluationStatus::Error,
            pass: None,
            score: None,
            exit_code: output.status.code(),
            stdout: Some(stdout),
            stderr: Some(stderr),
            parse_error: None,
        });
    }

    match parse_evaluator_result(&stdout) {
        Ok(result) => Ok(EvaluationRecord {
            command: cmd.clone(),
            ran_at,
            status: if result.pass {
                EvaluationStatus::Pass
            } else {
                EvaluationStatus::Fail
            },
            pass: Some(result.pass),
            score: result.score,
            exit_code: output.status.code(),
            stdout: Some(stdout),
            stderr: Some(stderr),
            parse_error: None,
        }),
        Err(e) => Ok(EvaluationRecord {
            command: cmd.clone(),
            ran_at,
            status: EvaluationStatus::Error,
            pass: None,
            score: None,
            exit_code: output.status.code(),
            stdout: Some(stdout),
            stderr: Some(stderr),
            parse_error: Some(e.to_string()),
        }),
    }
}

// ---------------------------------------------------------------------------
// Decision logic
// ---------------------------------------------------------------------------

/// Decide whether to keep or discard a candidate based on evaluation.
///
/// Mirrors `decideAutoresearchOutcome` from the TypeScript reference.
pub fn decide_outcome(
    keep_policy: KeepPolicy,
    last_kept_score: Option<f64>,
    candidate: &CandidateArtifact,
    evaluation: Option<&EvaluationRecord>,
) -> Decision {
    use CandidateStatus::*;

    match candidate.status {
        Abort => Decision {
            decision: DecisionStatus::Abort,
            reason: "candidate requested abort".into(),
            keep: false,
            evaluator: None,
            notes: vec!["run stopped by candidate artifact".into()],
        },
        Noop => Decision {
            decision: DecisionStatus::Noop,
            reason: "candidate reported noop".into(),
            keep: false,
            evaluator: None,
            notes: vec!["no code change was proposed".into()],
        },
        Interrupted => Decision {
            decision: DecisionStatus::Interrupted,
            reason: "candidate session was interrupted".into(),
            keep: false,
            evaluator: None,
            notes: vec!["supervisor should inspect worktree cleanliness before continuing".into()],
        },
        Candidate => {
            let eval = match evaluation {
                Some(e) => e,
                None => {
                    return Decision {
                        decision: DecisionStatus::Discard,
                        reason: "evaluator error".into(),
                        keep: false,
                        evaluator: None,
                        notes: vec!["candidate discarded because evaluator errored".into()],
                    };
                }
            };

            if eval.status == EvaluationStatus::Error {
                return Decision {
                    decision: DecisionStatus::Discard,
                    reason: "evaluator error".into(),
                    keep: false,
                    evaluator: Some(eval.clone()),
                    notes: vec!["candidate discarded because evaluator errored".into()],
                };
            }

            if eval.pass != Some(true) {
                return Decision {
                    decision: DecisionStatus::Discard,
                    reason: "evaluator reported failure".into(),
                    keep: false,
                    evaluator: Some(eval.clone()),
                    notes: vec!["candidate discarded because evaluator pass=false".into()],
                };
            }

            if keep_policy == KeepPolicy::PassOnly {
                return Decision {
                    decision: DecisionStatus::Keep,
                    reason: "pass_only keep policy accepted evaluator pass=true".into(),
                    keep: true,
                    evaluator: Some(eval.clone()),
                    notes: vec![
                        "candidate kept because sandbox opted into pass_only policy".into(),
                    ],
                };
            }

            // score_improvement policy
            match (last_kept_score, eval.score) {
                (None, Some(_score)) => Decision {
                    decision: DecisionStatus::Keep,
                    reason: "[bootstrap] first comparable score in score_improvement run".into(),
                    keep: true,
                    evaluator: Some(eval.clone()),
                    notes: vec!["candidate kept because no prior comparable score existed".into()],
                },
                (Some(prev), Some(score)) if score > prev => Decision {
                    decision: DecisionStatus::Keep,
                    reason: "score improved over last kept score".into(),
                    keep: true,
                    evaluator: Some(eval.clone()),
                    notes: vec!["candidate kept because evaluator score increased".into()],
                },
                (Some(_), Some(_)) => Decision {
                    decision: DecisionStatus::Discard,
                    reason: "score did not improve".into(),
                    keep: false,
                    evaluator: Some(eval.clone()),
                    notes: vec![
                        "candidate discarded because evaluator score was not better".into(),
                    ],
                },
                _ => Decision {
                    decision: DecisionStatus::Ambiguous,
                    reason: "evaluator pass without numeric score".into(),
                    keep: false,
                    evaluator: Some(eval.clone()),
                    notes: vec!["score_improvement policy requires a numeric score".into()],
                },
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Candidate artifact parsing
// ---------------------------------------------------------------------------

/// Parse a candidate artifact from raw JSON.
pub fn parse_candidate_artifact(raw: &str) -> Result<CandidateArtifact> {
    let artifact: CandidateArtifact = serde_json::from_str(raw)?;

    if artifact.base_commit.trim().is_empty() {
        return Err(AutoresearchError::Contract(
            "candidate artifact base_commit is required".into(),
        ));
    }

    if artifact.created_at.trim().is_empty() {
        return Err(AutoresearchError::Contract(
            "candidate artifact created_at is required".into(),
        ));
    }

    Ok(artifact)
}

// ---------------------------------------------------------------------------
// Placeholder lifecycle functions (to be filled in)
// ---------------------------------------------------------------------------

/// Prepare a new autoresearch run.
///
/// Validates the contract, creates the worktree, writes manifests,
/// seeds baseline evaluation, and returns the prepared runtime.
pub async fn prepare_runtime(
    contract: &MissionContract,
    project_root: &Path,
    worktree_path: &Path,
) -> Result<PreparedRuntime> {
    debug!(
        mission = %contract.mission_slug,
        "preparing autoresearch runtime (skeleton)"
    );

    let run_tag = build_run_tag();
    let run_id = build_run_id(&contract.mission_slug, &run_tag);
    let run_dir = project_root
        .join(".omc")
        .join("logs")
        .join("autoresearch")
        .join(&run_id);

    Ok(PreparedRuntime {
        run_id: run_id.clone(),
        run_tag,
        run_dir: run_dir.clone(),
        instructions_file: run_dir.join("bootstrap-instructions.md"),
        manifest_file: run_dir.join("manifest.json"),
        ledger_file: run_dir.join("iteration-ledger.json"),
        latest_evaluator_file: run_dir.join("latest-evaluator-result.json"),
        results_file: worktree_path.join("results.tsv"),
        state_file: project_root
            .join(".omc")
            .join("state")
            .join("autoresearch-state.json"),
        candidate_file: run_dir.join("candidate.json"),
        repo_root: project_root.to_path_buf(),
        worktree_path: worktree_path.to_path_buf(),
        task_description: format!(
            "autoresearch {} ({})",
            contract.mission_relative_dir, run_id
        ),
    })
}

/// Process one candidate artifact and return the decision status.
pub async fn process_candidate(
    _contract: &MissionContract,
    _manifest: &mut RunManifest,
    _project_root: &Path,
) -> Result<DecisionStatus> {
    Err(AutoresearchError::Runtime(
        "process_candidate not yet implemented".into(),
    ))
}

/// Stop an active autoresearch run.
pub async fn stop_runtime(_project_root: &Path) -> Result<()> {
    Err(AutoresearchError::Runtime(
        "stop_runtime not yet implemented".into(),
    ))
}
