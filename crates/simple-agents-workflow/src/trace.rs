use serde::{Deserialize, Serialize};

/// Monotonic event sequence number inside a single trace.
pub type TraceSequence = u64;

/// Aggregate execution trace for one workflow run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowTrace {
    /// Trace-level metadata.
    pub metadata: WorkflowTraceMetadata,
    /// Ordered event stream.
    pub events: Vec<TraceEvent>,
}

/// Stable metadata for a workflow trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowTraceMetadata {
    /// Globally unique trace identifier.
    pub trace_id: String,
    /// Workflow name for this run.
    pub workflow_name: String,
    /// Workflow IR version for this run.
    pub workflow_version: String,
    /// Workflow start timestamp (Unix epoch milliseconds).
    pub started_at_unix_ms: u64,
    /// Workflow completion timestamp (Unix epoch milliseconds).
    pub finished_at_unix_ms: Option<u64>,
}

/// A single ordered trace event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceEvent {
    /// Deterministic sequence number.
    pub seq: TraceSequence,
    /// Event timestamp (Unix epoch milliseconds).
    pub timestamp_unix_ms: u64,
    /// Event payload.
    #[serde(flatten)]
    pub kind: TraceEventKind,
}

/// Stable event taxonomy for trace/replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TraceEventKind {
    /// Node execution started.
    NodeEnter { node_id: String },
    /// Node execution completed successfully.
    NodeExit { node_id: String },
    /// Node execution failed.
    NodeError { node_id: String, message: String },
    /// Workflow reached a terminal state.
    Terminal { status: TraceTerminalStatus },
}

/// Workflow terminal status captured in trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceTerminalStatus {
    Completed,
    Failed,
}
