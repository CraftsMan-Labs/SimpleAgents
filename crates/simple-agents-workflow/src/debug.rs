use std::collections::BTreeMap;

use crate::replay::replay_trace;
use crate::runtime::{WorkflowEventKind, WorkflowRetryEvent, WorkflowRunResult};
use crate::trace::{TraceEventKind, TraceTerminalStatus, WorkflowTrace};

/// A normalized timeline row for debug UIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeTimelineEntry {
    /// Event index in runtime order.
    pub index: usize,
    /// Step number associated with the event.
    pub step: usize,
    /// Node id associated with the event.
    pub node_id: String,
    /// Event label.
    pub event: String,
    /// Optional event details.
    pub details: Option<String>,
}

/// Aggregated retry reasons grouped by node and operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryReasonSummary {
    /// Node id that retried.
    pub node_id: String,
    /// Operation (`llm` or `tool`).
    pub operation: String,
    /// Number of retry events observed.
    pub retries: usize,
    /// Distinct reason strings.
    pub reasons: Vec<String>,
}

/// Replay inspection payload that can drive diagnostics views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayTraceInspection {
    /// Whether replay validation passed.
    pub valid: bool,
    /// Total events in the trace.
    pub total_events: usize,
    /// Terminal status if available.
    pub terminal_status: Option<TraceTerminalStatus>,
    /// Validation violations when replay fails.
    pub violations: Vec<String>,
}

/// Builds a timeline view from runtime events.
pub fn node_timeline(result: &WorkflowRunResult) -> Vec<NodeTimelineEntry> {
    result
        .events
        .iter()
        .enumerate()
        .map(|(index, event)| {
            let (label, details) = match &event.kind {
                WorkflowEventKind::NodeStarted => ("node_started", None),
                WorkflowEventKind::NodeCompleted { data } => {
                    ("node_completed", Some(format!("{:?}", data)))
                }
                WorkflowEventKind::NodeFailed { message } => ("node_failed", Some(message.clone())),
            };

            NodeTimelineEntry {
                index,
                step: event.step,
                node_id: event.node_id.clone(),
                event: label.to_string(),
                details,
            }
        })
        .collect()
}

/// Summarizes retry reasons grouped by node and operation.
pub fn retry_reason_summary(retry_events: &[WorkflowRetryEvent]) -> Vec<RetryReasonSummary> {
    let mut grouped: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for retry in retry_events {
        grouped
            .entry((retry.node_id.clone(), retry.operation.clone()))
            .or_default()
            .push(retry.reason.clone());
    }

    grouped
        .into_iter()
        .map(|((node_id, operation), reasons)| {
            let mut unique = reasons;
            unique.sort();
            unique.dedup();
            RetryReasonSummary {
                node_id,
                operation,
                retries: unique.len(),
                reasons: unique,
            }
        })
        .collect()
}

/// Validates replayability and extracts terminal status for inspection UIs.
pub fn inspect_replay_trace(trace: &WorkflowTrace) -> ReplayTraceInspection {
    let terminal_status = trace.events.iter().find_map(|event| match event.kind {
        TraceEventKind::Terminal { status } => Some(status),
        _ => None,
    });

    match replay_trace(trace) {
        Ok(report) => ReplayTraceInspection {
            valid: true,
            total_events: report.total_events,
            terminal_status: Some(report.terminal_status),
            violations: Vec::new(),
        },
        Err(error) => ReplayTraceInspection {
            valid: false,
            total_events: trace.events.len(),
            terminal_status,
            violations: error
                .violations
                .iter()
                .map(|violation| violation.message.clone())
                .collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::runtime::{NodeExecutionData, WorkflowEvent, WorkflowRetryEvent};
    use crate::trace::{TraceEvent, TraceEventKind, WorkflowTrace, WorkflowTraceMetadata};

    use super::*;

    #[test]
    fn builds_timeline_entries() {
        let result = WorkflowRunResult {
            workflow_name: "wf".to_string(),
            terminal_node_id: "end".to_string(),
            node_executions: Vec::new(),
            events: vec![
                WorkflowEvent {
                    step: 0,
                    node_id: "start".to_string(),
                    kind: WorkflowEventKind::NodeStarted,
                },
                WorkflowEvent {
                    step: 0,
                    node_id: "start".to_string(),
                    kind: WorkflowEventKind::NodeCompleted {
                        data: NodeExecutionData::Start {
                            next: "end".to_string(),
                        },
                    },
                },
            ],
            retry_events: Vec::new(),
            node_outputs: Default::default(),
            trace: None,
            replay_report: None,
        };

        let timeline = node_timeline(&result);
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].event, "node_started");
        assert_eq!(timeline[1].event, "node_completed");
    }

    #[test]
    fn aggregates_retry_reasons() {
        let summary = retry_reason_summary(&[
            WorkflowRetryEvent {
                step: 1,
                node_id: "llm".to_string(),
                operation: "llm".to_string(),
                failed_attempt: 1,
                reason: "attempt 1 timed out after 10 ms".to_string(),
            },
            WorkflowRetryEvent {
                step: 1,
                node_id: "llm".to_string(),
                operation: "llm".to_string(),
                failed_attempt: 2,
                reason: "upstream overloaded".to_string(),
            },
        ]);

        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].node_id, "llm");
        assert_eq!(summary[0].retries, 2);
    }

    #[test]
    fn inspects_replay_trace() {
        let trace = WorkflowTrace {
            metadata: WorkflowTraceMetadata {
                trace_id: "trace".to_string(),
                workflow_name: "wf".to_string(),
                workflow_version: "v0".to_string(),
                started_at_unix_ms: 0,
                finished_at_unix_ms: Some(1),
            },
            events: vec![
                TraceEvent {
                    seq: 0,
                    timestamp_unix_ms: 0,
                    kind: TraceEventKind::NodeEnter {
                        node_id: "start".to_string(),
                    },
                },
                TraceEvent {
                    seq: 1,
                    timestamp_unix_ms: 1,
                    kind: TraceEventKind::NodeExit {
                        node_id: "start".to_string(),
                    },
                },
                TraceEvent {
                    seq: 2,
                    timestamp_unix_ms: 2,
                    kind: TraceEventKind::Terminal {
                        status: TraceTerminalStatus::Completed,
                    },
                },
            ],
        };

        let inspection = inspect_replay_trace(&trace);
        assert!(inspection.valid);
        assert_eq!(inspection.total_events, 3);
        assert_eq!(
            inspection.terminal_status,
            Some(TraceTerminalStatus::Completed)
        );
        assert!(inspection.violations.is_empty());
    }

    #[test]
    fn keeps_invalid_trace_violations() {
        let trace = WorkflowTrace {
            metadata: WorkflowTraceMetadata {
                trace_id: "trace".to_string(),
                workflow_name: "wf".to_string(),
                workflow_version: "v0".to_string(),
                started_at_unix_ms: 0,
                finished_at_unix_ms: Some(1),
            },
            events: vec![TraceEvent {
                seq: 0,
                timestamp_unix_ms: 0,
                kind: TraceEventKind::NodeEnter {
                    node_id: "start".to_string(),
                },
            }],
        };

        let inspection = inspect_replay_trace(&trace);
        assert!(!inspection.valid);
        assert!(!inspection.violations.is_empty());
        let _ = json!(inspection.violations);
    }
}
