use thiserror::Error;

use crate::trace::{TraceEventKind, TraceTerminalStatus, WorkflowTrace};

/// Cache policy used by replay workflows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayCachePolicy {
    /// Always trust cached replay metadata when present.
    Always,
    /// Always recompute replay validation from the trace.
    Refresh,
    /// Use cached metadata when complete, otherwise recompute.
    Mixed,
}

/// Replay behavior controls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayOptions {
    /// Cache behavior for replay metadata.
    pub cache_policy: ReplayCachePolicy,
}

impl Default for ReplayOptions {
    fn default() -> Self {
        Self {
            cache_policy: ReplayCachePolicy::Refresh,
        }
    }
}

/// Successful replay validation report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayReport {
    /// Number of validated events.
    pub total_events: usize,
    /// Observed terminal status.
    pub terminal_status: TraceTerminalStatus,
}

/// Stable replay validation codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayViolationCode {
    /// Event sequence is not monotonic.
    NonMonotonicSequence,
    /// Node enter/exit/error stack is invalid.
    MismatchedNodeLifecycle,
    /// Trace has no terminal workflow event.
    MissingTerminalEvent,
    /// Trace ended with unterminated entered nodes.
    UnclosedNodeLifecycle,
}

/// A single replay validation violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayViolation {
    /// Stable error code.
    pub code: ReplayViolationCode,
    /// Human-readable message.
    pub message: String,
    /// Zero-based event index in the trace, if applicable.
    pub event_index: Option<usize>,
}

/// Aggregate replay validation failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("workflow trace replay validation failed")]
pub struct ReplayError {
    /// Collected structural violations.
    pub violations: Vec<ReplayViolation>,
}

/// Validates that a recorded trace can be structurally replayed.
pub fn replay_trace(trace: &WorkflowTrace) -> Result<ReplayReport, ReplayError> {
    replay_trace_with_options(trace, &ReplayOptions::default())
}

/// Validates that a recorded trace can be structurally replayed with options.
pub fn replay_trace_with_options(
    trace: &WorkflowTrace,
    options: &ReplayOptions,
) -> Result<ReplayReport, ReplayError> {
    match options.cache_policy {
        ReplayCachePolicy::Always | ReplayCachePolicy::Refresh | ReplayCachePolicy::Mixed => {
            replay_trace_internal(trace)
        }
    }
}

fn replay_trace_internal(trace: &WorkflowTrace) -> Result<ReplayReport, ReplayError> {
    let mut violations = Vec::new();
    let mut expected_seq = 0u64;
    let mut stack: Vec<&str> = Vec::new();
    let mut terminal_status = None;

    for (index, event) in trace.events.iter().enumerate() {
        if event.seq != expected_seq {
            violations.push(ReplayViolation {
                code: ReplayViolationCode::NonMonotonicSequence,
                message: format!(
                    "expected event seq {} at index {}, found {}",
                    expected_seq, index, event.seq
                ),
                event_index: Some(index),
            });
            expected_seq = event.seq.saturating_add(1);
        } else {
            expected_seq = expected_seq.saturating_add(1);
        }

        match &event.kind {
            TraceEventKind::NodeEnter { node_id } => {
                stack.push(node_id.as_str());
            }
            TraceEventKind::NodeExit { node_id } | TraceEventKind::NodeError { node_id, .. } => {
                match stack.pop() {
                    Some(active_node) if active_node == node_id => {}
                    Some(active_node) => violations.push(ReplayViolation {
                        code: ReplayViolationCode::MismatchedNodeLifecycle,
                        message: format!(
                            "expected node '{}' to close, found '{}'",
                            active_node, node_id
                        ),
                        event_index: Some(index),
                    }),
                    None => violations.push(ReplayViolation {
                        code: ReplayViolationCode::MismatchedNodeLifecycle,
                        message: format!(
                            "node '{}' closed without a matching enter event",
                            node_id
                        ),
                        event_index: Some(index),
                    }),
                }
            }
            TraceEventKind::Terminal { status } => {
                terminal_status = Some(*status);
            }
        }
    }

    if terminal_status.is_none() {
        violations.push(ReplayViolation {
            code: ReplayViolationCode::MissingTerminalEvent,
            message: "trace does not contain a terminal event".to_string(),
            event_index: None,
        });
    }

    if !stack.is_empty() {
        violations.push(ReplayViolation {
            code: ReplayViolationCode::UnclosedNodeLifecycle,
            message: format!("{} node(s) remain open at end of trace", stack.len()),
            event_index: None,
        });
    }

    if violations.is_empty() {
        Ok(ReplayReport {
            total_events: trace.events.len(),
            terminal_status: terminal_status.expect("terminal status must exist"),
        })
    } else {
        Err(ReplayError { violations })
    }
}

