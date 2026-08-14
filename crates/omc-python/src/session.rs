//! Process-local Python session contract and lifecycle store.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::python_kernel::PythonKernel;
use crate::repl::{
    ExecuteResult, InterruptResult, MemoryInfo, PythonReplInput, ReplAction, ResetResult,
    StateResult,
};

pub(crate) const MAX_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MAX_CODE_BYTES: usize = 256 * 1024;
const MAX_SESSIONS: usize = 16;

/// Input contract exposed by the OMC Python tool.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonToolRequest {
    pub action: ReplAction,
    pub session_id: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub execution_timeout: Option<u64>,
    #[serde(default)]
    pub project_dir: Option<String>,
    #[serde(default)]
    pub allow_side_effects: bool,
}

impl PythonToolRequest {
    pub fn into_input(self) -> Result<(PythonReplInput, bool), PythonSessionError> {
        validate_session_id(&self.session_id)?;
        let timeout = self.execution_timeout.unwrap_or(DEFAULT_TIMEOUT_MS);
        if timeout == 0 || timeout > MAX_TIMEOUT_MS {
            return Err(PythonSessionError::InvalidRequest(format!(
                "executionTimeout must be between 1 and {MAX_TIMEOUT_MS} milliseconds"
            )));
        }
        if self
            .code
            .as_ref()
            .is_some_and(|code| code.len() > MAX_CODE_BYTES)
        {
            return Err(PythonSessionError::InvalidRequest(format!(
                "code exceeds {MAX_CODE_BYTES} bytes"
            )));
        }
        Ok((
            PythonReplInput {
                action: self.action,
                research_session_id: self.session_id,
                code: self.code,
                execution_label: None,
                execution_timeout: Some(timeout),
                queue_timeout: None,
                project_dir: self.project_dir,
            },
            self.allow_side_effects,
        ))
    }
}

/// One result returned by the session adapter.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonToolPayload {
    pub operation: &'static str,
    pub action: ReplAction,
    pub session_id: String,
    pub project_dir: String,
    pub session_scope: &'static str,
    pub result: Value,
    pub side_effects: Vec<String>,
}

