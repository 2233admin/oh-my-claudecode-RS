#[derive(clap::Subcommand, Debug, Clone)]
pub enum ToolCommand {
    /// List host-neutral capabilities
    Capabilities {
        /// Optional caller correlation ID
        #[arg(long)]
        request_id: Option<String>,
    },

    /// Route a task by semantic complexity
    Route {
        /// Task text to route
        #[arg(long)]
        task: String,

        /// Optional role hint
        #[arg(long)]
        agent_type: Option<String>,

        /// Number of previous failures for this task
        #[arg(long, default_value_t = 0)]
        previous_failures: usize,

        /// Optional caller correlation ID
        #[arg(long)]
        request_id: Option<String>,
    },

    /// Query committed Code Intel artifacts through its read-only CLI surface
    CodeIntelQuery {
        /// Code Intel repository key, not a filesystem path
        #[arg(long)]
        repo: String,

        /// Published Code Intel artifact root
        #[arg(long)]
        artifact_root: Option<String>,

        /// Optional checkout for freshness evaluation
        #[arg(long)]
        repo_path: Option<String>,

        /// Optional artifact schema filter
        #[arg(long)]
        artifact_schema: Option<String>,

        /// Optional artifact type filter
        #[arg(long)]
        artifact_type: Option<String>,

        /// Optional text filter applied by Code Intel
        #[arg(long)]
        contains: Option<String>,

        /// Optional canonical OMC artifact URI for bounded exact inspection
        #[arg(long)]
        artifact_uri: Option<String>,

        /// Maximum number of matching artifacts (1..100)
        #[arg(long)]
        limit: Option<u8>,

        /// Optional caller correlation ID
        #[arg(long)]
        request_id: Option<String>,
    },

    /// Read existing omc-team sessions, usage snapshot, or health
    TeamObservability {
        /// sessions, top, or doctor
        #[arg(long, value_parser = ["sessions", "top", "doctor"])]
        view: String,

        /// Project root containing .omc/team
        #[arg(long, default_value = ".")]
        root: String,

        /// Optional caller correlation ID
        #[arg(long)]
        request_id: Option<String>,
    },

    /// Read a bounded, read-only OMC/OMX interop state snapshot
    InteropSnapshot {
        /// Project root containing .omc and .omx state
        #[arg(long, default_value = ".")]
        root: String,

        /// Maximum records returned per state collection (1..100)
        #[arg(long, default_value_t = 20)]
        limit: usize,

        /// Optional caller correlation ID
        #[arg(long)]
        request_id: Option<String>,
    },

    /// Send one explicit task or message through the active OMC/OMX bridge
    InteropBridge {
        /// send_task or send_message
        #[arg(long, value_parser = ["send_task", "send_message"])]
        action: String,

        /// Source runtime: omc or omx
        #[arg(long, value_parser = ["omc", "omx"])]
        source: String,

        /// Target runtime: omc or omx
        #[arg(long, value_parser = ["omc", "omx"])]
        target: String,

        /// OMC task type, required for send_task
        #[arg(long = "type", value_parser = ["analyze", "implement", "review", "test", "custom"])]
        task_type: Option<String>,

        /// Task description, required for send_task
        #[arg(long)]
        description: Option<String>,

        /// Message content, required for send_message
        #[arg(long)]
        content: Option<String>,

        /// Project root containing .omc interop state
        #[arg(long, default_value = ".")]
        root: String,

        /// Explicitly allow the durable interop write
        #[arg(long)]
        allow_side_effects: bool,

        /// Optional caller correlation ID
        #[arg(long)]
        request_id: Option<String>,
    },

    /// Read Rust document symbols through a one-shot rust-analyzer adapter
    LspDocumentSymbols {
        /// Project root used to bound the file path
        #[arg(long, default_value = ".")]
        root: String,

        /// Project-relative Rust source file
        #[arg(long)]
        file: String,

        /// Request timeout in milliseconds (5000..60000)
        #[arg(long)]
        timeout_ms: Option<u64>,

        /// Optional caller correlation ID
        #[arg(long)]
        request_id: Option<String>,
    },

