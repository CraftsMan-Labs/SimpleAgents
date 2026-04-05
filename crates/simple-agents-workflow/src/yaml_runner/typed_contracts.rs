use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{
    YamlWorkflow, YamlWorkflowEvent, YamlWorkflowEventSink, YamlWorkflowRunOutput,
    YamlWorkflowTokenKind,
};

pub trait YamlWorkflowTypedEventSink: Send + Sync {
    fn emit_typed(&self, event: &YamlWorkflowTypedEvent);

    fn is_cancelled(&self) -> bool {
        false
    }
}

pub struct YamlWorkflowTypedEventSinkAdapter<'a> {
    typed_sink: &'a dyn YamlWorkflowTypedEventSink,
}

impl<'a> YamlWorkflowTypedEventSinkAdapter<'a> {
    pub fn new(typed_sink: &'a dyn YamlWorkflowTypedEventSink) -> Self {
        Self { typed_sink }
    }
}

impl YamlWorkflowEventSink for YamlWorkflowTypedEventSinkAdapter<'_> {
    fn emit(&self, event: &YamlWorkflowEvent) {
        self.typed_sink.emit_typed(&event.to_typed_event());
    }

    fn is_cancelled(&self) -> bool {
        self.typed_sink.is_cancelled()
    }
}

pub(super) struct YamlWorkflowEventSinkFanout<'a> {
    first: &'a dyn YamlWorkflowEventSink,
    second: &'a dyn YamlWorkflowEventSink,
}

impl<'a> YamlWorkflowEventSinkFanout<'a> {
    pub(super) fn new(
        first: &'a dyn YamlWorkflowEventSink,
        second: &'a dyn YamlWorkflowEventSink,
    ) -> Self {
        Self { first, second }
    }
}

impl YamlWorkflowEventSink for YamlWorkflowEventSinkFanout<'_> {
    fn emit(&self, event: &YamlWorkflowEvent) {
        self.first.emit(event);
        self.second.emit(event);
    }

    fn is_cancelled(&self) -> bool {
        self.first.is_cancelled() || self.second.is_cancelled()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YamlWorkflowNodeKind {
    LlmCall,
    Switch,
    CustomWorker,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YamlWorkflowNodeOutputRecord {
    pub node_id: String,
    pub node_kind: YamlWorkflowNodeKind,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YamlWorkflowRunTypedOutput {
    pub workflow_id: String,
    pub entry_node: String,
    pub trace: Vec<String>,
    pub terminal_node: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_output: Option<YamlWorkflowNodeOutputRecord>,
    pub node_outputs: Vec<YamlWorkflowNodeOutputRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YamlWorkflowEventType {
    WorkflowStarted,
    WorkflowCompleted,
    NodeStarted,
    NodeCompleted,
    NodeLlmInputResolved,
    NodeStreamDelta,
    NodeStreamThinkingDelta,
    NodeStreamOutputDelta,
    NodeToolCallRequested,
    NodeToolCallCompleted,
    NodeToolCallFailed,
    NodeToolRoundtripCompleted,
    NodeHealed,
    Unknown,
}

impl YamlWorkflowEventType {
    pub fn from_event_type(value: &str) -> Self {
        match value {
            "workflow_started" => Self::WorkflowStarted,
            "workflow_completed" => Self::WorkflowCompleted,
            "node_started" => Self::NodeStarted,
            "node_completed" => Self::NodeCompleted,
            "node_llm_input_resolved" => Self::NodeLlmInputResolved,
            "node_stream_delta" => Self::NodeStreamDelta,
            "node_stream_thinking_delta" => Self::NodeStreamThinkingDelta,
            "node_stream_output_delta" => Self::NodeStreamOutputDelta,
            "node_tool_call_requested" => Self::NodeToolCallRequested,
            "node_tool_call_completed" => Self::NodeToolCallCompleted,
            "node_tool_call_failed" => Self::NodeToolCallFailed,
            "node_tool_roundtrip_completed" => Self::NodeToolRoundtripCompleted,
            "node_healed" => Self::NodeHealed,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YamlWorkflowTypedEvent {
    pub event_type: YamlWorkflowEventType,
    pub raw_event_type: String,
    pub node_id: Option<String>,
    pub step_id: Option<String>,
    pub node_kind: Option<String>,
    pub streamable: Option<bool>,
    pub message: Option<String>,
    pub delta: Option<String>,
    pub token_kind: Option<YamlWorkflowTokenKind>,
    pub is_terminal_node_token: Option<bool>,
    pub elapsed_ms: Option<u128>,
    pub metadata: Option<Value>,
}

impl YamlWorkflowRunOutput {
    /// Project loose JSON output maps into explicit typed records.
    ///
    /// This keeps node outputs and terminal output while intentionally omitting
    /// token aggregates, timings, and metadata from the typed surface.
    pub fn to_typed_output(&self, workflow: &YamlWorkflow) -> YamlWorkflowRunTypedOutput {
        let node_kind_by_id: HashMap<&str, YamlWorkflowNodeKind> = workflow
            .nodes
            .iter()
            .map(|node| {
                let kind = if node.node_type.llm_call.is_some() {
                    YamlWorkflowNodeKind::LlmCall
                } else if node.node_type.switch.is_some() {
                    YamlWorkflowNodeKind::Switch
                } else if node.node_type.custom_worker.is_some() {
                    YamlWorkflowNodeKind::CustomWorker
                } else {
                    YamlWorkflowNodeKind::Unknown
                };
                (node.id.as_str(), kind)
            })
            .collect();

        let node_outputs: Vec<YamlWorkflowNodeOutputRecord> = self
            .outputs
            .iter()
            .map(|(node_id, value)| YamlWorkflowNodeOutputRecord {
                node_id: node_id.clone(),
                node_kind: node_kind_by_id
                    .get(node_id.as_str())
                    .copied()
                    .unwrap_or(YamlWorkflowNodeKind::Unknown),
                value: value.clone(),
            })
            .collect();

        let terminal_output = node_outputs
            .iter()
            .find(|record| record.node_id == self.terminal_node)
            .cloned()
            .or_else(|| {
                self.terminal_output
                    .as_ref()
                    .map(|value| YamlWorkflowNodeOutputRecord {
                        node_id: self.terminal_node.clone(),
                        node_kind: node_kind_by_id
                            .get(self.terminal_node.as_str())
                            .copied()
                            .unwrap_or(YamlWorkflowNodeKind::Unknown),
                        value: json!({ "output": value.clone() }),
                    })
            });

        YamlWorkflowRunTypedOutput {
            workflow_id: self.workflow_id.clone(),
            entry_node: self.entry_node.clone(),
            trace: self.trace.clone(),
            terminal_node: self.terminal_node.clone(),
            terminal_output,
            node_outputs,
        }
    }
}

impl YamlWorkflowEvent {
    /// Convert stringly-typed workflow event data into a typed event record.
    pub fn to_typed_event(&self) -> YamlWorkflowTypedEvent {
        YamlWorkflowTypedEvent {
            event_type: YamlWorkflowEventType::from_event_type(self.event_type.as_str()),
            raw_event_type: self.event_type.clone(),
            node_id: self.node_id.clone(),
            step_id: self.step_id.clone(),
            node_kind: self.node_kind.clone(),
            streamable: self.streamable,
            message: self.message.clone(),
            delta: self.delta.clone(),
            token_kind: self.token_kind.clone(),
            is_terminal_node_token: self.is_terminal_node_token,
            elapsed_ms: self.elapsed_ms,
            metadata: self.metadata.clone(),
        }
    }
}
