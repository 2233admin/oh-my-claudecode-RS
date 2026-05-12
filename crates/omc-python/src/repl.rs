use serde::{Deserialize, Serialize};

/// Actions the REPL supports.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReplAction {
    Execute,
    Interrupt,
    Reset,
    GetState,
}

/// Input for the Python REPL tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PythonReplInput {
    pub action: ReplAction,
    pub research_session_id: String,
    pub code: Option<String>,
    pub execution_label: Option<String>,
    pub execution_timeout: Option<u64>,
    pub queue_timeout: Option<u64>,
    pub project_dir: Option<String>,
}

/// Memory usage snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub rss_mb: f64,
    pub vms_mb: f64,
}

/// Structured marker produced during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkerInfo {
    pub r#type: String,
    pub subtype: Option<String>,
    pub content: String,
    pub line_number: u32,
    pub category: String,
}

/// Timing metadata for an execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingInfo {
    pub started_at: String,
    pub duration_ms: u64,
}

/// Error details from a failed execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionError {
    pub r#type: String,
    pub message: String,
    pub traceback: Option<String>,
}

/// Result of executing Python code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub markers: Vec<MarkerInfo>,
    pub timing: TimingInfo,
    pub memory: MemoryInfo,
    pub error: Option<ExecutionError>,
}

/// Result of `get_state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateResult {
    pub memory: MemoryInfo,
    pub variables: Vec<String>,
    pub variable_count: u32,
}

/// Result of `reset`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetResult {
    pub status: String,
    pub memory: MemoryInfo,
}

/// Result of `interrupt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterruptResult {
    pub status: String,
    pub terminated_by: Option<String>,
    pub termination_time_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_action() {
        let action: ReplAction = serde_json::from_str("\"execute\"").unwrap();
        assert_eq!(action, ReplAction::Execute);
    }

    #[test]
    fn deserialize_input() {
        let json = r#"{
            "action": "execute",
            "research_session_id": "test-123",
            "code": "print('hello')"
        }"#;
        let input: PythonReplInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.action, ReplAction::Execute);
        assert_eq!(input.code.as_deref(), Some("print('hello')"));
    }
}
