use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Typed workflow event for streaming and observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowEvent {
    /// Workflow execution has begun.
    WorkflowStarted { workflow_id: String },
    /// A workflow node has started executing.
    NodeStarted {
        node_id: String,
        node_type: NodeType,
    },
    /// A token delta from an LLM node (streaming).
    LlmTokenDelta {
        node_id: String,
        token: String,
        token_kind: TokenKind,
    },
    /// A workflow node has finished executing.
    NodeCompleted { node_id: String, output: Value },
    /// An LLM node requested a tool call.
    ToolCallRequested {
        node_id: String,
        tool_name: String,
        arguments: Value,
    },
    /// A tool call has finished.
    ToolCallCompleted {
        node_id: String,
        tool_name: String,
        output: Value,
    },
    /// A node is retrying after failure.
    NodeRetrying {
        node_id: String,
        attempt: u8,
        error: String,
    },
    /// A node has failed permanently.
    NodeFailed { node_id: String, error: String },
    /// Workflow execution completed.
    WorkflowCompleted {
        output: Value,
        metadata: Option<Value>,
    },
}

/// The type of workflow node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    LlmCall,
    Switch,
    CustomWorker,
    End,
    Unknown,
}

/// The kind of LLM token in a streaming delta.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Output,
    Reasoning,
}

/// Trait for receiving workflow events.
pub trait WorkflowEventSink: Send + Sync {
    /// Called for each workflow event.
    fn emit(&self, event: &WorkflowEvent);
    /// Return true to request cancellation.
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// Wraps a closure as a WorkflowEventSink.
pub struct CallbackSink<F: Fn(&WorkflowEvent) + Send + Sync>(pub F);

impl<F: Fn(&WorkflowEvent) + Send + Sync> WorkflowEventSink for CallbackSink<F> {
    fn emit(&self, event: &WorkflowEvent) {
        (self.0)(event);
    }
}

/// No-op sink that discards all events.
pub struct NoopSink;
impl WorkflowEventSink for NoopSink {
    fn emit(&self, _event: &WorkflowEvent) {}
}

/// Built-in pretty-printer for workflow events.
/// Streams LLM tokens to stdout, logs node lifecycle to stderr.
pub struct DefaultEventPrinter;

impl WorkflowEventSink for DefaultEventPrinter {
    fn emit(&self, event: &WorkflowEvent) {
        match event {
            WorkflowEvent::WorkflowStarted { workflow_id } => {
                eprintln!("[workflow] started: {workflow_id}");
            }
            WorkflowEvent::NodeStarted { node_id, node_type } => {
                eprintln!("[node] {node_id} ({node_type:?}) started");
            }
            WorkflowEvent::LlmTokenDelta {
                token, token_kind, ..
            } => {
                if *token_kind == TokenKind::Output {
                    use std::io::Write;
                    let _ = std::io::stdout().write_all(token.as_bytes());
                    let _ = std::io::stdout().flush();
                }
            }
            WorkflowEvent::NodeCompleted { node_id, .. } => {
                eprintln!("[node] {node_id} completed");
            }
            WorkflowEvent::ToolCallRequested {
                tool_name, node_id, ..
            } => {
                eprintln!("[tool] {node_id} calling {tool_name}");
            }
            WorkflowEvent::ToolCallCompleted {
                tool_name, node_id, ..
            } => {
                eprintln!("[tool] {node_id} {tool_name} done");
            }
            WorkflowEvent::NodeRetrying {
                node_id,
                attempt,
                error,
            } => {
                eprintln!("[retry] {node_id} attempt #{attempt}: {error}");
            }
            WorkflowEvent::NodeFailed { node_id, error } => {
                eprintln!("[error] {node_id}: {error}");
            }
            WorkflowEvent::WorkflowCompleted { .. } => {
                eprintln!("[workflow] completed");
            }
        }
    }
}

#[cfg(test)]
fn emit(sink: Option<&dyn WorkflowEventSink>, event: WorkflowEvent) {
    if let Some(s) = sink {
        s.emit(&event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_callback_sink_collects_events() {
        let events: Arc<Mutex<Vec<WorkflowEvent>>> = Arc::new(Mutex::new(vec![]));
        let events_clone = events.clone();
        let sink = CallbackSink(move |e: &WorkflowEvent| {
            events_clone.lock().unwrap().push(e.clone());
        });
        emit(
            Some(&sink),
            WorkflowEvent::WorkflowStarted {
                workflow_id: "test".into(),
            },
        );
        assert_eq!(events.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_event_serialization() {
        let event = WorkflowEvent::NodeStarted {
            node_id: "classify".into(),
            node_type: NodeType::LlmCall,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "node_started");
        assert_eq!(json["node_id"], "classify");
        assert_eq!(json["node_type"], "llm_call");
    }

    #[test]
    fn test_noop_sink_does_not_panic() {
        let sink = NoopSink;
        emit(
            Some(&sink),
            WorkflowEvent::WorkflowStarted {
                workflow_id: "x".into(),
            },
        );
    }
}
