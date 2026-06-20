use thiserror::Error;

use crate::repl::{
    ExecuteResult, InterruptResult, PythonReplInput, ReplAction, ResetResult, StateResult,
};

/// Errors that can occur during REPL operations.
#[derive(Debug, Error)]
pub enum ReplError {
    #[error("session {0} is busy")]
    SessionBusy(String),

    #[error("code is required for the execute action")]
    MissingCode,

    #[error("invalid session id: {0}")]
    InvalidSessionId(String),

    #[error("bridge connection failed: {0}")]
    ConnectionFailed(String),

    #[error("execution timed out after {0}ms")]
    ExecutionTimeout(u64),

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

/// Response from the REPL executor.
#[derive(Debug)]
pub enum ReplResponse {
    Execute(ExecuteResult),
    State(StateResult),
    Reset(ResetResult),
    Interrupt(InterruptResult),
}

/// Default execution timeout in milliseconds (5 minutes).
pub const DEFAULT_EXECUTION_TIMEOUT_MS: u64 = 300_000;

/// Default queue timeout in milliseconds (30 seconds).
pub const DEFAULT_QUEUE_TIMEOUT_MS: u64 = 30_000;

/// Trait for executing Python REPL operations.
///
/// Implementations handle the actual bridge communication
/// (e.g., JSON-RPC over Unix socket to a Python subprocess).
#[async_trait::async_trait]
pub trait PythonReplExecutor: Send + Sync {
    /// Execute Python code in the persistent environment.
    async fn execute(
        &self,
        session_id: &str,
        code: &str,
        timeout_ms: u64,
    ) -> Result<ExecuteResult, ReplError>;

    /// Interrupt running code.
    async fn interrupt(&self, session_id: &str) -> Result<InterruptResult, ReplError>;

    /// Clear the execution namespace.
    async fn reset(&self, session_id: &str) -> Result<ResetResult, ReplError>;

    /// Get memory usage and variable list.
    async fn get_state(&self, session_id: &str) -> Result<StateResult, ReplError>;
}

/// Process a REPL input and dispatch to the appropriate executor method.
pub async fn handle_repl_input(
    executor: &dyn PythonReplExecutor,
    input: &PythonReplInput,
) -> Result<ReplResponse, ReplError> {
    if input.research_session_id.is_empty() {
        return Err(ReplError::InvalidSessionId(
            "research_session_id must not be empty".into(),
        ));
    }

    let session_id = &input.research_session_id;

    match input.action {
        ReplAction::Execute => {
            let code = input.code.as_deref().ok_or(ReplError::MissingCode)?;
            let timeout = input
                .execution_timeout
                .unwrap_or(DEFAULT_EXECUTION_TIMEOUT_MS);
            let result = executor.execute(session_id, code, timeout).await?;
            Ok(ReplResponse::Execute(result))
        }
        ReplAction::Interrupt => {
            let result = executor.interrupt(session_id).await?;
            Ok(ReplResponse::Interrupt(result))
        }
        ReplAction::Reset => {
            let result = executor.reset(session_id).await?;
            Ok(ReplResponse::Reset(result))
        }
        ReplAction::GetState => {
            let result = executor.get_state(session_id).await?;
            Ok(ReplResponse::State(result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repl::{MemoryInfo, TimingInfo};

    struct MockExecutor;

    #[async_trait::async_trait]
    impl PythonReplExecutor for MockExecutor {
        async fn execute(
            &self,
            _sid: &str,
            _code: &str,
            _timeout: u64,
        ) -> Result<ExecuteResult, ReplError> {
            Ok(ExecuteResult {
                success: true,
                stdout: "ok".into(),
                stderr: String::default(),
                markers: vec![],
                timing: TimingInfo {
                    started_at: "2026-01-01T00:00:00Z".into(),
                    duration_ms: 10,
                },
                memory: MemoryInfo {
                    rss_mb: 50.0,
                    vms_mb: 100.0,
                },
                error: None,
            })
        }

        async fn interrupt(&self, _sid: &str) -> Result<InterruptResult, ReplError> {
            Ok(InterruptResult {
                status: "interrupted".into(),
                terminated_by: Some("SIGINT".into()),
                termination_time_ms: Some(100),
            })
        }

        async fn reset(&self, _sid: &str) -> Result<ResetResult, ReplError> {
            Ok(ResetResult {
                status: "ok".into(),
                memory: MemoryInfo {
                    rss_mb: 10.0,
                    vms_mb: 50.0,
                },
            })
        }

        async fn get_state(&self, _sid: &str) -> Result<StateResult, ReplError> {
            Ok(StateResult {
                memory: MemoryInfo {
                    rss_mb: 50.0,
                    vms_mb: 100.0,
                },
                variables: vec!["x".into()],
                variable_count: 1,
            })
        }
    }

    #[tokio::test]
    async fn test_execute() {
        let executor = MockExecutor;
        let input = PythonReplInput {
            action: ReplAction::Execute,
            research_session_id: "test".into(),
            code: Some("print(42)".into()),
            execution_label: None,
            execution_timeout: None,
            queue_timeout: None,
            project_dir: None,
        };
        let resp = handle_repl_input(&executor, &input).await.unwrap();
        assert!(matches!(resp, ReplResponse::Execute(r) if r.success));
    }

    #[tokio::test]
    async fn test_execute_missing_code() {
        let executor = MockExecutor;
        let input = PythonReplInput {
            action: ReplAction::Execute,
            research_session_id: "test".into(),
            code: None,
            execution_label: None,
            execution_timeout: None,
            queue_timeout: None,
            project_dir: None,
        };
        let err = handle_repl_input(&executor, &input).await.unwrap_err();
        assert!(matches!(err, ReplError::MissingCode));
    }

    #[tokio::test]
    async fn test_empty_session_id() {
        let executor = MockExecutor;
        let input = PythonReplInput {
            action: ReplAction::GetState,
            research_session_id: String::default(),
            code: None,
            execution_label: None,
            execution_timeout: None,
            queue_timeout: None,
            project_dir: None,
        };
        let err = handle_repl_input(&executor, &input).await.unwrap_err();
        assert!(matches!(err, ReplError::InvalidSessionId(_)));
    }
}