#[derive(Debug, Error)]
pub enum PythonSessionError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("python adapter is unavailable: {0}")]
    Unavailable(String),
    #[error("python session failed: {0}")]
    Failed(String),
    #[error("python execution timed out after {0}ms")]
    Timeout(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SessionKey {
    id: String,
    project_dir: PathBuf,
}

struct PythonSession {
    kernel: PythonKernel,
}

/// Process-local session store. MCP owns one for its process lifetime; the
/// CLI owns one for its current invocation.
pub struct PythonReplService {
    // ponytail: one process-global lock serializes cells; split to per-session
    // locks if concurrent Python sessions become a measured bottleneck.
    sessions: Mutex<HashMap<SessionKey, PythonSession>>,
}

impl Default for PythonReplService {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonReplService {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn execute(&self, input: &PythonReplInput) -> Result<ExecuteResult, PythonSessionError> {
        let code = input
            .code
            .as_deref()
            .ok_or_else(|| PythonSessionError::InvalidRequest("code is required".into()))?;
        if code.len() > MAX_CODE_BYTES {
            return Err(PythonSessionError::InvalidRequest(format!(
                "code exceeds {MAX_CODE_BYTES} bytes"
            )));
        }
        let project_dir = resolve_project_dir(input.project_dir.as_deref())?;
        let mut sessions = self.lock_sessions()?;
        let key = SessionKey {
            id: input.research_session_id.clone(),
            project_dir: project_dir.clone(),
        };
        if !sessions.contains_key(&key) && sessions.len() >= MAX_SESSIONS {
            return Err(PythonSessionError::Failed(format!(
                "session limit reached ({MAX_SESSIONS}); reset or reuse an existing session"
            )));
        }
        if !sessions.contains_key(&key) {
            let kernel =
                PythonKernel::start(&project_dir).map_err(PythonSessionError::Unavailable)?;
            sessions.insert(key.clone(), PythonSession { kernel });
        }
        sessions
            .get_mut(&key)
            .ok_or_else(|| PythonSessionError::Failed("session was not stored".into()))?
            .kernel
            .execute(code, input.execution_timeout.unwrap_or(DEFAULT_TIMEOUT_MS))
    }

    pub fn interrupt(
        &self,
        session_id: &str,
        project_dir: Option<&str>,
    ) -> Result<InterruptResult, PythonSessionError> {
        self.with_session(session_id, project_dir, |session| {
            session.kernel.restart()?;
            Ok(InterruptResult {
                status: "interrupted".into(),
                terminated_by: Some("process_restart".into()),
                termination_time_ms: Some(0),
            })
        })
    }

    pub fn reset(
        &self,
        session_id: &str,
        project_dir: Option<&str>,
    ) -> Result<ResetResult, PythonSessionError> {
        self.with_session(session_id, project_dir, |session| {
            session.kernel.restart()?;
            Ok(ResetResult {
                status: "ok".into(),
                memory: MemoryInfo {
                    rss_mb: 0.0,
                    vms_mb: 0.0,
                },
            })
        })
    }

    pub fn state(
        &self,
        session_id: &str,
        project_dir: Option<&str>,
    ) -> Result<StateResult, PythonSessionError> {
        self.with_session(session_id, project_dir, |session| session.kernel.state())
    }

    pub fn session_count(&self) -> Result<usize, PythonSessionError> {
        Ok(self.lock_sessions()?.len())
    }

    fn with_session<T>(
        &self,
        session_id: &str,
        project_dir: Option<&str>,
        operation: impl FnOnce(&mut PythonSession) -> Result<T, PythonSessionError>,
    ) -> Result<T, PythonSessionError> {
        validate_session_id(session_id)?;
        let project_dir = resolve_project_dir(project_dir)?;
        let mut sessions = self.lock_sessions()?;
        let key = SessionKey {
            id: session_id.to_string(),
            project_dir,
        };
        let session = sessions
            .get_mut(&key)
            .ok_or_else(|| PythonSessionError::InvalidRequest("session does not exist".into()))?;
        operation(session)
    }

    fn lock_sessions(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<SessionKey, PythonSession>>, PythonSessionError>
    {
        self.sessions
            .lock()
            .map_err(|_| PythonSessionError::Failed("session store was poisoned".into()))
    }
}

fn validate_session_id(session_id: &str) -> Result<(), PythonSessionError> {
    if session_id.trim().is_empty() || session_id.len() > 128 || session_id.contains('\0') {
        return Err(PythonSessionError::InvalidRequest(
            "sessionId must be 1..128 characters and must not contain NUL".into(),
        ));
    }
    Ok(())
}

fn resolve_project_dir(input: Option<&str>) -> Result<PathBuf, PythonSessionError> {
    let path = input.map_or_else(|| PathBuf::from("."), PathBuf::from);
    let path = path.canonicalize().map_err(|error| {
        PythonSessionError::InvalidRequest(format!("projectDir is invalid: {error}"))
    })?;
    if !path.is_dir() {
        return Err(PythonSessionError::InvalidRequest(
            "projectDir must be a directory".into(),
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn request_requires_explicit_side_effect_opt_in_only_at_boundary() {
        let request: PythonToolRequest = serde_json::from_value(json!({
            "action": "execute",
            "sessionId": "test",
            "code": "x = 1"
        }))
        .unwrap();
        let (_, allowed) = request.into_input().unwrap();
        assert!(!allowed);
    }

    #[test]
    fn local_kernel_keeps_state_between_calls() {
        let service = PythonReplService::new();
        let root = tempdir().unwrap();
        let first = PythonReplInput {
            action: ReplAction::Execute,
            research_session_id: "test".into(),
            code: Some("value = 41".into()),
            execution_label: None,
            execution_timeout: Some(10_000),
            queue_timeout: None,
            project_dir: Some(root.path().to_string_lossy().into_owned()),
        };
        let second = PythonReplInput {
            code: Some("print(value + 1)".into()),
            ..first.clone()
        };
        let first_result = service.execute(&first);
        if let Err(PythonSessionError::Unavailable(_)) = first_result {
            return;
        }
        assert!(first_result.unwrap().success);
        assert_eq!(service.execute(&second).unwrap().stdout.trim(), "42");
        assert_eq!(service.session_count().unwrap(), 1);
    }

    #[test]
    fn timeout_restarts_kernel_before_next_cell() {
        let service = PythonReplService::new();
        let root = tempdir().unwrap();
        let slow = PythonReplInput {
            action: ReplAction::Execute,
            research_session_id: "timeout-test".into(),
            code: Some("import time; time.sleep(0.2)".into()),
            execution_label: None,
            execution_timeout: Some(20),
            queue_timeout: None,
            project_dir: Some(root.path().to_string_lossy().into_owned()),
        };
        let error = service.execute(&slow).unwrap_err();
        if matches!(error, PythonSessionError::Unavailable(_)) {
            return;
        }
        assert!(matches!(error, PythonSessionError::Timeout(20)));
        let fast = PythonReplInput {
            code: Some("print(7)".into()),
            execution_timeout: Some(10_000),
            ..slow
        };
        assert_eq!(service.execute(&fast).unwrap().stdout.trim(), "7");
    }

    #[test]
    fn unicode_output_is_capped_by_utf8_bytes_and_reports_truncation() {
        let service = PythonReplService::new();
        let root = tempdir().unwrap();
        let input = PythonReplInput {
            action: ReplAction::Execute,
            research_session_id: "utf8-limit".into(),
            code: Some("print('🙂' * 1100000, end='')".into()),
            execution_label: None,
            execution_timeout: Some(30_000),
            queue_timeout: None,
            project_dir: Some(root.path().to_string_lossy().into_owned()),
        };
        let result = match service.execute(&input) {
            Ok(result) => result,
            Err(PythonSessionError::Unavailable(_)) => return,
            Err(error) => panic!("unicode output execution failed: {error}"),
        };
        assert!(result.stdout.len() <= 4 * 1024 * 1024);
        assert!(result.stdout.is_char_boundary(result.stdout.len()));
        assert!(
            result
                .markers
                .iter()
                .any(|marker| marker.subtype.as_deref() == Some("stdout_truncated"))
        );
    }
}
