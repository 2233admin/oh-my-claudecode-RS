//! Explicitly authorized subprocess Adapters using the shared bounded lifecycle.

use omc_shared::session_pool::{BoundedSessionPool, SessionPoolError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ProcessAdapterKey {
    pub command: PathBuf,
    pub args: Vec<String>,
}

struct ProcessSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Drop for ProcessSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessAdapterDiagnostics {
    pub opened: u64,
    pub reused: u64,
    pub evicted: u64,
    pub active: usize,
}

#[derive(Debug, Error)]
pub enum ProcessAdapterError {
    #[error("process Adapter execution requires explicit authorization")]
    NotAuthorized,
    #[error("process Adapter capacity reached ({0})")]
    Capacity(usize),
    #[error("process Adapter I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("process Adapter response is invalid: {0}")]
    InvalidResponse(String),
}

pub struct ProcessAdapterPool {
    sessions: BoundedSessionPool<ProcessAdapterKey, ProcessSession>,
    authorized: bool,
    diagnostics: ProcessAdapterDiagnostics,
}

impl ProcessAdapterPool {
    pub fn new(capacity: usize, idle_ttl: Duration, authorized: bool) -> Self {
        Self {
            sessions: BoundedSessionPool::new(capacity, idle_ttl),
            authorized,
            diagnostics: ProcessAdapterDiagnostics::default(),
        }
    }

    pub fn request(
        &mut self,
        key: ProcessAdapterKey,
        request: &Value,
    ) -> Result<Value, ProcessAdapterError> {
        if !self.authorized {
            return Err(ProcessAdapterError::NotAuthorized);
        }
        let (session, reused) = self
            .sessions
            .get_or_try_insert_with_status(key.clone(), || open_session(&key))
            .map_err(|error| match error {
                SessionPoolError::Capacity { limit } => ProcessAdapterError::Capacity(limit),
                SessionPoolError::Open(error) => ProcessAdapterError::Io(error),
            })?;
        let unhealthy = reused && !matches!(session.child.try_wait(), Ok(None));
        let result = if unhealthy {
            Err(ProcessAdapterError::InvalidResponse(
                "reused adapter is not healthy".into(),
            ))
        } else {
            if reused {
                self.diagnostics.reused += 1;
            } else {
                self.diagnostics.opened += 1;
            }
            exchange(session, request)
        };
        if result.is_err() {
            self.sessions.remove(&key);
            self.diagnostics.evicted += 1;
        }
        self.diagnostics.active = self.sessions.len();
        result
    }

    pub fn close(&mut self, key: &ProcessAdapterKey) -> bool {
        let removed = self.sessions.remove(key).is_some();
        self.diagnostics.active = self.sessions.len();
        removed
    }

    pub fn reap_idle(&mut self) -> usize {
        let reaped = self.sessions.reap_idle();
        self.diagnostics.evicted += reaped as u64;
        self.diagnostics.active = self.sessions.len();
        reaped
    }

    pub fn diagnostics(&self) -> ProcessAdapterDiagnostics {
        self.diagnostics.clone()
    }
}

fn open_session(key: &ProcessAdapterKey) -> std::io::Result<ProcessSession> {
    let mut child = Command::new(&key.command)
        .args(&key.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("stdin unavailable"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("stdout unavailable"))?;
    Ok(ProcessSession {
        child,
        stdin,
        stdout: BufReader::new(stdout),
    })
}

fn exchange(session: &mut ProcessSession, request: &Value) -> Result<Value, ProcessAdapterError> {
    writeln!(session.stdin, "{request}")?;
    session.stdin.flush()?;
    let mut response = String::new();
    if session.stdout.read_line(&mut response)? == 0 {
        return Err(ProcessAdapterError::InvalidResponse(
            "adapter exited".into(),
        ));
    }
    serde_json::from_str(&response)
        .map_err(|error| ProcessAdapterError::InvalidResponse(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_execution_is_denied_without_explicit_authorization() {
        let mut pool = ProcessAdapterPool::new(1, Duration::from_secs(1), false);
        let error = pool
            .request(
                ProcessAdapterKey {
                    command: "anything".into(),
                    args: Vec::new(),
                },
                &serde_json::json!({}),
            )
            .unwrap_err();
        assert!(matches!(error, ProcessAdapterError::NotAuthorized));
        assert_eq!(pool.diagnostics().active, 0);
    }
}