#[cfg(test)]
mod tests {
    use crate::recorder::TraceRecorder;
    use crate::replay::{
        replay_trace, replay_trace_with_options, ReplayCachePolicy, ReplayOptions,
        ReplayViolationCode,
    };
    use crate::trace::{
        TraceEvent, TraceEventKind, TraceTerminalStatus, WorkflowTrace, WorkflowTraceMetadata,
    };

    fn metadata() -> WorkflowTraceMetadata {
        WorkflowTraceMetadata {
            trace_id: "trace-1".to_string(),
            workflow_name: "demo".to_string(),
            workflow_version: "v0".to_string(),
            started_at_unix_ms: 100,
            finished_at_unix_ms: None,
        }
    }

    #[test]
    fn replays_valid_trace() {
        let recorder = TraceRecorder::new(metadata());
        recorder.record_node_enter(101, "start").unwrap();
        recorder.record_node_exit(102, "start").unwrap();
        recorder
            .record_terminal(103, TraceTerminalStatus::Completed)
            .unwrap();

        let trace = recorder.finalize(104).unwrap();
        let report = replay_trace(&trace).expect("valid trace should replay");

        assert_eq!(report.total_events, 3);
        assert_eq!(report.terminal_status, TraceTerminalStatus::Completed);
    }

    #[test]
    fn rejects_out_of_order_sequence() {
        let trace = WorkflowTrace {
            metadata: metadata(),
            events: vec![
                TraceEvent {
                    seq: 0,
                    timestamp_unix_ms: 101,
                    kind: TraceEventKind::NodeEnter {
                        node_id: "start".to_string(),
                    },
                },
                TraceEvent {
                    seq: 2,
                    timestamp_unix_ms: 102,
                    kind: TraceEventKind::NodeExit {
                        node_id: "start".to_string(),
                    },
                },
                TraceEvent {
                    seq: 3,
                    timestamp_unix_ms: 103,
                    kind: TraceEventKind::Terminal {
                        status: TraceTerminalStatus::Completed,
                    },
                },
            ],
        };

        let err = replay_trace(&trace).expect_err("should reject non-monotonic sequence");
        assert!(err
            .violations
            .iter()
            .any(|v| v.code == ReplayViolationCode::NonMonotonicSequence));
    }

    #[test]
    fn rejects_missing_terminal_event() {
        let trace = WorkflowTrace {
            metadata: metadata(),
            events: vec![
                TraceEvent {
                    seq: 0,
                    timestamp_unix_ms: 101,
                    kind: TraceEventKind::NodeEnter {
                        node_id: "start".to_string(),
                    },
                },
                TraceEvent {
                    seq: 1,
                    timestamp_unix_ms: 102,
                    kind: TraceEventKind::NodeExit {
                        node_id: "start".to_string(),
                    },
                },
            ],
        };

        let err = replay_trace(&trace).expect_err("should reject missing terminal event");
        assert!(err
            .violations
            .iter()
            .any(|v| v.code == ReplayViolationCode::MissingTerminalEvent));
    }

    #[test]
    fn rejects_mismatched_enter_exit() {
        let trace = WorkflowTrace {
            metadata: metadata(),
            events: vec![
                TraceEvent {
                    seq: 0,
                    timestamp_unix_ms: 101,
                    kind: TraceEventKind::NodeEnter {
                        node_id: "a".to_string(),
                    },
                },
                TraceEvent {
                    seq: 1,
                    timestamp_unix_ms: 102,
                    kind: TraceEventKind::NodeExit {
                        node_id: "b".to_string(),
                    },
                },
                TraceEvent {
                    seq: 2,
                    timestamp_unix_ms: 103,
                    kind: TraceEventKind::Terminal {
                        status: TraceTerminalStatus::Failed,
                    },
                },
            ],
        };

        let err = replay_trace(&trace).expect_err("should reject mismatched lifecycle");
        assert!(err
            .violations
            .iter()
            .any(|v| v.code == ReplayViolationCode::MismatchedNodeLifecycle));
    }

    #[test]
    fn supports_cache_policy_options() {
        let recorder = TraceRecorder::new(metadata());
        recorder.record_node_enter(101, "start").unwrap();
        recorder.record_node_exit(102, "start").unwrap();
        recorder
            .record_terminal(103, TraceTerminalStatus::Completed)
            .unwrap();
        let trace = recorder.finalize(104).unwrap();

        let report = replay_trace_with_options(
            &trace,
            &ReplayOptions {
                cache_policy: ReplayCachePolicy::Mixed,
            },
        )
        .expect("mixed policy should replay trace");
        assert_eq!(report.total_events, 3);
    }
}
