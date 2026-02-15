use std::sync::{Arc, Mutex};

use thiserror::Error;

use crate::trace::{
    TraceEvent, TraceEventKind, TraceTerminalStatus, WorkflowTrace, WorkflowTraceMetadata,
};

/// Thread-safe in-memory recorder for workflow trace events.
#[derive(Debug, Clone)]
pub struct TraceRecorder {
    state: Arc<Mutex<TraceRecorderState>>,
}

#[derive(Debug, Clone)]
struct TraceRecorderState {
    trace: WorkflowTrace,
    next_seq: u64,
    finalized: bool,
}

impl TraceRecorder {
    /// Creates a new recorder with empty event history.
    pub fn new(metadata: WorkflowTraceMetadata) -> Self {
        Self {
            state: Arc::new(Mutex::new(TraceRecorderState {
                trace: WorkflowTrace {
                    metadata,
                    events: Vec::new(),
                },
                next_seq: 0,
                finalized: false,
            })),
        }
    }

    /// Appends an arbitrary event to the trace.
    pub fn append_event(
        &self,
        timestamp_unix_ms: u64,
        kind: TraceEventKind,
    ) -> Result<TraceEvent, TraceRecordError> {
        let mut state = self.state.lock().expect("trace recorder mutex poisoned");

        if state.finalized {
            return Err(TraceRecordError::AlreadyFinalized);
        }

        let event = TraceEvent {
            seq: state.next_seq,
            timestamp_unix_ms,
            kind,
        };

        state.next_seq = state.next_seq.saturating_add(1);
        state.trace.events.push(event.clone());
        Ok(event)
    }

    /// Appends a node enter event.
    pub fn record_node_enter(
        &self,
        timestamp_unix_ms: u64,
        node_id: impl Into<String>,
    ) -> Result<TraceEvent, TraceRecordError> {
        self.append_event(
            timestamp_unix_ms,
            TraceEventKind::NodeEnter {
                node_id: node_id.into(),
            },
        )
    }

    /// Appends a node exit event.
    pub fn record_node_exit(
        &self,
        timestamp_unix_ms: u64,
        node_id: impl Into<String>,
    ) -> Result<TraceEvent, TraceRecordError> {
        self.append_event(
            timestamp_unix_ms,
            TraceEventKind::NodeExit {
                node_id: node_id.into(),
            },
        )
    }

    /// Appends a node error event.
    pub fn record_node_error(
        &self,
        timestamp_unix_ms: u64,
        node_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<TraceEvent, TraceRecordError> {
        self.append_event(
            timestamp_unix_ms,
            TraceEventKind::NodeError {
                node_id: node_id.into(),
                message: message.into(),
            },
        )
    }

    /// Appends a workflow terminal event.
    pub fn record_terminal(
        &self,
        timestamp_unix_ms: u64,
        status: TraceTerminalStatus,
    ) -> Result<TraceEvent, TraceRecordError> {
        self.append_event(timestamp_unix_ms, TraceEventKind::Terminal { status })
    }

    /// Returns a snapshot of the current trace.
    pub fn snapshot(&self) -> WorkflowTrace {
        let state = self.state.lock().expect("trace recorder mutex poisoned");
        state.trace.clone()
    }

    /// Finalizes the recorder and returns the immutable trace.
    pub fn finalize(&self, finished_at_unix_ms: u64) -> Result<WorkflowTrace, TraceRecordError> {
        let mut state = self.state.lock().expect("trace recorder mutex poisoned");

        if state.finalized {
            return Err(TraceRecordError::AlreadyFinalized);
        }

        state.trace.metadata.finished_at_unix_ms = Some(finished_at_unix_ms);
        state.finalized = true;
        Ok(state.trace.clone())
    }
}

/// Trace recorder failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TraceRecordError {
    /// Mutation attempted after recorder finalization.
    #[error("trace recorder is already finalized")]
    AlreadyFinalized,
}