    /// Inspect one explicit launch/attach session through an external stdio DAP adapter
    DebugInspect {
        /// Executable or command name for the external DAP adapter
        #[arg(long)]
        adapter_command: String,

        /// Optional JSON array of adapter arguments
        #[arg(long)]
        adapter_args_json: Option<String>,

        /// launch or attach
        #[arg(long)]
        mode: String,

        /// threads, stackTrace, scopes, variables, modules, loadedSources, or output
        #[arg(long)]
        action: String,

        /// Project root used as the adapter working directory
        #[arg(long, default_value = ".")]
        root: String,

        /// JSON object passed to the launch request
        #[arg(long)]
        launch_arguments: Option<String>,

        /// JSON object passed to the attach request
        #[arg(long)]
        attach_arguments: Option<String>,

        #[arg(long)]
        thread_id: Option<i64>,
        #[arg(long)]
        frame_id: Option<i64>,
        #[arg(long)]
        variables_reference: Option<i64>,

        /// Request timeout in milliseconds (5000..300000)
        #[arg(long)]
        timeout_ms: Option<u64>,

        /// Required because launch/attach starts or connects an external session
        #[arg(long, default_value_t = false)]
        allow_side_effects: bool,

        #[arg(long)]
        request_id: Option<String>,
    },

    /// Execute an explicit-side-effect Python cell in a process-local session
    PythonRepl {
        /// execute, get_state, reset, or interrupt
        #[arg(long, value_parser = ["execute", "get_state", "reset", "interrupt"])]
        action: String,

        /// Stable session key
        #[arg(long)]
        session_id: String,

        /// Python cell source; required for execute
        #[arg(long)]
        code: Option<String>,

        /// Existing working directory for the Python subprocess
        #[arg(long, default_value = ".")]
        root: String,

        /// Execution timeout in milliseconds (1..300000)
        #[arg(long)]
        timeout_ms: Option<u64>,

        /// Required for execute, reset, or interrupt
        #[arg(long, default_value_t = false)]
        allow_side_effects: bool,

        #[arg(long)]
        request_id: Option<String>,
    },

    /// Advance a host-neutral clarify-plan-execute-verify workflow from supplied evidence
    WorkflowAdvance {
        /// Current workflow stage
        #[arg(long)]
        current_stage: String,

        #[arg(long, default_value_t = false)]
        requirements_clarified: bool,
        #[arg(long, default_value_t = false)]
        all_tasks_assigned: bool,
        #[arg(long, default_value_t = false)]
        plan_approved: bool,
        #[arg(long, default_value_t = false)]
        all_tasks_completed: bool,
        #[arg(long, default_value_t = false)]
        verification_passed: bool,
        #[arg(long, default_value_t = false)]
        has_failures: bool,
        #[arg(long, default_value_t = false)]
        has_blockers: bool,
        #[arg(long, default_value_t = 0)]
        fix_attempts: u32,
        #[arg(long, default_value_t = 3)]
        max_fix_attempts: u32,

        /// Optional caller correlation ID
        #[arg(long)]
        request_id: Option<String>,
    },

    /// Validate a structured subagent result against required fields
    ResultValidate {
        #[arg(long)]
        result_type: String,
        #[arg(long)]
        payload: String,
        /// Comma-separated required top-level fields
        #[arg(long, value_delimiter = ',')]
        required_fields: Vec<String>,
        #[arg(long)]
        request_id: Option<String>,
    },

    /// Apply a stale-safe, hash-anchored file edit
    HashEdit {
        #[arg(long, default_value = ".")]
        root: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        start_line: usize,
        #[arg(long)]
        end_line: usize,
        /// JSON array such as [{"line":1,"sha256":"..."}]
        #[arg(long)]
        anchors_json: String,
        #[arg(long)]
        replacement: String,
        #[arg(long)]
        expected_file_sha256: Option<String>,
        #[arg(long)]
        request_id: Option<String>,
    },
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum GoalCommand {
    /// Create a new planned goal
    Create {
        /// Stable goal identifier
        #[arg(long)]
        id: String,

        /// Goal objective
        #[arg(long)]
        objective: String,

        /// Optional agency or human owner
        #[arg(long)]
        owner: Option<String>,

        /// Optional task ID used for dispatch correlation
        #[arg(long)]
        task_id: Option<String>,
    },

    /// List all project goals
    List,

    /// Show one goal
    Show {
        #[arg(long)]
        id: String,
    },

    /// Move a planned or blocked goal to active
    Start {
        #[arg(long)]
        id: String,
    },

    /// Mark a goal blocked with a durable reason
    Block {
        #[arg(long)]
        id: String,
        #[arg(long)]
        reason: String,
    },

    /// Append a checkpoint to a goal
    Checkpoint {
        #[arg(long)]
        id: String,
        #[arg(long)]
        checkpoint_id: String,
        #[arg(long)]
        summary: String,
    },

    /// Mark an active goal completed
    Complete {
        #[arg(long)]
        id: String,
    },
}
